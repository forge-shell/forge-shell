# RFC-008 — Plan Caching & Ahead-of-Time Compilation

| Field          | Value                        |
|----------------|------------------------------|
| Status         | Draft                        |
| Author         | Ajitem Sahasrabuddhe         |
| Created        | 2026-04-09                   |
| Last Updated   | 2026-04-09                   |
| Supersedes     | —                            |
| Superseded By  | —                            |

---

## Summary

This RFC defines the plan caching system for Forge Shell and the future
`forge compile` command for ahead-of-time (AOT) compilation of `.fgs` scripts.
Plan caching accelerates repeated script execution by persisting the
platform-lowered `ExecutionPlan` to disk. AOT compilation takes this further,
producing a standalone native binary from a `.fgs` script.

---

## Motivation

Without caching, every invocation of a `.fgs` script runs the full pipeline:
lex → parse → HIR → platform lower → execute. For scripts invoked frequently
(deployment scripts, CI steps, shell functions), this overhead is measurable.

For scripted environments where startup latency matters — CI pipelines,
container entrypoints, serverless functions — a pre-compiled binary is
significantly more attractive than an interpreted script.

---

## Design

### 1. Plan Caching

#### Cache Location

```
~/.forge/cache/
├── scripts/
│   ├── {sha256}.linux.plan
│   ├── {sha256}.macos.plan
│   └── {sha256}.windows.plan
└── meta.json          # cache metadata, version tracking
```

#### Cache Key

The cache key is a compound hash:

```
SHA-256(
  script_source_bytes
  + forge_version_string
  + platform_identifier        # "linux" | "macos" | "windows"
  + arch_identifier            # "x86_64" | "aarch64"
)
```

This ensures that:
- Any change to the script source invalidates the cache
- A Forge Shell upgrade invalidates all cached plans
- Plans are platform-specific — a Linux plan is never used on macOS

#### Cache Format

Plans are serialised using `postcard` — a compact, `no_std`-compatible binary
serialisation format for Rust. Plans are versioned: a plan whose version
does not match the current Forge binary is silently discarded and regenerated.

```rust
#[derive(Serialize, Deserialize)]
struct CachedPlan {
    version:    u32,            // Forge plan format version
    forge_ver:  String,         // Forge binary version
    platform:   String,
    arch:       String,
    created_at: u64,            // Unix timestamp
    plan:       ExecutionPlan,
}
```

#### Cache Lookup Flow

```
forge script.fgs
    │
    ├─ compute cache key
    │
    ├─ cache hit? ──YES──▶ deserialise plan ──▶ execute
    │
    └─ cache miss ──────▶ lex → parse → HIR → lower
                                                │
                                                ├─ serialise to cache
                                                │
                                                └─ execute
```

#### Cache Management Commands

```bash
# Show cache statistics
forge cache stats

# Clear all cached plans
forge cache clear

# Clear cache for a specific script
forge cache clear ./deploy.fgs

# Disable cache for a single invocation
forge --no-cache script.fgs

# Pre-warm cache without executing
forge cache warm ./deploy.fgs
forge cache warm ./scripts/*.fgs
```

---

### 2. Ahead-of-Time Compilation (`forge compile`)

AOT compilation takes a `.fgs` script through the full pipeline and then
emits a native executable via Cranelift (embedded in `wasmtime`) or a future
LLVM backend.

#### Usage

```bash
# Compile for the current platform
forge compile deploy.fgs

# Produces: deploy (Unix) or deploy.exe (Windows)

# Compile with output name
forge compile deploy.fgs --output dist/deploy

# Cross-compile (future)
forge compile deploy.fgs --target x86_64-windows
forge compile deploy.fgs --target aarch64-linux
```

#### Compilation Pipeline

```
deploy.fgs
    │
    ▼ lex → parse → HIR → platform lower
    │
    ▼ ExecutionPlan
    │
    ▼ IR generation (Cranelift IR)
    │
    ▼ Native code emission
    │
    ▼ Link with forge-runtime (minimal Rust runtime)
    │
    ▼ deploy (native binary)
```

The compiled binary embeds a minimal `forge-runtime` — the execution engine
and built-in implementations. It does not require Forge Shell to be installed
on the target system.

#### Binary Size Considerations

A compiled `.fgs` script embeds:
- The execution engine (`forge-exec`)
- Only the built-in commands used by the script (link-time dead code elimination)
- The platform lowering backend for the target platform

Estimated binary size for a simple deploy script: 2–5 MB (similar to a
minimal Go binary).

#### Limitations

- Scripts that use `import` with dynamic paths cannot be AOT compiled
- Plugins (WASM) are embedded as WASM modules — they are not natively compiled
- `eval()` and dynamic code execution are not supported in compiled binaries

---

### 3. Cache Eviction Policy

The plan cache has a configurable size limit. Default: 500 MB.

Eviction policy: **LRU (Least Recently Used)**

```toml
# ~/.forge/config.fgs
[cache]
max_size_mb   = 500
max_age_days  = 30    # plans older than 30 days are evicted regardless of size
enabled       = true
```

---

## Drawbacks

- **Cache invalidation complexity** — the compound cache key must be carefully
  designed. A bug in key computation leads to stale plans being used.
- **Binary format stability** — the `postcard` serialisation format for
  `ExecutionPlan` must be versioned carefully. Any structural change to
  `ExecutionPlan` requires a format version bump.
- **AOT compilation scope** — full AOT compilation is a significant
  engineering effort (IR generation, linking). This is a v2+ feature.
- **Cross-compilation** — targeting Windows from Linux (or vice versa) requires
  the full platform lowering for the target, not just the host. The cross-
  compilation backend must run all three lowering implementations.

---

## Alternatives Considered

### Alternative A — Bytecode Caching (like Python .pyc)

**Approach:** Cache the HIR (before platform lowering) rather than the
`ExecutionPlan`.
**Rejected because:** The HIR is platform-agnostic — caching it saves
lex/parse/HIR time but not the lowering step. Since lowering is the most
complex stage, skipping it matters more than skipping parsing.

### Alternative B — No Caching

**Approach:** Always run the full pipeline.
**Rejected because:** For scripts invoked hundreds of times (e.g. a custom
`ll` alias implemented as a `.fgs` function), the lex → lower overhead is
perceptible. Caching pays for itself quickly.

---

## Unresolved Questions

- [ ] Should the cache be shared across users on multi-user systems, or always
      per-user?
- [ ] Should `forge compile` be available in v1 or deferred to v2?
- [ ] What is the cross-compilation story? Which target triples are supported?
- [ ] Should compiled binaries support `forge --version` output to identify
      their Forge version?
- [ ] How should the cache handle scripts that `import` other `.fgs` files?
      The import graph must be included in the cache key.

---

## Implementation Plan

### Affected Crates

- `forge-exec/cache` — cache read/write, key computation, eviction
- `forge-cli` — `forge cache` subcommand, `forge compile` subcommand
- `forge-lang/ast` — must be serialisable (`Serialize`/`Deserialize`)
- `forge-exec/plan` — `ExecutionPlan` must be serialisable

### Dependencies

- Requires RFC-002 (Evaluation Pipeline) — `ExecutionPlan` is the cached
  artefact

### Milestones

**Phase 1 — Plan Caching (v1)**
1. Add `Serialize`/`Deserialize` to `ExecutionPlan` via `postcard`
2. Implement cache key computation
3. Implement cache read/write in `forge-exec/cache`
4. Implement cache eviction (LRU, size limit, age limit)
5. Wire cache into the execution pipeline
6. Implement `forge cache stats/clear/warm` commands
7. Integration tests — cache hit/miss, invalidation

**Phase 2 — AOT Compilation (v2)**
1. Evaluate Cranelift vs LLVM as IR backend
2. Design `forge-runtime` minimal embedded runtime
3. Implement IR generation from `ExecutionPlan`
4. Implement `forge compile` command
5. Cross-compilation support

---

## References

- [`postcard` — Compact binary serialisation](https://docs.rs/postcard)
- [Cranelift Code Generator](https://cranelift.dev)
- [Python .pyc bytecode format](https://peps.python.org/pep-3147/)
- [Go build cache](https://pkg.go.dev/cmd/go#hdr-Build_and_test_caching)
