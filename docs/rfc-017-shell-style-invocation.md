# RFC-017 — Shell-Style Command Invocation

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

Allow built-in and external commands to be invoked at statement level using
shell-style bare syntax — `cmd arg -flag "str"` — in addition to the existing
function-call form `cmd(arg, -flag, "str")`. The two forms are fully
interchangeable at statement level; function-call syntax remains the only form
inside expressions (let bindings, if conditions, etc.).

---

## Motivation

Every user migrating from bash, zsh, fish, or PowerShell arrives with thousands
of lines of `git status`, `ls -la`, `cargo build --release`. The current
requirement to parenthesise every call (`git("status")`, `ls(-la)`) introduces
a mandatory rewrite step that is purely syntactic, adds no safety, and creates
a "why bother?" first impression.

ForgeScript is a typed language — but that typing applies to variables and
expressions, not to command invocations, which are inherently untyped
(stdin/stdout/exit-code). There is no loss of safety in accepting shell-style
at statement level where the semantics are unambiguous.

Without this feature, migrations from other shells require touching every
command line. With it, a large fraction of shell scripts "just work" with
minimal edits.

---

## Design

### Scope: Statement-Level Only

Shell-style is **only** parsed at the top level of a statement. Inside
expressions — let bindings, if conditions, function arguments — function-call
syntax is required. This prevents ambiguity:

```forge
# Statement level — shell-style OK
git commit -m "fix typo"
ls -la | grep ".rs"

# Expression level — function-call required
let files = ls("-la")
if grep("-q", "TODO", "src/main.rs") { echo("found TODO") }
```

### Trigger Rule

At statement level, when the current token is an `Ident` and the next token is
one of the following **shell-arg-start** tokens, the parser switches to
shell-style invocation mode:

| Next token              | Meaning                     |
|-------------------------|-----------------------------|
| `StringLit`             | String argument             |
| `InterpolatedStr`       | Interpolated string         |
| `Integer`               | Numeric argument            |
| `Float`                 | Float argument              |
| `Bool`                  | Boolean argument            |
| `EnvVar` (`$X`)         | Environment variable        |
| `Ident`                 | Bareword string             |
| `Minus` + `Ident`       | Short flag (`-f`)           |
| `Minus Minus` + `Ident` | Long flag (`--flag`)        |

The ident followed by `(` (function call), `=` (assignment), or any binary
operator not covered above is **not** shell-style.

### Argument Types

| Syntax        | Parsed as                       | Example              |
|---------------|---------------------------------|----------------------|
| `"string"`    | String literal                  | `grep "foo"`         |
| `-flag`       | String `"-flag"`                | `ls -la`             |
| `--flag`      | String `"--flag"`               | `cargo --release`    |
| `$VAR`        | EnvVar reference                | `echo $HOME`         |
| `bareword`    | String literal (ident → string) | `git status`         |
| `42` / `3.14` | Numeric literal                 | `sleep 1`            |

### Pipe Chaining

Shell-style invocations compose with `|`:

```forge
ls -la | grep ".rs" | sort
find "." -name "*.rs" | wc -l
```

After parsing the left-side shell invocation, the statement parser continues
to consume `|` + right-side (also shell-style if applicable) until
end-of-statement.

### What Requires Quoting

Absolute and relative paths containing `/` must be quoted (the lexer emits `/`
as the `Slash` (division) token, not as part of a bare string):

```forge
cd "/tmp"         # OK
ls "/home/user"   # OK
ls /home/user     # NOT OK — / is the division token
```

Globs (`*.rs`) must be quoted. These can be addressed by a future
RFC-018 — Glob Expansion.

### Disambiguation Examples

| Input                    | Parsed as                              |
|--------------------------|----------------------------------------|
| `ls -la`                 | `ls("-la")`                            |
| `git status`             | `git("status")`                        |
| `git commit -m "x"`      | `git("commit", "-m", "x")`            |
| `ls \| grep "foo"`       | `Pipe(ls(), grep("foo"))`              |
| `n = n - 1`              | assignment (`-` is binary Sub)         |
| `double(-n)`             | function call, `-n` is negation        |
| `ls(-l)`                 | function-call form (unchanged)         |
| `cargo build --release`  | `cargo("build", "--release")`          |

The key invariant: shell-style only fires at **statement level** when the first
non-ident token is a shell-arg-start. Inside expressions, the expression
parser handles everything.

---

## Drawbacks

- **Absolute paths need quoting.** `ls /tmp` won't work; `ls "/tmp"` will.
  This is a minor ergonomic gap that can be closed later with path-token
  support in the lexer.
- **Bareword ambiguity.** `x y` at statement level now means "call x with
  bareword y", not two separate statements. Two-statement-per-line syntax was
  already a parse error, so no existing valid code breaks.
- **Statement-level only.** `let result = git status` doesn't work; users need
  `let result = git("status")`. This is intentional — expression contexts need
  unambiguous syntax.

---

## Alternatives Considered

### Alternative A — Always-on shell-style (everywhere)

**Approach:** Apply shell-style parsing in all expression positions, not just
statements.

**Rejected because:** `let y = x - z` would be ambiguous (`x minus z` vs
`call x with flag -z`). Keeping shell-style statement-only eliminates the
ambiguity entirely.

### Alternative B — Shell mode / script mode toggle

**Approach:** A `#!forge:mode = "shell"` directive switches the entire file
into shell-style mode.

**Rejected because:** Adds complexity, fragments the language, and makes
library code harder to reason about. A single consistent grammar is preferable.

### Alternative C — Require explicit `$()` for command invocation

**Approach:** `$(git status)` syntax for shell-style, function calls for
everything else.

**Rejected because:** It's a novel syntax that doesn't help bash migrants at
all — bash uses `$()` for capture, not invocation.

---

## Unresolved Questions

- [ ] Should `ls /tmp` work? Requires lexer to recognise `/`-prefixed tokens
      as path literals in command position. Defer to RFC-018.
- [ ] Should bare `ls` (no args, no parens) invoke the command or reference
      the variable? Currently prints `null`. Could be fixed by recognising
      known command names at statement level. Deferred.
- [ ] Glob expansion: `find "." -name "*.rs"` works (quoted). `find . -name *.rs`
      would need glob token support. Defer to RFC-018.

---

## Implementation Plan

### Affected Crates

- `forge-parser` — primary change: shell-style statement dispatch
- `forge-hir` — remove `is_declared` guard for call callee names

### Dependencies

- No dependency on other RFCs.

### Milestones

1. Remove `is_declared` callee guard in HIR lowerer (`forge-hir/src/lower.rs`)
2. Add `is_shell_invocation`, `parse_shell_args` to parser
3. Update `parse_statement` to dispatch shell-style + pipe chains
4. Add fixture `13_shell_invocation.fgs` + generated expected output

---

## References

- [RFC-001 — ForgeScript Syntax](./rfc-001-forgescript-syntax.md)
- [RFC-002 — Evaluation Pipeline](./rfc-002-evaluation-pipeline.md)
- [RFC-011 — forge-migrate](./rfc-011-forge-migrate.md)
