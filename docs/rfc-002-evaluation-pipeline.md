# RFC-002 — Evaluation Pipeline & Platform Lowering

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

This RFC defines the ForgeScript evaluation pipeline — the staged process by
which a `.fgs` source file is transformed into OS-native execution. The
pipeline terminates in a platform lowering step where a High-Level IR (HIR)
is translated into platform-specific `ExecutionPlan` nodes by a backend
selected once at process startup. This is the architectural mechanism that
enables Forge Shell's cross-platform guarantee.

---

## Motivation

A naive cross-platform shell implementation scatters `if cfg!(windows)` blocks
throughout the interpreter logic. This approach is unmaintainable: platform
differences compound, test coverage becomes combinatorial, and new platform
support requires surgery across the entire codebase.

Forge Shell requires a principled separation between:

- **What** a script intends to do (language semantics — AST and HIR)
- **How** that intent is realised on a given OS (execution mechanics —
  platform lowering)

This separation enables the platform-specific code to live in exactly one
place, behind a single trait, with clearly defined interfaces.

---

## Design

### 1. Pipeline Stages

```
Source (.fgs)
    │
    ▼
┌─────────┐
│  Lexer  │  Tokenises source. Normalises line endings. Strips BOM.
└────┬────┘  Emits: Token stream
     │
     ▼
┌─────────┐
│ Parser  │  Recursive descent. Fails fast on syntax errors.
└────┬────┘  Emits: AST (Abstract Syntax Tree)
     │
     ▼
┌─────────┐
│   HIR   │  Name resolution. Type inference. Semantic validation.
└────┬────┘  Built-in command resolution happens here.
     │       Emits: HIR (High-Level IR)
     ▼
┌──────────────────┐
│ Platform Lowering │  HIR nodes → platform-specific ExecutionPlan.
└────────┬─────────┘  Backend selected once at startup.
         │            Emits: ExecutionPlan
         ▼
┌──────────────────┐
│ Execution Engine │  Walks ExecutionPlan. Manages I/O, pipes, jobs.
└────────┬─────────┘
         ▼
     OS / Kernel
```

Each stage is a pure transformation with a defined input and output type.
No stage reaches backwards into a prior stage's output.

---

### 2. Lexer (`forge-lang/lexer`)

**Input:** Raw UTF-8 source bytes
**Output:** `Vec<Token>`

Responsibilities:
- Strip BOM (`\xEF\xBB\xBF`) silently
- Normalise `\r\n` → `\n` before any other processing
- Reject non-UTF-8 input with a clear diagnostic
- Tokenise: identifiers, keywords, literals, operators, punctuation
- Preserve span information (byte offset, line, column) on every token
  for error reporting

The lexer is entirely platform-agnostic. It has no knowledge of paths,
processes, or OS APIs.

---

### 3. Parser (`forge-lang/parser`)

**Input:** `Vec<Token>`
**Output:** `Ast`

A recursive descent parser. Pratt parsing is used for expressions to handle
operator precedence cleanly.

Responsibilities:
- Produce a complete, well-typed AST
- Emit structured parse errors with span information
- Fail fast — do not attempt error recovery in v1

The parser is entirely platform-agnostic.

---

### 4. AST (`forge-lang/ast`)

The AST is the canonical in-memory representation of a `.fgs` program. It is:

- **Platform-agnostic** — no OS-specific types or concepts
- **Serialisable** — must be serialisable to/from a binary format for caching
- **Span-preserving** — every node carries its source location

Key AST node categories:

```
Declarations:  LetDecl, FnDecl, StructDecl, EnumDecl, ImportDecl
Expressions:   Literal, Ident, BinOp, UnaryOp, Call, Index, Field,
               If, Match, Block, Closure, Interpolation
Statements:    ExprStmt, ReturnStmt, BreakStmt, ContinueStmt
Commands:      Pipeline, Redirect, Spawn, Exec
Types:         PrimType, PathType, ListType, MapType, OptionType,
               ResultType, FnType, StructType, EnumType
```

---

### 5. HIR (`forge-lang/hir`)

**Input:** `Ast`
**Output:** `Hir`

The HIR lowers the AST through:

- **Name resolution** — identifiers resolved to their declaration sites
- **Type inference** — types inferred where not explicitly annotated
- **Built-in resolution** — command names resolved to `Builtin` variants
  or `ExternalCommand` (PATH lookup deferred to platform lowering)
- **Semantic validation** — type errors, unused variables, unreachable code

The HIR is still platform-agnostic. A `Builtin::Ls` node in the HIR carries
no knowledge of `readdir()` vs `FindFirstFile()`. That is the platform
lowering's concern.

---

### 6. Platform Lowering (`forge-lower`)

**Input:** `Hir`
**Output:** `ExecutionPlan`

This is where platform divergence is handled — and only here.

#### The Trait

```rust
pub trait PlatformLowering: Send + Sync {
    fn lower_spawn   (&self, cmd: &SpawnProcess)      -> ExecutionPlan;
    fn lower_signal  (&self, handler: &SignalHandler)  -> ExecutionPlan;
    fn lower_file_op (&self, op: &FileOperation)       -> ExecutionPlan;
    fn lower_env_op  (&self, op: &EnvOperation)        -> ExecutionPlan;
    fn lower_path    (&self, path: &ForgePath)         -> NativePath;
    fn lower_job_op  (&self, op: &JobOperation)        -> ExecutionPlan;
    fn lower_builtin (&self, cmd: &Builtin)            -> ExecutionPlan;
}
```

#### Backend Selection

Selected once at process startup. Stored as an `Arc<dyn PlatformLowering>`
threaded through the execution context. Never re-selected per command.

```rust
pub fn select_platform() -> Arc<dyn PlatformLowering> {
    match std::env::consts::OS {
        "linux"   => Arc::new(UnixLowering::new()),
        "macos"   => Arc::new(MacOSLowering::new()),
        "windows" => Arc::new(WindowsLowering::new()),
        other     => panic!("Unsupported platform: {other}"),
    }
}
```

#### Backends

| Backend | File | Notes |
|---|---|---|
| `UnixLowering` | `unix.rs` | Linux implementation |
| `MacOSLowering` | `macos.rs` | Delegates to `UnixLowering`, overrides where macOS diverges |
| `WindowsLowering` | `windows.rs` | Full independent implementation |

#### Lowering Examples

**Process spawn:**
```
HIR: SpawnProcess { cmd: "git", args: ["status"] }

Unix:
  1. resolve "git" in PATH (colon-separated)
  2. Op::Fork
  3. Op::Execve { path: "/usr/bin/git", args: ["status"] }
  4. Op::Waitpid

Windows:
  1. resolve "git" in PATH (semicolon-separated)
  2. probe: git → git.exe → git.cmd → git.bat
  3. Op::CreateProcess { cmd: "C:\\Program Files\\Git\\bin\\git.exe status" }
  4. Op::WaitForSingleObject
```

**Signal registration:**
```
HIR: RegisterHandler { signal: Interrupt, body: Block }

Unix:    Op::Sigaction { signal: SIGINT, handler }
Windows: Op::SetConsoleCtrlHandler { event: CTRL_C_EVENT, handler }
```

**PATH prepend:**
```
HIR: PrependToPath { value: ForgePath("/usr/local/bin") }

Unix:
  Op::Setenv { key: "PATH", value: "/usr/local/bin:" + current }

Windows:
  Op::SetEnvironmentVariable {
    key: "PATH",
    value: "C:\\usr\\local\\bin;" + current   // lower_path() applied
  }
```

---

### 7. ExecutionPlan (`forge-exec/plan`)

An `ExecutionPlan` is a linear sequence of `Op` variants that the execution
engine can walk without further platform knowledge.

```rust
pub enum Op {
    // Process
    Fork,
    Execve       { path: NativePath, args: Vec<String>, env: Env },
    CreateProcess{ cmd: String, env: Env },
    Waitpid      { pid: Pid },
    WaitObject   { handle: Handle },

    // I/O
    Pipe         { read_fd: Fd, write_fd: Fd },
    Redirect     { from: Fd, to: RedirectTarget },
    Closefd      { fd: Fd },

    // Environment
    Setenv       { key: String, value: String },
    Unsetenv     { key: String },

    // Signals
    Sigaction    { signal: Signal, handler: Handler },
    CtrlHandler  { event: CtrlEvent, handler: Handler },

    // Builtins (executed directly, no subprocess)
    Builtin      { cmd: BuiltinCmd },

    // Control
    ExitWith     { code: i32 },
    Noop,
}
```

`ExecutionPlan` is serialisable — enabling plan caching.

---

### 8. Execution Engine (`forge-exec/engine`)

**Input:** `ExecutionPlan`
**Output:** `ExitStatus`

The engine walks the `ExecutionPlan` sequentially, dispatching each `Op` to
the appropriate OS call. The engine is platform-aware only in the sense that
it calls OS APIs directly — but by this point, all platform decisions have
already been made by the lowering step. The engine contains no branching on
OS type.

Responsibilities:
- Walk `ExecutionPlan` ops in order
- Manage file descriptors for pipes and redirections
- Track spawned child processes and job state
- Collect and propagate exit codes
- Invoke built-in command implementations from `forge-builtins`

---

### 9. Plan Caching (`forge-exec/cache`)

Parsed and lowered plans are cached to accelerate repeated script execution.

**Cache location:**
```
~/.forge/cache/scripts/
├── {content_hash}.{platform}.plan
```

**Cache key components:**
- SHA-256 of script source content
- Forge version string
- Platform identifier (`linux`, `macos`, `windows`)

**Invalidation:**
- Any change to script content (hash mismatch)
- Forge binary version change
- Manual `forge cache clear`

**Cache format:**
- Binary serialisation via `bincode` or `postcard`
- Plans are versioned — an old plan version is silently discarded and
  regenerated

**Future:** The cache enables a `forge compile` subcommand that
ahead-of-time compiles `.fgs` scripts to native executables via the
cached plan + a native code emission backend.

---

### 10. Dry-Run Mode

Because the `ExecutionPlan` is a data structure, it can be inspected without
execution. `forge --dry-run script.fgs` runs the full pipeline through
platform lowering but skips the execution engine, instead printing the plan:

```
$ forge --dry-run deploy.fgs

ExecutionPlan for deploy.fgs (linux):
  [0] Setenv        { key: "APP_ENV", value: "production" }
  [1] Fork          {}
  [2] Execve        { path: /usr/bin/kubectl, args: [apply, -f, manifest.yaml] }
  [3] Waitpid       { pid: <child> }
  [4] Fork          {}
  [5] Execve        { path: /usr/bin/kubectl, args: [rollout, status, deployment/app] }
  [6] Waitpid       { pid: <child> }
```

This is a first-class debugging and auditing tool, especially for AI agent
workflows.

---

## Drawbacks

- **More layers than a simple interpreter** — the pipeline has more moving
  parts than a tree-walking interpreter. Initial implementation effort is higher.
- **Plan serialisation complexity** — caching requires a stable binary format
  for `ExecutionPlan`. Format migrations must be handled carefully.
- **HIR design iteration** — the HIR is the most complex stage. Getting the
  node types right requires careful design before implementation.

---

## Alternatives Considered

### Alternative A — Tree-Walking Interpreter with Platform Branches

**Approach:** Walk the AST directly, branching on `std::env::consts::OS`
at each command node.
**Rejected because:** Platform branching spreads throughout the evaluator.
Every new command or operation requires touching platform-specific code in
multiple places. Untestable combinatorially.

### Alternative B — Compile to Host Shell

**Approach:** Transpile `.fgs` to bash on Unix and PowerShell on Windows.
**Rejected because:** This makes Forge Shell a transpiler, not a shell. Error
messages reference the target language, not ForgeScript. Behaviour diverges
wherever bash and PowerShell semantics differ — which is everywhere.

### Alternative C — Single IR, Platform Flags on Ops

**Approach:** One unified `Op` type with a `platform: Platform` field.
**Rejected because:** This conflates the what and the how in the IR. The
execution engine would need to branch per-op, recreating the scattered
branching problem.

---

## Unresolved Questions

- [ ] Should the HIR include a dedicated `PipelineNode` or should pipelines
      be expressed as nested `SpawnProcess` nodes with connected file descriptors?
- [ ] Should `ExecutionPlan` support branching (for `if`/`match` in scripts)
      or should control flow be resolved before plan generation?
- [ ] What is the plan cache eviction policy? LRU? Size limit? TTL?
- [ ] Should `forge --dry-run` output JSON for machine consumption in agent
      workflows?

---

## Implementation Plan

### Affected Crates

- `forge-lang/lexer`
- `forge-lang/parser`
- `forge-lang/ast`
- `forge-lang/hir`
- `forge-lower`
- `forge-exec`

### Dependencies

- Requires RFC-001 (ForgeScript Syntax) to be accepted — HIR types are
  derived from the language definition.
- RFC-003 (Built-in Commands) must be in progress — `lower_builtin()` needs
  a stable built-in command set to lower.

### Milestones

1. Define `Op` enum and `ExecutionPlan` type in `forge-exec/plan`
2. Implement `PlatformLowering` trait in `forge-lower`
3. Implement `UnixLowering` — process spawn, pipes, signals
4. Implement `WindowsLowering` — `CreateProcess`, `WaitForSingleObject`,
   `SetConsoleCtrlHandler`
5. Implement `MacOSLowering` — delegates to Unix, overrides where needed
6. Implement execution engine in `forge-exec/engine`
7. Implement plan caching in `forge-exec/cache`
8. Wire pipeline end-to-end in `forge-cli`
9. Integration tests: cross-platform execution of simple `.fgs` scripts

---

## References

- [Crafting Interpreters — Tree-Walking vs Bytecode](https://craftinginterpreters.com)
- [Nushell Engine Design](https://github.com/nushell/nushell/blob/main/docs/engine.md)
- [LLVM IR Design Principles](https://llvm.org/docs/LangRef.html)
- [V8 Ignition Interpreter](https://v8.dev/blog/ignition-interpreter)
- [Windows Process Creation](https://learn.microsoft.com/en-us/windows/win32/procthread/creating-processes)
