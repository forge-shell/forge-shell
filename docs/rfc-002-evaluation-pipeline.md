# RFC-002 — Evaluation Pipeline & Platform Lowering

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
 Platform Lowering    — HIR → ExecutionPlan          [forge-lower]
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

---

### 3. Parser — `forge-lang/parser`

**Input:** `Vec<Token>`
**Output:** `AST`

- Recursive descent parser
- Produces a fully-formed AST on success
- AST is **immutable after construction** — no subsequent pass mutates it
- Parses script header directives into a `ScriptMeta` node attached to the root
- Parses all syntax defined in RFC-001:
  - Literals, bindings, control flow, functions, structs, enums
  - Imports and module declarations
  - `spawn`, `join!`, `Context` concurrency syntax
  - Per-expression overflow operators `+|`, `+%`, `-|`, `-%`, `*|`, `*%`

---

### 4. Type Checker — `forge-lang/typeck`

**Input:** `AST`
**Output:** `TypedAST`

- Dedicated stage — separate from HIR lowering
- Performs type inference and semantic validation
- Collects **all** type errors in a single pass — not fail-fast
- Uses **poison values** (`TyKind::Poison`) when an error is found, allowing
  the pass to continue without cascading noise errors
- If any errors are collected, the pipeline halts after this stage — HIR
  lowering never runs on invalid input

**Responsibilities:**
- Type inference for all literal types
- Function signature validation
- `Result` and `Option` propagation checking (`?` operator)
- Overflow operator type checking (`+|`, `+%`)
- `spawn` / `join!` return type inference
- `Context` usage validation

---

### 5. HIR Lowering — `forge-lang/hir`

**Input:** `TypedAST`
**Output:** `HIR`

- Structural transformation — not semantic validation
- Name resolution: variables, functions, modules
- Scope flattening
- Desugaring: `for` loops, `?` operator, `join!` macro
- Produces a simpler, more regular IR than the AST
- HIR nodes carry resolved types from the type checker — no type inference here

---

### 6. Platform Lowering — `forge-lower`

**Input:** `HIR`
**Output:** `ExecutionPlan`

- Backend selected once at process startup based on the host OS
- All OS-specific behaviour is isolated here — no platform logic leaks into
  higher stages
- Implemented behind the `PlatformLowering` trait:

```rust
pub trait PlatformLowering {
    fn lower(&self, hir: &HIR) -> Result<ExecutionPlan, Vec<Diagnostic>>;
}
```

- Concrete backends: `UnixLowering` (Linux, macOS), `WindowsLowering`
- The `ExecutionPlan` is the only artifact that crosses the platform boundary

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

---

### 8. Execution Engine — `forge-engine`

**Input:** `ExecutionPlan`
**Output:** Exit code + stdout/stderr streams

- Executes the `ExecutionPlan` in full on every run — no incremental
  re-evaluation
- Stateless between runs — no cached intermediate execution state
- Dispatches `Op::Spawn` and `Op::Join` to the concurrency runtime
- Dispatches `Op::WithContext` with timeout/cancel propagation
- Delegates OS-native process spawning to `forge-lower` backends

**Incremental re-evaluation — explicit non-goal for v1:**
Shell scripts interact with file system, environment, and network state that
cannot be assumed stable between runs. Safely skipping execution steps
requires hermetic sandboxing and explicit dependency tracking — a separate
system, not a bolt-on. Deferred to post-v1 as a dedicated RFC if demand
exists.

---

### 9. Error Pipeline

Every pass returns:

```rust
Result<Output, Vec<Diagnostic>>
```

The `Diagnostic` type is defined in `forge-lang/diagnostics` and shared across
all passes:

```rust
pub struct Diagnostic {
    pub code:    ErrorCode,   // e.g. E001
    pub message: String,      // human-readable description
    pub span:    Span,        // file, line, column
    pub help:    Option<String>, // actionable suggestion
    pub notes:   Vec<String>, // additional context
}
```

**Error surfacing rules:**
- Within a pass: collect all errors, continue with poison values
- Between passes: if `Vec<Diagnostic>` is non-empty, halt — do not run the
  next stage
- Final error report: all diagnostics from all completed passes, rendered
  together

---

## Drawbacks

- **Immutable pipeline adds allocation overhead.** Each stage allocates a new
  data structure rather than mutating in place. For large scripts this may be
  measurable — mitigated by RFC-008 plan caching which amortises the cost
  across runs.
- **Dedicated type checker stage adds complexity.** A combined HIR + typeck
  pass would be simpler to implement initially — but produces worse errors and
  is harder to test in isolation.

---

## Alternatives Considered

### Alternative A — Type checking inside HIR lowering

**Rejected because:** Mixing semantic validation with structural
transformation produces a pass that is hard to test, hard to debug, and
produces one error at a time. A dedicated `forge-lang/typeck` crate with a
single responsibility is worth the extra stage.

### Alternative B — Mutable AST with annotating passes

**Rejected because:** A mutable shared AST in Rust requires `Arc<Mutex<...>>`
throughout, fighting the borrow checker at every turn. Immutable data passed
between owned stages is idiomatic Rust and enables concurrent pass execution.

### Alternative C — Fail-fast error handling

**Rejected because:** Reporting one error at a time forces the developer to
fix and recompile repeatedly. Collecting all errors within a pass and
reporting them together — as Rust, TypeScript, and Go do — saves significant
iteration time.

### Alternative D — Incremental re-evaluation in v1

**Rejected because:** Correctness trumps optimisation. Scripts are not pure
functions — file system, environment, and network state cannot be assumed
stable. Getting incremental execution wrong silently produces incorrect
results. v1 is boring and correct.

---

## Unresolved Questions

All previously unresolved questions have been resolved. See resolution summary
below.

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
| `forge-lang/parser` | Recursive descent parser, AST construction |
| `forge-lang/ast` | AST node type definitions |
| `forge-lang/typeck` | Type checker, typed AST |
| `forge-lang/hir` | HIR node definitions, HIR lowering pass |
| `forge-lang/diagnostics` | Shared `Diagnostic` type, error rendering |
| `forge-lower` | `PlatformLowering` trait, `Op` enum, `ExecutionPlan` |
| `forge-lower/unix` | Unix backend (Linux + macOS) |
| `forge-lower/windows` | Windows backend |
| `forge-engine` | Execution engine |

### Dependencies

- Requires RFC-001 to be accepted first — the pipeline exists to evaluate
  ForgeScript as defined in RFC-001.
- RFC-008 (Plan Caching) depends on this RFC — specifically the serialisable
  `ExecutionPlan` defined here.

### Milestones

1. Define all IR types: `AST`, `TypedAST`, `HIR`, `ExecutionPlan`, `Op`
2. Implement `forge-lang/lexer` with full RFC-001 token set
3. Implement `forge-lang/parser` — expressions, bindings, control flow
4. Implement `forge-lang/parser` — functions, structs, enums, imports
5. Implement `forge-lang/parser` — concurrency syntax, overflow operators
6. Implement `forge-lang/typeck` — type inference, poison values, diagnostic collection
7. Implement `forge-lang/hir` — name resolution, scope flattening, desugaring
8. Implement `forge-lower` — `PlatformLowering` trait and `Op` enum
9. Implement `forge-lower/unix` — Unix backend
10. Implement `forge-lower/windows` — Windows backend
11. Implement `forge-engine` — full plan execution, concurrency dispatch
12. Integration tests for full pipeline on ubuntu-latest, macos-latest, windows-latest

---

## References

- [Rust Compiler Guide — Overview](https://rustc-dev-guide.rust-lang.org/overview.html)
- [Rust Compiler Guide — Type Checking](https://rustc-dev-guide.rust-lang.org/type-checking.html)
- [Nushell Engine](https://github.com/nushell/nushell/tree/main/crates/nu-engine)
- [Postcard Serialisation Format](https://github.com/jamesmunns/postcard)
- [RFC-001 — ForgeScript Language Syntax & Type System](./RFC-001-forgescript-syntax.md)
- [RFC-008 — Plan Caching & AOT Compilation](./RFC-008-plan-caching-aot.md)
