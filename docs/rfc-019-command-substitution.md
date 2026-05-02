# RFC-019 — Command Substitution `$(...)`

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

Add `$(...)` command substitution so the stdout of a command can be captured
into a ForgeScript variable. `let result = $(git log --oneline -5)` runs
`git log --oneline -5`, captures its stdout, trims the trailing newline, and
binds the result as a `str` value. Shell-style syntax is supported inside `$()`:
`$(ls -la)`, `$(find "./src" -name "*.rs")`.

---

## Motivation

Command substitution is the primary mechanism for using command output as data
in shell scripts. Every bash/zsh script that does:

```bash
branch=$(git branch --show-current)
count=$(ls | wc -l)
version=$(cat Cargo.toml | grep '^version' | cut -d'"' -f2)
```

...must continue to work in ForgeShell with minimal changes. Without `$(...)`,
these patterns require multi-step rewrites using pipes, temp files, or
ForgeScript-specific APIs that don't exist yet.

---

## Design

### Lexer Change: `DollarParen` Token

Currently `$(` emits `Dollar` followed by `LParen` as two separate tokens.
Add a `DollarParen` token that the lexer emits when it sees `$` followed
immediately by `(`:

**File:** `crates/forge-lexer/src/lexer.rs`
**Change:** In the `'$'` match arm, add a check before the `is_ascii_alphabetic`
check:

```rust
'$' => {
    if self.peek() == Some(&'(') {
        self.advance(); // consume '('
        TokenKind::DollarParen
    } else if self.peek().is_some_and(|c| c.is_ascii_alphabetic() || *c == '_') {
        // existing EnvVar handling ...
    } else {
        TokenKind::Dollar
    }
}
```

**File:** `crates/forge-lexer/src/lib.rs`
**Change:** Add `DollarParen` variant to `TokenKind`.

### Parser Change: `Expr::CmdSubst`

**File:** `crates/forge-ast/src/expr.rs`
**Change:** Add `CmdSubst(Box<Expr>)` variant to `Expr`.

**File:** `crates/forge-parser/src/parser.rs`
**Change:** In `parse_primary`, handle `TokenKind::DollarParen`:

```rust
TokenKind::DollarParen => {
    self.advance(); // consume $(
    // Parse inner as shell-style or function-call expression
    let inner = if matches!(self.peek_kind(), TokenKind::Ident(_))
        && self.is_shell_invocation()
    {
        let name = self.expect_identifier()?;
        let args = self.parse_shell_args()?;
        Expr::Call { callee: Box::new(Expr::Ident(name)), args }
    } else {
        self.parse_expression(0)?
    };
    self.expect_kind(&TokenKind::RParen)?;
    Ok(Expr::CmdSubst(Box::new(inner)))
}
```

### HIR Change: `HirExpr::CmdSubst`

**File:** `crates/forge-hir/src/hir.rs`
**Change:** Add `CmdSubst { inner: Box<HirExpr>, span: Span }` to `HirExpr`.

**File:** `crates/forge-hir/src/lower.rs`
**Change:** In `lower_expr`, handle `Expr::CmdSubst`:

```rust
Expr::CmdSubst(inner) => {
    let hir_inner = self.lower_expr(*inner)?;
    Ok(HirExpr::CmdSubst {
        inner: Box::new(hir_inner),
        span: Span::default(),
    })
}
```

### Backend Change: `Op::CaptureOutput`

**File:** `crates/forge-backend/src/plan.rs`
**Change:** Add new `Op` variant:

```rust
/// Run a command and capture its stdout as a string value.
CaptureOutput {
    command: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    result_var: String,
},
```

**File:** `crates/forge-backend/src/lower.rs`
**Change:** Handle `HirExpr::CmdSubst` in `lower_expr_to_ops` and in
`decompose_expr_for_bind`:

For `lower_expr_to_ops`:

```rust
HirExpr::CmdSubst { inner, .. } => {
    let tmp = "__cmdsubst__".to_string();
    let inner_ops = self.lower_expr_to_ops(inner)?;
    // Replace the innermost RunProcess or CallFn with CaptureOutput
    match inner_ops.into_iter().next() {
        Some(Op::RunProcess { command, args, env, .. }) => {
            Ok(vec![Op::CaptureOutput { command, args, env, result_var: tmp }])
        }
        Some(Op::CallFn { name, args, .. }) => {
            Ok(vec![Op::CaptureOutput {
                command: name,
                args: args.iter().map(|v| Self::value_to_string_arg(v)).collect(),
                env: vec![],
                result_var: tmp,
            }])
        }
        _ => Err(BackendError::Unsupported {
            reason: "command substitution requires a simple command".to_string(),
        }),
    }
}
```

For `decompose_expr_for_bind` (so `let x = $(cmd)` works):

```rust
HirExpr::CmdSubst { .. } => {
    let tmp = format!("__cmdsubst_{dest}__");
    let mut ops = self.lower_expr_to_ops(expr)?;
    // Rename result_var in the last op to tmp
    if let Some(Op::CaptureOutput { result_var, .. }) = ops.last_mut() {
        *result_var = tmp.clone();
    }
    Ok((ops, Value::VarRef(tmp)))
}
```

### Executor Change: Handle `Op::CaptureOutput`

**File:** `crates/forge-exec/src/executor.rs`
**Change:** In `execute_op`, handle `Op::CaptureOutput`:

```rust
Op::CaptureOutput { command, args, env, result_var } => {
    let resolved_args: Vec<String> = args.iter()
        .map(|a| self.context.resolve_to_string(&Value::Str(a.clone())))
        .collect();

    let output = if self.registry.is_builtin(command) {
        // v1: run the builtin normally and capture via thread-local buffer
        // full implementation uses a write-capturing context
        self.capture_builtin_output(command, &resolved_args)?
    } else {
        let out = std::process::Command::new(command)
            .args(&resolved_args)
            .current_dir(&self.context.cwd)
            .envs(&self.context.env)
            .envs(env.iter().map(|(k, v)| (k, v)))
            .output()
            .map_err(ExecError::Io)?;
        self.context.last_exit = out.status.code().unwrap_or(-1);
        String::from_utf8_lossy(&out.stdout).into_owned()
    };

    // Trim trailing newline (same as bash $())
    let trimmed = output.trim_end_matches('\n').to_string();
    self.context.set_var(result_var, Value::Str(trimmed));
}
```

### Usage Examples

```forge
let branch = $(git branch --show-current)
let count = $(ls | wc -l)
let version = $(cat "Cargo.toml" | grep "^version")

echo branch
echo count

if branch == "main" {
    echo("on main branch")
}
```

---

## Drawbacks

- **Builtin stdout capture is non-trivial.** Builtins currently write directly
  to stdout via `println!`. Capturing requires either redirecting
  `std::io::stdout` or refactoring builtins to write to a configurable `Write`
  trait object. The v1 implementation handles external commands fully and
  defers builtin capture to a follow-up.
- **Multi-line output is a single string.** `$(git log)` produces a string with
  embedded newlines. Splitting into a list is a separate feature.
- **Exit code is silently consumed.** `$(cmd)` captures stdout; if `cmd` exits
  non-zero, `last_exit` is updated but no error is raised (matching bash
  semantics). Strict mode will still fail on non-zero exit if enabled.

---

## Alternatives Considered

### Alternative A — Backtick syntax `` `cmd` ``

**Approach:** Use `` `git status` `` like older shells.

**Rejected because:** Backtick is deprecated even in bash. `$(...)` is the
modern standard and easier to nest.

### Alternative B — Explicit `capture(cmd, args...)` builtin

**Approach:** `let x = capture("git", "status")` — a ForgeScript builtin.

**Rejected because:** Doesn't help migration. Every bash migrant expects `$()`.

---

## Unresolved Questions

- [ ] Builtin stdout capture: refactor builtins to accept a `dyn Write` output
      sink vs. using thread-local stdout redirection. Decide before implementation.
- [ ] Should `$(cmd)` in strict mode propagate non-zero exit as an error?
      Current proposal: follows `last_exit` + strict mode rules. Confirm.
- [ ] Multi-line output: `$(git log)` as a `str` vs. `list<str>`? Deferred.

---

## Implementation Plan

### Affected Crates

- `forge-lexer` — add `DollarParen` token
- `forge-ast` — add `Expr::CmdSubst`
- `forge-parser` — parse `$(...)` in `parse_primary`
- `forge-hir` — add `HirExpr::CmdSubst`, lower it
- `forge-backend` — add `Op::CaptureOutput`, lower `CmdSubst`
- `forge-exec` — execute `Op::CaptureOutput`

### Dependencies

- Requires RFC-017 (shell-style invocation) — **already landed**.
- RFC-018 (path literals) not required but recommended first.

### Milestones

1. Lexer: `DollarParen` token
2. AST + Parser: `Expr::CmdSubst` + `parse_primary` arm
3. HIR: `HirExpr::CmdSubst` + lowering
4. Backend: `Op::CaptureOutput` + lowering
5. Executor: `Op::CaptureOutput` — external commands first, builtins v2
6. Tests + fixtures

---

## References

- [RFC-017 — Shell-Style Invocation](./rfc-017-shell-style-invocation.md)
- [RFC-018 — Path Literals](./rfc-018-path-literals.md)
- POSIX Shell: Command Substitution §2.6.3
