# RFC-004 — Path Type & Environment Variable Model

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

This RFC defines two foundational cross-platform abstractions in ForgeScript:
the `path` type and the environment variable model. Both are first-class
language features — not conventions or library utilities. Together they
eliminate the largest class of cross-platform scripting bugs without requiring
script authors to think about OS differences.

---

## Motivation

The two most common sources of cross-platform shell script breakage are:

1. **Path handling** — separator characters (`/` vs `\`), drive letters,
   UNC paths, reserved filenames, and case sensitivity differences.
2. **Environment variable handling** — `$VAR` vs `%VAR%` syntax, case
   sensitivity, PATH separator (`:`  vs `;`), and variable scoping.

Neither of these should require script authors to write platform-conditional
code. They are implementation details of the OS, not concerns of the script.

---

## Design

### Part 1 — The `path` Type

#### 1.1 Core Principle

`path` is a distinct type from `string`. There is no implicit coercion
between them. A string is not a path. A path is not a string.

```forge
let s: string = "/usr/bin"
let p: path   = path("/usr/bin")

let bad: path = "/usr/bin"     # ❌ type error — must be explicit
```

This single rule prevents the majority of path-related cross-platform bugs.

#### 1.2 Path Construction

```forge
# Absolute paths
let abs = path("/usr/local/bin")              # Unix-style — preferred in source
let abs = path("C:\\Program Files\\Forge")    # Windows-style — accepted, normalised

# Relative paths
let rel = path("./scripts/deploy.fgs")
let rel = path("../config")

# Home directory
let home_dir = home()                         # Returns path — not string
let config   = home() / ".config" / "forge"

# Current working directory
let cwd = pwd()                               # Returns path

# From environment variable (explicit conversion)
let bin = path($GOPATH) / "bin"
```

#### 1.3 Path Arithmetic

The `/` operator is overloaded for `path` values. It is the only supported
way to join path components. String concatenation for paths is a type error.

```forge
let base    = path("/usr/local")
let bin     = base / "bin"                    # path("/usr/local/bin")
let forge   = bin / "forge"                   # path("/usr/local/bin/forge")

# Chaining
let log = home() / ".forge" / "logs" / "forge.log"

# ❌ Never do this — type error
let bad = "/usr/local" + "/bin"               # string concat, not path join
```

#### 1.4 Path Methods

```forge
let p = path("/home/ajitem/.config/forge/config.fgs")

p.exists()           # bool
p.is_file()          # bool
p.is_dir()           # bool
p.is_symlink()       # bool

p.parent()           # path → /home/ajitem/.config/forge
p.file_name()        # string → "config.fgs"
p.stem()             # string → "config"
p.extension()        # Option<string> → Some("fgs")

p.to_string()        # Native platform string — / on Unix, \ on Windows
p.to_slash()         # Always forward slashes — for display/logging

p.absolute()         # Result<path> — resolves relative paths
p.canonicalise()     # Result<path> — resolves symlinks, normalises

p.with_extension("bak")     # path — replace extension
p.with_file_name("new.fgs") # path — replace filename

p.starts_with(base)  # bool
p.ends_with("fgs")   # bool

p.components()       # [string] — path segments split by separator
```

#### 1.5 Path Normalisation Rules

The following normalisation is applied by the `path()` constructor and at
every OS boundary:

| Rule | Input | Normalised |
|------|-------|------------|
| Backslash to forward slash (internal) | `C:\Users\ajitem` | `C:/Users/ajitem` |
| Collapse double slashes | `//usr//bin` | `/usr/bin` |
| Resolve `.` segments | `./foo/./bar` | `foo/bar` |
| Preserve `..` segments | `../foo` | `../foo` (resolved at OS boundary) |
| Strip trailing slash | `/usr/bin/` | `/usr/bin` |
| Drive letter case normalise (Windows) | `c:\` | `C:\` |

Forward slashes are the canonical internal representation. Backslashes are
emitted only when passing paths to Windows OS APIs.

#### 1.6 Windows Reserved Filename Detection

The following Windows-reserved filenames are detected and warned on all
platforms — including Linux and macOS:

```
CON PRN AUX NUL COM0-COM9 LPT0-LPT9
```

```forge
let p = path("./output/nul.fgs")
# Warning: "nul" is a reserved Windows device name.
# This path will not behave as expected on Windows.
# Consider renaming to avoid compatibility issues.
```

This warning fires on Linux and macOS too — making portability issues visible
before a script reaches a Windows machine.

#### 1.7 UNC Paths (Windows)

UNC paths (`\\server\share`) are supported as a path variant:

```forge
let unc = path("\\\\server\\share\\data")
# Normalised internally as: //server/share/data
```

UNC paths are a no-op warning on Linux and macOS (they are treated as relative
paths starting with `//`).

#### 1.8 PATH Environment Variable

`$PATH` is a specialised `[path]` — a list of `path` values. It is never
a raw string in ForgeScript.

```forge
# Reading
let paths = $PATH                  # [path]

# Mutation
$PATH.prepend(path("/usr/local/bin"))
$PATH.append(home() / ".cargo" / "bin")
$PATH.remove(path("/usr/local/bin"))
$PATH.contains(path("/usr/bin"))   # bool

# Iteration
for dir in $PATH {
  if dir.exists() {
    print(dir)
  }
}
```

When `$PATH` is passed to a child process, it is joined with `:` on Unix and
`;` on Windows by the platform lowering layer. The script author never sees
the separator.

---

### Part 2 — Environment Variable Model

#### 2.1 Variable Access Syntax

```forge
# Read — fails at runtime if unset
let home = $HOME

# Safe read — returns Option<string>
let token = $GITHUB_TOKEN?

# Read with default
let port = $PORT? |> unwrap_or("8080")

# Read as typed value
let port_num = $PORT?.and_then(|s| s.parse::<int>())
                     .unwrap_or(8080)
```

#### 2.2 Case Sensitivity

ForgeScript treats environment variables as case-sensitive in source code.
However, on Windows the OS is case-insensitive. Forge bridges this gap:

- **Reading** — on Windows, if `$GITHUB_TOKEN` is not found, Forge performs a
  case-insensitive lookup and emits a warning if a match is found under a
  different case.
- **Setting** — Forge always writes in the exact case specified. On Windows,
  this may shadow an existing variable of different case.
- **Best practice** — use `SCREAMING_SNAKE_CASE` for all environment variables.
  Document expected variable names. This eliminates the case ambiguity.

#### 2.3 Setting Variables

```forge
# Set — visible in current script scope and child processes spawned after
set PORT = "8080"

# Export — identical to set in ForgeScript (all set variables are exported
# to child processes by default)
export API_URL = "https://api.forge-shell.dev"

# Scoped set — visible only within a block
{
  set DEBUG = "true"
  run("my-server")?
}
# DEBUG is unset here

# Unset
unset TEMP_TOKEN
```

#### 2.4 Variable Scoping Rules

```
Script scope       — variables set at the top level of a script
Block scope        — variables set inside { } are dropped when the block exits
Function scope     — variables set inside fn are dropped when the function returns
Child processes    — inherit all variables set in parent scope at spawn time
                     subsequent changes to parent scope do NOT propagate
```

```forge
set FOO = "parent"

{
  set FOO = "block"
  print($FOO)       # "block"
}

print($FOO)         # "parent" — block scope dropped
```

#### 2.5 Typed Environment Variable Access

Environment variables are always `string` at the OS level. ForgeScript
provides ergonomic typed access:

```forge
# Explicit parse
let port: int = $PORT?.and_then(|s| s.parse()).unwrap_or(8080)

# Typed access helper (standard library)
let port = env::get_int("PORT", default: 8080)?
let debug = env::get_bool("DEBUG", default: false)?
let hosts = env::get_list("ALLOWED_HOSTS", separator: ",")?
```

#### 2.6 .env File Support

```forge
# Load .env file into current scope
load_env(".env")

# Load with override — existing vars are replaced
load_env(".env.production", override: true)

# .env format
# KEY=value
# KEY="value with spaces"
# KEY='literal value'
# # comment
# export KEY=value  (export keyword is optional and ignored)
```

`.env` files are UTF-8. CRLF is normalised. BOM is stripped.

#### 2.7 Reserved Variable Names

The following variable names are reserved by Forge Shell:

```
$PATH       Typed [path] list
$HOME       Home directory as path
$FORGE_VERSION  Forge Shell version string
$FORGE_OS       Platform identifier: "linux" | "macos" | "windows"
$FORGE_ARCH     CPU architecture: "x86_64" | "aarch64" | ...
$FORGE_SHELL    Path to the forge binary
$?          Exit code of the last command (int)
$0          Name of the current script (string)
$#          Number of arguments passed to the script (int)
$@          All arguments as a list [string]
```

---

## Drawbacks

- **Explicit path construction is verbose** — `path("/usr/bin")` is more
  typing than `"/usr/bin"`. Power users will find this annoying for one-liners.
- **PATH-as-list breaks raw string manipulation** — users who know tricks like
  `export PATH="/new:$PATH"` must learn the new model.
- **No implicit string-to-path coercion** — some friction when passing paths
  to functions that expect `string`. Explicit `.to_string()` required.
- **Case sensitivity model is complex** — the Windows case-insensitive lookup
  with warning is a pragmatic compromise, not a clean solution.

---

## Alternatives Considered

### Alternative A — Paths as Strings with Helpers

**Approach:** Keep paths as strings but provide `join_path()`, `normalise_path()`
helper functions.
**Rejected because:** Without a distinct type, nothing prevents string
concatenation for paths. The helpers would be advisory, not enforced. The
majority of path bugs would persist.

### Alternative B — Two Path Types (Unix and Windows)

**Approach:** `UnixPath` and `WindowsPath` as distinct types, with explicit
conversion between them.
**Rejected because:** This surfaces platform differences into script logic,
which is exactly what Forge Shell is trying to prevent. Scripts would need
`#[cfg]` equivalents to handle both types.

### Alternative C — PATH Remains a String

**Approach:** Keep `$PATH` as a colon/semicolon-separated string, provide
`path_list()` and `path_join()` helpers.
**Rejected because:** The separator difference is a perennial source of bugs.
Making PATH a typed list is the only way to make PATH manipulation truly
cross-platform without requiring the script author to think about separators.

---

## Unresolved Questions

- [ ] Should `path()` accept template strings? `path("/home/{username}/.config")`
- [ ] Should there be a path literal syntax (e.g. `p"/usr/bin"`) to reduce
      verbosity?
- [ ] How should symlink cycles be detected and reported in `canonicalise()`?
- [ ] Should `$?` be an `int` or a `Result` variant?
- [ ] Should `.env` loading support variable interpolation? (e.g. `KEY=${OTHER}`)
- [ ] How are non-UTF-8 paths handled? Reject? Lossy convert? Separate type?

---

## Implementation Plan

### Affected Crates

- `forge-lang/ast` — `PathExpr`, `PathType` AST nodes
- `forge-lang/hir` — path type inference, env var type resolution
- `forge-lower` — `lower_path()` implementation per platform
- `forge-builtins` — `load_env()`, `env::get_int()` etc.
- `forge-core` — internal `ForgePath` and `EnvMap` types

### Dependencies

- Requires RFC-001 (ForgeScript Syntax) — type system foundation
- Requires RFC-002 (Evaluation Pipeline) — `lower_path()` is part of
  `PlatformLowering`

### Milestones

1. Define `ForgePath` type in `forge-core` using `camino::Utf8PathBuf`
2. Define `EnvMap` type — case-preserving, case-insensitive lookup on Windows
3. Implement `path()` constructor with normalisation rules
4. Implement `/` operator for path arithmetic
5. Implement all `path` methods
6. Implement Windows reserved filename detection
7. Implement `$PATH` as typed list in HIR and execution context
8. Implement typed env var access helpers
9. Implement `.env` file loading
10. Integration tests — path and env behaviour on all three platforms

---

## References

- [`camino` crate — UTF-8 typed paths](https://docs.rs/camino)
- [Windows Reserved File Names](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)
- [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html)
- [The Twelve-Factor App — Config](https://12factor.net/config)
- [dotenv specification](https://hexdocs.pm/dotenvy/dotenv-file-format.html)
