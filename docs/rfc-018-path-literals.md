# RFC-018 — Path Literals

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

Extend the ForgeShell parser so that path-like token sequences — `/tmp`,
`./src`, `../lib`, `~/projects/forge` — are recognised as path string arguments
in shell-style invocations and string expressions. A path starts with one of
four anchors (`/`, `./`, `../`, `~/`) and continues until whitespace or a
statement-end token. Users can then write `ls /tmp`, `cd ~/projects`, and
`find ./src -name "*.rs"` without quoting paths.

---

## Motivation

`ls /tmp`, `cd /home/user`, `git add .`, `find ./src -name "*.rs"` — these are
not exotic syntax. They are the muscle memory of every Unix shell user. Forcing
`ls("/tmp")` and `cd("./src")` is a 100% rewrite rate for every path argument
in every migrated script. This is the largest single source of friction after
RFC-017 lands.

The lexer currently emits `Slash` for `/`, `Dot` for `.`, `DotDot` for `..`,
and crashes with `UnexpectedChar` on `~`. None of these produce usable path
tokens.

---

## Design

### Approach: Parser-Level Path Assembly

The lexer has no previous-token tracking and cannot distinguish `/` (division)
from `/tmp` (path start) without context. The parser can. Path assembly happens
in `parse_shell_args` (RFC-017) where context is unambiguous: every token in
shell-arg position is an argument, not an expression.

### Tilde Token

Add `TokenKind::Tilde` to the lexer for `~`. Currently `~` causes
`LexError::UnexpectedChar`.

**File:** `crates/forge-lexer/src/lexer.rs`
**Change:** Add `'~' => TokenKind::Tilde` in `lex_symbol`.

**File:** `crates/forge-lexer/src/lib.rs`
**Change:** Add `Tilde` variant to `TokenKind`.

### Path Anchors

A path starts with exactly one of four anchor token sequences:

| Anchor      | Token sequence       | Example       |
|-------------|----------------------|---------------|
| Absolute    | `Slash`              | `/tmp`        |
| Home-rel    | `Tilde Slash`        | `~/projects`  |
| Current-dir | `Dot Slash`          | `./src`       |
| Parent-dir  | `DotDot Slash`       | `../lib`      |

After the anchor, a path continues consuming: `Ident`, `Dot`, `DotDot`,
`Slash`, `Minus`, `Integer`, and `Star` (for glob components, see RFC-020)
tokens until it hits a token that cannot be part of a path (newline, semicolon,
EOF, `|`, `>`, `<`, `(`, `)`, `}`, `,`, string-start).

### Path Assembly in the Parser

**File:** `crates/forge-parser/src/parser.rs`

In `parse_shell_args`, before the `Ident` and `Minus` match arms, add a
`is_path_start` check:

```rust
fn is_path_start(&self) -> bool {
    match self.peek_kind() {
        TokenKind::Slash => true,        // /abs
        TokenKind::Tilde => matches!(self.peek_kind_at(1), TokenKind::Slash), // ~/
        TokenKind::Dot   => matches!(self.peek_kind_at(1), TokenKind::Slash), // ./
        TokenKind::DotDot => matches!(self.peek_kind_at(1), TokenKind::Slash), // ../
        _ => false,
    }
}

fn parse_path_arg(&mut self) -> Result<Arg, ParseError> {
    let mut path = String::new();
    // Consume the anchor
    match self.peek_kind().clone() {
        TokenKind::Slash => { self.advance(); path.push('/'); }
        TokenKind::Tilde => { self.advance(); self.advance(); path.push_str("~/"); }
        TokenKind::Dot   => { self.advance(); self.advance(); path.push_str("./"); }
        TokenKind::DotDot => { self.advance(); self.advance(); path.push_str("../"); }
        _ => unreachable!(),
    }
    // Consume path components
    loop {
        match self.peek_kind().clone() {
            TokenKind::Ident(s) => { self.advance(); path.push_str(&s); }
            TokenKind::Integer(n) => { self.advance(); path.push_str(&n.to_string()); }
            TokenKind::Dot => { self.advance(); path.push('.'); }
            TokenKind::DotDot => { self.advance(); path.push_str(".."); }
            TokenKind::Slash => { self.advance(); path.push('/'); }
            TokenKind::Minus => {
                // `-` inside a path component (e.g. `my-dir`)
                // only consume if next is Ident (not a flag)
                if matches!(self.peek_kind_at(1), TokenKind::Ident(_)) {
                    self.advance(); path.push('-');
                } else { break; }
            }
            _ => break,
        }
    }
    Ok(Arg::Positional(Expr::Literal(Literal::Str(path))))
}
```

In `parse_shell_args`, add before the `Ident` arm:

```rust
_ if self.is_path_start() => self.parse_path_arg()?,
```

### `~` Expansion

`~/projects` is assembled as the literal string `"~/projects"`. At execution
time, the executor (or the Unix backend's `expand_path`) expands `~` to
`$HOME`. The backend already implements `expand_path` with tilde support
(`unix.rs`), so no additional executor changes are needed beyond passing the
path string through.

### Examples After Implementation

```forge
ls /tmp
cd ~/projects/forge
find ./src -name "*.rs"
cat /etc/hosts
cp /tmp/file.txt ./output.txt
git add ../README.md
```

---

## Drawbacks

- **Path tokens are only assembled in shell-arg position.** Inside expressions
  (`let p = /tmp`) paths still require quoting. This is intentional — `/` is
  unambiguously division in expression context.
- **Consecutive `/` in expressions is still division.** `a / b` is arithmetic.
  Only the unary path anchor form (`/` at the start of a shell arg) assembles
  a path.

---

## Alternatives Considered

### Alternative A — Lexer-level `PathLit` token

**Approach:** Give the lexer a prev-token field and emit `PathLit` when `/`
follows a non-expression token.

**Rejected because:** Adds stateful complexity to a currently pure character
scanner. The parser already has context; using it is cleaner and keeps the
lexer simple.

### Alternative B — Always-quoted paths

**Approach:** Document that paths must always be quoted: `ls "/tmp"`.

**Rejected because:** This is exactly the muscle-memory problem we're solving.
`ls /tmp` must work.

---

## Unresolved Questions

- [ ] Should `/` inside interpolated strings be treated as a path separator?
      (`"path: /tmp/{name}"` — currently works as a string, no change needed.)
- [ ] Should path glob components (`/tmp/*.rs`) be handled here or deferred to
      RFC-020? **Deferred to RFC-020.**

---

## Implementation Plan

### Affected Crates

- `forge-lexer` — add `Tilde` token, fix `~` crash
- `forge-parser` — add `is_path_start`, `parse_path_arg`, update `parse_shell_args`

### Dependencies

- Requires RFC-017 (shell-style invocation) — **already landed**.

### Milestones

1. `forge-lexer`: add `Tilde` variant to `TokenKind`; emit it for `~`
2. `forge-parser`: add `is_path_start()` + `parse_path_arg()` methods
3. `forge-parser`: call `parse_path_arg` from `parse_shell_args`
4. Add fixture `14_path_args.fgs` + expected output
5. Update integration test list

---

## References

- [RFC-017 — Shell-Style Invocation](./rfc-017-shell-style-invocation.md)
- [RFC-020 — Glob Expansion](./rfc-020-glob-expansion.md)
