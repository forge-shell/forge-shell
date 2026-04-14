# RFC-003 — Built-in Command Specification & Behaviour Contract

| Field          | Value                        |
|----------------|------------------------------|
| Status         | **In Review**                |
| Author         | Ajitem Sahasrabuddhe         |
| Created        | 2026-04-09                   |
| Last Updated   | 2026-04-14                   |
| Supersedes     | —                            |
| Superseded By  | —                            |

---

## Summary

This RFC defines the built-in command runtime for Forge Shell — a
BusyBox-inspired set of 32 native commands implemented in Rust, behaviourally
identical across Linux, macOS, and Windows. Every built-in supports a
dual-output architecture: rich terminal rendering for interactive use and
structured `StructuredOutput` for AI/MCP consumption. Platform-specific
behaviour is fully isolated behind the `PlatformBackend` trait defined in
RFC-002.

---

## Motivation

Traditional shell commands are thin wrappers around Unix system calls with
decades of platform-specific behaviour baked in. `ls` flags differ between
Linux and macOS. `rm` does not exist on Windows. `echo` behaves differently
across bash, zsh, and PowerShell. Every portability gap is a silent failure
waiting to happen.

Forge Shell's answer is to own the implementation entirely:

- Commands are implemented in Rust — not delegated to the host OS
- Behaviour is identical on all three platforms by construction
- Output is rich in the terminal and structured for AI agents
- Platform differences are isolated, documented, and surfaced explicitly

---

## Design

### 1. Foundation — BusyBox-inspired Built-in Runtime

Every Forge Shell built-in command is:

- Implemented natively in Rust in `forge-core/builtins`
- Behaviourally identical on Linux, macOS, and Windows
- Backed by a platform-specific implementation in `forge-backend/unix` or
  `forge-backend/windows`
- Capable of dual output — rich terminal rendering and structured data

```
forge-core/builtins    — unified interface, argument spec, StructuredOutput schema
        ↓
PlatformBackend trait  — forge-backend
        ↓
UnixBackend            — forge-backend/unix   (Linux + macOS)
WindowsBackend         — forge-backend/windows
```

---

### 2. v1 Built-in Command Set — 32 Commands

#### File System

| Command | Description |
|---|---|
| `ls` | List directory contents — rich table, git status per file, icons |
| `tree` | Directory tree — rich rendering, always available cross-platform |
| `cp` | Copy files and directories |
| `mv` | Move or rename files and directories |
| `rm` | Remove files and directories |
| `mkdir` | Create directories |
| `rmdir` | Remove empty directories |
| `touch` | Create empty file or update timestamp |
| `find` | Find files matching criteria |
| `stat` | File metadata |
| `du` | Disk usage — human-readable sizes, visual bars per directory |
| `df` | Disk free — human-readable, visual usage bars |
| `hash` | File hashing — MD5, SHA256, SHA512. Cross-platform shasum/certutil replacement |

#### Text & Streams

| Command | Description |
|---|---|
| `cat` | View files — syntax highlighting, line numbers, markdown rendering, search |
| `echo` | Print to stdout |
| `grep` | Search — syntax-aware, ripgrep-style output with file and line context |
| `diff` | File diff — side-by-side, syntax highlighted |
| `head` | First N lines of a file |
| `tail` | Last N lines of a file |
| `sort` | Sort lines |
| `uniq` | Remove duplicate lines |
| `wc` | Word, line, and character count |
| `jq` | Query and transform JSON |
| `yq` | Query and transform YAML |
| `tq` | Query and transform TOML |

#### Environment & Process

| Command | Description |
|---|---|
| `env` | Print environment — grouped, sorted, searchable table |
| `set` | Set environment variable |
| `unset` | Unset environment variable |
| `which` | Locate a command — shows path, binary type, version if detectable |
| `exit` | Exit the shell |
| `pwd` | Print working directory |
| `cd` | Change directory |

#### Network

| Command | Description |
|---|---|
| `fetch` | HTTP requests — GET, POST, download, response inspection |
| `ping` | ICMP ping |

#### Forge-specific

| Command | Description |
|---|---|
| `forge run` | Execute a `.fgs` script |
| `forge check` | Type-check without executing |
| `forge fmt` | Format a `.fgs` file |
| `forge plugin` | Plugin management |
| `forge migrate` | Migrate bash scripts to ForgeScript |
| `bench` | Benchmark a command — multiple runs, statistical output |
| `watch` | Re-run a command on file or directory change |

**Explicitly deferred to post-v1:**
`chmod`/`chown`, `tar`/`zip`, `ps`/`kill`, `ssh`/`scp`, `sed`, `awk`,
`http`, `digest`, `cut`, `tr`, `history`

---

### 3. Error Model

Built-in commands return typed `Result` values. Exit codes are not used
internally — they are a POSIX artefact produced only at process boundary for
external consumers.

```rust
pub struct CommandError {
    pub code:    CommandErrorCode,
    pub message: String,
    pub context: Option<String>,
}

pub enum CommandErrorCode {
    NotFound,
    PermissionDenied,
    InvalidArgument,
    IoError,
    NetworkError,
    PlatformUnsupported,
    // ...
}
```

Every built-in returns:

```rust
Result<StructuredOutput, CommandError>
```

---

### 4. Argument Model

Built-in commands accept three equivalent invocation forms (RFC-001 Section 16):

```forge
# Positional
ls /home/user

# POSIX-style flags
ls /home/user --show_hidden --sort name

# Named typed arguments
ls path: p"/home/user", show_hidden: true, sort: "name"
```

All three forms expand to the canonical named-argument form before the type
checker runs. Type validation is applied uniformly regardless of invocation
form.

Every built-in declares its argument signature explicitly — positional order,
flag names, types, and defaults:

```rust
pub struct LsArgs {
    pub path:        PathBuf,           // positional[0]
    pub show_hidden: bool,              // --show_hidden, default: false
    pub sort:        LsSort,            // --sort, default: LsSort::Name
    pub output:      Option<OutputFmt>, // --output
}

pub enum LsSort {
    Name, Size, Modified,
}
```

---

### 5. Dual-Output Architecture

Every built-in produces output in one of three modes, selected by the
execution engine based on context:

```rust
pub enum OutputMode {
    RichTerminal,  // colours, tables, icons — interactive terminal
    PlainText,     // piped — no formatting
    Structured,    // StructuredOutput envelope — AI/MCP context
}
```

**Output mode selection:**

| Context | Output mode |
|---|---|
| Interactive terminal (TTY) | `RichTerminal` |
| Piped to another command | `PlainText` |
| AI/MCP agent context | `Structured` |
| Explicit `--output json` | `Structured` |

Built-ins never detect their own output mode — the execution engine injects
a `CommandContext` at invocation:

```rust
pub struct CommandContext {
    pub output_mode: OutputMode,
    pub platform:    Platform,
    pub working_dir: PathBuf,
}
```

---

### 6. StructuredOutput Schema

All built-in commands emit a common `StructuredOutput` envelope wrapping a
command-specific typed payload. This gives RFC-007's AI agent layer a stable,
consistent entry point for all built-in output.

```rust
pub struct StructuredOutput {
    pub command:  String,
    pub version:  u32,
    pub status:   OutputStatus,
    pub payload:  OutputPayload,
    pub metadata: OutputMetadata,
}

pub enum OutputStatus {
    Success,
    Failure { error: CommandError },
}

pub struct OutputMetadata {
    pub duration_ms: u64,
    pub platform:    Platform,
    pub warnings:    Vec<String>,
}

pub enum OutputPayload {
    Ls(Vec<FileEntry>),
    Tree(Vec<TreeNode>),
    Find(Vec<PathEntry>),
    Stat(FileMetadata),
    Du(Vec<DiskUsageEntry>),
    Df(Vec<DiskFreeEntry>),
    Hash(FileHash),
    Cat(FileContent),
    Grep(Vec<GrepMatch>),
    Diff(FileDiff),
    Fetch(HttpResponse),
    Env(Vec<EnvEntry>),
    Jq(serde_json::Value),
    Yq(serde_json::Value),
    Tq(serde_json::Value),
    // ... one variant per built-in
}
```

**Key payload types:**

```rust
pub struct FileEntry {
    pub name:        String,
    pub path:        PathBuf,
    pub size:        Option<u64>,
    pub modified:    Option<DateTime<Utc>>,
    pub kind:        FileKind,
    pub permissions: Option<String>,
    pub git_status:  Option<GitStatus>,
}

pub enum FileKind {
    File, Directory, Symlink, Junction,
}

pub struct FileHash {
    pub path:      PathBuf,
    pub algorithm: HashAlgorithm,
    pub digest:    String,
}

pub enum HashAlgorithm {
    Md5, Sha256, Sha512,
}
```

---

### 7. `cat` — Enhanced Viewer

`cat` is promoted to a first-class viewing and rendering tool:

| Feature | Default | Override |
|---|---|---|
| Syntax highlighting | ✅ On in terminal | `--no-highlight` |
| Line numbers | ✅ On in terminal | `--no-line-numbers` |
| Git diff markers | ✅ On in terminal if in git repo | `--no-git` |
| Pager | ❌ Off | `--pager` |
| Markdown rendering | ❌ Off | `--render` |
| Plain output | ❌ Off | `--plain` — disables all formatting |

In `PlainText` and `Structured` modes, all formatting is suppressed
automatically — no flags needed.

---

### 8. Built-in Shadowing & Override Model

Built-ins win by default. Shadowing requires an explicit declaration.

**Name resolution order:**

```
1. Explicit script-level override   (#!forge:override or plugin manifest)
2. Forge built-in                   (default)
3. Plugin commands (non-override)
4. User-defined functions
```

**Script-level override:**

```forge
#!/usr/bin/env forge
#!forge:override = "ls"

fn ls(path: path, show_hidden: bool = false) -> Result<StructuredOutput, CommandError> {
    # custom implementation
}
```

**Plugin manifest override:**

```toml
[plugin]
name     = "rich-ls"
overrides = ["ls"]
```

**A function named `ls` without an explicit `#!forge:override` declaration
is a compile-time error — not a silent shadow.**

---

### 9. User Config — Shell-level Overrides

Shell-level overrides are declared in the user config file. They apply to
the **interactive shell only** — `.fgs` scripts are hermetic and unaffected.

**Config file locations:**

| Platform | Path |
|---|---|
| Linux / macOS | `~/.config/forge/config.toml` |
| Windows | `%APPDATA%\forge\config.toml` |

```toml
[overrides]
ls    = "system"               # delegate to OS $PATH
grep  = "/usr/local/bin/rg"    # delegate to specific binary

[fmt]
argument_style = "named"       # "named" | "flags" | "preserve"
```

**Hermeticity rule:** Config overrides never bleed into `.fgs` script
execution. A script that behaves differently on two machines due to local
config is a portability violation — ForgeScript's core covenant.

---

### 10. Platform Difference Documentation

Platform differences are surfaced at three layers:

**Layer 1 — Static: RFC and source documentation**

Every built-in with platform differences has an explicit table:

| Behaviour | Linux | macOS | Windows |
|---|---|---|---|
| Hidden files | `.dotfiles` | `.dotfiles` | Hidden attribute |
| Permissions | Unix rwx | Unix rwx | ACL model |
| Path separators | `/` | `/` | `\` normalised to `/` |
| `ping` | Raw ICMP socket | Raw ICMP socket | `IcmpSendEcho` Win32 API |
| `env` PATH separator | `:` | `:` | `;` |
| `find` symlinks | symlink-aware | symlink-aware | junction-aware |

**Layer 2 — Runtime: `OutputMetadata.warnings`**

```rust
metadata: OutputMetadata {
    warnings: vec![
        "stat: permission model on Windows uses ACLs — Unix rwx representation is approximate"
    ]
}
```

**Layer 3 — Compile-time: `forge check --platform`**

```
warning[W012]: platform difference detected
  --> deploy.fgs:14:5
   |
14 | stat path: p"/etc/forge/config.toml"
   | ^^^^
   = note: permission model differs on Windows — ACLs vs Unix rwx
   = help: use forge::fs::permissions() for cross-platform permission handling
```

---

## Drawbacks

- **32 built-ins is a significant implementation surface.** Each command
  requires a Unix and Windows backend implementation, a `StructuredOutput`
  payload type, and integration tests on all three platforms.
- **Rich terminal output adds rendering complexity.** Syntax highlighting,
  git integration, and markdown rendering require additional dependencies.
- **Three invocation forms add parser complexity.** The expansion layer must
  handle all 32 commands correctly.

---

## Alternatives Considered

### Alternative A — Delegate to OS commands

**Rejected:** OS commands behave differently across platforms. Delegation
makes cross-platform parity impossible — a direct violation of Forge Shell's
core covenant.

### Alternative B — POSIX flags only, no named arguments

**Rejected:** POSIX flags are stringly-typed and platform-inconsistent.
Named typed arguments are validated at parse time and self-documenting.
The three-form invocation model gives ergonomics without sacrificing safety.

### Alternative C — `curl` and `wget` as aliases for `fetch`

**Rejected:** `forge migrate` handles `curl`/`wget` → `fetch` migration
automatically. Aliases would add maintenance burden and discourage developers
from learning the ForgeScript way.

### Alternative D — stdlib-only, no built-in commands

**Rejected:** A shell with no built-in commands is not a shell. The
BusyBox-inspired model is the correct foundation for cross-platform parity.

### Alternative E — `PlatformLowering` as trait name

**Rejected:** `PlatformBackend` is clearer and more universally understood.
`UnixBackend` and `WindowsBackend` read naturally as concrete implementations.

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | Canonical v1 command set | 32 commands across 5 groups — see Section 2 |
| UQ-2 | Error model | `Result<StructuredOutput, CommandError>` — not exit codes |
| UQ-3 | Argument style | Named typed args + `--flags` + positional — all equivalent |
| UQ-4 | Built-in shadowing | Explicit override required — built-ins win by default |
| UQ-5 | Platform differences | Three-layer model — docs, runtime warnings, compile-time warnings |
| UQ-6 | `StructuredOutput` schema | Shared envelope with typed payload per command |
| UQ-7 | Output mode selection | TTY detection + context injection + explicit `--output` flag |
| UQ-8 | Positional argument mapping | Every built-in declares positional order explicitly |

**Foundation decisions adopted before UQs:**
- BusyBox-inspired native built-in runtime
- Dual-output architecture — `RichTerminal` and `StructuredOutput`

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-core/builtins` | Unified command interfaces, argument specs, `StructuredOutput` schemas |
| `forge-core/output` | `StructuredOutput`, `OutputPayload`, `OutputMode`, `CommandContext` |
| `forge-backend` | `PlatformBackend` trait |
| `forge-backend/unix` | Unix implementations of all 32 built-ins |
| `forge-backend/windows` | Windows implementations of all 32 built-ins |
| `forge-engine` | Output mode selection, `CommandContext` injection |

### Dependencies

- Requires RFC-001 (invocation syntax, type system) to be accepted first.
- Requires RFC-002 (`PlatformBackend` trait, `ExecutionPlan`) to be accepted first.
- RFC-007 (AI Agent Layer) depends on `StructuredOutput` defined here.
- RFC-012 (REPL) depends on the command set and argument model defined here.

### Milestones

1. Define `StructuredOutput`, `OutputPayload`, `CommandContext` in `forge-core/output`
2. Define `CommandError` and `CommandErrorCode`
3. Implement File System built-ins — Unix + Windows backends
4. Implement Text & Streams built-ins — Unix + Windows backends
5. Implement `cat` enhanced viewer — syntax highlighting, markdown rendering
6. Implement Environment & Process built-ins — Unix + Windows backends
7. Implement Network built-ins — `fetch`, `ping`
8. Implement Forge-specific commands — `forge run`, `forge check`, `forge fmt`, `forge plugin`, `forge migrate`
9. Implement `bench` and `watch`
10. Implement `forge check --platform` compile-time platform warnings
11. Integration tests for all 32 commands on ubuntu-latest, macos-latest, windows-latest

---

## References

- [BusyBox](https://busybox.net/)
- [bat — A cat clone with wings](https://github.com/sharkdp/bat)
- [ripgrep](https://github.com/BurntSushi/ripgrep)
- [eza — A modern replacement for ls](https://github.com/eza-community/eza)
- [delta — A syntax-highlighting pager for git](https://github.com/dandavison/delta)
- [RFC-001 — ForgeScript Language Syntax & Type System](./RFC-001-forgescript-syntax.md)
- [RFC-002 — Evaluation Pipeline & Platform Backend](./RFC-002-evaluation-pipeline.md)
- [RFC-007 — AI Agent Layer & MCP Protocol Integration](./RFC-007-ai-agent-layer.md)
- [RFC-012 — ForgeScript REPL & Interactive Shell](./RFC-012-repl.md)