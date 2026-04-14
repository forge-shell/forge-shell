# RFC-005 — Plugin System & WASM Capability Model

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

This RFC defines the Forge Shell plugin system — a WebAssembly (WASM) based
extensibility model that allows third-party developers to add commands,
completions, prompt hooks, and output formatters to Forge Shell. Plugins are
sandboxed via a capability model and run identically on all three platforms.

---

## Motivation

No shell can anticipate every workflow. Forge Shell needs a safe, cross-
platform, language-agnostic extension mechanism. The requirements are:

- **Cross-platform** — a plugin built once runs on Linux, macOS, and Windows
- **Sandboxed** — plugins cannot access resources beyond their declared permissions
- **Language-agnostic** — plugins can be written in any WASM-targeting language
- **Versioned** — the plugin API is stable and independently versioned
- **Discoverable** — plugins are installed from a central registry

WebAssembly satisfies all five requirements. It is the only viable option
that meets all of them simultaneously.

---

## Design

### 1. WASM as the Plugin Runtime

Plugins are compiled to WASM modules (`.wasm`). Forge Shell hosts them via
`wasmtime` — the same engine used by the Wasmtime project and the Bytecode
Alliance.

```
Plugin source (Rust/Go/C/AssemblyScript/...)
              ↓
        WASM compiler
              ↓
     plugin-name.wasm
              ↓
  forge plugin install plugin-name.wasm
              ↓
    wasmtime hosts the module
              ↓
  Forge Shell invokes exported functions
```

The plugin author never needs to write platform-specific code. The `.wasm`
binary is identical on all three platforms.

---

### 2. Plugin Manifest

Every plugin ships with a `forge-plugin.toml` manifest:

```toml
[plugin]
name        = "forge-git"
version     = "1.2.0"
description = "First-class Git integration for Forge Shell"
author      = "Ajitem Sahasrabuddhe"
homepage    = "https://github.com/forge-shell/forge-git"
license     = "MIT"
min_forge   = "0.1.0"    # minimum Forge Shell version

[capabilities]
exec        = ["git"]          # may spawn only the git binary
filesystem  = ["read"]         # read-only filesystem access
network     = false            # no outbound network access
env         = ["read"]         # may read environment variables
stdin       = true             # may read from stdin
stdout      = true             # may write to stdout
stderr      = true             # may write to stderr

[[commands]]
name        = "git-status"
description = "Show working tree status with enhanced output"
usage       = "git-status [path]"

[[completions]]
command     = "git"
description = "Completions for git subcommands and flags"

[[hooks]]
event       = "prompt"
description = "Adds git branch and status to the prompt"
```

---

### 3. Capability Model

Capabilities are declared in `forge-plugin.toml` and enforced at the WASM
boundary by the plugin host. A plugin cannot access any resource not listed
in its capabilities.

#### Available Capabilities

| Capability | Values | Description |
|---|---|---|
| `exec` | `["cmd1", "cmd2"]` or `["*"]` | Commands the plugin may spawn |
| `filesystem` | `["read"]`, `["read", "write"]` | Filesystem access level |
| `filesystem.paths` | `["./", "/tmp"]` | Path restrictions (optional) |
| `network` | `true` / `false` | Any outbound network access |
| `network.hosts` | `["api.github.com"]` | Host allowlist (if network = true) |
| `env` | `["read"]`, `["read", "write"]` | Environment variable access |
| `env.keys` | `["GITHUB_TOKEN", "HOME"]` | Specific variable allowlist |
| `stdin` | `true` / `false` | Read from stdin |
| `stdout` | `true` / `false` | Write to stdout |
| `stderr` | `true` / `false` | Write to stderr |

#### Capability Enforcement

Capabilities are enforced by the `wasmtime` host via WASI preview 2's
component model. Calls that exceed declared capabilities raise a
`CapabilityViolation` error — the plugin is terminated and Forge Shell
reports the violation clearly.

```
Error: Plugin 'forge-git' attempted network access to 'api.github.com'
       but 'network' capability was not declared in forge-plugin.toml.

       To allow this, add to forge-plugin.toml:
         [capabilities]
         network = true
         network.hosts = ["api.github.com"]

       Then reinstall the plugin: forge plugin install forge-git
```

---

### 4. Plugin API

The plugin API is a set of host functions that Forge Shell exposes to plugins.
Plugins call these functions to interact with the shell.

#### Host Functions (exposed to plugins)

```rust
// Forge Shell exposes these to the WASM module

// Command registration
forge_register_command(name: str, description: str, usage: str)
forge_register_completion(command: str)
forge_register_hook(event: HookEvent)

// I/O
forge_write_stdout(data: &[u8])
forge_write_stderr(data: &[u8])
forge_read_stdin() -> Vec<u8>

// Process execution (subject to exec capability)
forge_exec(cmd: str, args: &[str]) -> ExecResult

// Filesystem (subject to filesystem capability)
forge_fs_read(path: str) -> Result<Vec<u8>>
forge_fs_write(path: str, data: &[u8]) -> Result<()>
forge_fs_list(path: str) -> Result<Vec<DirEntry>>

// Environment (subject to env capability)
forge_env_get(key: str) -> Option<str>
forge_env_set(key: str, value: str)   // requires env.write capability

// Structured output
forge_emit_record(record: &Record)    // emit structured data
forge_emit_table(table: &Table)
```

#### Plugin Exports (called by Forge Shell)

```rust
// Forge Shell calls these on the WASM module

// Lifecycle
fn forge_plugin_init()              // called once at plugin load
fn forge_plugin_destroy()           // called at shell exit

// Command execution
fn forge_run_command(args: &[str]) -> CommandResult

// Completion
fn forge_complete(partial: str, context: &CompletionContext) -> Vec<Completion>

// Hooks
fn forge_hook_prompt() -> PromptFragment
fn forge_hook_pre_exec(cmd: &str) -> HookResult
fn forge_hook_post_exec(cmd: &str, exit_code: i32) -> HookResult
```

---

### 5. Plugin Installation & Management

```bash
# Install from registry
forge plugin install forge-git
forge plugin install forge-kubectl

# Install specific version
forge plugin install forge-git@1.2.0

# Install from local file
forge plugin install ./my-plugin.wasm

# Install from URL
forge plugin install https://plugins.forge-shell.dev/forge-git-1.2.0.wasm

# List installed plugins
forge plugin list

# Show plugin details and declared capabilities
forge plugin info forge-git

# Update a plugin
forge plugin update forge-git
forge plugin update --all

# Remove a plugin
forge plugin remove forge-git

# Disable without removing
forge plugin disable forge-git
forge plugin enable forge-git
```

---

### 6. Plugin Storage

```
~/.forge/
├── plugins/
│   ├── forge-git/
│   │   ├── forge-plugin.toml     # manifest
│   │   ├── forge-git.wasm        # compiled plugin
│   │   └── data/                 # plugin-writable data dir (if filesystem.write)
│   └── forge-kubectl/
│       ├── forge-plugin.toml
│       └── forge-kubectl.wasm
├── cache/
└── config.fgs
```

---

### 7. Plugin Registry

The official registry is hosted at `plugins.forge-shell.dev`. It is a simple
static registry — a JSON index of published plugins.

```json
{
  "plugins": [
    {
      "name":        "forge-git",
      "version":     "1.2.0",
      "description": "First-class Git integration",
      "author":      "forge-shell",
      "url":         "https://plugins.forge-shell.dev/forge-git-1.2.0.wasm",
      "sha256":      "abc123...",
      "min_forge":   "0.1.0"
    }
  ]
}
```

All plugin downloads are verified against their declared SHA-256 hash before
installation. Forge Shell refuses to install a plugin whose hash does not match.

---

### 8. First-Party Reference Plugins

Forge Shell ships with two reference plugins that demonstrate the API and
serve as integration tests:

| Plugin | Commands | Capabilities |
|---|---|---|
| `forge-git` | `git-status`, `git-log`, `git-branch` | `exec: ["git"]`, `filesystem: ["read"]` |
| `forge-kubectl` | `kube-ctx`, `kube-pods`, `kube-logs` | `exec: ["kubectl"]`, `env: ["read"]` |

---

### 9. Writing a Plugin (Rust Example)

```rust
// forge-plugin SDK crate
use forge_plugin_sdk::{plugin, command, completion, PromptHook};

#[plugin]
struct GitPlugin;

#[command(name = "git-status", description = "Enhanced git status")]
fn git_status(args: &[String]) -> CommandResult {
    let output = forge_exec("git", &["status", "--porcelain"])?;
    // format and emit structured output
    CommandResult::ok()
}

#[completion(command = "git")]
fn git_complete(partial: &str, ctx: &CompletionContext) -> Vec<Completion> {
    // return completions
    vec![]
}

#[prompt_hook]
fn prompt_fragment() -> PromptFragment {
    let branch = forge_exec("git", &["branch", "--show-current"])
        .ok()
        .map(|o| o.stdout_string());
    PromptFragment::new(branch.unwrap_or_default())
}
```

---

## Drawbacks

- **WASM overhead** — function calls across the WASM boundary have non-trivial
  overhead. Plugins that make many small calls will be slower than native code.
- **WASM toolchain requirement** — plugin authors must target WASM. Some
  languages (e.g. shell scripts themselves) cannot compile to WASM.
- **Limited WASI surface** — WASI preview 2 is still maturing. Some OS APIs
  are not yet available in WASM.
- **Plugin SDK maintenance** — the `forge-plugin-sdk` crate must be maintained
  and versioned carefully. Breaking changes affect all plugins.

---

## Alternatives Considered

### Alternative A — Native Dynamic Libraries (.so/.dylib/.dll)

**Approach:** Load plugins as native shared libraries.
**Rejected because:** Native libraries are platform-specific — a `.so` does not
run on Windows. Cross-platform plugin distribution requires three separate
builds. No sandbox model exists for native libraries.

### Alternative B — HashiCorp go-plugin (gRPC-based)

**Approach:** Plugins are separate processes communicating over gRPC.
**Rejected because:** Requires Go or a gRPC-capable language. Process-per-plugin
is heavyweight. Cross-platform distribution still requires platform-specific builds.

### Alternative C — Script Plugins (.fgs files)

**Approach:** Plugins are `.fgs` scripts.
**Rejected because:** ForgeScript scripts cannot safely be sandboxed — they have
full access to the shell's execution model. A malicious `.fgs` plugin could
spawn arbitrary processes, exfiltrate environment variables, or modify shell
state.

---

## Unresolved Questions

- [ ] What is the WASM ABI version strategy? How are breaking API changes
      communicated and handled?
- [ ] Should the plugin SDK be a separate crate or part of `forge-plugin`?
- [ ] Should plugins be allowed to define their own structured output types,
      or are they limited to the built-in record/table types?
- [ ] What is the registry governance model? Who can publish to the official
      registry?
- [ ] Should plugin signatures (code signing) be required for registry
      submissions?
- [ ] What resource limits apply to plugins? (memory, CPU time per call)

---

## Implementation Plan

### Affected Crates

- `forge-plugin` — WASM host, capability enforcement, plugin lifecycle
- New: `forge-plugin-sdk` — SDK crate for plugin authors (separate repo)
- `forge-cli` — `forge plugin` subcommand implementation

### Dependencies

- Requires RFC-002 (Evaluation Pipeline) — plugins integrate at the HIR
  resolution stage
- Requires RFC-003 (Built-in Commands) — plugin commands follow the same
  `BuiltinCommand` interface

### Milestones

1. Define plugin ABI v1 — host functions and plugin exports
2. Implement `wasmtime` plugin host in `forge-plugin`
3. Implement capability enforcement layer
4. Implement `forge plugin install/list/remove` subcommands
5. Implement plugin registry client
6. Write `forge-plugin-sdk` crate for Rust plugin authors
7. Build `forge-git` reference plugin
8. Build `forge-kubectl` reference plugin
9. Integration tests — plugin installation and execution on all three platforms

---

## References

- [wasmtime](https://wasmtime.dev)
- [WASI Preview 2](https://github.com/WebAssembly/WASI/blob/main/preview2/README.md)
- [Bytecode Alliance Component Model](https://component-model.bytecodealliance.org)
- [Extism — Universal Plugin System](https://extism.org)
- [Zellij Plugin System](https://zellij.dev/documentation/plugins)
