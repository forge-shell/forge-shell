# RFC-005 — Plugin System & WASM Capability Model

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

This RFC defines the Forge Shell plugin system — a WebAssembly (WASM) based
extensibility model that allows third-party developers to add commands,
completions, prompt segments, and aliases to Forge Shell. Plugins are
sandboxed via a capability model, communicate with the host exclusively via
postcard serialisation, and run identically on all three platforms. The host
owns all output rendering decisions — plugins are first-class citizens
indistinguishable from built-in commands at the output layer.

---

## Motivation

No shell can anticipate every workflow. Forge Shell needs a safe, cross-
platform, language-agnostic extension mechanism. The requirements are:

- **Cross-platform** — a plugin built once runs on Linux, macOS, and Windows
- **Sandboxed** — plugins cannot access resources beyond their declared capabilities
- **Language-agnostic** — plugins can be written in any WASM-targeting language
- **Versioned** — the plugin ABI is stable and independently versioned
- **Discoverable** — plugins are found via a lightweight static index
- **Safe** — resource limits protect the shell from buggy or malicious plugins

WebAssembly via `wasmtime` satisfies all six requirements simultaneously.

---

## Design

### 1. Plugin Architecture

```
forge-shell (host)
    ├── wasmtime runtime
    ├── capability enforcement layer
    ├── resource limit enforcement (fuel metering + memory limits)
    └── output rendering (owns RichTerminal / PlainText / Structured)
            ↑ postcard — always
plugin.wasm
    ├── plugin logic
    ├── forge-plugin-sdk (Rust or Go)
    └── declared capabilities + limits
```

Plugins communicate with the host exclusively via postcard serialisation.
The host owns all output rendering — `RichTerminal`, `PlainText`, and
`Structured` (JSON for AI/MCP). Plugins never import `serde_json` and never
make rendering decisions.

---

### 2. Plugin Manifest — `forge-plugin.toml`

Every plugin ships a `forge-plugin.toml` manifest:

```toml
[plugin]
name        = "forge-git"
version     = "1.2.0"
abi         = "1"               # target ABI version
description = "Git integration for Forge Shell"
authors     = ["Ajitem Sahasrabuddhe"]
license     = "MIT"
homepage    = "https://github.com/forge-shell/forge-git"

# Capabilities — declared at install time, enforced at runtime
[capabilities]
exec        = ["git"]           # allowed external commands
filesystem  = "read"            # "none" | "read" | "read-write"
network     = false             # true | false
env         = "read"            # "none" | "read" | "read-write"

# Resource limits — within global ceiling
[limits]
memory      = "32MB"            # default — omit to use default
cpu_timeout = "5s"              # default — omit to use default

# Commands provided by this plugin
[[commands]]
name        = "git-log"
description = "Rich git log viewer"
positional  = ["path"]          # positional argument order

[[commands]]
name        = "git-status"
description = "Rich git status viewer"

# Aliases provided by this plugin
[aliases]
gst  = "git-status"
glog = "git-log"
gl   = "git pull"
gp   = "git push"
gco  = "git checkout"

# Prompt segments provided by this plugin
[[prompt.segments]]
name        = "git"
description = "Git branch and status"

# Built-in overrides — requires explicit declaration
overrides   = []                # e.g. ["ls"] to override built-in ls

# Output schemas for custom payload types
[[output.schemas]]
command     = "git-log"
type        = "GitLog"          # Rust type in forge-plugin-sdk shared types
version     = 1

[[output.schemas]]
command     = "git-status"
type        = "GitStatus"
version     = 1
```

---

### 3. ABI Versioning

The plugin ABI is the contract between Forge Shell and every plugin. It is
versioned as a simple integer — not semver.

**Rules:**
- ABI version incremented only on breaking changes to host function interface
- Additive changes (new host functions) do not bump the ABI version
- Forge Shell supports multiple ABI versions simultaneously
- Deprecation window: minimum 12 months before removing an ABI version
- Deprecation announced in release notes with a migration guide
- `forge plugin check` warns if an installed plugin targets a deprecated ABI

**Crate structure:**

```
forge-plugin/abi/v1    — ABI v1 host implementation
forge-plugin/abi/v2    — ABI v2 host implementation (when it exists)
```

---

### 4. Plugin SDK — Separate Repositories

The plugin SDK lives in separate repositories — independent release cadence,
distinct audience, lighter CI for plugin authors.

| Repository | Purpose |
|---|---|
| `github.com/forge-shell/forge-shell` | Main repo |
| `github.com/forge-shell/forge-plugin-sdk` | Rust plugin SDK |
| `github.com/forge-shell/forge-plugin-sdk-go` | Go plugin SDK |

**SDK version is pinned to ABI version:**
- `forge-plugin-sdk v1.x` → targets ABI v1
- `forge-plugin-sdk v2.x` → targets ABI v2

Community SDKs for other WASM-targeting languages (Zig, C, AssemblyScript)
are welcome and documented at forge-shell.dev.

---

### 5. Output Architecture — Host Owns All Rendering

Plugins communicate output via postcard-serialised typed data. The host
detects the `OutputMode` from `CommandContext` and renders accordingly:

```
Plugin (WASM)
    ↓  postcard — always, no exceptions
Forge Shell Host
    ↓  CommandContext::output_mode
    ├── RichTerminal  → rich table, colours, icons
    ├── PlainText     → plain text, no formatting
    └── Structured    → JSON (AI/MCP consumers — RFC-007)
```

**Benefits:**
- Plugins never import `serde_json` — postcard only
- New output modes automatically supported by all plugins — no plugin updates needed
- Plugins are indistinguishable from built-ins at the output layer
- Type information preserved all the way from plugin to host

**Custom output types — shared via SDK:**

```rust
// forge-plugin-sdk — shared types
// Plugin serialises with postcard, host deserialises to this type
#[derive(Serialize, Deserialize)]
pub struct GitStatus {
    pub branch:   String,
    pub ahead:    u32,
    pub behind:   u32,
    pub staged:   Vec<GitFile>,
    pub unstaged: Vec<GitFile>,
    pub untracked: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct GitFile {
    pub path:   String,
    pub status: GitFileStatus,
}
```

**Host rendering logic** ships alongside the `.wasm` binary in the plugin
package — the plugin author writes both the WASM logic and the host-side
rendering for their custom types.

---

### 6. Capability Model

Capabilities are declared in the manifest and enforced at the WASM host level
by wasmtime — not by the plugin. A plugin cannot access resources beyond its
declared capabilities regardless of what its code attempts.

| Capability | Values | Description |
|---|---|---|
| `exec` | `["git", "kubectl"]` | Allowed external commands — allowlist |
| `filesystem` | `"none"` / `"read"` / `"read-write"` | Filesystem access level |
| `network` | `true` / `false` | Network access |
| `env` | `"none"` / `"read"` / `"read-write"` | Environment variable access |

**Install-time transparency:**

```
Installing: forge-kubectl v2.1.0
Source:      github.com/ajitem/forge-kubectl
Verified:    ✅ Reviewed by Forge Shell team
Capabilities: exec["kubectl", "helm"], filesystem:read, env:read
Limits:      memory: 64MB (↑ above default 32MB), cpu: 15s (↑ above default 5s)
             ⚠️  This plugin requests above-default resource limits
Proceed? [y/N]
```

Above-default resource limits are always surfaced at install time — never a
runtime surprise.

---

### 7. Resource Limits

**Global ceiling — hard limits enforced by wasmtime, never exceeded:**

| Resource | Default | Global ceiling | On exceed |
|---|---|---|---|
| Memory | 32 MB | 256 MB | Default exceeded → warning; ceiling → hard kill |
| CPU timeout | 5 seconds | 60 seconds | Default exceeded → warning; ceiling → hard kill |
| Stack depth | 512 KB | 1 MB | Ceiling exceeded → hard kill |
| File descriptors | 16 | 64 | Ceiling exceeded → hard kill |

**Rationale for defaults:**
- A shell plugin is not a server process — it must feel interactive
- 32 MB covers git operations, JSON parsing, template rendering
- 5 second timeout — operations longer than this feel broken interactively
- Plugins needing more declare it explicitly in `[limits]`

**Implementation:** wasmtime fuel metering for CPU, wasmtime memory limits
for allocation — enforced at the WASM host level, not by plugin code.

```toml
# Plugin that legitimately needs more
[limits]
memory      = "128MB"   # e.g. ripgrep indexing large repos
cpu_timeout = "30s"     # e.g. docker image metadata fetch
```

---

### 8. Plugin Distribution — Decentralised Model

Forge Shell uses a Go-style decentralised model. No plugin binaries are
hosted by the Forge Shell project.

**Install resolution — three tiers:**

```
Tier 1 — Canonical short name (official + verified only)
  forge-git → plugins.forge-shell.dev/forge-git → github.com/forge-shell/forge-git

Tier 2 — Publisher namespace (registered publishers)
  ajitem/forge-kubectl → plugins.forge-shell.dev/ajitem/forge-kubectl → github.com/ajitem/forge-kubectl

Tier 3 — Direct source URL (no index needed)
  github.com/ajitem/forge-kubectl@v2.1.0
  gitlab.com/user/plugin@v1.0.0
```

**Install examples:**

```bash
forge plugin install forge-git                         # Tier 1
forge plugin install forge-git@v1.2.0                  # Tier 1 — pinned
forge plugin install ajitem/forge-kubectl              # Tier 2
forge plugin install ajitem/forge-kubectl@v2.0.0       # Tier 2 — pinned
forge plugin install github.com/user/plugin            # Tier 3
forge plugin install github.com/user/plugin@v1.0.0     # Tier 3 — pinned
```

**What Forge Shell hosts — minimally:**

| Component | What it is | Hosting cost |
|---|---|---|
| `plugins.forge-shell.dev` | Static discovery index | Minimal |
| `sum.forge-shell.dev` | Append-only hash log — tamper detection | Minimal |
| `github.com/forge-shell/plugin-index` | TOML index source — PR-based submissions | Zero |
| Official plugins (`forge-shell/forge-*`) | GitHub repos | Zero |

**Plugin index entry:**

```toml
# plugin-index/plugins.toml

[[plugin]]
name       = "forge-git"
vanity     = "forge-git"                        # canonical short name
source     = "github.com/forge-shell/forge-git"
verified   = true
official   = true
public_key = "ed25519:AAAA..."

[[plugin]]
name       = "forge-kubectl"
vanity     = "ajitem/forge-kubectl"             # publisher namespace
source     = "github.com/ajitem/forge-kubectl"
verified   = true
official   = false
public_key = "ed25519:BBBB..."
```

**Publisher namespace registration:**

```toml
# plugin-index/publishers.toml
[[publisher]]
namespace  = "ajitem"
github     = "ajitem-sahasrabuddhe"
verified   = true
public_key = "ed25519:CCCC..."
```

Namespace ownership prevents typosquatting. Once `ajitem` is claimed, only
that publisher can register `ajitem/*` vanity URLs.

---

### 9. Code Signing

| Plugin tier | Signing | Install behaviour |
|---|---|---|
| Official | ✅ Required | Silent — always trusted |
| Verified | ✅ Required | Silent — badge confirms review + binary integrity |
| Unverified | ❌ Optional | Warning shown — user confirms |

**Signing algorithm:** Ed25519

**Signing toolchain:**

```bash
forge plugin keygen                           # generate Ed25519 keypair
forge plugin sign plugin.wasm --key my.key   # sign → plugin.wasm.sig
forge plugin verify plugin.wasm              # verify locally before publishing
```

**Public key stored in `plugin-index/plugins.toml`** — verified at install
time against the downloaded `.wasm` binary. Ensures the installed binary
matches what was reviewed.

---

### 10. Plugin Lifecycle Commands

```bash
forge plugin install forge-git              # install
forge plugin install forge-git@v1.2.0      # install pinned version
forge plugin remove forge-git              # remove
forge plugin update forge-git              # update to latest
forge plugin update --all                  # update all installed plugins
forge plugin list                          # list installed plugins
forge plugin search docker                 # search the index
forge plugin info forge-git                # show manifest details
forge plugin check                         # check for deprecated ABIs
forge plugin keygen                        # generate signing keypair
forge plugin sign plugin.wasm              # sign a plugin binary
forge plugin verify plugin.wasm            # verify a plugin binary
```

---

## Drawbacks

- **WASM toolchain requirement** — plugin authors must target WASM. Shell
  scripts themselves cannot compile to WASM.
- **Limited WASI surface** — WASI preview 2 is still maturing. Some OS APIs
  are not yet available in WASM.
- **SDK maintenance** — `forge-plugin-sdk` must be versioned carefully.
  Breaking ABI changes require coordinated SDK releases.
- **Host rendering per plugin** — plugin authors must write host-side rendering
  code alongside their WASM binary. More work per plugin, but correct.

---

## Alternatives Considered

### Alternative A — Native Dynamic Libraries

**Rejected:** Platform-specific — `.so` does not run on Windows. No sandbox
model. Three separate builds per plugin.

### Alternative B — gRPC-based plugins (HashiCorp go-plugin style)

**Rejected:** Requires Go or gRPC. Process-per-plugin is heavyweight.
Platform-specific builds still required.

### Alternative C — `.fgs` script plugins

**Rejected:** ForgeScript scripts cannot be safely sandboxed — full access to
the shell's execution model. A malicious `.fgs` plugin is a security disaster.

### Alternative D — Centralised registry (crates.io style)

**Rejected:** Hosting, maintaining, and securing a global registry is
expensive and operationally complex. The Go-style decentralised model achieves
discovery without hosting binaries.

### Alternative E — JSON for plugin→host communication

**Rejected:** JSON loses type information at the boundary — `i64::MAX` loses
precision in JavaScript parsers, `path` and `url` become untyped strings,
enums lose variant information. Postcard preserves all type information.
JSON is used only at the AI/MCP boundary, where the host converts from typed
data.

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | WASM ABI version strategy | Multiple versions simultaneously — 12 month deprecation window |
| UQ-2 | Plugin SDK location | Separate repos — `forge-plugin-sdk`, `forge-plugin-sdk-go` |
| UQ-3 | Structured output types | Postcard always — host owns all rendering via `OutputMode` |
| UQ-4 | Registry governance | Go-style decentralised — static index, vanity URLs, no hosted binaries |
| UQ-5 | Code signing | Required for verified/official, optional with warning for unverified |
| UQ-6 | Resource limits | Global ceiling + manifest declaration — sensible interactive defaults |

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-plugin` | WASM host, capability enforcement, resource limits, plugin lifecycle |
| `forge-plugin/abi/v1` | ABI v1 host function implementations |
| `forge-plugin/index` | Plugin index client — resolution, vanity URLs |
| `forge-plugin/signing` | Ed25519 signing and verification |
| `forge-cli/plugin` | `forge plugin` subcommand implementations |

**Separate repositories:**

| Repository | Responsibility |
|---|---|
| `forge-plugin-sdk` | Rust SDK — shared types, postcard helpers, ABI bindings |
| `forge-plugin-sdk-go` | Go SDK |
| `plugin-index` | Static index — `plugins.toml`, `publishers.toml` |
| `forge-git` | Official reference plugin |
| `forge-docker` | Official reference plugin |
| `forge-kubectl` | Official reference plugin |

### Dependencies

- Requires RFC-001 (ForgeScript syntax) — plugin command invocation
- Requires RFC-002 (evaluation pipeline) — plugin resolution at HIR stage
- Requires RFC-003 (built-in commands) — `StructuredOutput` shared with plugins
- Requires RFC-009 (plugin registry) — index format and distribution

### Milestones

1. Define plugin ABI v1 — host functions and plugin exports
2. Implement wasmtime plugin host in `forge-plugin`
3. Implement capability enforcement layer
4. Implement resource limit enforcement — fuel metering + memory limits
5. Implement plugin index client — resolution, vanity URLs, three-tier lookup
6. Implement Ed25519 signing and verification
7. Implement `forge plugin` subcommands — install, remove, update, list, search
8. Write `forge-plugin-sdk` — Rust SDK with shared types and postcard helpers
9. Write `forge-plugin-sdk-go` — Go SDK
10. Build `forge-git` reference plugin
11. Build `forge-docker` reference plugin
12. Build `forge-kubectl` reference plugin
13. Integration tests on ubuntu-latest, macos-latest, windows-latest

---

## References

- [wasmtime — Fast and secure runtime for WebAssembly](https://wasmtime.dev/)
- [WASI Preview 2](https://github.com/WebAssembly/WASI/blob/main/preview2/README.md)
- [Go module system](https://go.dev/ref/mod)
- [sum.golang.org — Go transparency log](https://sum.golang.org/)
- [postcard serialisation format](https://github.com/jamesmunns/postcard)
- [Ed25519 signatures](https://ed25519.cr.yp.to/)
- [VS Code extension API](https://code.visualstudio.com/api)
- [RFC-002 — Evaluation Pipeline](./RFC-002-evaluation-pipeline.md)
- [RFC-003 — Built-in Command Specification](./RFC-003-builtin-commands.md)
- [RFC-007 — AI Agent Layer](./RFC-007-ai-agent-layer.md)
- [RFC-009 — Plugin Registry](./RFC-009-plugin-registry.md)