# Forge Shell — Design Document

> This document is the authoritative reference for Forge Shell's architecture,
> design philosophy, and cross-platform covenant. All contributors, maintainers,
> and collaborators should read this before writing code.
>
> **File:** `ARCHITECTURE.md` — lives at the repository root.

---

## Table of Contents

1. [Vision](#1-vision)
2. [The Cross-Platform Covenant](#2-the-cross-platform-covenant)
3. [Architecture Overview](#3-architecture-overview)
4. [ForgeScript — The Scripting Language](#4-forgescript--the-scripting-language)
5. [Evaluation Pipeline](#5-evaluation-pipeline)
6. [Platform Lowering](#6-platform-lowering)
7. [Built-in Commands](#7-built-in-commands)
8. [Plugin System](#8-plugin-system)
9. [AI Agent Layer](#9-ai-agent-layer)
10. [Crate Structure](#10-crate-structure)
11. [Key Design Decisions](#11-key-design-decisions)
12. [What Forge Shell Is Not](#12-what-forge-shell-is-not)
13. [Contribution Guidelines](#13-contribution-guidelines)

---

## 1. Vision

Forge Shell is a modern, cross-platform shell designed for developers, DevOps
engineers, and AI agent execution environments. It is a daily driver, a
scripting platform, and an automation host — all three, simultaneously, on all
major operating systems.

### The Problem

Every popular shell today is Unix-first, Windows-later (or never):

| Shell      | Linux | macOS | Windows       | Unified Scripts |
|------------|-------|-------|---------------|-----------------|
| Bash       | ✅    | ✅    | ❌ (WSL only) | ❌              |
| Zsh        | ✅    | ✅    | ❌            | ❌              |
| Fish       | ✅    | ✅    | ❌            | ❌              |
| PowerShell | ✅    | ✅    | ✅            | ⚠️ Windows-flavoured |
| Nushell    | ✅    | ✅    | ✅            | ⚠️ No POSIX compat |
| **Forge**  | ✅    | ✅    | ✅            | ✅ First-class  |

Forge Shell fills a genuine gap: a shell where a `.fgs` script written on a
MacBook runs identically on an Ubuntu CI runner and a Windows developer
machine — without modification, without WSL, without conditional blocks.

### Goals

- **Cross-platform parity** — one script, three platforms, identical behaviour
- **Modern scripting language** — ForgeScript (`.fgs`), typed, expressive, safe
- **Developer-first UX** — syntax highlighting, semantic completion, git/k8s awareness
- **Extensible** — WASM-based plugin system with a capability model
- **AI-ready** — structured I/O and a tool registration API for agent workflows

---

## 2. The Cross-Platform Covenant

This is a binding design contract. Every feature, built-in, and API must honour
these guarantees. When in doubt, reference this section.

### Guarantees

```
✅  Path separators are normalised
    Use `/` in all .fgs scripts. Forge resolves to the platform
    separator only at OS boundary (process spawn, file system calls).

✅  $PATH is a typed list, never a raw string
    Internally, PATH is always []string. Joining with `:` or `;`
    happens only at process spawn time, invisibly to the script.

✅  Line endings are normalised
    The lexer strips `\r` before tokenisation. All Forge I/O uses
    LF internally. CRLF is handled transparently.

✅  Executables resolve without extensions
    `run("git")` resolves to `git`, `git.exe`, `git.cmd`, or `git.bat`
    on Windows, in that order. Script authors never specify extensions.

✅  Built-in commands behave identically
    `ls`, `cp`, `mv`, `rm`, `echo`, `mkdir`, `env` and others are
    implemented natively in Forge — not delegated to OS utilities.
    Behaviour is consistent across all platforms.

✅  Environment variables are case-normalised
    Forge warns on case-ambiguous variable access. Scripts should
    treat env vars as case-sensitive for portability.

✅  UTF-8 everywhere
    BOM is stripped silently. UTF-16 encoded scripts are rejected
    with a clear error. On Windows, Forge sets the console code page
    to 65001 (UTF-8) at startup.

✅  .fgs scripts are committed with LF line endings
    All Forge project templates include a `.gitattributes` that
    enforces LF for `.fgs` files.

⚠️  Job control is best-effort on Windows
    `fg`, `bg`, and `Ctrl+Z` suspension are Unix process group
    concepts. On Windows, Forge provides partial equivalents via
    Job Objects. Limitations are documented per-command.

⚠️  File permissions are advisory on Windows
    `chmod` is a no-op on Windows with a visible warning. Scripts
    must not rely on `chmod` for correctness — use Forge's execution
    model instead.

⚠️  sudo / UAC elevation is not abstracted
    Privilege escalation is inherently platform-specific. Forge does
    not attempt to unify `sudo` and UAC. Scripts requiring elevation
    must handle this explicitly or document the requirement.
```

### The Portability Rule

> If a `.fgs` script cannot run correctly on all three platforms, it is either
> a bug in Forge or a documented limitation. There is no third option.

---

## 3. Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                  forge-cli (binary)                  │
└────────────────────────┬────────────────────────────┘
                         │
         ┌───────────────┼───────────────┐
         ▼               ▼               ▼
   forge-repl       forge-lang      forge-agent
   (REPL, line      (Lexer, Parser, (AI tool layer,
    editing,         AST, HIR,       structured I/O,
    highlighting)    type system)    MCP protocol)
         │               │
         └───────┬────────┘
                 ▼
           forge-lower
      (Platform Lowering Trait +
       Unix / macOS / Windows backends)
                 │
                 ▼
           forge-exec
      (Execution Plan + Engine +
       Plan Cache)
                 │
                 ▼
           forge-core
      (Process execution, pipes,
       signal handling, job control)
                 │
                 ▼
          forge-builtins
      (ls, cp, mv, rm, echo, env,
       mkdir, which, kill, sleep…)
                 │
                 ▼
           forge-plugin
      (WASM plugin host, plugin API,
       capability model, registry)
```

Each crate has a single, well-defined responsibility. Dependencies flow
downward. No crate reaches upward.

---

## 4. ForgeScript — The Scripting Language

ForgeScript (`.fgs`) is Forge's native scripting language. It is not POSIX sh.
It is not a thin wrapper around bash. It is a purpose-designed language with
a type system, first-class paths, and structured error handling.

### File Extension

`.fgs` — ForgeScript. Chosen to be unique, unambiguous, and Googleable.
Not `.fsh` (Fish Shell), not `.fs` (F#), not `.fg` (the Unix `fg` built-in).

### Design Principles

- **Typed** — variables have types. Paths are not strings.
- **Explicit** — no magic, no surprising coercions
- **Safe** — errors are values, not silent failures
- **Readable** — optimised for humans reading scripts six months later
- **Cross-platform by construction** — platform traps are type errors or warnings

### Key Language Features

#### First-Class Path Type

```forge
# Paths use / on all platforms. Resolved at OS boundary.
let config = home() / ".config" / "forge" / "config.fgs"

# Path arithmetic is typed — not string concatenation
let logs = config.parent() / "logs"
```

#### PATH as a Typed List

```forge
# PATH is always a list internally
$PATH.prepend("/usr/local/bin")
$PATH.append(home() / ".cargo" / "bin")

# Never do this — it is a type error in ForgeScript
$PATH = $PATH + ":/usr/local/bin"  # ❌ compile error
```

#### Structured Error Handling

```forge
# Commands return Result, not raw exit codes
let result = run("git status")

match result {
  Ok(output) => print(output)
  Err(e)     => print("git failed: {e.message} (exit {e.code})")
}

# Or use the ? operator to propagate errors
let output = run("git status")?
```

#### Environment Variables

```forge
# Reading — always a string or Option<string>
let home = $HOME               # string
let token = $GITHUB_TOKEN?     # Option<string> — safe, won't panic

# Setting — scoped to the current process and children
set API_URL = "https://api.forge-shell.dev"

# Exporting — visible to spawned processes
export DEBUG = "true"
```

### Script Entry Point

ForgeScript files are executed by the `forge` binary directly:

```bash
forge deploy.fgs
forge deploy.fgs --env production
```

No shebang required. On Unix, a shebang is accepted for compatibility:

```forge
#!/usr/bin/env forge
# deploy.fgs
```

On Windows, `.fgs` files are associated with the `forge` binary at install time.

---

## 5. Evaluation Pipeline

ForgeScript source is evaluated through a staged pipeline. Each stage has a
single responsibility and a clean interface to the next.

```
Source (.fgs file or REPL input)
        │
        ▼
   ┌─────────┐
   │  Lexer  │  Tokenises source. Strips \r. Handles BOM. UTF-8 only.
   └────┬────┘
        ▼
   ┌─────────┐
   │ Parser  │  Produces an AST. Platform-agnostic. Fails fast on syntax errors.
   └────┬────┘
        ▼
   ┌─────────┐
   │   AST   │  Abstract Syntax Tree. No platform knowledge. Fully serialisable.
   └────┬────┘
        ▼
   ┌─────────┐
   │   HIR   │  High-level IR. Name resolution, type inference, semantic analysis.
   └────┬────┘  Built-in commands resolved here. Still platform-agnostic.
        ▼
   ┌──────────────────┐
   │ Platform Lowering │  HIR nodes lowered to platform-specific ExecutionPlans.
   └────────┬─────────┘  Selected once at startup. Unix / macOS / Windows backend.
            ▼
   ┌──────────────────┐
   │ Execution Engine │  Walks ExecutionPlan. Manages I/O, pipes, errors, exit codes.
   └────────┬─────────┘
            ▼
        OS / Kernel
```

### Plan Caching

Parsed and lowered plans are cached to accelerate repeated script execution:

```
~/.forge/cache/scripts/
├── deploy.fgs.<hash>.linux.plan
├── deploy.fgs.<hash>.macos.plan
└── deploy.fgs.<hash>.windows.plan
```

Cache keys are derived from: script content hash + Forge version + platform.
Cache is invalidated automatically on any change to the script or Forge binary.

This enables a future `forge compile` command for ahead-of-time compilation of
`.fgs` scripts to native executables.

---

## 6. Platform Lowering

Platform lowering is the mechanism by which HIR nodes are translated into
OS-native operation sequences. It is the architectural heart of Forge's
cross-platform guarantee.

### The Trait

```rust
pub trait PlatformLowering: Send + Sync {
    fn lower_spawn(&self, cmd: &SpawnProcess)     -> ExecutionPlan;
    fn lower_signal(&self, handler: &SignalHandler) -> ExecutionPlan;
    fn lower_file_op(&self, op: &FileOperation)   -> ExecutionPlan;
    fn lower_env_op(&self, op: &EnvOperation)     -> ExecutionPlan;
    fn lower_path(&self, path: &ForgePath)        -> NativePath;
    fn lower_job_op(&self, op: &JobOperation)     -> ExecutionPlan;
}
```

### Backend Selection

The backend is selected **once at process startup** — not per command, not per
script. This eliminates scattered `if cfg!(windows)` blocks in eval logic.

```rust
let platform: Arc<dyn PlatformLowering> = match std::env::consts::OS {
    "linux"   => Arc::new(UnixLowering::new()),
    "macos"   => Arc::new(MacOSLowering::new()),   // extends UnixLowering
    "windows" => Arc::new(WindowsLowering::new()),
    other     => return Err(UnsupportedPlatform(other.into())),
};
```

### macOS Extends Unix

macOS is not a separate implementation from scratch. `MacOSLowering` delegates
to `UnixLowering` and overrides only where macOS diverges:

- `stat` `birthtime` field (creation time — absent on Linux)
- APFS case-insensitivity handling
- `open` command mapping (`xdg-open` on Linux)
- Gatekeeper / SIP awareness for permission operations

### Platform Lowering Examples

#### Process Spawn

```
HIR: SpawnProcess { cmd: "git", args: ["status"] }

Unix:    fork() → execve("/usr/bin/git", ["status"]) → waitpid()
Windows: resolve git.exe → CreateProcess("git.exe status") → WaitForSingleObject()
```

#### Signal Handling

```
HIR: RegisterInterruptHandler { body: Block }

Unix:    sigaction(SIGINT, handler)
Windows: SetConsoleCtrlHandler(handler, TRUE) — maps CTRL_C_EVENT
```

#### PATH Manipulation

```
HIR: PrependToPath { value: ForgePath("/usr/local/bin") }

Unix:    setenv("PATH", "/usr/local/bin:" + current)
Windows: SetEnvironmentVariable("PATH", "C:\usr\local\bin;" + current)
         (ForgePath resolved to native form via lower_path())
```

---

## 7. Built-in Commands

Forge implements a set of common commands natively rather than delegating to OS
utilities. This is the mechanism that makes scripts cross-platform: the
behaviour is defined by Forge, not by the host system.

### Native Built-ins

| Command | Notes |
|---------|-------|
| `ls`    | Structured output. Identical columns on all platforms. |
| `cp`    | Recursive by default with `--recursive`. No silent overwrites. |
| `mv`    | Atomic where the OS allows. Clear error on cross-device move. |
| `rm`    | Requires `--recursive` for directories. No `-rf` shorthand. |
| `mkdir` | `--parents` flag unified. No `mkdir -p` vs `/MD` inconsistency. |
| `cat`   | Line-ending normalised output. |
| `echo`  | Consistent escape handling. No platform quoting surprises. |
| `env`   | Lists environment as structured data. |
| `which` | PATH resolution without extension guessing on the caller's side. |
| `pwd`   | Always returns a normalised path with `/` separators. |
| `sleep` | Accepts `1s`, `500ms`, `2m`. No Windows `timeout` workaround needed. |
| `kill`  | Abstracted. Maps to SIGTERM on Unix, TerminateProcess on Windows. |
| `chmod` | No-op on Windows with a visible warning. Advisory on Unix. |

### The Built-in Philosophy

> If a command's behaviour differs between platforms in a way that would break
> a script, Forge must own that command.

Built-ins are implemented in `forge-builtins` behind the `PlatformLowering`
trait. They emit structured data — not raw text — which the output layer
formats for human or machine consumption.

---

## 8. Plugin System

Forge's plugin system uses WebAssembly (WASM) as the plugin runtime. This
provides a language-agnostic, sandboxed, cross-platform extension model.

### Why WASM

- **Language-agnostic** — plugins can be written in Rust, Go, C, AssemblyScript, or any WASM-targeting language
- **Sandboxed** — plugins cannot access the filesystem, network, or processes beyond their declared capabilities
- **Cross-platform** — a `.wasm` plugin built once runs on all three platforms
- **Versioned API** — the plugin ABI is stable and versioned independently of Forge

### Plugin Capabilities (Capability Model)

Plugins declare capabilities at install time. Forge enforces them at runtime.

```toml
# forge-plugin.toml
name        = "forge-git"
version     = "1.0.0"
description = "Git integration for Forge Shell"

[capabilities]
exec        = ["git"]          # may spawn the git binary
filesystem  = ["read"]         # read-only filesystem access
network     = false            # no network access
env         = ["read"]         # may read environment variables
```

### Plugin API Surface

Plugins can:

- Register new commands
- Register completion providers
- Register prompt hooks (e.g., git status in prompt)
- Emit structured output

Plugins cannot:

- Access capabilities not declared at install time
- Modify Forge internals
- Intercept or modify output of other commands

### Plugin Installation

```bash
forge plugin install forge-git
forge plugin install ./local-plugin.wasm
forge plugin list
forge plugin remove forge-git
```

---

## 9. AI Agent Layer

Forge Shell is designed to serve as an execution environment for AI agents.
The agent layer provides structured I/O, tool registration, and a session
protocol compatible with the Model Context Protocol (MCP).

### Structured Output

All built-in commands can emit structured data:

```bash
forge --output json ls
forge --output ndjson ps
```

This makes Forge commands directly consumable by LLMs and agent frameworks
without brittle text parsing.

### Tool Registration API

Forge commands and `.fgs` functions can be registered as AI tools:

```forge
#[tool]
fn deploy(env: string, dry_run: bool) -> Result<DeployOutput> {
  # ...
}
```

Forge generates a JSON Schema for the function signature automatically.

### Safety Model

The agent layer enforces a safety model by default:

- **Confirmation mode** — destructive operations require explicit approval
- **Dry-run mode** — `forge --dry-run script.fgs` shows the execution plan without running it
- **Audit log** — all agent-invoked commands are logged with timestamps and arguments
- **Capability scoping** — agent sessions declare allowed command sets at session start

### MCP Compatibility

Forge implements the Model Context Protocol, making it usable as an MCP host
for any MCP-compatible AI agent or framework.

---

## 10. Crate Structure

```
forge-shell/                   (Cargo workspace root)
├── crates/
│   ├── forge-cli/             Binary entry point. Wires all crates together.
│   ├── forge-repl/            REPL, reedline integration, syntax highlighting, prompt.
│   ├── forge-lang/
│   │   ├── lexer/             Tokeniser. UTF-8, BOM stripping, \r normalisation.
│   │   ├── parser/            Recursive descent parser. Produces AST.
│   │   ├── ast/               AST node definitions. Platform-agnostic.
│   │   └── hir/               High-level IR. Name resolution, type inference.
│   ├── forge-lower/           Platform lowering.
│   │   ├── trait.rs           PlatformLowering trait definition.
│   │   ├── unix.rs            Unix backend.
│   │   ├── macos.rs           macOS backend (extends unix).
│   │   └── windows.rs         Windows backend.
│   ├── forge-exec/            Execution engine.
│   │   ├── plan.rs            ExecutionPlan definition.
│   │   ├── engine.rs          Walks and executes plans.
│   │   └── cache.rs           Plan caching and invalidation.
│   ├── forge-core/            Process execution, pipes, job control.
│   │   └── platform/          OS-specific implementations behind traits.
│   ├── forge-builtins/        Native built-in command implementations.
│   ├── forge-plugin/          WASM plugin host, plugin API, capability enforcement.
│   └── forge-agent/           AI agent layer, MCP protocol, tool registration.
├── plugins/                   First-party reference plugins.
│   ├── forge-git/
│   └── forge-kubectl/
├── docs/
│   ├── rfcs/                  RFC documents for significant design decisions.
│   └── design/                Architecture diagrams and deep-dives.
├── tests/
│   └── integration/
│       ├── cross/             Must pass on all three platforms.
│       ├── unix/              Unix-specific behaviour tests.
│       └── windows/           Windows-specific behaviour tests.
└── Cargo.toml                 Workspace root.
```

---

## 11. Key Design Decisions

This section records significant design decisions and the reasoning behind
them. When a decision is revisited, a new entry is added — the history is
preserved.

### ADR-001: Rust as the Implementation Language

**Decision:** Forge Shell is implemented in Rust.

**Reasoning:**
- Zero-cost abstractions and no GC — critical for a responsive interactive shell
- Enums and pattern matching are ideal for AST and parser construction
- WASM plugin hosting via `wasmtime` — best-in-class support
- Memory safety without runtime overhead
- Excellent cross-compilation story via `cross` and `cargo`

**Trade-offs:** Slower compile times. Steeper contributor onboarding curve.

---

### ADR-002: JIT-Style Platform Lowering Pipeline

**Decision:** ForgeScript is evaluated through a staged pipeline that terminates
in a platform-specific lowering step, analogous to a JIT compiler emitting
platform-specific machine code.

**Reasoning:** Separates language semantics (AST, HIR) from execution
mechanics (syscalls, process APIs). Platform branching is confined to one
layer — `forge-lower` — rather than scattered through the evaluator.

**Trade-offs:** More architectural layers than a simple interpreter. Initial
complexity is higher but maintenance complexity is lower.

---

### ADR-003: Built-ins Implemented Natively

**Decision:** Common commands (`ls`, `cp`, `mv`, `rm`, `echo`, etc.) are
implemented as Forge built-ins, not delegated to OS utilities.

**Reasoning:** Cross-platform behavioural consistency requires owning the
implementation. Delegating to `ls` on macOS and `dir` on Windows produces
different output, different flags, and different error semantics.

**Trade-offs:** Significant implementation effort. Must keep up with expected
command behaviours.

---

### ADR-004: WASM Plugin System

**Decision:** Plugins are WASM modules, hosted via `wasmtime`, with a
capability-based permission model.

**Reasoning:** WASM is language-agnostic, cross-platform, and sandboxed.
A `.wasm` plugin built once runs on Linux, macOS, and Windows without
recompilation. The capability model prevents plugins from exceeding their
declared permissions.

**Trade-offs:** WASM has a performance overhead for plugin calls. Plugin
authors must target WASM, which limits available language features.

---

### ADR-005: `.fgs` File Extension

**Decision:** ForgeScript files use the `.fgs` extension.

**Reasoning:** `.fsh` conflicts with Fish Shell. `.fs` conflicts with F#.
`.fg` conflicts with the Unix `fg` built-in (foreground job command — a term
every shell user knows). `.fgs` is unique, Googleable, and reads naturally
as "ForgeScript".

---

### ADR-006: PATH as a Typed List

**Decision:** `$PATH` is a typed `[]string` in ForgeScript, not a
colon/semicolon-delimited string.

**Reasoning:** The platform-specific separator (`:` vs `;`) is an OS
implementation detail that should not leak into script logic. Treating PATH
as a list eliminates an entire class of platform bugs.

---

## 12. What Forge Shell Is Not

To keep the project focused, the following are explicit non-goals:

- **Not a POSIX sh replacement.** Forge does not aim for POSIX compatibility.
  Running `bash` scripts through Forge is not a goal.
- **Not a terminal emulator.** Forge is a shell. Terminal rendering is the
  responsibility of the terminal emulator the user has chosen.
- **Not a package manager.** `forge plugin install` is a plugin manager, not
  a system package manager.
- **Not a full-screen TUI.** Forge is a line-oriented shell. Full-screen
  applications may be launched from Forge but are not built into it.
  (`ratatui` is explicitly excluded from the core dependency set.)
- **Not a drop-in replacement for PowerShell.** The PowerShell scripting model
  is not compatible with ForgeScript by design.

---

## 13. Contribution Guidelines

### Before Writing Code

1. Read this document in full.
2. Check `docs/rfcs/` for any RFC covering the area you want to work in.
3. If no RFC exists for a significant change, write one first.
   See `docs/rfcs/RFC-000-template.md` for the RFC format.
4. Open an issue to discuss your approach before submitting a PR.

### Cross-Platform Rule

> Every PR that touches `forge-core`, `forge-builtins`, `forge-lower`, or
> `forge-exec` must pass the CI matrix on all three platforms before merge.
> No exceptions.

### Platform-Specific Code

All platform-specific code lives in `forge-lower/src/` behind the
`PlatformLowering` trait. Do not add `#[cfg(target_os = "windows")]` blocks
outside of `forge-lower`, `forge-core/src/platform/`, or test code.

### CI Matrix

All PRs run against:

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
    rust: [stable]
```

A PR that passes on Linux and macOS but breaks Windows is not mergeable.

### Commit Messages

Follow the Conventional Commits specification:

```
feat(forge-lang): add Path type to HIR
fix(forge-builtins): normalise ls output on Windows
docs(design): add ADR-007 for job control model
chore(ci): add windows-latest to matrix
```

### Adding a New Built-in Command

1. Add the command to `forge-builtins`
2. Implement `PlatformLowering` hooks if OS-specific behaviour is required
3. Add integration tests to `tests/integration/cross/` — must pass on all platforms
4. Document behaviour differences (if any) in the command's doc comment
5. Update the built-in commands table in this document

