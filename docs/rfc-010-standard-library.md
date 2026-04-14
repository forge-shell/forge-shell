# RFC-010 — ForgeScript Standard Library

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

This RFC defines the ForgeScript standard library — the set of modules
available to every `.fgs` script without installation. The standard library
covers filesystem operations, string manipulation, environment access,
process management, networking utilities, JSON/YAML processing, date and time,
logging, and testing utilities.

---

## Motivation

A scripting language without a standard library forces users to reinvent
common utilities in every script. The standard library should cover the 80%
of script needs that do not require a plugin, providing a solid foundation
that script authors can rely on across all three platforms.

The standard library complements built-in commands — where built-ins are
invokable from the REPL and pipelines, the standard library provides
programmatic APIs usable within ForgeScript functions.

---

## Design

### 1. Import Model

Standard library modules are imported with the `forge::` prefix:

```forge
import forge::fs
import forge::str
import forge::env
import forge::process
import forge::net
import forge::json
import forge::yaml
import forge::time
import forge::log
import forge::test
```

Specific symbols can be imported directly:

```forge
from forge::fs   import { read_file, write_file, exists }
from forge::str  import { split, trim, starts_with }
from forge::time import { now, Duration }
```

---

### 2. Module Definitions

#### `forge::fs` — Filesystem

```forge
# Reading
forge::fs::read_file(path)          -> Result<string>
forge::fs::read_bytes(path)         -> Result<[byte]>
forge::fs::read_lines(path)         -> Result<[string]>

# Writing
forge::fs::write_file(path, string) -> Result<unit>
forge::fs::write_bytes(path, bytes) -> Result<unit>
forge::fs::append_file(path, string)-> Result<unit>

# Filesystem operations
forge::fs::exists(path)             -> bool
forge::fs::is_file(path)            -> bool
forge::fs::is_dir(path)             -> bool
forge::fs::is_symlink(path)         -> bool

forge::fs::copy(from, to)           -> Result<unit>
forge::fs::move(from, to)           -> Result<unit>
forge::fs::remove(path)             -> Result<unit>
forge::fs::remove_dir(path)         -> Result<unit>   # must be empty
forge::fs::remove_all(path)         -> Result<unit>   # recursive

forge::fs::create_dir(path)         -> Result<unit>
forge::fs::create_dir_all(path)     -> Result<unit>   # mkdir -p equivalent

forge::fs::list_dir(path)           -> Result<[DirEntry]>
forge::fs::walk_dir(path)           -> Result<[DirEntry]>  # recursive

forge::fs::temp_file()              -> Result<path>
forge::fs::temp_dir()               -> Result<path>

# Metadata
forge::fs::metadata(path)           -> Result<FileMetadata>
forge::fs::file_size(path)          -> Result<int>
forge::fs::modified_at(path)        -> Result<DateTime>
forge::fs::created_at(path)         -> Result<Option<DateTime>>  # None on Linux

# Watching
forge::fs::watch(path, fn(WatchEvent))  -> WatchHandle
```

---

#### `forge::str` — String Utilities

```forge
forge::str::split(s, sep)           -> [string]
forge::str::split_once(s, sep)      -> Option<(string, string)>
forge::str::join(parts, sep)        -> string

forge::str::trim(s)                 -> string
forge::str::trim_start(s)           -> string
forge::str::trim_end(s)             -> string

forge::str::starts_with(s, prefix)  -> bool
forge::str::ends_with(s, suffix)    -> bool
forge::str::contains(s, needle)     -> bool

forge::str::replace(s, from, to)    -> string
forge::str::replace_all(s, from, to)-> string

forge::str::to_upper(s)             -> string
forge::str::to_lower(s)             -> string

forge::str::pad_left(s, width, char)  -> string
forge::str::pad_right(s, width, char) -> string

forge::str::repeat(s, n)            -> string
forge::str::reverse(s)              -> string

forge::str::parse_int(s)            -> Result<int>
forge::str::parse_float(s)          -> Result<float>
forge::str::parse_bool(s)           -> Result<bool>

forge::str::lines(s)                -> [string]
forge::str::chars(s)                -> [string]
forge::str::bytes(s)                -> [byte]

forge::str::len(s)                  -> int   # character count (not bytes)
forge::str::byte_len(s)             -> int   # byte count

# Regex (basic)
forge::str::matches(s, pattern)     -> bool
forge::str::find(s, pattern)        -> Option<Match>
forge::str::find_all(s, pattern)    -> [Match]
forge::str::replace_regex(s, pattern, replacement) -> string
```

---

#### `forge::env` — Environment

```forge
forge::env::get(key)                -> Option<string>
forge::env::get_or(key, default)    -> string
forge::env::get_int(key, default)   -> int
forge::env::get_bool(key, default)  -> bool
forge::env::get_list(key, sep)      -> [string]
forge::env::get_path(key)           -> Option<path>

forge::env::set(key, value)         -> unit
forge::env::unset(key)              -> unit
forge::env::all()                   -> {string: string}

forge::env::load_dotenv(file)       -> Result<unit>
forge::env::load_dotenv_override(file) -> Result<unit>

forge::env::platform()              -> string   # "linux" | "macos" | "windows"
forge::env::arch()                  -> string   # "x86_64" | "aarch64" | ...
forge::env::home()                  -> path
forge::env::cwd()                   -> path
forge::env::temp_dir()              -> path
```

---

#### `forge::process` — Process Management

```forge
# Execution
forge::process::run(cmd, args)      -> Result<Output>
forge::process::run_str(cmd_str)    -> Result<Output>
forge::process::spawn(cmd, args)    -> Result<Child>
forge::process::exec(cmd, args)     -> !   # replaces current process

# Output struct
struct Output {
  stdout:    string
  stderr:    string
  exit_code: int
  success:   bool
}

# Child process
child.wait()                        -> Result<ExitStatus>
child.kill()                        -> Result<unit>
child.pid()                         -> int
child.stdin()                       -> OutputStream
child.stdout()                      -> InputStream

# Current process
forge::process::exit(code)          -> !
forge::process::pid()               -> int
forge::process::args()              -> [string]

# Signal handling (Unix — graceful degradation on Windows)
forge::process::on_interrupt(fn())  -> unit
forge::process::on_terminate(fn())  -> unit
```

---

#### `forge::net` — Networking Utilities

```forge
# HTTP (simple client — not a full HTTP library)
forge::net::get(url)                -> Result<Response>
forge::net::post(url, body)         -> Result<Response>
forge::net::request(Request)        -> Result<Response>

struct Response {
  status:  int
  headers: {string: string}
  body:    string
}

struct Request {
  method:  string
  url:     string
  headers: {string: string}
  body:    Option<string>
  timeout: Option<Duration>
}

# DNS
forge::net::resolve(hostname)       -> Result<[string]>   # IP addresses

# Ports
forge::net::is_port_open(host, port) -> bool
```

---

#### `forge::json` — JSON Processing

```forge
# Parse
forge::json::parse(s)               -> Result<Value>
forge::json::parse_file(path)       -> Result<Value>

# Serialise
forge::json::to_string(value)       -> string
forge::json::to_string_pretty(value)-> string
forge::json::to_file(path, value)   -> Result<unit>

# Query (JQ-style path access)
forge::json::get(value, path)       -> Option<Value>
forge::json::get_str(value, path)   -> Option<string>
forge::json::get_int(value, path)   -> Option<int>

# Value type
enum Value {
  Null
  Bool(bool)
  Int(int)
  Float(float)
  String(string)
  Array([Value])
  Object({string: Value})
}
```

---

#### `forge::yaml` — YAML Processing

```forge
forge::yaml::parse(s)               -> Result<Value>
forge::yaml::parse_file(path)       -> Result<Value>
forge::yaml::to_string(value)       -> string
forge::yaml::to_file(path, value)   -> Result<unit>

# YAML uses the same Value type as forge::json
```

---

#### `forge::time` — Date and Time

```forge
# Current time
forge::time::now()                  -> DateTime
forge::time::now_utc()              -> DateTime

# Constructors
forge::time::from_unix(secs)        -> DateTime
forge::time::parse(s, format)       -> Result<DateTime>

# DateTime methods
dt.format(fmt)                      -> string
dt.unix_timestamp()                 -> int
dt.year()                           -> int
dt.month()                          -> int
dt.day()                            -> int
dt.hour()                           -> int
dt.minute()                         -> int
dt.second()                         -> int
dt.add(Duration)                    -> DateTime
dt.sub(Duration)                    -> DateTime
dt.diff(other)                      -> Duration

# Duration
Duration::seconds(n)                -> Duration
Duration::minutes(n)                -> Duration
Duration::hours(n)                  -> Duration
Duration::days(n)                   -> Duration

dur.as_seconds()                    -> int
dur.as_minutes()                    -> float
dur.as_hours()                      -> float

# Timing
forge::time::sleep(Duration)        -> unit
forge::time::measure(fn())          -> (result, Duration)
```

---

#### `forge::log` — Logging

```forge
forge::log::debug(msg)              -> unit
forge::log::info(msg)               -> unit
forge::log::warn(msg)               -> unit
forge::log::error(msg)              -> unit

# Structured logging
forge::log::info_with(msg, {string: Value})  -> unit

# Log level control
forge::log::set_level(LogLevel)     -> unit
forge::log::get_level()             -> LogLevel

enum LogLevel { Debug, Info, Warn, Error, Off }

# Default output: stderr
# Configurable via $FORGE_LOG_LEVEL environment variable
```

---

#### `forge::test` — Testing Utilities

```forge
# Assertions
forge::test::assert(condition, msg)           -> unit
forge::test::assert_eq(a, b)                  -> unit
forge::test::assert_ne(a, b)                  -> unit
forge::test::assert_ok(result)                -> unit
forge::test::assert_err(result)               -> unit
forge::test::assert_contains(s, substring)    -> unit
forge::test::assert_matches(s, pattern)       -> unit

# Test definition
#[test]
fn test_deploy_staging() {
  let result = deploy("staging", dry_run: true)
  forge::test::assert_ok(result)
}

# Test fixtures
forge::test::temp_dir()             -> path    # auto-cleaned after test
forge::test::fixture(name)          -> path    # load from tests/fixtures/

# Running tests
# forge test                        — runs all #[test] functions
# forge test test_deploy_*          — runs matching tests
# forge test --verbose              — show output for passing tests
```

---

### 3. Standard Library Availability

The standard library is built into the Forge Shell binary. It requires no
import installation and is always available offline. All modules are
implemented in Rust within `forge-builtins` and exposed to ForgeScript via
the FFI bridge in `forge-lang`.

All standard library functions are cross-platform. Functions that have
platform-specific behaviour (e.g. `forge::fs::created_at` returns `None` on
Linux) document this explicitly.

---

## Drawbacks

- **Large implementation surface** — a full standard library is significant
  ongoing work.
- **API stability commitment** — once shipped, standard library APIs are
  effectively permanent. Deprecation is possible but painful.
- **Scope creep risk** — the boundary between standard library and plugin
  territory is blurry. Clear criteria are needed: if it requires network
  access to a third-party service, it's a plugin. If it's a general utility
  available offline, it's stdlib.

---

## Alternatives Considered

### Alternative A — Minimal Stdlib, Everything Else via Plugins

**Approach:** stdlib is just `forge::fs`, `forge::env`, `forge::process`.
Everything else (json, yaml, net, time) is a plugin.
**Rejected because:** JSON processing and time utilities are needed in almost
every non-trivial script. Requiring a plugin install for `forge::json` creates
unnecessary friction and breaks offline usage.

### Alternative B — No Stdlib, Use Shell Pipeline Composition

**Approach:** ForgeScript scripts compose built-in commands via pipelines
instead of calling library functions.
**Rejected because:** Pipeline composition is appropriate for simple
transformations. Complex logic (parsing JSON, computing time deltas,
writing structured logs) requires proper API access within script functions.

---

## Unresolved Questions

- [ ] Should `forge::net` be in the standard library or a plugin? It requires
      outbound network access which feels plugin-like.
- [ ] Should `forge::yaml` be in stdlib? YAML is common in DevOps but not
      universal.
- [ ] What regex flavour does `forge::str` support? PCRE? RE2? A subset?
- [ ] Should there be a `forge::crypto` module for hashing (SHA-256, etc)?
- [ ] How are stdlib updates delivered? They are part of the Forge binary —
      stdlib updates require a Forge Shell upgrade.

---

## Implementation Plan

### Affected Crates

- `forge-builtins` — stdlib implementations in Rust
- `forge-lang` — stdlib module imports, FFI bridge to Rust implementations
- `forge-cli` — `forge test` subcommand

### Dependencies

- Requires RFC-001 (ForgeScript Syntax) — type system defines stdlib
  function signatures
- Requires RFC-002 (Evaluation Pipeline) — stdlib functions are resolved
  at the HIR stage

### Milestones

1. Implement `forge::env` — simplest module, needed early
2. Implement `forge::str` — needed for any non-trivial scripting
3. Implement `forge::fs` — mirrors built-in commands as programmatic APIs
4. Implement `forge::process` — run/spawn/exec from script functions
5. Implement `forge::json` — needed for cloud/DevOps workflows
6. Implement `forge::log` — structured logging for scripts
7. Implement `forge::test` + `forge test` command
8. Implement `forge::time`
9. Implement `forge::yaml`
10. Implement `forge::net` (if included in stdlib — see unresolved questions)

---

## References

- [Deno Standard Library](https://deno.land/std)
- [Go Standard Library](https://pkg.go.dev/std)
- [Nushell Standard Library](https://www.nushell.sh/book/standard_library.html)
- [Python Standard Library](https://docs.python.org/3/library/)
