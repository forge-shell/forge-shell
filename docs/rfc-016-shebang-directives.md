# RFC-016 — Shebang Directives

| Field         | Value                |
|---------------|----------------------|
| Status        | Draft                |
| Author        | Ajitem Sahasrabuddhe |
| Created       | 2026-04-17           |
| Last Updated  | 2026-04-17           |
| Supersedes    | —                    |
| Superseded By | —                    |

---

## Summary

ForgeScript files may begin with `#!` lines that serve two distinct purposes:
Unix shebangs (`#!/usr/bin/env forge`) for direct script execution, and
ForgeScript directives (`#!abi:stable`) for per-file pipeline configuration.
This RFC defines the syntax, permitted directive keys, lexer representation,
parser handling, and execution semantics for both forms.

---

## Motivation

During implementation of the `forge-lexer` crate, a design question emerged:
`#!` lines were initially treated as comments and discarded. This works for
Unix shebangs but silently swallows any structured directives the pipeline
needs to act on — for example, ABI stability declarations and plugin
configuration.

Without a formal specification, each pipeline stage would need ad-hoc
handling. This RFC defines the contract once so every stage — lexer, parser,
platform backend, executor — knows exactly what to expect.

---

## Design

### Lexer Representation

The lexer emits `#!` lines as a dedicated token variant rather than
discarding them as comments:

```rust
/// A shebang or directive line starting with `#!`.
/// The inner string is the content after `#!`, trimmed of whitespace.
///
/// Examples:
///   `#!/usr/bin/env forge`  →  ShebangDirective("/usr/bin/env forge")
///   `#!abi:stable`          →  ShebangDirective("abi:stable")
ShebangDirective(String),
```

`#!` lines may appear **anywhere in a file**, not only on line 1. The lexer
produces `ShebangDirective` tokens wherever it encounters them. The parser
enforces positional constraints (see below).

### Syntax

```
shebang_line ::= "#!" <content> <newline>
content      ::= unix_shebang | directive
unix_shebang ::= "/" <anything>          # starts with '/'
directive    ::= <key> ":" <value>
key          ::= [a-z][a-z0-9_-]*
value        ::= <anything except newline>
```

### Parser Handling

The parser collects all leading `ShebangDirective` tokens into a
`Vec<Directive>` on the `Program` node before any statements are parsed.
Directives after the first statement are a **parse error** —
`ParseError::DirectiveAfterStatement`.

```rust
pub struct Program {
    pub directives: Vec<Directive>,
    pub stmts: Vec<Stmt>,
}

pub struct Directive {
    pub kind: DirectiveKind,
    pub span: Span,
}

pub enum DirectiveKind {
    /// `#!/path/to/interpreter` — Unix shebang, ignored at runtime on all platforms
    UnixShebang(String),
    /// `#!abi:stable` / `#!abi:unstable`
    Abi(AbiStability),
    /// `#!plugin:<name>` — declares this file is a plugin entry point
    Plugin(String),
    /// An unrecognised directive — preserved for forward compatibility
    Unknown { key: String, value: String },
}

pub enum AbiStability {
    Stable,
    Unstable,
}
```

### Directive Reference

| Directive | Example | Meaning |
|---|---|---|
| Unix shebang | `#!/usr/bin/env forge` | Direct execution on Unix. Ignored on Windows. |
| `abi` | `#!abi:stable` | Declares the script's public API follows stable ABI rules. Enforced by RFC-014 versioning policy. |
| `abi` | `#!abi:unstable` | Explicitly opts out of ABI stability guarantees. |
| `plugin` | `#!plugin:my-plugin` | Marks file as a WASM plugin entry point. Consumed by `forge-plugin`. |

### Execution Semantics

- **`UnixShebang`** — silently ignored by the executor on all platforms.
  Present only for OS-level direct execution support.
- **`Abi(Stable)`** — the executor and plugin host enforce that no
  unstable APIs are called. Produces `ExecError::UnstableApiInStableScript`
  if violated.
- **`Abi(Unstable)`** — no enforcement. Default if `#!abi` is absent.
- **`Plugin`** — consumed by `forge-plugin` during plugin loading. The
  executor treats the file as a plugin entry point rather than a script.
- **`Unknown`** — emitted as a `tracing::warn!` at runtime, then ignored.
  This preserves forward compatibility — a script written for a future
  version of Forge runs without error on an older version.

### Windows Behaviour

On Windows, `#!/usr/bin/env forge` has no OS-level effect — Windows does
not use shebangs for execution. The lexer and parser handle the token
identically on all platforms. The executor ignores `UnixShebang` on all
platforms. No special-casing is required anywhere in the pipeline.

### Example Scripts

**Direct Unix execution:**
```
#!/usr/bin/env forge
let x = 42
echo x
```

**Stable ABI script (importable as a library):**
```
#!abi:stable

pub fn greet(name: str) -> str {
    "Hello, {name}!"
}
```

**Plugin entry point:**
```
#!plugin:my-tool
#!abi:stable

pub fn run(args: list<str>) -> str {
    # plugin implementation
}
```

---

## Drawbacks

- Adds a new token variant and AST node type before the parser is fully
  implemented, increasing scope of Issue #8 and #11 slightly.
- `Unknown` directive forward-compatibility means malformed directives
  (`#!typo:value`) are silently warned rather than hard errors. A strict
  mode flag could be added later but is out of scope here.

---

## Alternatives Considered

### Alternative A — Treat all `#!` as comments (discard)

**Approach:** The lexer skips `#!` lines entirely, same as `#` comments.

**Rejected because:** Directives like `#!abi:stable` are never seen by the
pipeline. There is no way to attach per-file metadata without a separate
mechanism.

### Alternative B — Separate directive syntax (not `#!`)

**Approach:** Use a different syntax for directives, e.g. `@directive abi:stable`
or a frontmatter block.

**Rejected because:** `#!` is the established Unix convention. Reusing it
for both shebangs and directives keeps the syntax minimal and familiar.
Frontmatter blocks (YAML/TOML headers) add parsing complexity with no
benefit.

### Alternative C — Only allow directives on line 1

**Approach:** The lexer only produces `ShebangDirective` for line 1.

**Rejected because:** A file may have both a Unix shebang on line 1 and
`#!abi:stable` on line 2. Restricting to line 1 forces a choice between
the two. The parser's "directives must precede statements" rule is
sufficient constraint without artificially limiting line numbers.

---

## Unresolved Questions

- [ ] **UQ-1:** Should `Unknown` directives be a hard error under a
  `--strict` flag, or always a warning? Deferred — `--strict` mode is not
  yet defined.
- [ ] **UQ-2:** Should `#!abi:stable` be enforceable at import time (i.e.
  the importer can assert the imported module is stable)? Depends on the
  module system design in RFC-001.
- [ ] **UQ-3:** Are there additional directive keys needed for the AI agent
  layer (RFC-007)? To be revisited when RFC-007 implementation begins.

---

## Implementation Plan

### Affected Crates

- `forge-lexer` — add `ShebangDirective(String)` to `TokenKind`
- `forge-ast` — add `Directive`, `DirectiveKind`, `AbiStability`; add
  `directives: Vec<Directive>` to `Program`
- `forge-parser` — collect leading directives before parsing statements;
  add `ParseError::DirectiveAfterStatement`
- `forge-exec` — enforce `Abi(Stable)` constraint; ignore `UnixShebang`
- `forge-plugin` — consume `Plugin` directive during plugin loading

### Dependencies

- RFC-001 (module system) — needed to fully resolve UQ-2
- RFC-007 (AI agent layer) — needed to resolve UQ-3
- RFC-014 (release policy) — ABI stability enforcement links to versioning

### Milestones

1. `forge-lexer`: add `ShebangDirective` token variant and tests — **Issue #32**
2. `forge-ast`: add `Directive` types and update `Program` — **Issue #33**
3. `forge-parser`: collect and validate directives — **Issue #34**
4. `forge-exec`: enforce ABI directive at runtime — defer to v0.3

Milestones 1–3 target **v0.2 — Pipeline** (current milestone).
Milestone 4 targets **v0.3 — Execution**.

---

## References

- [Unix shebang](https://en.wikipedia.org/wiki/Shebang_(Unix))
- [RFC-001 — ForgeScript Language Syntax & Type System](https://github.com/forge-shell/forge-shell/blob/main/docs/rfc-001-forgescript-syntax.md)
- [RFC-007 — AI Agent Layer & MCP Protocol Integration](https://github.com/forge-shell/forge-shell/blob/main/docs/rfc-007-ai-agent-layer.md)
- [RFC-014 — Release Policy & Versioning](https://github.com/forge-shell/forge-shell/blob/main/docs/rfc-014-release-policy.md)