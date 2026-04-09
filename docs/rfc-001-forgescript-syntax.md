# RFC-001 — ForgeScript Language Syntax & Type System

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

ForgeScript (`.fgs`) is the native scripting language of Forge Shell. It is a
statically-typed, expression-oriented language designed for cross-platform
shell scripting. This RFC defines the core syntax, primitive types, composite
types, control flow, functions, error handling, and the module system.
ForgeScript is not POSIX sh. It makes no attempt at POSIX compatibility.

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

### 2. Comments

```forge
# Single-line comment

#~
  Multi-line comment.
  Useful for script headers and documentation blocks.
~#
```

---

### 3. Variables & Binding

Variables are immutable by default. Mutability is opt-in and explicit.

```forge
# Immutable binding
let name = "forge"

# Mutable binding
let mut count = 0
count = count + 1

# Type is inferred but can be annotated explicitly
let port: int = 8080
let debug: bool = false
```

Variable names use `snake_case`. Constants use `SCREAMING_SNAKE_CASE`.

```forge
const MAX_RETRIES = 3
```

---

### 4. Primitive Types

| Type     | Description                        | Example              |
|----------|------------------------------------|----------------------|
| `string` | UTF-8 string                       | `"hello"`            |
| `int`    | 64-bit signed integer              | `42`, `-7`           |
| `float`  | 64-bit IEEE 754 float              | `3.14`               |
| `bool`   | Boolean                            | `true`, `false`      |
| `path`   | First-class cross-platform path    | `path("/usr/bin")`   |
| `unit`   | The empty type (no meaningful value)| implicit             |

#### String Literals

```forge
let plain   = "hello world"
let interp  = "Hello, {name}!"          # string interpolation
let raw     = r"no \n escape here"      # raw string
let multi   = """
  multi-line
  string literal
"""
```

#### Path Type

`path` is a first-class type — not a string alias. Paths use `/` as the
separator in source code on all platforms. Resolution to the OS-native
separator happens only at the OS boundary.

```forge
let config  = path("/etc/forge/config.fgs")    # absolute
let relative = path("./scripts/deploy.fgs")    # relative

# Path arithmetic via / operator
let home_cfg = home() / ".config" / "forge"

# Path methods
config.exists()          # bool
config.parent()          # path
config.file_name()       # string
config.extension()       # Option<string>
config.to_string()       # platform-native string representation
```

String-to-path coercion is explicit — there is no implicit conversion:

```forge
let p: path = path("/usr/bin")    # ✅ explicit
let p: path = "/usr/bin"          # ❌ type error — string is not a path
```

---

### 5. Composite Types

#### Option

```forge
let token: Option<string> = $GITHUB_TOKEN?    # safe env var access

match token {
  Some(t) => print("Token: {t}")
  None    => print("No token set")
}

# Unwrap with default
let val = token.unwrap_or("default")
```

#### Result

```forge
let result: Result<string, Error> = run("git status")

match result {
  Ok(output) => print(output)
  Err(e)     => print("Failed: {e.message}")
}

# Propagate with ?
let output = run("git status")?
```

#### List

```forge
let items: [string] = ["a", "b", "c"]

items.push("d")
items.len()           # int
items[0]              # string
items.contains("a")   # bool

# Iteration
for item in items {
  print(item)
}
```

#### Map

```forge
let env: {string: string} = {
  "APP_ENV": "production",
  "PORT":    "8080",
}

env["APP_ENV"]              # string
env.get("MISSING")?         # Option<string>
env.contains_key("PORT")    # bool
```

#### Struct

```forge
struct Config {
  host:    string
  port:    int
  debug:   bool
  log_dir: path
}

let cfg = Config {
  host:    "localhost",
  port:    8080,
  debug:   false,
  log_dir: home() / ".forge" / "logs",
}

print(cfg.host)
```

#### Enum

```forge
enum Environment {
  Development
  Staging
  Production(string)    # variant with data
}

let env = Environment::Production("us-east-1")

match env {
  Environment::Development     => print("dev mode")
  Environment::Staging         => print("staging mode")
  Environment::Production(r)   => print("production: {r}")
}
```

---

### 6. Environment Variables

```forge
# Read — always string or Option<string>
let home   = $HOME              # string — panics if unset (use ? for safety)
let token  = $GITHUB_TOKEN?     # Option<string> — safe

# Set — scoped to current process and its children
set PORT = "8080"

# Export — visible to all spawned child processes
export DEBUG = "true"

# Unset
unset TEMP_VAR

# $PATH is a typed list — never a raw string
$PATH.prepend(path("/usr/local/bin"))
$PATH.append(home() / ".cargo" / "bin")
$PATH.contains(path("/usr/bin"))    # bool
```

---

### 7. Control Flow

#### if / else

```forge
if count > 0 {
  print("positive")
} else if count == 0 {
  print("zero")
} else {
  print("negative")
}

# if as expression
let label = if debug { "debug" } else { "release" }
```

#### match

```forge
match status_code {
  200       => print("OK")
  404       => print("Not Found")
  500..=599 => print("Server Error")
  _         => print("Unknown: {status_code}")
}
```

#### for

```forge
# Iterate over a list
for item in items {
  print(item)
}

# Range
for i in 0..10 {
  print(i)
}

# Inclusive range
for i in 1..=5 {
  print(i)
}

# With index
for (i, item) in items.enumerate() {
  print("{i}: {item}")
}
```

#### while

```forge
let mut attempts = 0

while attempts < MAX_RETRIES {
  let result = run("curl https://api.forge-shell.dev")?
  attempts = attempts + 1
}
```

#### loop / break

```forge
loop {
  let line = read_line()?
  if line == "quit" {
    break
  }
  print(line)
}

# loop as expression — break with value
let result = loop {
  let val = compute()?
  if val > 100 {
    break val
  }
}
```

---

### 8. Functions

```forge
# Basic function
fn greet(name: string) -> string {
  "Hello, {name}!"    # last expression is return value
}

# Explicit return
fn divide(a: float, b: float) -> Result<float, string> {
  if b == 0.0 {
    return Err("division by zero")
  }
  Ok(a / b)
}

# No return value
fn log(msg: string) {
  print("[forge] {msg}")
}

# Default arguments
fn connect(host: string, port: int = 8080) -> Result<unit, Error> {
  # ...
}

# Variadic arguments
fn print_all(args: ...string) {
  for arg in args {
    print(arg)
  }
}
```

#### Closures

```forge
let double = |x: int| -> int { x * 2 }

let items = [1, 2, 3, 4, 5]
let doubled = items.map(|x| x * 2)
let evens   = items.filter(|x| x % 2 == 0)
```

---

### 9. Error Handling

Errors are values. There are no exceptions.

```forge
# ? operator — propagate error to caller
fn deploy(env: string) -> Result<unit, Error> {
  let status = run("kubectl apply -f manifest.yaml")?
  let output = run("kubectl rollout status deployment/app")?
  Ok(())
}

# Handle inline
fn safe_deploy(env: string) {
  match deploy(env) {
    Ok(_)  => print("Deployed successfully")
    Err(e) => {
      print("Deploy failed: {e.message} (code: {e.code})")
      exit(1)
    }
  }
}
```

#### Error Type

```forge
struct Error {
  message: string
  code:    int
  cause:   Option<Error>
}
```

Custom error types are defined as enums:

```forge
enum DeployError {
  ManifestNotFound(path)
  KubectlFailed(int)
  Timeout(int)
}
```

---

### 10. Running Commands

```forge
# Run a command — returns Result<Output, Error>
let result = run("git status")

# Run with arguments as a list — preferred for dynamic args
let result = run(["git", "commit", "-m", message])

# Capture stdout
let output = run("git log --oneline")?.stdout

# Pipe commands
let result = run("git log --oneline") | run("grep feat")

# Background execution
let job = spawn("long-running-task")
job.wait()?

# Inherit stdio (output goes to terminal, not captured)
exec("vim {file}")
```

---

### 11. Modules & Imports

```forge
# Import from another .fgs file
import "./lib/deploy.fgs" as deploy
import "./lib/utils.fgs"  as utils

deploy::run_all()
utils::log("done")

# Import specific symbols
from "./lib/config.fgs" import { load_config, Config }

let cfg = load_config()?
```

Standard library modules are imported without a path:

```forge
import forge::fs
import forge::net
import forge::process
import forge::env
```

---

### 12. Script Metadata

Scripts can declare metadata at the top of the file:

```forge
#! forge
#! version  = "1.0.0"
#! description = "Deploy script for the production environment"
#! author   = "Ajitem Sahasrabuddhe"

# Script body begins here
```

---

### 13. Reserved Keywords

```
let mut const fn struct enum match if else for while loop
break continue return import from as in Ok Err Some None
true false set export unset spawn exec path
```

---

## Drawbacks

- **Not POSIX compatible** — existing shell scripts cannot be run as `.fgs`
  files. Migration requires rewriting. This is intentional but is a real
  adoption friction point.
- **Type system adds verbosity** — simple one-liner scripts require more
  ceremony than equivalent bash.
- **Typed PATH is unfamiliar** — developers accustomed to manipulating `$PATH`
  as a string will need to adjust.
- **Learning curve** — the `Result`/`Option` model is familiar to Rust and Go
  developers but may be new to shell scripters.

---

## Alternatives Considered

### Alternative A — POSIX-compatible syntax with extensions

**Approach:** Start from sh syntax and add types and cross-platform features
on top.
**Rejected because:** POSIX syntax carries deep Unix assumptions (string-only
variables, exit-code-as-error, `fork`-centric process model). Extending it
cleanly is effectively impossible — every new feature fights the existing model.

### Alternative B — Python-like syntax

**Approach:** Use Python's indentation-based syntax as the foundation.
**Rejected because:** Indentation-sensitive parsing is fragile in a shell
context where copy-paste, here-docs, and pipe continuations are common. Also
creates confusion — ForgeScript is not Python.

### Alternative C — Nushell-compatible syntax

**Approach:** Adopt Nushell's syntax for familiarity.
**Rejected because:** Forge Shell is a distinct project with a distinct
identity. Nushell compatibility would constrain design decisions and create
user confusion about which project to use.

---

## Unresolved Questions

- [ ] Should `path` literals have a dedicated syntax (e.g. `p"/usr/bin"`) or
      always use the `path()` constructor?
- [ ] Should string interpolation use `{var}` or `${var}`? The latter
      conflicts with environment variable syntax.
- [ ] How are circular imports detected and reported?
- [ ] Should the standard library be built-in or distributed as first-party
      plugins?
- [ ] What is the policy for integer overflow — panic, wrap, or saturate?
- [ ] Should ForgeScript support async/await for long-running operations?

---

## Implementation Plan

### Affected Crates

- `forge-lang/lexer` — tokeniser for all syntax defined here
- `forge-lang/parser` — recursive descent parser producing AST nodes
- `forge-lang/ast` — AST node type definitions
- `forge-lang/hir` — type resolution, name resolution, HIR lowering

### Dependencies

- No RFC dependencies — this is the foundational RFC.
- RFC-002 (Evaluation Pipeline) depends on this RFC being accepted first.

### Milestones

1. Lexer: tokenise all literal types, operators, keywords
2. Parser: expressions, let bindings, control flow
3. Parser: functions, structs, enums
4. Parser: imports and module declarations
5. AST: complete node definitions for all syntax above
6. HIR: type inference for primitives and composites
7. HIR: name resolution for variables, functions, modules
8. Integration tests for all syntax forms on all three platforms

---

## References

- [Nushell Language Design](https://www.nushell.sh/book/types_of_data.html)
- [Rust Reference — Expressions](https://doc.rust-lang.org/reference/expressions.html)
- [Fish Shell Scripting](https://fishshell.com/docs/current/language.html)
- [Elvish Shell Language](https://elv.sh/ref/language.html)
- [PowerShell Language Specification](https://learn.microsoft.com/en-us/powershell/scripting/lang-spec/chapter-01)