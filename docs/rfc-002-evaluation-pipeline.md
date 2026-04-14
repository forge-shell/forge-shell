# RFC-002 — Evaluation Pipeline & Platform Backend

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

This RFC defines the ForgeScript evaluation pipeline — the staged process by
which a `.fgs` source file is transformed into OS-native execution. The
pipeline is a series of pure transformations, each producing a new immutable
data structure. Type checking is a dedicated stage. Errors are collected
within each pass and halt the pipeline between passes. The `ExecutionPlan` is
serialisable by design, enabling RFC-008 plan caching without retrofitting.
OS-specific behaviour is fully isolated behind the `PlatformBackend` trait.

---

## Motivation

A well-defined pipeline is the backbone of a maintainable compiler and
runtime. Without clear stage boundaries, concerns bleed into each other —
type checking inside name resolution, error handling scattered across passes,
platform-specific logic leaking into the AST. This RFC establishes the
contracts between every stage so that each crate has a single, testable
responsibility.

---

## Design

### 1. Pipeline Overview

```
Source (.fgs)
     ↓
   Lexer              — characters → tokens          [forge-lang/lexer]
     ↓
   Parser             — tokens → AST                 [forge-lang/parser]
     ↓
 Type Checker         — typed AST, all errors collected  [forge-lang/typeck]
     ↓
 HIR Lowering         — structural transformation    [forge-lang/hir]
     ↓
 PlatformBackend      — HIR → ExecutionPlan          [forge-backend]
   ├── UnixBackend    — Linux + macOS                [forge-backend/unix]
   └── WindowsBackend — Windows                      [forge-backend/windows]
     ↓
 Execution Engine     — full plan execution          [forge-engine]
```

Each stage:
- Takes ownership of its input
- Produces a new, distinct output data structure
- Never mutates its input
- Returns `Result<Output, Vec<Diagnostic>>` — never a single error

---

### 2. Lexer — `forge-lang/lexer`

**Input:** Raw source bytes (`&str`)
**Output:** `Vec<Token>`

- Normalises CRLF → LF before tokenisation
- Strips UTF-8 BOM silently
- Rejects UTF-16 with a clear diagnostic
- Tokenises all literal prefixes: `p"..."`, `r"..."`, `u"..."`
- Tokenises integer base prefixes: `0x`, `0o`, `0b`
- Tokenises numeric separators: `1_000_000`
- Tokenises `#!forge:` directive lines in the script header
- Tokenises invocation forms: positional, `--flag`, named arguments

---

### 3. Parser — `forge-lang/parser`

**Input:** `Vec<Token>`
**Output:** `AST`

- Recursive descent parser
- Produces a fully-formed AST on success
- AST is **immutable after construction** — no subsequent pass mutates it
- Parses script header directives into a `ScriptMeta` node at the root
- Expands all three invocation forms to canonical named-argument form:

```
ls /home/user --show_hidden --sort name
        ↓  parser expansion
ls path: p"/home/user", show_hidden: true, sort: "name"
```

- Applies positional type inference rules (RFC-001 Section 17)
- Parses all syntax defined in RFC-001

---

### 4. Type Checker — `forge-lang/typeck`

**Input:** `AST`
**Output:** `TypedAST`

- Dedicated stage — separate from HIR lowering
- Performs type inference and semantic validation
- Collects **all** type errors in a single pass — not fail-fast
- Uses **poison values** (`TyKind::Poison`) when an error is found, allowing
  the pass to continue without cascading noise errors
- If any errors are collected, the pipeline halts after this stage

**Responsibilities:**
- Type inference for all literal types
- Function signature validation
- `Result` and `Option` propagation checking (`?` operator)
- Overflow operator type checking (`+|`, `+%`)
- `spawn` / `join!` return type inference
- `Context` usage validation
- Invocation argument type validation

---

### 5. HIR Lowering — `forge-lang/hir`

**Input:** `TypedAST`
**Output:** `HIR`

- Structural transformation — not semantic validation
- Name resolution: variables, functions, modules
- Scope flattening
- Desugaring: `for` loops, `?` operator, `join!` macro
- Produces a simpler, more regular IR than the AST
- HIR nodes carry resolved types from the type checker

---

### 6. Platform Backend — `forge-backend`

**Input:** `HIR`
**Output:** `ExecutionPlan`

- Backend selected once at process startup based on the host OS
- All OS-specific behaviour is isolated here — no platform logic leaks into
  higher stages
- Implemented behind the `PlatformBackend` trait:

```rust
pub trait PlatformBackend {
    fn execute(&self, plan: &ExecutionPlan) -> Result<StructuredOutput, CommandError>;
}

pub struct UnixBackend;
pub struct WindowsBackend;

impl PlatformBackend for UnixBackend { ... }
impl PlatformBackend for WindowsBackend { ... }
```

**Crate structure:**

```
forge-backend/          — PlatformBackend trait + shared types
forge-backend/unix      — UnixBackend (Linux + macOS)
forge-backend/windows   — WindowsBackend
```

**Built-in command backend implementations:**

| Command | UnixBackend | WindowsBackend |
|---|---|---|
| `ls` | `readdir` + dotfile convention | `FindFirstFile` + hidden attribute |
| `stat` | `stat(2)` syscall, Unix rwx | `GetFileAttributesEx`, ACL model |
| `ping` | Raw ICMP socket | `IcmpSendEcho` Win32 API |
| `env` | Colon-separated `$PATH` | Semicolon-separated `%PATH%`, case-insensitive |
| `find` | `walkdir` — symlink-aware | `walkdir` — junction-aware |

---

### 7. ExecutionPlan & Op

The `ExecutionPlan` is a tree of `Op` nodes. It is **serialisable by design**
to enable RFC-008 plan caching without retrofitting.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Op {
    Exec        { program: String, args: Vec<String> },
    SetEnv      { key: String, value: String },
    UnsetEnv    { key: String },
    ChangeDir   { path: PathBuf },
    Pipe        { left: Box<Op>, right: Box<Op> },
    Sequence    { ops: Vec<Op> },
    Conditional { cond: Box<Op>, then: Box<Op>, else_: Option<Box<Op>> },
    Spawn       { op: Box<Op> },
    Join        { ops: Vec<Op> },
    WithContext { ctx: ContextSpec, op: Box<Op> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextSpec {
    WithTimeout { duration_ms: u64 },
    WithCancel  { cancel_id: u64 },
}
```

**Rules:**
- No raw pointers in `Op` nodes
- No `Box<dyn Trait>` — plain enums and structs only
- All nodes derive `serde::Serialize` and `serde::Deserialize`
- Target serialisation format: `postcard` (RFC-008)
- v1 ships without caching — pipeline always runs end to end
- RFC-008 adds caching on top — `ExecutionPlan` is ready by construction

---

### 8. Execution Engine — `forge-engine`

**Input:** `ExecutionPlan`
**Output:** Exit code + stdout/stderr streams

- Executes the `ExecutionPlan` in full on every run — no incremental
  re-evaluation
- Stateless between runs — no cached intermediate execution state
- Dispatches `Op::Spawn` and `Op::Join` to the concurrency runtime
- Dispatches `Op::WithContext` with timeout/cancel propagation
- Delegates OS-native process spawning to `forge-backend` implementations
- Selects `OutputMode` based on context:

```rust
pub enum OutputMode {
    RichTerminal,  // colours, tables, icons — interactive terminal
    PlainText,     // piped — no formatting
    Structured,    // StructuredOutput envelope — AI/MCP context
}

pub struct CommandContext {
    pub output_mode: OutputMode,
    pub platform:    Platform,
    pub working_dir: PathBuf,
}
```

**Output mode selection:**

| Context | Output mode |
|---|---|
| Interactive terminal (TTY) | `RichTerminal` |
| Piped | `PlainText` |
| AI/MCP agent context | `Structured` |
| Explicit `--output json` | `Structured` |

**Incremental re-evaluation — explicit non-goal for v1.**

---

### 9. Error Pipeline

Every pass returns:

```rust
Result<Output, Vec<Diagnostic>>
```

The `Diagnostic` type is defined in `forge-lang/diagnostics`:

```rust
pub struct Diagnostic {
    pub code:    ErrorCode,
    pub message: String,
    pub span:    Span,
    pub help:    Option<String>,
    pub notes:   Vec<String>,
}
```

**Error surfacing rules:**
- Within a pass: collect all errors, continue with poison values
- Between passes: if `Vec<Diagnostic>` is non-empty, halt
- Final report: all diagnostics from all completed passes, rendered together

---

## Drawbacks

- **Immutable pipeline adds allocation overhead.** Mitigated by RFC-008 plan
  caching which amortises the cost across runs.
- **Dedicated type checker stage adds complexity.** Worth it for error quality
  and testability.
- **Three invocation forms add parser complexity.** The expansion layer must
  be correct and consistent across all 32 built-in commands.

---

## Alternatives Considered

### Alternative A — Type checking inside HIR lowering
**Rejected:** Mixed concerns, one error at a time, hard to test in isolation.

### Alternative B — Mutable AST with annotating passes
**Rejected:** Requires `Arc<Mutex<...>>` throughout — fights Rust's ownership
model. Immutable data passed between owned stages is idiomatic.

### Alternative C — Fail-fast error handling
**Rejected:** One error at a time forces repeated recompilation. Collecting
all errors within a pass saves significant iteration time.

### Alternative D — Incremental re-evaluation in v1
**Rejected:** Correctness trumps optimisation. Scripts are not pure functions.
v1 is boring and correct.

### Alternative E — `PlatformLowering` as trait name
**Rejected:** Jargon-heavy. `PlatformBackend` is universally understood in
systems programming and reads naturally with `UnixBackend` / `WindowsBackend`
as concrete implementations.

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | Where does type checking happen? | Dedicated `forge-lang/typeck` stage between AST and HIR |
| UQ-2 | Is the AST immutable? | Immutable — each pass produces a new distinct data structure |
| UQ-3 | How are errors surfaced? | Collect-all within a pass, halt between passes if errors exist |
| UQ-4 | Is `ExecutionPlan` serialisable? | Yes by design — caching implementation deferred to RFC-008 |
| UQ-5 | Incremental re-evaluation? | Always full execution in v1 — explicitly deferred |

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-lang/lexer` | Tokeniser |
| `forge-lang/parser` | Recursive descent parser, AST, invocation expansion |
| `forge-lang/ast` | AST node type definitions |
| `forge-lang/typeck` | Type checker, typed AST |
| `forge-lang/hir` | HIR node definitions, HIR lowering |
| `forge-lang/diagnostics` | Shared `Diagnostic` type, error rendering |
| `forge-backend` | `PlatformBackend` trait, `Op` enum, `ExecutionPlan` |
| `forge-backend/unix` | UnixBackend (Linux + macOS) |
| `forge-backend/windows` | WindowsBackend |
| `forge-engine` | Execution engine, output mode selection |

### Dependencies

- Requires RFC-001 to be accepted first.
- RFC-003 (Built-in Commands) depends on `PlatformBackend` defined here.
- RFC-008 (Plan Caching) depends on the serialisable `ExecutionPlan` defined here.

### Milestones

1. Define all IR types: `AST`, `TypedAST`, `HIR`, `ExecutionPlan`, `Op`
2. Implement `forge-lang/lexer` with full RFC-001 token set
3. Implement `forge-lang/parser` — expressions, bindings, control flow
4. Implement `forge-lang/parser` — functions, structs, enums, imports
5. Implement `forge-lang/parser` — concurrency syntax, overflow operators
6. Implement `forge-lang/parser` — invocation form expansion and type inference
7. Implement `forge-lang/typeck` — type inference, poison values, diagnostics
8. Implement `forge-lang/hir` — name resolution, scope flattening, desugaring
9. Implement `forge-backend` — `PlatformBackend` trait and `Op` enum
10. Implement `forge-backend/unix` — all 32 built-in Unix implementations
11. Implement `forge-backend/windows` — all 32 built-in Windows implementations
12. Implement `forge-engine` — full plan execution, output mode selection
13. Integration tests on ubuntu-latest, macos-latest, windows-latest

---

## References

- [Rust Compiler Guide — Overview](https://rustc-dev-guide.rust-lang.org/overview.html)
- [Rust Compiler Guide — Type Checking](https://rustc-dev-guide.rust-lang.org/type-checking.html)
- [Nushell Engine](https://github.com/nushell/nushell/tree/main/crates/nu-engine)
- [Postcard Serialisation Format](https://github.com/jamesmunns/postcard)
- [RFC-001 — ForgeScript Language Syntax & Type System](./RFC-001-forgescript-syntax.md)
- [RFC-003 — Built-in Command Specification](./RFC-003-builtin-commands.md)
- [RFC-008 — Plan Caching & AOT Compilation](./RFC-008-plan-caching-aot.md)