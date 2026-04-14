# RFC-001 — ForgeScript Language Syntax & Type System

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

ForgeScript (`.fgs`) is the native scripting language of Forge Shell. It is a
statically-typed, expression-oriented language designed for cross-platform
shell scripting. This RFC defines the core syntax, primitive types, composite
types, control flow, functions, error handling, the module system, and
concurrency primitives. ForgeScript is not POSIX sh. It makes no attempt at
POSIX compatibility.

---

## Motivation

Every existing shell scripting language carries decades of Unix assumptions
baked into its syntax. Variables are untyped strings. Paths are strings.
Errors are exit codes. These assumptions make cross-platform scripting
fragile — a single `/` vs `\` mismatch, a missing `.exe`, or an inconsistent
`echo` flag silently breaks a script on a different OS.

ForgeScript is designed from first principles with three goals:

1. **Cross-platform by construction** — platform traps are type errors or
   explicit warnings, not silent runtime failures.
2. **Readable at a glance** — a `.fgs` script should be understandable to any
   developer, not just shell experts.
3. **Safe by default** — errors are values, not ignored exit codes.

---

## Design

### 1. File Format

- Extension: `.fgs`
- Encoding: UTF-8. BOM is stripped silently. UTF-16 is rejected with a clear error.
- Line endings: LF. CRLF is normalised by the lexer before tokenisation.
- Committed with LF enforced via `.gitattributes`.

---

### 2. Script Header & Metadata

Every ForgeScript file may begin with an optional metadata block. Line 1 is
the standard OS shebang. Subsequent `#!forge:` lines are ForgeScript
directives, parsed by the `forge` runtime on all platforms.

```forge
#!/usr/bin/env forge
#!forge:version     = "1.0.0"
#!forge:description = "Deploy script for the production environment"
#!forge:author      = "Ajitem Sahasrabuddhe"
#!forge:overflow    = "saturate"

# Script body begins here
```

**Rules:**
- Line 1: real OS shebang — interpreted by the kernel on Linux/macOS,
  ignored on Windows.
- `#!forge:key = "value"` lines: ForgeScript runtime directives. Parsed by
  `forge` regardless of platform. Unknown keys are a compile-time warning.
- On Windows, scripts are invoked via `forge run script.fgs` or file
  association. The `#!forge:` metadata block is parsed identically.

**Supported `#!forge:` directives (v1):**

| Directive | Values | Default |
|---|---|---|
| `version` | semver string | — |
| `description` | string | — |
| `author` | string | — |
| `overflow` | `"panic"` \| `"saturate"` \| `"wrap"` | `"panic"` |

---

### 3. Comments

```forge
# Single-line comment

#~
  Multi-line comment.
  Useful for script headers and documentation blocks.
~#
```

---

### 4. Literal Types (v1)

ForgeScript uses a **prefix literal system**. Typed literals carry semantic
meaning beyond raw strings and are validated at parse time, before the script
runs.

#### Primitive Literals

| Literal | Type | Example | Static Validation |
|---|---|---|---|
| `"..."` | `str` | `"hello world"` | UTF-8 validity |
| `p"..."` | `path` | `p"/etc/forge/config.toml"` | Reserved names, invalid chars, separator sanity |
| `r"..."` | `regex` | `r"^\d{4}-\d{2}-\d{2}$"` | Valid regex syntax |
| `u"..."` | `url` | `u"https://forge-shell.dev"` | Valid scheme, well-formed structure |
| `42` | `int` | `42`, `-7`, `0xFF`, `0o77`, `0b1010` | i64 range, base prefix validity |
| `3.14` | `float` | `3.14`, `-0.5` | Valid f64 |
| `true` / `false` | `bool` | `true`, `false` | — |

**Integer conventions:**
- Default integer type: `i64` — following Rust/Go convention.
- Supported base prefixes: `0x` (hexadecimal), `0o` (octal), `0b` (binary).
- Numeric separators: `1_000_000` is valid.

**Path literal notes:**
- `p"..."` literals are statically validated — reserved Windows filenames
  (`CON`, `NUL`, `PRN`, etc.) and invalid characters are rejected at parse
  time.
- Runtime path composition uses the path joining operator `/`, not string
  interpolation:

```forge
let base = p"/home/user"
let full = base / "projects" / name   # path::join — typed and validated
```

**Deferred to post-v1:** `b"..."` (bytes), `d"..."` (duration), `v"..."` (semver), `t"..."` (template).

---

### 5. String Interpolation

String interpolation uses clean brace syntax. Full expressions are supported
inside interpolation delimiters.

```forge
let name = "Ajitem"
let count = 5

echo "Hello {name}, you have {count} messages."
echo "Result is {a + b}"
echo "Items: {list.len()}"
```

**Rules:**
- Delimiter: `{expr}` — no dollar sign. Avoids conflict with environment
  variable syntax.
- Full expressions supported inside `{}` — not just variable names.
- Literal brace characters are escaped by doubling: `{{` and `}}`.
- Applies to `str` literals only. `p"..."`, `r"..."`, and `u"..."` do **not**
  support interpolation — they are statically validated at parse time.

```forge
echo "Set notation: {{1, 2, 3}}"   # prints: Set notation: {1, 2, 3}
```

---

### 6. Variables & Binding

Variables are immutable by default. Mutability is opt-in and explicit.

```forge
let name = "forge"           # immutable
let mut counter = 0          # mutable
const MAX_RETRIES = 3        # compile-time constant
```

---

### 7. Integer Overflow Semantics

**Default behaviour:** Panic on integer overflow. Overflow in a scripting
context is almost always a bug. Silent overflow corrupts values and allows
scripts to continue with wrong data.

```forge
let x = i64::MAX
let y = x + 1    # error: integer overflow — script halts
```

**Opt-in alternatives** — in priority order:

| Mechanism | Syntax | Granularity |
|---|---|---|
| Per-expression operators | `a +| b` (saturate), `a +% b` (wrap) | Per-operation |
| CLI flag | `forge run --overflow=saturate script.fgs` | Per-invocation |
| Script metadata directive | `#!forge:overflow = "saturate"` | Per-script |
| Default | panic | Always, unless overridden |

**Per-expression operators:**

```forge
let a = i64::MAX
let b = a +| 1    # saturate: b == i64::MAX
let c = a +% 1    # wrap:     c == i64::MIN
```

Supported variants: `+|`, `-|`, `*|` (saturating); `+%`, `-%`, `*%` (wrapping).

---

### 8. Control Flow

```forge
# if / else
if count > 0 {
    echo "non-zero"
} else {
    echo "zero"
}

# match
match status {
    Ok(value) => echo "got {value}"
    Err(e)    => echo "error: {e}"
}

# for loop
for file in ls(p"/etc/forge") {
    echo "{file}"
}

# while loop
let mut i = 0
while i < 10 {
    i = i + 1
}

# loop with break
loop {
    let line = read_line()
    if line == "" { break }
    echo "{line}"
}
```

---

### 9. Functions

```forge
fn greet(name: str) -> str {
    "Hello, {name}!"
}

fn divide(a: int, b: int) -> Result<int, str> {
    if b == 0 {
        Err("division by zero")
    } else {
        Ok(a / b)
    }
}
```

---

### 10. Error Handling

Errors are values. There are no exceptions. The `Result` and `Option` types
are the error handling primitives.

```forge
let result = divide(10, 0)

match result {
    Ok(value) => echo "result: {value}"
    Err(e)    => echo "failed: {e}"
}

# Propagation operator
fn run() -> Result<(), str> {
    let value = divide(10, 2)?   # propagates Err automatically
    echo "value: {value}"
    Ok(())
}
```

---

### 11. Concurrency

ForgeScript uses a **Go-inspired structured concurrency model**. Any block can
be spawned — there is no `async` keyword propagation and no function colouring
problem.

#### `spawn` — concurrent task

```forge
let handle = spawn {
    ls p"/home/user"
}

let result = handle.await()
```

#### Fire and forget

```forge
spawn {
    upload_logs p"/var/log/forge"
}
# continues immediately
```

#### `join!` — parallel coordination

Replaces bash's `cmd & wait` pattern with structured, typed coordination:

```forge
let (build, lint, test) = join! {
    spawn { run("cargo build") },
    spawn { run("cargo clippy") },
    spawn { run("cargo test") }
}
```

#### `Context` — cancellation and timeouts

Directly modelled on Go's `context` package:

```forge
# Timeout-bounded execution
let ctx = Context::with_timeout(d"30s")

let result = spawn(ctx) {
    fetch(u"https://api.forge-shell.dev/status")
}

match result {
    Ok(data) => echo "{data}"
    Err(e)   => echo "failed or timed out: {e}"
}

# Manual cancellation
let (ctx, cancel) = Context::with_cancel()

spawn(ctx) {
    long_running_task()
}

cancel()   # cancel the task explicitly
```

**v1 concurrency primitives:**

| Primitive | Purpose |
|---|---|
| `spawn { }` | Launch concurrent task, returns `Task<T>` |
| `spawn { }` (no handle) | Fire-and-forget background task |
| `handle.await()` | Block until single task completes |
| `join! { }` | Run multiple tasks concurrently, wait for all |
| `Context::with_timeout(d)` | Deadline-bounded execution |
| `Context::with_cancel()` | Manual cancellation |

**Deferred to post-v1:** channels, `select!`, pipeline concurrency, task priorities.

---

### 12. Structs & Enums

```forge
struct Config {
    host: str,
    port: int,
    debug: bool,
}

enum Status {
    Ok,
    Failed(str),
    Pending { retries: int },
}
```

---

### 13. Modules & Imports

```forge
import forge::fs
import forge::env
import ./utils
import ./utils::{ read_config, write_output }
import ./utils as u
```

**Circular import policy:**
- Detected at compile time via DFS topological sort on the import graph.
- Circular imports are a **hard error** — not a warning, not a runtime panic.
- The full cycle path is shown in the error output:

```
error[E001]: circular import detected
  --> a.fgs:1:8
   |
 1 | import b
   |        ^ imported here
   |
   = cycle: a.fgs → b.fgs → c.fgs → a.fgs
   = help: consider extracting shared logic into a new module
```

- Implementation home: `forge-lang/resolver` crate.

---

### 14. Standard Library

The ForgeScript standard library is **compiled directly into the `forge`
binary**. It is not distributed as plugins and has no install step.

**Rationale:**
- Stdlib is infrastructure — it must be available unconditionally, without
  network access or registry availability.
- The WASM plugin sandbox is the correct trust boundary for third-party code,
  not for stdlib. `forge::fs` requires raw OS access; it cannot run sandboxed.
- Distributing stdlib as plugins creates a bootstrap problem — the registry
  itself depends on stdlib modules.

**v1 stdlib modules:**

| Module | Responsibility |
|---|---|
| `forge::fs` | File system operations |
| `forge::str` | String manipulation |
| `forge::env` | Environment variable access |
| `forge::json` | JSON parsing and serialisation |
| `forge::time` | Time and duration |
| `forge::process` | Process spawning and management |
| `forge::test` | Testing primitives |

---

### 15. Reserved Keywords

```
let mut const fn struct enum match if else for while loop
break continue return import from as in Ok Err Some None
true false set export unset spawn exec path join context
```

---

## Drawbacks

- **Not POSIX compatible** — existing shell scripts cannot be run as `.fgs`
  files. Migration requires rewriting. This is intentional but is a real
  adoption friction point.
- **Type system adds verbosity** — simple one-liner scripts require more
  ceremony than equivalent bash.
- **Prefix literals are unfamiliar** — developers without Rust/Python
  background may need time to internalise `p"..."`, `r"..."`, `u"..."`.
- **Learning curve** — the `Result`/`Option` model and `spawn`/`join!`
  concurrency are familiar to Go/Rust developers but may be new to shell
  scripters.

---

## Alternatives Considered

### Alternative A — POSIX-compatible syntax with extensions

**Approach:** Start from sh syntax and add types and cross-platform features
on top.
**Rejected because:** POSIX syntax carries deep Unix assumptions. Extending it
cleanly is effectively impossible — every new feature fights the existing
model.

### Alternative B — `path()` constructor instead of `p"..."` literal

**Approach:** Use `path("/usr/bin")` as a function-call constructor rather
than a prefix literal.
**Rejected because:** A unified prefix literal system (`p"..."`, `r"..."`,
`u"..."`) enables consistent static validation at parse time and is
unambiguously a literal rather than a callable. The ergonomic cost of
`path("/usr/bin")` at volume is significant. The consistency risk of a
one-off constructor is also real.

### Alternative C — `${var}` string interpolation

**Approach:** Use `${var}` for interpolation, following bash convention.
**Rejected because:** `$` is environment variable territory in every major
shell. Overloading it inside strings creates a mental conflict between
ForgeScript variables and env var lookups.

### Alternative D — Full async/await

**Approach:** Adopt Rust/JavaScript-style `async fn` and `.await`.
**Rejected because:** `async` infects the entire call tree — functions must be
marked `async` to call other `async` functions (the function colouring
problem). In a scripting language, this is unnecessary friction. The
`spawn`/`join!` model allows any block to be concurrent without restructuring
the call tree.

---

## Unresolved Questions

All previously unresolved questions have been resolved. See resolution summary
below.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | `path` literal syntax | Prefix literal system: `p"..."`, `r"..."`, `u"..."` |
| UQ-2 | String interpolation syntax | `{var}` / `{expr}` with `{{` `}}` for literal braces |
| UQ-3 | Circular import handling | Compile-time hard error, full cycle path in error output |
| UQ-4 | Standard library architecture | Built-in — compiled into the `forge` binary |
| UQ-5 | Integer overflow semantics | Panic by default; opt-in via `#!forge:overflow`, CLI flag, or per-expression `+|` `+%` operators |
| UQ-6 | Async/await scope | Go-inspired `spawn`, `join!`, `Context` — no function colouring |

---

## Implementation Plan

### Affected Crates

- `forge-lang/lexer` — tokeniser for all literal types, prefix sigils, operators, keywords
- `forge-lang/parser` — recursive descent parser producing AST nodes
- `forge-lang/ast` — AST node type definitions
- `forge-lang/hir` — type resolution, name resolution, HIR lowering
- `forge-lang/resolver` — import graph construction, circular import detection

### Dependencies

- No RFC dependencies — this is the foundational RFC.
- RFC-002 (Evaluation Pipeline) depends on this RFC being accepted first.

### Milestones

1. Lexer: tokenise all literal types, prefix sigils (`p"..."`, `r"..."`, `u"..."`), operators, keywords
2. Lexer: integer base prefixes (`0x`, `0o`, `0b`), numeric separators
3. Parser: expressions, let bindings, control flow
4. Parser: functions, structs, enums
5. Parser: imports and module declarations
6. Parser: `spawn`, `join!`, `Context` concurrency syntax
7. AST: complete node definitions for all syntax above
8. HIR: type inference for primitives and composites
9. HIR: name resolution for variables, functions, modules
10. Resolver: import graph construction, DFS cycle detection, error reporting
11. Integration tests for all syntax forms on ubuntu-latest, macos-latest, windows-latest

---

## References

- [Nushell Language Design](https://www.nushell.sh/book/types_of_data.html)
- [Rust Reference — Expressions](https://doc.rust-lang.org/reference/expressions.html)
- [Fish Shell Scripting](https://fishshell.com/docs/current/language.html)
- [Elvish Shell Language](https://elv.sh/ref/language.html)
- [PowerShell Language Specification](https://learn.microsoft.com/en-us/powershell/scripting/lang-spec/chapter-01)
- [Go Context Package](https://pkg.go.dev/context)
- [Go Concurrency Patterns](https://go.dev/blog/pipelines)
- [Rust Saturating Arithmetic](https://doc.rust-lang.org/std/primitive.i64.html#method.saturating_add)
