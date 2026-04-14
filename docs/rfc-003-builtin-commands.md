# RFC-003 — Built-in Command Specification & Behaviour Contract

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

This RFC defines which commands Forge Shell implements natively as built-ins,
the behaviour contract each built-in must honour on all three platforms, and
the architecture for implementing them in `forge-builtins`. Native built-ins
are the primary mechanism by which Forge Shell achieves cross-platform
behavioural consistency.

---

## Motivation

Cross-platform shell scripting fails primarily because common commands behave
differently across operating systems:

- `ls` on macOS and `ls` on Linux have different flags and output formats
- `echo` on bash vs `echo` in `/bin/sh` handle escape sequences differently
- `mkdir -p` has no direct Windows equivalent outside of PowerShell
- `sleep 1` works on Unix; `timeout /t 1` is the Windows equivalent
- `kill` sends Unix signals; Windows has no equivalent concept

If Forge Shell delegates these commands to the host OS, scripts break the
moment they run on a different platform. The only robust solution is to own
the implementation.

---

## Design

### 1. Built-in Philosophy

> If a command's behaviour differs between platforms in a way that would break
> a portable `.fgs` script, Forge must own that command.

Built-ins are implemented in `forge-builtins` as Rust functions that emit
structured data. They are resolved at the HIR stage and lowered through
`PlatformLowering` where OS-specific behaviour is required.

Built-ins are **not** thin wrappers around OS utilities. They are
self-contained implementations.

---

### 2. Built-in Command Registry

#### Filesystem

| Command  | Description | Platform Notes |
|----------|-------------|----------------|
| `ls`     | List directory contents | Unified columns. Structured output. |
| `cp`     | Copy files and directories | `--recursive` for directories. No silent overwrites. |
| `mv`     | Move / rename files | Atomic on same filesystem. Error on cross-device without `--force`. |
| `rm`     | Remove files | Requires `--recursive` for directories. No `-rf` shorthand. Requires `--force` to suppress confirmation. |
| `mkdir`  | Create directories | `--parents` creates intermediate directories. |
| `rmdir`  | Remove empty directory | Fails if not empty — use `rm --recursive`. |
| `touch`  | Create file or update timestamp | Creates empty file if not exists. |
| `cat`    | Concatenate and print files | Line-ending normalised. |
| `head`   | Print first N lines | `-n` flag. Default 10 lines. |
| `tail`   | Print last N lines | `-n` flag. `-f` follow mode. |
| `find`   | Search for files | Subset of POSIX find. Structured output. |
| `stat`   | File metadata | Structured output. `birthtime` available on macOS only — documented. |
| `chmod`  | Change file permissions | No-op on Windows with a visible warning. |
| `chown`  | Change file ownership | No-op on Windows with a visible warning. |
| `ln`     | Create links | Symlinks require elevated rights on Windows pre-11 — documented. |

#### Navigation

| Command  | Description | Platform Notes |
|----------|-------------|----------------|
| `cd`     | Change directory | Normalises path separators. |
| `pwd`    | Print working directory | Always uses `/` as separator in output. |
| `pushd`  | Push directory onto stack | |
| `popd`   | Pop directory from stack | |

#### Text Processing

| Command  | Description | Platform Notes |
|----------|-------------|----------------|
| `echo`   | Print to stdout | Consistent escape handling. No platform quoting surprises. |
| `print`  | Alias for `echo` in ForgeScript | Preferred in `.fgs` scripts. |
| `grep`   | Search text for pattern | Basic regex. Structured match output. |
| `sort`   | Sort lines | |
| `uniq`   | Filter duplicate lines | |
| `wc`     | Word / line / byte count | |
| `cut`    | Extract fields from lines | |
| `tr`     | Translate characters | |
| `sed`    | Stream editor (subset) | Basic substitution only in v1. |

#### Environment & Process

| Command  | Description | Platform Notes |
|----------|-------------|----------------|
| `env`    | List environment variables | Structured output. |
| `export` | Set and export variable | |
| `set`    | Set variable | |
| `unset`  | Remove variable | |
| `which`  | Locate a command in PATH | Extension-aware on Windows. |
| `where`  | Alias for `which` | |
| `ps`     | List processes | Structured output. Platform-normalised columns. |
| `kill`   | Send signal / terminate process | Maps to `TerminateProcess` on Windows. |
| `sleep`  | Pause execution | Accepts `1s`, `500ms`, `2m`. No Windows `timeout` needed. |
| `exit`   | Exit the shell | |

#### I/O & Streams

| Command  | Description | Platform Notes |
|----------|-------------|----------------|
| `read`   | Read line from stdin | |
| `write`  | Write to file | |
| `pipe`   | Explicit pipe construction | |

#### Networking (Subset)

| Command  | Description | Platform Notes |
|----------|-------------|----------------|
| `curl`   | HTTP requests | Thin wrapper — requires `curl` on PATH. Not a built-in implementation. Considered for v2. |
| `ping`   | ICMP ping | Platform-normalised output. |

#### Shell Utilities

| Command  | Description | Platform Notes |
|----------|-------------|----------------|
| `source` | Execute script in current scope | |
| `alias`  | Define command alias | |
| `history`| Access command history | |
| `time`   | Measure command duration | Structured output with nanosecond precision. |
| `true`   | Exit with code 0 | |
| `false`  | Exit with code 1 | |

---

### 3. Structured Output

All built-in commands emit **structured data** internally. The output layer
formats this for display:

```forge
# Human-readable (default)
ls

# Structured JSON — for scripts and agents
ls --output json

# NDJSON — for streaming pipelines
ls --output ndjson

# Table — formatted table (default for interactive sessions)
ls --output table
```

Structured output means built-ins can be consumed directly in ForgeScript
without text parsing:

```forge
let files = ls("./src") | where { |f| f.size > 1_000_000 }

for file in files {
  print("{file.name} is {file.size} bytes")
}
```

---

### 4. Built-in Implementation Architecture

```rust
// forge-builtins/src/lib.rs

pub trait BuiltinCommand: Send + Sync {
    fn name(&self) -> &'static str;
    fn run(&self, args: BuiltinArgs, ctx: &ExecutionContext) -> BuiltinResult;
}

pub struct BuiltinArgs {
    pub positional: Vec<Value>,
    pub flags:      HashMap<String, Value>,
    pub stdin:      Option<InputStream>,
}

pub struct BuiltinResult {
    pub output:    OutputStream,    // structured data
    pub exit_code: i32,
}
```

Each built-in is a struct implementing `BuiltinCommand`. Platform-specific
behaviour is delegated to `PlatformLowering` via the `ExecutionContext`.

```rust
// forge-builtins/src/fs/ls.rs

pub struct Ls;

impl BuiltinCommand for Ls {
    fn name(&self) -> &'static str { "ls" }

    fn run(&self, args: BuiltinArgs, ctx: &ExecutionContext) -> BuiltinResult {
        let path = args.path_or_cwd()?;
        let entries = ctx.platform.lower_file_op(&FileOperation::ReadDir { path })?;
        let formatted = format_entries(entries, &args.flags, ctx.output_format);
        BuiltinResult::ok(formatted)
    }
}
```

---

### 5. Error Behaviour Contract

All built-ins must honour this error contract:

| Situation | Behaviour |
|---|---|
| File not found | `Err` with message and the path that was not found |
| Permission denied | `Err` with message. Never silently skip. |
| Partial failure (e.g. cp with multiple sources) | Report each failure. Continue by default. `--fail-fast` to stop on first error. |
| No-op on Windows (chmod, chown) | `Ok` with a visible warning on stderr. Script continues. |
| Unsupported operation | `Err` with a clear "not supported on {platform}" message. |

---

### 6. Safety Defaults

Built-ins are safe by default:

- `rm` requires `--recursive` for directories. No `-rf` shorthand.
- `rm` prompts for confirmation on 3+ files unless `--force` is passed.
- `mv` does not silently overwrite. `--force` required.
- `cp` does not silently overwrite. `--force` required.
- `ln` warns on Windows about symlink elevation requirements.

These defaults can be changed per-invocation with flags. They cannot be
changed globally — this is intentional. Scripts that rely on dangerous
defaults are fragile scripts.

---

### 7. Reserved Built-in Names

The following names are reserved for built-in commands. External commands with
these names are shadowed. Users must use full paths to invoke them:

```
ls cp mv rm mkdir rmdir touch cat head tail find stat chmod chown ln
cd pwd pushd popd echo print grep sort uniq wc cut tr sed
env export set unset which where ps kill sleep exit
read write source alias history time true false
```

---

## Drawbacks

- **Large implementation surface** — implementing every built-in correctly on
  three platforms is significant work. Each built-in is a small project.
- **Behaviour divergence risk** — our `ls` must match user expectations built
  from decades of GNU coreutils. Subtle differences will cause friction.
- **Maintenance burden** — every built-in must be maintained indefinitely.
  We cannot deprecate `ls`.
- **Incomplete POSIX parity** — power users will find missing flags. This is
  a known and accepted trade-off.

---

## Alternatives Considered

### Alternative A — Delegate to OS Utilities

**Approach:** On Unix, call `/bin/ls`. On Windows, call PowerShell's `Get-ChildItem`.
**Rejected because:** Output format, flags, and error behaviour diverge. Scripts
break when moving between platforms. This is the exact problem Forge Shell exists
to solve.

### Alternative B — Minimal Built-ins, Rely on Plugins

**Approach:** Implement only `cd`, `exit`, `source`. Everything else is a plugin.
**Rejected because:** The cross-platform guarantee cannot be upheld if core
filesystem operations are delegated to third-party plugins. Plugins can use the
built-in system but cannot replace it for portable scripts.

### Alternative C — GNU Coreutils via WASM

**Approach:** Compile GNU coreutils to WASM and host them as built-ins.
**Rejected because:** GNU coreutils are Unix-centric. Compiling them to WASM
does not resolve the underlying Unix assumptions in their design.

---

## Unresolved Questions

- [ ] Which `sed` subset is supported in v1? Full POSIX sed is complex.
- [ ] Should `grep` support PCRE or POSIX BRE/ERE only?
- [ ] Should `find` support the full POSIX `find` expression language or a
      simplified ForgeScript-native query syntax?
- [ ] Should `curl` be a genuine built-in in v1 or deferred to a plugin?
- [ ] How are built-in flag conflicts handled? (e.g. `ls -l` vs `ls --long`)
- [ ] Should built-ins support `--help` by default?

---

## Implementation Plan

### Affected Crates

- `forge-builtins` — all built-in implementations
- `forge-lower` — `lower_builtin()` and `lower_file_op()` implementations
- `forge-lang/hir` — built-in resolution during HIR lowering

### Dependencies

- Requires RFC-001 (ForgeScript Syntax) — structured output types
- Requires RFC-002 (Evaluation Pipeline) — `BuiltinCommand` trait and
  `ExecutionContext` are defined by the pipeline architecture

### Milestones

1. Define `BuiltinCommand` trait and `BuiltinResult` type
2. Implement filesystem built-ins: `ls`, `cp`, `mv`, `rm`, `mkdir`, `cd`, `pwd`
3. Implement text built-ins: `echo`, `cat`, `grep`, `head`, `tail`
4. Implement process built-ins: `ps`, `kill`, `sleep`, `env`, `which`
5. Implement structured output layer (`--output json/ndjson/table`)
6. Integration tests for each built-in on all three platforms
7. Implement remaining built-ins per priority

---

## References

- [GNU Coreutils](https://www.gnu.org/software/coreutils/)
- [PowerShell Cmdlets](https://learn.microsoft.com/en-us/powershell/scripting/developer/cmdlet/cmdlet-overview)
- [Nushell Built-in Commands](https://www.nushell.sh/commands/)
- [POSIX Shell Command Language](https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html)
