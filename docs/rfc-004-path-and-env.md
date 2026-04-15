# RFC-004 — Path Type & Environment Variable Model

| Field          | Value                        |
|----------------|------------------------------|
| Status         | **In Review**                |
| Author         | Ajitem Sahasrabuddhe         |
| Created        | 2026-04-09                   |
| Last Updated   | 2026-04-15                   |
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

Paths and environment variables are the two most common sources of
cross-platform scripting failures:

- `/` vs `\` separator bugs silently break scripts on Windows
- `CON`, `NUL`, and other reserved filenames cause cryptic failures
- `PATH` separator (`:` vs `;`) breaks manual string manipulation
- Stringly-typed env vars cause silent type errors at runtime
- `.env` file loading varies wildly across tools and frameworks

ForgeScript addresses all of these by making `path` a first-class typed value
and by providing a typed access layer over the OS string-based env var model.

---

## Design

### 1. The `path` Type

`path` is a first-class type in ForgeScript. It is not a string alias — it
carries normalisation rules, platform semantics, and separator handling.

#### 1.1 Literal Syntax

Path literals use the `p"..."` prefix — statically validated at parse time:

```forge
let config = p"/etc/forge/config.toml"
let home   = p"~/projects"
let rel    = p"./src/main.fgs"
```

Reserved Windows filenames (`CON`, `NUL`, `PRN`, `AUX`, `COM1`–`COM9`,
`LPT1`–`LPT9`) are rejected at parse time on all platforms.

#### 1.2 Normalisation — Construction Time

Paths are normalised eagerly at the moment of construction — whether via
`p"..."` literal, dynamic construction, or the `/` join operator.

**Normalisation rules:**

| Input | Normalised form |
|---|---|
| `/home/user/../user/projects` | `/home/user/projects` |
| `C:\Users\Ajitem` | `C:/Users/Ajitem` |
| `/home/user/./projects/` | `/home/user/projects` |
| `~/projects` | `/home/ajitem/projects` — tilde expanded at runtime |
| `//double//slashes` | `/double/slashes` |
| `\\server\share` | `\\server\share` — UNC paths preserved on Windows |

**Rules applied:**
- Separator normalisation: `\` → `/` — canonical internal representation
- Dot segment resolution: `.` and `..` resolved eagerly
- Trailing slash stripped
- Tilde expansion: at construction time when runtime is available
- Double slashes collapsed — except UNC paths on Windows

**Equality semantics:** Two `path` values are equal if and only if their
normalised forms are identical. No OS call required to compare paths.

#### 1.3 Path Composition — The `/` Operator

Runtime path composition uses the `/` operator — not string interpolation:

```forge
let base    = p"/home/user"
let full    = base / "projects" / name    # typed join, validated
let config  = base / p".config/forge"    # mixed literal and path
```

The `/` operator:
- Validates each segment for invalid characters
- Checks for reserved Windows filenames on each segment
- Returns a normalised `path` — never a raw string

#### 1.4 Windows Reserved Filename Policy

| Platform | Behaviour |
|---|---|
| Linux / macOS | Compile-time warning `W021` |
| Windows | Hard error — construction fails |
| `forge check --platform=windows` | Escalates warning to hard error |

**Reserved names (case-insensitive, any extension):**
`CON`, `NUL`, `PRN`, `AUX`, `COM1`–`COM9`, `LPT1`–`LPT9`

**Warning format:**

```
warning[W021]: Windows reserved filename detected
  --> deploy.fgs:8:14
   |
 8 | let log = p"CON.log"
   |           ^^^^^^^^^^ reserved on Windows
   = note: this path will fail if this script is run on Windows
   = help: rename to avoid: CON.log → forge-con.log
```

**Applies to:**
- `p"..."` literals — caught at parse time
- Dynamically constructed paths — caught at construction time
- Paths received from external input — warned via `OutputMetadata.warnings`

#### 1.5 Path API — `forge::fs`

```forge
import forge::fs

# Metadata
forge::fs::exists(p"/etc/forge")           # bool
forge::fs::is_file(p"/etc/forge/config")   # bool
forge::fs::is_dir(p"/etc/forge")           # bool
forge::fs::metadata(p"/etc/forge/config")  # Result<FileMetadata, FsError>

# Manipulation
forge::fs::join(p"/home/user", "projects") # same as / operator
forge::fs::parent(p"/home/user/projects")  # p"/home/user"
forge::fs::filename(p"/home/user/file.txt") # "file.txt"
forge::fs::extension(p"/home/user/file.txt") # "txt"
forge::fs::stem(p"/home/user/file.txt")    # "file"

# Conversion
forge::fs::to_str(p"/home/user")           # str — platform-native separators
forge::fs::from_str("/home/user")          # Result<path, PathError>
```

---

### 2. The Environment Variable Model

#### 2.1 Core Principle — Typed Access, String Storage

Environment variables are stored as strings at the OS boundary — this is a
fundamental OS constraint that ForgeScript does not attempt to change. All
child processes receive string env vars regardless of how they were set.

ForgeScript provides a typed access layer on top — typed accessors parse on
read and serialise on write. The developer works with types. The OS sees
strings. The boundary is explicit and clear.

#### 2.2 Write API — Typed, Serialised to String

```forge
forge::env::set("PORT",    8080)              # int   → "8080"
forge::env::set("DEBUG",   true)              # bool  → "true"
forge::env::set("TIMEOUT", 30)                # int   → "30"
forge::env::set("HOME",    p"/home/ajitem")   # path  → "/home/ajitem"
forge::env::set("API_URL", u"https://api.forge-shell.dev")  # url → string

forge::env::unset("LEGACY_VAR")              # remove from environment
```

#### 2.3 Read API — Typed Accessors

```forge
let port    = forge::env::get_int("PORT")?
let debug   = forge::env::get_bool("DEBUG")?
let timeout = forge::env::get_int("TIMEOUT")?
let home    = forge::env::get_path("HOME")?
let api     = forge::env::get_url("API_URL")?
let raw     = forge::env::get_str("ANY_VAR")?   # raw string — always available
```

**Supported typed accessors:**

| Accessor | Parses to | Notes |
|---|---|---|
| `get_str` | `str` | Raw — always succeeds if var exists |
| `get_int` | `int` (`i64`) | Fails if not parseable as integer |
| `get_float` | `float` (`f64`) | Fails if not parseable as float |
| `get_bool` | `bool` | Accepts `"true"/"false"`, `"1"/"0"`, `"yes"/"no"` |
| `get_path` | `path` | Normalised at read time |
| `get_url` | `url` | Validated at read time |

**Parse error format:**

```
error[E042]: environment variable type mismatch
  --> deploy.fgs:12:14
   |
12 | let port = forge::env::get_int("PORT")?
   |            ^^^^^^^^^^^^^^^^^^^^^^^^^^^
   = note: PORT="808O" — expected int, found "808O"
   = help: check the value of PORT in your environment
```

#### 2.4 `$PATH` as `list<path>`

The system `PATH` variable is exposed as a typed `list<path>` — not a raw
string. ForgeScript parses it at startup using the platform-native separator
(`:` on Unix, `;` on Windows) and owns the typed representation.

```forge
# Read PATH as typed list
let paths = forge::env::path()                  # list<path>

# Mutate
forge::env::path().prepend(p"~/.local/bin")
forge::env::path().append(p"/usr/local/go/bin")

# Parse a raw PATH string — for migration and external input
let paths = forge::env::path_from_str(raw)?     # Result<list<path>, EnvError>
```

**Serialisation:** When ForgeScript spawns a child process, `list<path>` is
serialised back to the OS-native format transparently — `:` on Unix, `;` on
Windows. Script authors never handle the separator.

#### 2.5 Environment Variable Scoping

Environment variables follow the Unix process model:

- `forge::env::set()` in a script: **process-wide** — visible to all
  subsequent code and child processes spawned by this script
- `set` at the REPL: **session-wide** — visible to all subsequent commands
  in this shell session
- Changes **never propagate to the parent shell** — this is a fundamental OS
  constraint, not a ForgeScript restriction

```forge
forge::env::set("DATABASE_URL", "postgres://localhost/mydb")

run("psql")      # sees DATABASE_URL ✅
run("migrate")   # sees DATABASE_URL ✅

# Parent shell that launched this script: never sees DATABASE_URL
```

**Scoping summary:**

| Context | Scope | Parent sees it? |
|---|---|---|
| `forge::env::set()` in script | Process-wide | ❌ Never |
| `set` at REPL | Session-wide | ❌ Never |
| `with_env { }` block | Block-scoped — reverts after | ❌ Never |
| Child process spawned | Inherits parent env | N/A |

#### 2.6 `with_env` — Block-scoped Temporary Overrides

For cases requiring temporary env var overrides without polluting the process
environment — equivalent to the Unix `env VAR=value command` pattern, but
typed and explicit:

```forge
# Override for one block only — reverts after
with_env { DATABASE_URL: "postgres://localhost/testdb" } {
    run("cargo test")
}
# DATABASE_URL reverts here — previous value restored

# Multiple overrides
with_env {
    DATABASE_URL: "postgres://localhost/testdb",
    LOG_LEVEL:    "debug",
    PORT:         9090,
} {
    run("integration-tests")
}
```

#### 2.7 `.env` File Loading

ForgeScript takes an explicit-first approach to `.env` loading — informed by
Go's philosophy (no magic loading) and direnv's security model (explicit trust
grants).

**Core principle: `.env` loading is always explicit. No magic directory
scanning. No automatic loading without developer intent.**

**Explicit loading API:**

```forge
forge::env::load(p".env")                        # load specific file
forge::env::load_optional(p".env.local")         # no error if file missing
forge::env::load_cascade(env: "dev")             # full environment cascade
```

**Environment cascade — load order (highest precedence first):**

```
.env.[environment].local    # highest — local machine overrides
.env.local                  # local machine base
.env.[environment]          # environment-specific
.env                        # base — lowest precedence
```

`FORGE_ENV` environment variable drives the cascade environment:

```bash
FORGE_ENV=production forge run deploy.fgs
# load_cascade() loads: .env → .env.local → .env.production → .env.production.local
```

**Directory auto-run — `.forge-env` script:**

For developers who want automatic env loading when entering a directory,
ForgeScript uses a `.forge-env` script — a ForgeScript file that the developer
explicitly writes, granted execution permission via `forge env trust`:

```forge
# .forge-env — developer writes exactly what loads
forge::env::load(p".env")
forge::env::load_optional(p".env.local")
```

```bash
forge env trust          # grant trust to .forge-env in current directory
forge env trust --list   # list all trusted directories
forge env trust --revoke # revoke trust for current directory
```

Trust grants are stored in `~/.config/forge/trusted-envs.toml` — never in
the project directory.

**`.gitignore` recommendations** (generated by `forge migrate`):

```gitignore
.env.local
.env.*.local
.forge-env.local
```

---

## Drawbacks

- **Construction-time normalisation has a cost.** Every path operation
  allocates a new normalised string. For scripts that manipulate thousands of
  paths this may be measurable — mitigated by RFC-008 plan caching.
- **Typed env var accessors add API surface.** Six typed accessors
  (`get_str`, `get_int`, `get_float`, `get_bool`, `get_path`, `get_url`)
  instead of one. Mitigated by the raw `get_str` fallback always being
  available.
- **`.forge-env` is unfamiliar.** Developers coming from direnv know `.envrc`.
  The new name requires documentation and discovery.

---

## Alternatives Considered

### Alternative A — Use-time path normalisation

**Rejected:** Inconsistent equality semantics and debugging confusion.
Construction-time normalisation fails early and produces predictable equality.

### Alternative B — Hard error for Windows reserved filenames everywhere

**Rejected:** Overly restrictive for Linux-only scripts. Warning on
non-Windows with `forge check --platform=windows` escalation gives developers
the information they need without blocking legitimate use cases.

### Alternative C — Raw string `PATH`

**Rejected:** The `:` vs `;` separator inconsistency is exactly the class of
bug ForgeScript is designed to eliminate. `list<path>` with automatic
OS-native serialisation removes the entire problem class.

### Alternative D — Fully typed env var storage

**Rejected:** Child processes receive strings — always. Fully typed storage
would require serialisation at every process boundary, creating a leaky
abstraction. Typed access over string storage is honest and practical.

### Alternative E — Auto-load `.env` by default

**Rejected:** Auto-loading `.env` in a shell is more dangerous than in a web
framework — the blast radius is larger. Go's explicit-first philosophy and
direnv's trust model both validate this position. Explicit loading with
`.forge-env` for auto-run gives developers control without magic.

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | Path normalisation timing | Construction-time — eager, consistent, fail-early |
| UQ-2 | Windows reserved filenames on non-Windows | Warning on Linux/macOS, hard error on Windows |
| UQ-3 | `$PATH` as typed list | `list<path>` — typed, OS-native serialisation |
| UQ-4 | `.env` file loading | Explicit always — `.forge-env` + `forge env trust` for auto-run |
| UQ-5 | Env var typing | Typed access, string storage — typed accessors parse on read |
| UQ-6 | Env var scoping | Process-wide by default, `with_env` for block-scoped overrides |

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-lang/typeck` | `path` type checking, typed env var accessor validation |
| `forge-lang/hir` | `path` normalisation during HIR lowering |
| `forge-core/path` | Path normalisation, `/` operator, Windows reserved name validation |
| `forge-core/env` | Typed env var accessors, `with_env` block, `load_cascade` |
| `forge-backend/unix` | PATH serialisation — colon-separated |
| `forge-backend/windows` | PATH serialisation — semicolon-separated, UNC path handling |
| `forge-lang/diagnostics` | `W021` warning, `E042` type mismatch error |

### Dependencies

- Requires RFC-001 (path literal syntax, `p"..."`) to be accepted first.
- Requires RFC-002 (evaluation pipeline) to be accepted first.
- RFC-013 (Shell Configuration Model) handles `config.toml` `[path]` and
  `[env]` sections — PATH prepend/append config, auto-load settings.

### Milestones

1. Implement `forge-core/path` — normalisation, `/` operator, reserved name detection
2. Implement `W021` warning and `forge check --platform=windows` escalation
3. Implement `forge::env` typed accessors — `get_int`, `get_bool`, `get_path` etc.
4. Implement `forge::env::path()` — `list<path>` with OS-native serialisation
5. Implement `forge::env::path_from_str()` — raw PATH string parsing
6. Implement `with_env` block scoping
7. Implement `.env` loading API — `load`, `load_optional`, `load_cascade`
8. Implement `forge env trust` command and trusted-envs registry
9. Implement `FORGE_ENV` cascade driver
10. Integration tests on ubuntu-latest, macos-latest, windows-latest

---

## References

- [POSIX Path Resolution](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap04.html)
- [Windows Reserved Filenames](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)
- [direnv — Unclutter your .profile](https://direnv.net/)
- [Vite Environment Variables](https://vitejs.dev/guide/env-and-mode)
- [Go os.Getenv](https://pkg.go.dev/os#Getenv)
- [The Twelve-Factor App — Config](https://12factor.net/config)
- [RFC-001 — ForgeScript Language Syntax](./RFC-001-forgescript-syntax.md)
- [RFC-002 — Evaluation Pipeline](./RFC-002-evaluation-pipeline.md)
- [RFC-003 — Built-in Command Specification](./RFC-003-builtin-commands.md)
- [RFC-013 — Shell Configuration Model](./RFC-013-shell-config.md)