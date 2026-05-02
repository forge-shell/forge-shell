# RFC-020 — Glob Expansion

| Field          | Value                        |
|----------------|------------------------------|
| Status         | Draft                        |
| Author         | Ajitem Sahasrabuddhe         |
| Created        | 2026-05-02                   |
| Last Updated   | 2026-05-02                   |
| Supersedes     | —                            |
| Superseded By  | —                            |

---

## Summary

Allow shell-style glob patterns — `*.rs`, `**/*.toml`, `src/?.rs`,
`[0-9]*.txt` — in shell-style command invocations. Quoted strings (`"*.rs"`)
are never expanded; only bare glob patterns in shell-arg position are expanded
at execution time against the current working directory. Expansion is performed
by the executor immediately before passing arguments to a command.

---

## Motivation

```bash
ls *.toml
find . -name *.rs
cp src/*.rs /tmp/backup/
rm -rf build/**
```

Every shell user writes these daily. After RFC-018, `find . -name "*.rs"` works
(quoted glob, passed verbatim). But `find . -name *.rs` (bareword glob) fails
because `*` is the multiplication operator token. ForgeShell will not feel like
a real shell until bareword globs work.

---

## Design

### Overview: Parser Assembles, Executor Expands

1. **Parser** (`parse_shell_args`): recognises glob-containing token sequences
   in shell-arg position and emits `Expr::Literal(Literal::Glob(pattern))`.
2. **Backend**: passes `Literal::Glob` through as `Value::Glob(pattern)`.
3. **Executor**: just before running a command, for each `Value::Glob(pattern)`
   argument, expands against `cwd` and replaces the single arg with N string
   args (the matched paths). If no paths match, passes the literal pattern
   (same as bash `nullglob` disabled, which is the default).

Quoted strings bypass glob expansion entirely — they arrive as `Value::Str`
and are never expanded.

### AST Change: `Literal::Glob`

**File:** `crates/forge-ast/src/expr.rs`

Add `Glob(String)` to the `Literal` enum:

```rust
pub enum Literal {
    Int(i64), Float(f64), Str(String), Bool(bool), Null,
    Glob(String),   // bareword glob, expanded at runtime
}
```

### Backend Change: `Value::Glob`

**File:** `crates/forge-backend/src/plan.rs`

Add `Glob(String)` to the `Value` enum:

```rust
pub enum Value {
    Int(i64), Float(f64), Str(String), Bool(bool), List(Vec<Value>), Null,
    VarRef(String), EnvRef(String),
    Glob(String),   // expanded by executor before command invocation
}
```

### Parser Change: Glob Assembly in `parse_shell_args`

**File:** `crates/forge-parser/src/parser.rs`

Glob patterns are assembled from sequences of `Star`, `Ident`, `Dot`,
`DotDot`, `Slash`, `Integer`, and `Minus` tokens in which at least one `Star`
appears. The assembly stops at the same tokens that end any shell arg.

Add `is_glob_start()` helper:

```rust
fn is_glob_start(&self) -> bool {
    // Starts with * directly
    matches!(self.peek_kind(), TokenKind::Star)
    // Or an ident immediately followed by * (e.g. `foo*`)
    || (matches!(self.peek_kind(), TokenKind::Ident(_))
        && matches!(self.peek_kind_at(1), TokenKind::Star))
}
```

Add `parse_glob_arg()`:

```rust
fn parse_glob_arg(&mut self) -> Result<Arg, ParseError> {
    let mut pattern = String::new();
    loop {
        match self.peek_kind().clone() {
            TokenKind::Star => {
                self.advance();
                if self.check_kind(&TokenKind::Star) {
                    self.advance();
                    pattern.push_str("**");
                } else {
                    pattern.push('*');
                }
            }
            TokenKind::Ident(s) => { self.advance(); pattern.push_str(&s); }
            TokenKind::Integer(n) => { self.advance(); pattern.push_str(&n.to_string()); }
            TokenKind::Dot => { self.advance(); pattern.push('.'); }
            TokenKind::Slash => { self.advance(); pattern.push('/'); }
            TokenKind::Minus => {
                if matches!(self.peek_kind_at(1), TokenKind::Ident(_) | TokenKind::Star) {
                    self.advance(); pattern.push('-');
                } else { break; }
            }
            _ => break,
        }
    }
    Ok(Arg::Positional(Expr::Literal(Literal::Glob(pattern))))
}
```

In `parse_shell_args`, add before the `Ident` arm:

```rust
_ if self.is_glob_start() => self.parse_glob_arg()?,
```

### HIR Change: Pass `Glob` Through

**File:** `crates/forge-hir/src/hir.rs` — add `Glob(String)` to `HirLiteral`.

**File:** `crates/forge-hir/src/lower.rs`

In `lower_expr` for `Expr::Literal`:

```rust
Literal::Glob(pattern) => HirLiteral::Glob(pattern),
```

### Backend Change: `Value::Glob` in `lower_literal`

**File:** `crates/forge-backend/src/lower.rs`

```rust
HirLiteral::Glob(pattern) => Value::Glob(pattern),
```

### Executor Change: Glob Expansion at Command Invocation

**File:** `crates/forge-exec/src/executor.rs`

Add an `expand_args` method called just before running any command:

```rust
fn expand_args(&self, args: &[Value]) -> Vec<String> {
    let mut result = Vec::new();
    for arg in args {
        match arg {
            Value::Glob(pattern) => {
                let expanded = self.expand_glob(pattern);
                if expanded.is_empty() {
                    // No matches: pass pattern verbatim (bash default)
                    result.push(pattern.clone());
                } else {
                    result.extend(expanded);
                }
            }
            other => result.push(self.context.resolve_to_string(other)),
        }
    }
    result
}

fn expand_glob(&self, pattern: &str) -> Vec<String> {
    // Use the `glob` crate, patterns are relative to cwd
    let base = &self.context.cwd;
    glob::glob(&base.join(pattern).to_string_lossy())
        .into_iter()
        .flatten()
        .filter_map(|p| p.ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}
```

Add `glob = "0.3"` to `crates/forge-exec/Cargo.toml`.

### Glob Patterns Supported

| Pattern       | Matches                              |
|---------------|--------------------------------------|
| `*.rs`        | All `.rs` files in cwd               |
| `*.toml`      | All `.toml` files in cwd             |
| `src/*.rs`    | All `.rs` files in `src/`            |
| `**/*.rs`     | All `.rs` files recursively          |
| `?.rs`        | Single-char-named `.rs` files        |
| `[0-9]*.txt`  | `.txt` files starting with a digit   |

### Quoted Globs (No Expansion)

```forge
find "." -name "*.rs"    # *.rs passed verbatim to find — find does its own matching
ls "*.toml"              # error: no such file — correct bash behaviour
ls *.toml                # expanded to: ls a.toml b.toml c.toml
```

---

## Drawbacks

- **Adds `glob` crate dependency** (or manual implementation). The `glob` crate
  is well-maintained and tiny.
- **Expansion happens at execution time** against the executor's `cwd`, which
  may not match the parser's compile-time location. This is correct for
  interactive shells but means script semantics depend on the working directory
  at runtime — same as all shells.
- **`**` (recursive) glob requires care** — deeply nested trees can be slow.
  The executor should impose a configurable depth limit (related to
  `#!forge:max-depth`).

---

## Alternatives Considered

### Alternative A — Always expand strings containing `*`

**Approach:** In the executor, if any `Value::Str` argument contains `*`,
expand it as a glob.

**Rejected because:** Breaks passing literal `*` to commands like
`grep "a*b" file.txt`. Quoting must suppress expansion, so the parser must
mark which values are globs.

### Alternative B — Explicit `glob()` function

**Approach:** `ls(glob("*.rs"))` — a ForgeScript builtin that returns a list.

**Rejected because:** No migration benefit; completely unlike any shell syntax.

---

## Unresolved Questions

- [ ] Should unmatched globs be an error (zsh default) or passed verbatim (bash
      default)? Proposed: bash default (verbatim). Add `#!forge:glob-errors`
      directive in a future RFC if needed.
- [ ] Depth limit for `**` patterns? Proposed: configurable via a future
      `#!forge:max-glob-depth` directive, default unlimited.
- [ ] Interaction with RFC-019 `$(...)`: `$(ls *.rs)` should expand the glob
      before running `ls`. This follows naturally from expansion happening at
      the executor level before any command runs.

---

## Implementation Plan

### Affected Crates

- `forge-lexer` — no change (reuses existing `Star`, `Dot`, `Ident` tokens)
- `forge-ast` — add `Literal::Glob`
- `forge-parser` — add `is_glob_start`, `parse_glob_arg`, update `parse_shell_args`
- `forge-hir` — add `HirLiteral::Glob`, lower `Literal::Glob`
- `forge-backend` — add `Value::Glob`, pass through in `lower_literal`
- `forge-exec` — add `expand_args`, `expand_glob`; add `glob` crate dependency

### Dependencies

- Requires RFC-017 (shell-style invocation) — **already landed**.
- RFC-018 (path literals) recommended first for `src/*.rs` style paths to work.
- RFC-020 can be partially implemented without RFC-018 (`*.rs` in cwd works
  without path support; `src/*.rs` requires RFC-018 path assembly).

### Milestones

1. AST: `Literal::Glob`; HIR: `HirLiteral::Glob`; Backend: `Value::Glob`
2. Parser: `is_glob_start`, `parse_glob_arg`
3. Executor: `expand_args`, `expand_glob` (with `glob` crate)
4. Tests + fixtures

---

## References

- [RFC-017 — Shell-Style Invocation](./rfc-017-shell-style-invocation.md)
- [RFC-018 — Path Literals](./rfc-018-path-literals.md)
- [glob crate](https://crates.io/crates/glob)
- POSIX Shell: Pathname Expansion §2.6.6
