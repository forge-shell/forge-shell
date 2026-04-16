# RFC-008 — Plan Caching

| Field          | Value                        |
|----------------|------------------------------|
| Status         | **In Review**                |
| Author         | Ajitem Sahasrabuddhe         |
| Created        | 2026-04-09                   |
| Last Updated   | 2026-04-15                   |
| Supersedes     | —                            |
| Superseded By  | —                            |

---

## Summary

This RFC defines the plan caching system for Forge Shell. Plan caching
accelerates repeated script execution by persisting the platform-lowered
`ExecutionPlan` to disk, skipping the lex/parse/HIR/lowering pipeline on
cache hits. The cache key covers the full import graph — all imported `.fgs`
files recursively — ensuring correctness when dependencies change. AOT
compilation (`forge compile`) is explicitly deferred to post-v1.

---

## Motivation

Without caching, every invocation of a `.fgs` script runs the full pipeline:
lex → parse → type check → HIR → platform backend → execute. For scripts
invoked frequently — deployment scripts, CI steps, shell functions, REPL
autoloaded functions — this overhead is measurable and unnecessary when
neither the script nor its dependencies have changed.

Plan caching is enabled by a decision already made in RFC-002: the
`ExecutionPlan` and all `Op` nodes derive `serde::Serialize` and
`serde::Deserialize`. The serialisation infrastructure exists — this RFC
specifies how to use it.

---

## Design

### 1. Cache Location — Per-User Always

The cache is always isolated per OS user. No system-wide shared cache exists.

| Platform | Path |
|---|---|
| Linux | `~/.cache/forge/plans/` |
| macOS | `~/Library/Caches/forge/plans/` |
| Windows | `%LOCALAPPDATA%\forge\cache\plans\` |

**Rules:**
- Cache is always per OS user — no sharing across users, ever
- No configuration option to share — this is a hard rule, not a preference
- CI jobs running as isolated users or in containers get per-job isolation automatically
- Security: one user cannot poison another user's cache

---

### 2. Cache Key

The cache key is a SHA-256 hash that covers the full import graph, the Forge
version, the platform, and the architecture.

```rust
// Step 1 — collect full import graph
let import_graph = resolver.collect_imports("script.fgs")?;
// Returns: ["script.fgs", "utils.fgs", "deploy/helpers.fgs"]
// Reuses the same graph built by forge-lang/resolver for circular import detection

// Step 2 — hash all files, sorted for determinism
let mut hasher = Sha256::new();
for file in import_graph.sorted_by_path() {
    hasher.update(file.path.as_bytes());
    hasher.update(file.content.as_bytes());
}

// Step 3 — include forge version and target
hasher.update(FORGE_VERSION.as_bytes());
hasher.update(platform_identifier().as_bytes());   // "linux" | "macos" | "windows"
hasher.update(arch_identifier().as_bytes());       // "x86_64" | "aarch64"

let cache_key = hex::encode(hasher.finalize());
```

**What invalidates the cache:**

| Change | Effect |
|---|---|
| Any file in import graph modified | Cache miss — full pipeline re-runs |
| New import added to any file | Cache miss — import graph changed |
| Forge Shell version upgrade | All cached plans invalidated |
| Platform change | Miss — plans are platform-specific |
| Architecture change | Miss — plans are arch-specific |

**Why files are sorted before hashing:** File system ordering is not
deterministic across platforms. Sorting by path guarantees the same hash
regardless of OS or file system.

**Import graph reuse:** The resolver already builds the full import graph
for circular import detection (RFC-002). Cache key computation reuses this
graph — no additional resolution work.

---

### 3. Cache Format

Plans are serialised using `postcard` — compact, `no_std`-compatible,
already a dependency for `ExecutionPlan` serialisation.

```rust
#[derive(Serialize, Deserialize)]
pub struct CachedPlan {
    pub format_version: u32,        // cache format version — not Forge version
    pub forge_version:  String,     // Forge binary version string
    pub platform:       String,     // "linux" | "macos" | "windows"
    pub arch:           String,     // "x86_64" | "aarch64"
    pub created_at:     u64,        // Unix timestamp
    pub import_graph:   Vec<String>, // sorted list of files in import graph
    pub plan:           ExecutionPlan,
}
```

**Cache file naming:**

```
~/.cache/forge/plans/{cache_key}.plan
```

**Format version:** If the `format_version` field does not match the current
Forge binary's expected format version, the cached plan is silently discarded
and regenerated. This handles cache format changes across Forge versions
without requiring a manual cache clear.

---

### 4. Cache Lookup Flow

```
forge run script.fgs
        ↓
Resolve import graph
        ↓
Compute cache key (SHA-256 of graph + forge_version + platform + arch)
        ↓
Cache hit? ──YES──▶ Deserialise CachedPlan
                            ↓
                   Format version matches? ──NO──▶ Discard → cache miss
                            ↓ YES
                   Execute ExecutionPlan
        ↓ NO (cache miss)
Lex → Parse → Type Check → HIR → PlatformBackend
        ↓
Serialise CachedPlan to disk
        ↓
Execute ExecutionPlan
```

**Cache writes are best-effort.** A cache write failure (disk full, permission
error) is logged as a warning but never fails the script execution. The script
runs correctly — just without caching for this invocation.

**Cache reads are validated.** A corrupted or truncated cache file is detected
during postcard deserialisation and treated as a cache miss. The corrupted
file is deleted and regenerated.

---

### 5. Cache Eviction

The cache grows as new scripts are executed. Eviction runs automatically
on shell startup — quick, non-blocking, background task.

**Eviction policy:**

| Policy | Detail |
|---|---|
| Max size | 512 MB total cache size — configurable |
| Max age | 30 days — plans older than this are evicted |
| LRU | When size limit reached, least recently used plans evicted first |

**Configurable in `config.toml`:**

```toml
[cache]
max_size_mb  = 512     # total cache size limit
max_age_days = 30      # evict plans older than this
enabled      = true    # disable caching entirely
```

---

### 6. Cache Management Commands

```bash
# Show cache statistics
forge cache stats
# Output:
#   Plans cached:    142
#   Cache size:      48.3 MB / 512 MB
#   Oldest plan:     12 days ago
#   Newest plan:     2 minutes ago

# Clear all cached plans
forge cache clear

# Clear cache for a specific script (and its import graph)
forge cache clear ./deploy.fgs

# Pre-warm cache without executing — useful in CI setup steps
forge cache warm ./deploy.fgs
forge cache warm ./scripts/*.fgs

# Show which files are in a script's import graph
forge cache graph ./deploy.fgs
# Output:
#   deploy.fgs
#   └── utils.fgs
#   └── helpers/deploy.fgs
#       └── helpers/k8s.fgs

# Disable cache for a single invocation
forge run --no-cache script.fgs
```

---

### 7. Cache Warming in CI

For CI environments where startup latency matters, cache warming can be added
as a setup step:

```yaml
# GitHub Actions example
- name: Warm Forge plan cache
  run: forge cache warm ./scripts/*.fgs

- name: Run deploy script
  run: forge run ./scripts/deploy.fgs --env staging
  # Cache hit — pipeline skipped, ExecutionPlan executes directly
```

The warmed cache is valid for the duration of the CI job. Since CI typically
runs as an isolated user, the cache is automatically scoped to the job.

---

### 8. AOT Compilation — Explicitly Deferred

`forge compile` — producing a self-contained native binary from a `.fgs`
script — is explicitly deferred to post-v1.

**Why deferred:**

- Requires a native code generation backend (Cranelift or LLVM) — significant
  engineering investment
- Requires a minimal embedded Forge runtime for path operations, env vars,
  and process spawning
- Cross-compilation (targeting Windows from Linux) adds further complexity
- Plan caching already covers the primary startup latency use case for v1
- AOT should follow a stable v1 with demonstrated demand — not precede it

**Post-v1 AOT design considerations (not specified here):**

- Self-contained binary — no Forge installation required on target machine
- Single binary artifact — ideal for CLI tool distribution
- Cross-compilation support for all three target platforms
- `forge compile --target x86_64-pc-windows-msvc script.fgs`

This RFC covers plan caching only. A future RFC-008b or RFC-014 will specify
AOT compilation when the time is right.

---

## Drawbacks

- **Cache staleness on indirect imports.** If an imported file changes on
  disk between cache key computation and plan execution — a race condition —
  the stale plan executes for that one invocation. This is an inherent
  limitation of file-system-based caching and is considered acceptable. The
  next invocation will recompute correctly.
- **Cache key computation adds latency on cold starts.** Hashing all files
  in the import graph takes time. For scripts with large import graphs (10+
  files) this may be measurable. Mitigated by the fact that the resolver
  already walks the import graph — hashing is incremental over the same walk.
- **Per-user cache means repeated cold starts on multi-user machines.** On
  shared servers where many users run the same scripts, each user pays the
  cold-start cost independently. The security benefit outweighs this cost.

---

## Alternatives Considered

### Alternative A — HIR Caching (bytecode style)

**Approach:** Cache the HIR (before platform backend) rather than the
`ExecutionPlan`.

**Rejected because:** The HIR is platform-agnostic — caching it saves
lex/parse/type-check/HIR time but not the platform backend step. The platform
backend is the most complex stage. Skipping it matters more than skipping
parsing. `ExecutionPlan` caching saves the entire pipeline.

### Alternative B — Shared System Cache

**Approach:** Single cache at `/var/cache/forge/` shared across all users.

**Rejected because:** A shared cache means one user's plan is executed by
another user. Even read-only, this is a trust boundary violation — user A
could poison the cache for user B by crafting a script that produces a
malicious cached plan. Per-user isolation is the correct security model.

### Alternative C — Script Source Hash Only (no import graph)

**Approach:** Cache key based only on the entry script's source.

**Rejected because:** If any imported file changes, the cache hit returns a
stale plan built with the old import. This is a silent correctness bug — the
worst possible failure mode for a caching system. Full import graph hashing
is required for correctness.

### Alternative D — Directory Hash

**Approach:** Cache key includes hash of entire script directory.

**Rejected because:** Any file change in the directory — README, config files,
data files — invalidates the cache. This is far too aggressive and defeats
the purpose of caching in project directories where scripts live alongside
other files.

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | Cache shared across users? | Per-user always — no sharing, no exceptions |
| UQ-2 | `forge compile` in v1? | Deferred entirely — plan caching only in v1 |
| UQ-3 | Cross-compilation story? | Not applicable — deferred with AOT |
| UQ-4 | Compiled binaries `forge --version`? | Not applicable — deferred with AOT |
| UQ-5 | Cache key for imported scripts? | Full import graph hash — all files in graph |

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-cache` | Cache read/write, key computation, eviction, validation |
| `forge-lang/resolver` | Import graph collection — reused for cache key |
| `forge-engine` | Cache lookup integration — check before pipeline, write after |
| `forge-cli/cache` | `forge cache` subcommands — stats, clear, warm, graph |

### Dependencies

- Requires RFC-002 (evaluation pipeline) — `ExecutionPlan` with `Serialize`/`Deserialize`
- Requires RFC-013 (shell config) — `[cache]` config section

### Milestones

1. Implement import graph collection in `forge-lang/resolver` — reuse circular import graph
2. Implement cache key computation — SHA-256 of sorted import graph + version + platform
3. Implement `CachedPlan` serialisation/deserialisation via `postcard`
4. Implement cache read in `forge-engine` — lookup before pipeline
5. Implement cache write in `forge-engine` — best-effort after pipeline
6. Implement cache validation — format version check, corruption detection
7. Implement cache eviction — LRU + age + size limit, background on startup
8. Implement `forge cache stats` — plan count, size, age range
9. Implement `forge cache clear` — all and per-script
10. Implement `forge cache warm` — pre-warm without executing
11. Implement `forge cache graph` — show import graph for a script
12. Implement `--no-cache` flag for single-invocation bypass
13. Integration tests — cache hit/miss, invalidation, corruption recovery, eviction

---

## References

- [postcard — Compact binary serialisation](https://docs.rs/postcard)
- [Go build cache](https://pkg.go.dev/cmd/go#hdr-Build_and_test_caching)
- [Python .pyc bytecode caching](https://peps.python.org/pep-3147/)
- [Cargo build cache](https://doc.rust-lang.org/cargo/guide/build-cache.html)
- [RFC-002 — Evaluation Pipeline](./RFC-002-evaluation-pipeline.md)
- [RFC-013 — Shell Configuration Model](./RFC-013-shell-config.md)# RFC-008 — Plan Caching

| Field          | Value                        |
|----------------|------------------------------|
| Status         | **In Review**                |
| Author         | Ajitem Sahasrabuddhe         |
| Created        | 2026-04-09                   |
| Last Updated   | 2026-04-15                   |
| Supersedes     | —                            |
| Superseded By  | —                            |

---

## Summary

This RFC defines the plan caching system for Forge Shell. Plan caching
accelerates repeated script execution by persisting the platform-lowered
`ExecutionPlan` to disk, skipping the lex/parse/HIR/lowering pipeline on
cache hits. The cache key covers the full import graph — all imported `.fgs`
files recursively — ensuring correctness when dependencies change. AOT
compilation (`forge compile`) is explicitly deferred to post-v1.

---

## Motivation

Without caching, every invocation of a `.fgs` script runs the full pipeline:
lex → parse → type check → HIR → platform backend → execute. For scripts
invoked frequently — deployment scripts, CI steps, shell functions, REPL
autoloaded functions — this overhead is measurable and unnecessary when
neither the script nor its dependencies have changed.

Plan caching is enabled by a decision already made in RFC-002: the
`ExecutionPlan` and all `Op` nodes derive `serde::Serialize` and
`serde::Deserialize`. The serialisation infrastructure exists — this RFC
specifies how to use it.

---

## Design

### 1. Cache Location — Per-User Always

The cache is always isolated per OS user. No system-wide shared cache exists.

| Platform | Path |
|---|---|
| Linux | `~/.cache/forge/plans/` |
| macOS | `~/Library/Caches/forge/plans/` |
| Windows | `%LOCALAPPDATA%\forge\cache\plans\` |

**Rules:**
- Cache is always per OS user — no sharing across users, ever
- No configuration option to share — this is a hard rule, not a preference
- CI jobs running as isolated users or in containers get per-job isolation automatically
- Security: one user cannot poison another user's cache

---

### 2. Cache Key

The cache key is a SHA-256 hash that covers the full import graph, the Forge
version, the platform, and the architecture.

```rust
// Step 1 — collect full import graph
let import_graph = resolver.collect_imports("script.fgs")?;
// Returns: ["script.fgs", "utils.fgs", "deploy/helpers.fgs"]
// Reuses the same graph built by forge-lang/resolver for circular import detection

// Step 2 — hash all files, sorted for determinism
let mut hasher = Sha256::new();
for file in import_graph.sorted_by_path() {
    hasher.update(file.path.as_bytes());
    hasher.update(file.content.as_bytes());
}

// Step 3 — include forge version and target
hasher.update(FORGE_VERSION.as_bytes());
hasher.update(platform_identifier().as_bytes());   // "linux" | "macos" | "windows"
hasher.update(arch_identifier().as_bytes());       // "x86_64" | "aarch64"

let cache_key = hex::encode(hasher.finalize());
```

**What invalidates the cache:**

| Change | Effect |
|---|---|
| Any file in import graph modified | Cache miss — full pipeline re-runs |
| New import added to any file | Cache miss — import graph changed |
| Forge Shell version upgrade | All cached plans invalidated |
| Platform change | Miss — plans are platform-specific |
| Architecture change | Miss — plans are arch-specific |

**Why files are sorted before hashing:** File system ordering is not
deterministic across platforms. Sorting by path guarantees the same hash
regardless of OS or file system.

**Import graph reuse:** The resolver already builds the full import graph
for circular import detection (RFC-002). Cache key computation reuses this
graph — no additional resolution work.

---

### 3. Cache Format

Plans are serialised using `postcard` — compact, `no_std`-compatible,
already a dependency for `ExecutionPlan` serialisation.

```rust
#[derive(Serialize, Deserialize)]
pub struct CachedPlan {
    pub format_version: u32,        // cache format version — not Forge version
    pub forge_version:  String,     // Forge binary version string
    pub platform:       String,     // "linux" | "macos" | "windows"
    pub arch:           String,     // "x86_64" | "aarch64"
    pub created_at:     u64,        // Unix timestamp
    pub import_graph:   Vec<String>, // sorted list of files in import graph
    pub plan:           ExecutionPlan,
}
```

**Cache file naming:**

```
~/.cache/forge/plans/{cache_key}.plan
```

**Format version:** If the `format_version` field does not match the current
Forge binary's expected format version, the cached plan is silently discarded
and regenerated. This handles cache format changes across Forge versions
without requiring a manual cache clear.

---

### 4. Cache Lookup Flow

```
forge run script.fgs
        ↓
Resolve import graph
        ↓
Compute cache key (SHA-256 of graph + forge_version + platform + arch)
        ↓
Cache hit? ──YES──▶ Deserialise CachedPlan
                            ↓
                   Format version matches? ──NO──▶ Discard → cache miss
                            ↓ YES
                   Execute ExecutionPlan
        ↓ NO (cache miss)
Lex → Parse → Type Check → HIR → PlatformBackend
        ↓
Serialise CachedPlan to disk
        ↓
Execute ExecutionPlan
```

**Cache writes are best-effort.** A cache write failure (disk full, permission
error) is logged as a warning but never fails the script execution. The script
runs correctly — just without caching for this invocation.

**Cache reads are validated.** A corrupted or truncated cache file is detected
during postcard deserialisation and treated as a cache miss. The corrupted
file is deleted and regenerated.

---

### 5. Cache Eviction

The cache grows as new scripts are executed. Eviction runs automatically
on shell startup — quick, non-blocking, background task.

**Eviction policy:**

| Policy | Detail |
|---|---|
| Max size | 512 MB total cache size — configurable |
| Max age | 30 days — plans older than this are evicted |
| LRU | When size limit reached, least recently used plans evicted first |

**Configurable in `config.toml`:**

```toml
[cache]
max_size_mb  = 512     # total cache size limit
max_age_days = 30      # evict plans older than this
enabled      = true    # disable caching entirely
```

---

### 6. Cache Management Commands

```bash
# Show cache statistics
forge cache stats
# Output:
#   Plans cached:    142
#   Cache size:      48.3 MB / 512 MB
#   Oldest plan:     12 days ago
#   Newest plan:     2 minutes ago

# Clear all cached plans
forge cache clear

# Clear cache for a specific script (and its import graph)
forge cache clear ./deploy.fgs

# Pre-warm cache without executing — useful in CI setup steps
forge cache warm ./deploy.fgs
forge cache warm ./scripts/*.fgs

# Show which files are in a script's import graph
forge cache graph ./deploy.fgs
# Output:
#   deploy.fgs
#   └── utils.fgs
#   └── helpers/deploy.fgs
#       └── helpers/k8s.fgs

# Disable cache for a single invocation
forge run --no-cache script.fgs
```

---

### 7. Cache Warming in CI

For CI environments where startup latency matters, cache warming can be added
as a setup step:

```yaml
# GitHub Actions example
- name: Warm Forge plan cache
  run: forge cache warm ./scripts/*.fgs

- name: Run deploy script
  run: forge run ./scripts/deploy.fgs --env staging
  # Cache hit — pipeline skipped, ExecutionPlan executes directly
```

The warmed cache is valid for the duration of the CI job. Since CI typically
runs as an isolated user, the cache is automatically scoped to the job.

---

### 8. AOT Compilation — Explicitly Deferred

`forge compile` — producing a self-contained native binary from a `.fgs`
script — is explicitly deferred to post-v1.

**Why deferred:**

- Requires a native code generation backend (Cranelift or LLVM) — significant
  engineering investment
- Requires a minimal embedded Forge runtime for path operations, env vars,
  and process spawning
- Cross-compilation (targeting Windows from Linux) adds further complexity
- Plan caching already covers the primary startup latency use case for v1
- AOT should follow a stable v1 with demonstrated demand — not precede it

**Post-v1 AOT design considerations (not specified here):**

- Self-contained binary — no Forge installation required on target machine
- Single binary artifact — ideal for CLI tool distribution
- Cross-compilation support for all three target platforms
- `forge compile --target x86_64-pc-windows-msvc script.fgs`

This RFC covers plan caching only. A future RFC-008b or RFC-014 will specify
AOT compilation when the time is right.

---

## Drawbacks

- **Cache staleness on indirect imports.** If an imported file changes on
  disk between cache key computation and plan execution — a race condition —
  the stale plan executes for that one invocation. This is an inherent
  limitation of file-system-based caching and is considered acceptable. The
  next invocation will recompute correctly.
- **Cache key computation adds latency on cold starts.** Hashing all files
  in the import graph takes time. For scripts with large import graphs (10+
  files) this may be measurable. Mitigated by the fact that the resolver
  already walks the import graph — hashing is incremental over the same walk.
- **Per-user cache means repeated cold starts on multi-user machines.** On
  shared servers where many users run the same scripts, each user pays the
  cold-start cost independently. The security benefit outweighs this cost.

---

## Alternatives Considered

### Alternative A — HIR Caching (bytecode style)

**Approach:** Cache the HIR (before platform backend) rather than the
`ExecutionPlan`.

**Rejected because:** The HIR is platform-agnostic — caching it saves
lex/parse/type-check/HIR time but not the platform backend step. The platform
backend is the most complex stage. Skipping it matters more than skipping
parsing. `ExecutionPlan` caching saves the entire pipeline.

### Alternative B — Shared System Cache

**Approach:** Single cache at `/var/cache/forge/` shared across all users.

**Rejected because:** A shared cache means one user's plan is executed by
another user. Even read-only, this is a trust boundary violation — user A
could poison the cache for user B by crafting a script that produces a
malicious cached plan. Per-user isolation is the correct security model.

### Alternative C — Script Source Hash Only (no import graph)

**Approach:** Cache key based only on the entry script's source.

**Rejected because:** If any imported file changes, the cache hit returns a
stale plan built with the old import. This is a silent correctness bug — the
worst possible failure mode for a caching system. Full import graph hashing
is required for correctness.

### Alternative D — Directory Hash

**Approach:** Cache key includes hash of entire script directory.

**Rejected because:** Any file change in the directory — README, config files,
data files — invalidates the cache. This is far too aggressive and defeats
the purpose of caching in project directories where scripts live alongside
other files.

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | Cache shared across users? | Per-user always — no sharing, no exceptions |
| UQ-2 | `forge compile` in v1? | Deferred entirely — plan caching only in v1 |
| UQ-3 | Cross-compilation story? | Not applicable — deferred with AOT |
| UQ-4 | Compiled binaries `forge --version`? | Not applicable — deferred with AOT |
| UQ-5 | Cache key for imported scripts? | Full import graph hash — all files in graph |

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-cache` | Cache read/write, key computation, eviction, validation |
| `forge-lang/resolver` | Import graph collection — reused for cache key |
| `forge-engine` | Cache lookup integration — check before pipeline, write after |
| `forge-cli/cache` | `forge cache` subcommands — stats, clear, warm, graph |

### Dependencies

- Requires RFC-002 (evaluation pipeline) — `ExecutionPlan` with `Serialize`/`Deserialize`
- Requires RFC-013 (shell config) — `[cache]` config section

### Milestones

1. Implement import graph collection in `forge-lang/resolver` — reuse circular import graph
2. Implement cache key computation — SHA-256 of sorted import graph + version + platform
3. Implement `CachedPlan` serialisation/deserialisation via `postcard`
4. Implement cache read in `forge-engine` — lookup before pipeline
5. Implement cache write in `forge-engine` — best-effort after pipeline
6. Implement cache validation — format version check, corruption detection
7. Implement cache eviction — LRU + age + size limit, background on startup
8. Implement `forge cache stats` — plan count, size, age range
9. Implement `forge cache clear` — all and per-script
10. Implement `forge cache warm` — pre-warm without executing
11. Implement `forge cache graph` — show import graph for a script
12. Implement `--no-cache` flag for single-invocation bypass
13. Integration tests — cache hit/miss, invalidation, corruption recovery, eviction

---

## References

- [postcard — Compact binary serialisation](https://docs.rs/postcard)
- [Go build cache](https://pkg.go.dev/cmd/go#hdr-Build_and_test_caching)
- [Python .pyc bytecode caching](https://peps.python.org/pep-3147/)
- [Cargo build cache](https://doc.rust-lang.org/cargo/guide/build-cache.html)
- [RFC-002 — Evaluation Pipeline](./RFC-002-evaluation-pipeline.md)
- [RFC-013 — Shell Configuration Model](./RFC-013-shell-config.md)