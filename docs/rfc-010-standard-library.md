# RFC-010 — ForgeScript Standard Library

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

This RFC defines the ForgeScript standard library — the set of modules
compiled directly into the `forge` binary and available to every `.fgs` script
without installation. The standard library follows Go-style compatibility
guarantees: no breaking changes within a major version. The stdlib covers
filesystem operations, string manipulation, environment access, networking,
serialisation, cryptography, process management, time, logging, and testing.

---

## Motivation

A scripting language without a standard library forces users to reinvent
common utilities in every script. The stdlib covers the 80% of script needs
that do not require a plugin, providing a solid, always-available foundation
that script authors can rely on across all three platforms.

The stdlib complements built-in commands — where built-ins are invokable from
the REPL and pipelines, the stdlib provides programmatic APIs usable within
ForgeScript functions.

---

## Design

### 1. Import Model

Standard library modules are imported with the `forge::` prefix:

```forge
import forge::fs
import forge::str
import forge::env
import forge::net
import forge::codec
import forge::json
import forge::yaml
import forge::toml
import forge::crypto
import forge::process
import forge::time
import forge::log
import forge::test
```

Specific symbols can be imported directly:

```forge
from forge::fs     import { read_file, write_file, exists }
from forge::str    import { split, trim, starts_with }
from forge::crypto import { sha256, hmac_sha256 }
```

---

### 2. Compatibility Guarantee

The ForgeScript stdlib follows Go-style compatibility:

> Any `.fgs` script that uses `forge::` stdlib functions and compiles
> correctly with Forge Shell v1.x will compile and run correctly with any
> future Forge Shell v1.y release, where y > x.

| Change type | Policy |
|---|---|
| Bug fixes | Any patch release — behaviour corrected, API unchanged |
| New functions added | Any minor release — always backward compatible |
| New modules added | Any minor release — new imports never break old scripts |
| Function signature changes | Never within a major version |
| Function removal | Never within a major version |
| Security vulnerability fixes | May change behaviour in minor releases — documented |

**Deprecation process:**

```forge
#[deprecated(since = "1.3.0", replaced_by = "forge::crypto::sha256_file")]
fn forge::crypto::hash_file(p: path) -> Result<str>
```

Deprecated functions compile with a warning — never an error. Removed only
in the next major version (2.x), listed in the migration guide.

**`forge::experimental` namespace** — modules not yet covered by the
compatibility guarantee:

```forge
import forge::experimental::net::websocket
```

Experimental modules can change between minor releases. They graduate to
`forge::` proper after at least one minor release with no API changes needed.

See RFC-014 for the full release policy and versioning model.

---

### 3. Module Definitions

#### `forge::fs` — File System

```forge
# Reading
forge::fs::read_file(p: path)           -> Result<str>
forge::fs::read_bytes(p: path)          -> Result<list<int>>
forge::fs::read_lines(p: path)          -> Result<list<str>>

# Writing
forge::fs::write_file(p: path, s: str)  -> Result<()>
forge::fs::write_bytes(p: path, b: list<int>) -> Result<()>
forge::fs::append_file(p: path, s: str) -> Result<()>

# Existence and type
forge::fs::exists(p: path)              -> bool
forge::fs::is_file(p: path)             -> bool
forge::fs::is_dir(p: path)              -> bool
forge::fs::is_symlink(p: path)          -> bool

# Operations
forge::fs::copy(from: path, to: path)   -> Result<()>
forge::fs::move(from: path, to: path)   -> Result<()>
forge::fs::remove(p: path)              -> Result<()>
forge::fs::remove_dir(p: path)          -> Result<()>   # must be empty
forge::fs::remove_all(p: path)          -> Result<()>   # recursive

forge::fs::create_dir(p: path)          -> Result<()>
forge::fs::create_dir_all(p: path)      -> Result<()>   # mkdir -p

forge::fs::list_dir(p: path)            -> Result<list<DirEntry>>
forge::fs::walk_dir(p: path)            -> Result<list<DirEntry>>  # recursive

forge::fs::temp_file()                  -> Result<path>
forge::fs::temp_dir()                   -> Result<path>

# Metadata
forge::fs::metadata(p: path)            -> Result<FileMetadata>
forge::fs::file_size(p: path)           -> Result<int>
forge::fs::modified_at(p: path)         -> Result<DateTime>

# Permissions
forge::fs::permissions(p: path)         -> Result<Permissions>
forge::fs::set_permissions(p: path, perm: Permissions) -> Result<()>

struct DirEntry {
    path:     path,
    name:     str,
    kind:     FileKind,
    size:     Option<int>,
    modified: Option<DateTime>,
}

enum FileKind { File, Directory, Symlink, Junction }
```

---

#### `forge::str` — String Manipulation

```forge
# Basic operations
forge::str::len(s: str)                  -> int
forge::str::is_empty(s: str)             -> bool
forge::str::trim(s: str)                 -> str
forge::str::trim_start(s: str)           -> str
forge::str::trim_end(s: str)             -> str
forge::str::to_upper(s: str)             -> str
forge::str::to_lower(s: str)             -> str

# Search
forge::str::contains(s: str, sub: str)   -> bool
forge::str::starts_with(s: str, prefix: str) -> bool
forge::str::ends_with(s: str, suffix: str)   -> bool
forge::str::find(s: str, sub: str)       -> Option<int>
forge::str::count(s: str, sub: str)      -> int

# Manipulation
forge::str::replace(s: str, from: str, to: str)     -> str
forge::str::replace_all(s: str, from: str, to: str) -> str
forge::str::split(s: str, sep: str)      -> list<str>
forge::str::join(parts: list<str>, sep: str) -> str
forge::str::repeat(s: str, n: int)       -> str
forge::str::pad_left(s: str, n: int, ch: str)  -> str
forge::str::pad_right(s: str, n: int, ch: str) -> str
forge::str::truncate(s: str, n: int)     -> str

# Parsing
forge::str::parse_int(s: str)            -> Result<int>
forge::str::parse_float(s: str)          -> Result<float>
forge::str::parse_bool(s: str)           -> Result<bool>

# Regex — RE2 flavour (linear time, no catastrophic backtracking)
forge::str::matches(s: str, pattern: regex)               -> bool
forge::str::find_regex(s: str, pattern: regex)             -> Option<Match>
forge::str::find_all(s: str, pattern: regex)               -> list<Match>
forge::str::captures(s: str, pattern: regex)               -> Option<Captures>
forge::str::captures_all(s: str, pattern: regex)           -> list<Captures>
forge::str::replace_regex(s: str, pattern: regex, rep: str)     -> str
forge::str::replace_all_regex(s: str, pattern: regex, rep: str) -> str
forge::str::split_regex(s: str, pattern: regex)            -> list<str>

struct Match    { text: str, start: int, end: int }
struct Captures { full: Match, groups: list<Option<Match>> }
```

**Regex flavour:** RE2 — guaranteed linear time execution, no catastrophic
backtracking. Supports named capture groups `(?P<name>...)`. Does not support
lookahead, lookbehind, or backreferences. Implementation: Rust `regex` crate.

---

#### `forge::env` — Environment Variables

As specified in RFC-004. Typed accessors over string storage.

```forge
forge::env::get_str(key: str)    -> Result<str>
forge::env::get_int(key: str)    -> Result<int>
forge::env::get_float(key: str)  -> Result<float>
forge::env::get_bool(key: str)   -> Result<bool>
forge::env::get_path(key: str)   -> Result<path>
forge::env::get_url(key: str)    -> Result<url>

forge::env::set(key: str, value: any) -> ()
forge::env::unset(key: str)           -> ()
forge::env::exists(key: str)          -> bool
forge::env::all()                     -> map<str, str>

forge::env::path()                    -> list<path>
forge::env::path_from_str(s: str)     -> Result<list<path>>

forge::env::load(p: path)             -> Result<()>
forge::env::load_optional(p: path)    -> Result<()>
forge::env::load_cascade(env: str)    -> Result<()>
```

---

#### `forge::net` — Networking

Comprehensive networking following Go's philosophy — full primitives in stdlib.

```forge
# HTTP
forge::net::get(url: url)                                     -> Result<HttpResponse>
forge::net::post(url: url, body: str)                         -> Result<HttpResponse>
forge::net::request(method: str, url: url, opts: HttpOpts)    -> Result<HttpResponse>

struct HttpResponse { status: int, headers: map<str, str>, body: str, bytes: list<int> }
struct HttpOpts     { headers: map<str, str>, body: Option<str>, timeout_ms: Option<int> }

# TCP/UDP
forge::net::dial(network: str, address: str)                  -> Result<Connection>
forge::net::listen(network: str, address: str)                -> Result<Listener>
forge::net::dial_tls(address: str, config: TlsConfig)         -> Result<Connection>

# Unix domain sockets — Linux/macOS only, documented limitation on Windows
forge::net::dial_unix(p: path)                                -> Result<Connection>

# DNS — unified lookup with typed record enum
forge::net::lookup(domain: str, record: DnsRecord)            -> Result<list<DnsResult>>
forge::net::resolve(hostname: str)                            -> Result<list<str>>   # A + AAAA

enum DnsRecord { A, AAAA, MX, TXT, CNAME, NS, PTR, SOA, SRV }

enum DnsResult {
    A(str),
    AAAA(str),
    MX     { priority: int, exchange: str },
    TXT    (str),
    CNAME  (str),
    NS     (str),
    PTR    (str),
    SOA    { mname: str, rname: str, serial: u32 },
    SRV    { priority: int, weight: int, port: int, target: str },
}

# IP utilities
forge::net::parse_ip(s: str)                                  -> Result<IpAddr>
forge::net::parse_cidr(s: str)                                -> Result<CidrBlock>
```

**Multiple records:** `lookup` returns `Result<list<DnsResult>>` — multiple
MX, TXT, NS records are returned as multiple list elements naturally.

**Platform note:** `dial_unix` is not supported on Windows — returns
`Err(CommandError::PlatformUnsupported)` with a clear message.

---

#### `forge::codec` — Serialisation Trait System

A serde-inspired trait system. The `Format` trait is implemented by `forge::json`,
`forge::yaml`, `forge::toml`, and optionally by plugins.

```forge
# Core traits
trait Serialize {
    fn serialize(self) -> Result<Value>
}

trait Deserialize {
    fn deserialize(v: Value) -> Result<Self>
}

trait Format {
    fn encode(v: Value) -> Result<str>
    fn decode(s: str)   -> Result<Value>
}

# Universal intermediate representation
enum Value {
    Null,
    Bool(bool),
    Int(int),
    Float(float),
    Str(str),
    List(list<Value>),
    Map(map<str, Value>),
}

# Top-level functions
forge::codec::to_str(v: Serialize, fmt: Format)              -> Result<str>
forge::codec::from_str(s: str, fmt: Format)                  -> Result<Value>
forge::codec::to_file(v: Serialize, p: path, fmt: Format)    -> Result<()>
forge::codec::from_file(p: path, fmt: Format)                -> Result<Value>
forge::codec::transcode(s: str, from: Format, to: Format)    -> Result<str>
```

**`#[derive(Serialize, Deserialize)]`** — automatic derivation for structs
and enums:

```forge
#[derive(Serialize, Deserialize)]
struct DeployConfig {
    env:      str,
    replicas: int,
    image:    str,
}

let config  = DeployConfig { env: "prod", replicas: 3, image: "forge:1.0" }
let json    = forge::codec::to_str(config, forge::json::Format)?
let yaml    = forge::codec::to_str(config, forge::yaml::Format)?
let back    = forge::codec::from_str(json, forge::json::Format)? as DeployConfig
```

**Plugin-extensible:** Third-party plugins can implement `Format` — `forge-csv`,
`forge-msgpack`, `forge-cbor`, `forge-dotenv` etc. — and integrate seamlessly
with `forge::codec::transcode`.

---

#### `forge::json` — JSON

Implements `forge::codec::Format`. Additional JSON-specific utilities.

```forge
# Implements Format — works with forge::codec::*
forge::json::Format    # the format token — passed to forge::codec functions

# JSON-specific extras
forge::json::get(v: Value, path: str)           -> Option<Value>   # jq-style path
forge::json::get_all(v: Value, path: str)        -> list<Value>
forge::json::pretty(v: Value)                   -> Result<str>
forge::json::merge(base: Value, overlay: Value) -> Value           # deep merge
forge::json::validate(v: Value, schema: Value)  -> Result<()>      # JSON Schema
```

---

#### `forge::yaml` — YAML

Implements `forge::codec::Format`.

```forge
forge::yaml::Format      # the format token

# YAML-specific extras
forge::yaml::parse_multi(s: str) -> Result<list<Value>>   # multi-document YAML
```

---

#### `forge::toml` — TOML

Implements `forge::codec::Format`.

```forge
forge::toml::Format      # the format token

# TOML-specific extras
forge::toml::parse_datetime(s: str) -> Result<DateTime>   # TOML datetime type
```

---

#### `forge::crypto` — Cryptographic Primitives

Comprehensive crypto in stdlib — Go-inspired. Primitives only — protocol-level
crypto (JWT, PGP, X.509) is plugin territory. Implementation: `ring` crate
(Google's BoringSSL-based, extensively audited).

```forge
# Hashing
forge::crypto::sha256(data: str)               -> str          # hex digest
forge::crypto::sha256_bytes(data: str)         -> list<int>    # raw bytes
forge::crypto::sha512(data: str)               -> str
forge::crypto::sha512_bytes(data: str)         -> list<int>
forge::crypto::md5(data: str)                  -> str          # legacy only

# File hashing — mirrors hash built-in command
forge::crypto::sha256_file(p: path)            -> Result<str>
forge::crypto::sha512_file(p: path)            -> Result<str>

# HMAC
forge::crypto::hmac_sha256(key: str, data: str) -> str
forge::crypto::hmac_sha512(key: str, data: str) -> str

# Symmetric encryption
forge::crypto::aes_encrypt(key: str, data: str) -> Result<str>
forge::crypto::aes_decrypt(key: str, data: str) -> Result<str>

# Asymmetric — Ed25519 (same algorithm used for plugin signing)
forge::crypto::ed25519_keygen()                            -> KeyPair
forge::crypto::ed25519_sign(key: PrivateKey, data: str)    -> str
forge::crypto::ed25519_verify(key: PublicKey, data: str, sig: str) -> bool

# Cryptographically secure random
forge::crypto::random_bytes(n: int)            -> list<int>
forge::crypto::random_hex(n: int)              -> str

# Constant-time comparison — prevents timing attacks
forge::crypto::constant_eq(a: str, b: str)     -> bool

# Encoding — commonly paired with crypto
forge::crypto::base64_encode(data: str)        -> str
forge::crypto::base64_decode(s: str)           -> Result<str>
forge::crypto::hex_encode(data: str)           -> str
forge::crypto::hex_decode(s: str)              -> Result<str>

struct KeyPair    { public: PublicKey, private: PrivateKey }
struct PublicKey  { bytes: list<int> }
struct PrivateKey { bytes: list<int> }
```

**Plugin territory — protocol-level crypto:**

```
forge-jwt        → JWT creation and validation
forge-pgp        → PGP signing and verification
forge-x509       → X.509 certificate handling
forge-ssh-agent  → SSH agent protocol
```

---

#### `forge::process` — Process Management

```forge
# Run and wait
forge::process::run(cmd: str, args: list<str>)               -> Result<ProcessOutput>
forge::process::run_shell(cmd: str)                          -> Result<ProcessOutput>

# Capture output
forge::process::capture(cmd: str, args: list<str>)           -> Result<str>
forge::process::capture_stderr(cmd: str, args: list<str>)    -> Result<str>

# Spawn without waiting
forge::process::spawn(cmd: str, args: list<str>)             -> Result<Child>
forge::process::spawn_with_env(cmd: str, args: list<str>, env: map<str, str>) -> Result<Child>

# Child process management
forge::process::wait(child: Child)                           -> Result<ProcessOutput>
forge::process::kill(child: Child)                           -> Result<()>
forge::process::pid(child: Child)                            -> int

# Current process
forge::process::exit(code: int)                              -> !   # never returns
forge::process::args()                                       -> list<str>

struct ProcessOutput { exit_code: int, stdout: str, stderr: str, success: bool }
```

---

#### `forge::time` — Time and Duration

```forge
# Current time
forge::time::now()                                           -> DateTime
forge::time::now_utc()                                       -> DateTime

# Duration construction — d"..." literal deferred to post-v1
forge::time::duration_ms(ms: int)                            -> Duration
forge::time::duration_secs(s: int)                           -> Duration
forge::time::duration_mins(m: int)                           -> Duration
forge::time::duration_hours(h: int)                          -> Duration

# DateTime operations
forge::time::add(dt: DateTime, d: Duration)                  -> DateTime
forge::time::sub(dt: DateTime, d: Duration)                  -> DateTime
forge::time::diff(a: DateTime, b: DateTime)                  -> Duration
forge::time::before(a: DateTime, b: DateTime)                -> bool
forge::time::after(a: DateTime, b: DateTime)                 -> bool

# Formatting and parsing
forge::time::format(dt: DateTime, layout: str)               -> str
forge::time::parse(s: str, layout: str)                      -> Result<DateTime>
forge::time::unix(dt: DateTime)                              -> int   # Unix timestamp
forge::time::from_unix(ts: int)                              -> DateTime

# Sleeping
forge::time::sleep(d: Duration)                              -> ()

struct DateTime { ... }   # opaque — use format/parse for string conversion
struct Duration { ms: int }
```

**Note:** `d"..."` duration literals are deferred to post-v1 (RFC-001). Until
then, use `forge::time::duration_secs(30)` etc.

---

#### `forge::log` — Structured Logging

```forge
forge::log::debug(msg: str)                                  -> ()
forge::log::info(msg: str)                                   -> ()
forge::log::warn(msg: str)                                   -> ()
forge::log::error(msg: str)                                  -> ()

# Structured logging with fields
forge::log::debug_fields(msg: str, fields: map<str, Value>)  -> ()
forge::log::info_fields(msg: str, fields: map<str, Value>)   -> ()
forge::log::warn_fields(msg: str, fields: map<str, Value>)   -> ()
forge::log::error_fields(msg: str, fields: map<str, Value>)  -> ()

# Log level control
forge::log::set_level(level: LogLevel)                       -> ()
forge::log::get_level()                                      -> LogLevel

enum LogLevel { Debug, Info, Warn, Error }
```

Log output format — configurable in `config.toml`:

```toml
[log]
format = "text"    # "text" | "json"
level  = "info"    # "debug" | "info" | "warn" | "error"
```

---

#### `forge::test` — Testing Primitives

```forge
# Assertions
forge::test::assert(condition: bool, msg: str)               -> ()
forge::test::assert_eq(a: any, b: any, msg: str)             -> ()
forge::test::assert_ne(a: any, b: any, msg: str)             -> ()
forge::test::assert_ok(result: Result<any>)                  -> ()
forge::test::assert_err(result: Result<any>)                 -> ()
forge::test::assert_contains(s: str, sub: str)               -> ()
forge::test::assert_matches(s: str, pattern: regex)          -> ()

# Test runner integration
#[test]
fn my_test() -> Result<()> {
    forge::test::assert_eq(1 + 1, 2, "arithmetic works")
    Ok(())
}
```

Run tests with `forge test`:

```bash
forge test                    # run all tests in current directory
forge test ./scripts/         # run tests in directory
forge test --verbose          # verbose output
forge test --filter deploy    # run tests matching pattern
```

---

### 4. Module Summary Table

| Module | Responsibility | v1 |
|---|---|---|
| `forge::fs` | File system operations | ✅ |
| `forge::str` | String manipulation, RE2 regex | ✅ |
| `forge::env` | Environment variable access | ✅ |
| `forge::net` | HTTP, TCP/UDP, DNS, TLS, IP utilities | ✅ |
| `forge::codec` | Serialisation trait — Serialize, Deserialize, Format | ✅ |
| `forge::json` | JSON format + get, pretty, validate, merge | ✅ |
| `forge::yaml` | YAML format + parse_multi | ✅ |
| `forge::toml` | TOML format + parse_datetime | ✅ |
| `forge::crypto` | Hashing, HMAC, AES, Ed25519, random, encoding | ✅ |
| `forge::process` | Process spawning and management | ✅ |
| `forge::time` | Time and duration | ✅ |
| `forge::log` | Structured logging | ✅ |
| `forge::test` | Testing primitives | ✅ |
| `forge::experimental` | Unstable modules — not covered by guarantee | ✅ |

---

## Drawbacks

- **Stdlib updates require a Forge Shell upgrade.** There is no way to get a
  stdlib patch without upgrading the binary. Mitigated by the semver patch
  release model — security fixes ship quickly as patch releases.
- **`forge::codec` adds design complexity.** The trait system is more
  sophisticated than three independent modules. Mitigated by `#[derive]`
  making the common case simple.
- **`forge::net` is comprehensive.** A large networking stdlib means more
  surface area to maintain and audit. Mitigated by using battle-tested
  underlying Rust crates (`reqwest`, `tokio`).

---

## Alternatives Considered

### Alternative A — Minimal Stdlib (fs, env, process only)

**Rejected:** JSON and networking are needed in almost every non-trivial
DevOps script. Requiring plugin installs for `forge::json` creates friction
and breaks offline usage.

### Alternative B — Three Separate Serialisation Modules (no forge::codec)

**Rejected:** Three modules that happen to share a `Value` type is less
elegant than a trait system. `forge::codec` enables plugins to add new formats
and enables `transcode` across all formats uniformly.

### Alternative C — PCRE Regex

**Rejected:** PCRE's catastrophic backtracking is a ReDoS attack vector.
RE2's linear time guarantee is the correct choice for a shell where user
input is common. Go's `regexp` makes the same choice.

### Alternative D — Plugin-Based Crypto

**Rejected:** Security-critical code belongs in the stdlib where it is
maintained by the core team, receives security audits, and is always available.
Go's comprehensive `crypto/*` stdlib is the right model.

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | `forge::net` stdlib or plugin? | Comprehensive stdlib — Go philosophy |
| UQ-2 | `forge::yaml` in stdlib? | Yes — plus `forge::toml` and `forge::codec` |
| UQ-3 | Regex flavour | RE2 — Rust `regex` crate |
| UQ-4 | `forge::crypto` module? | Yes — comprehensive, `ring` crate |
| UQ-5 | Stdlib update model | Go-style compatibility — see RFC-014 |

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-stdlib/fs` | `forge::fs` implementation |
| `forge-stdlib/str` | `forge::str` + regex via `regex` crate |
| `forge-stdlib/env` | `forge::env` — extends RFC-004 implementation |
| `forge-stdlib/net` | `forge::net` — HTTP via `reqwest`, TCP via `tokio::net` |
| `forge-stdlib/codec` | `forge::codec` trait definitions and `Value` type |
| `forge-stdlib/json` | `forge::json` — Format impl via `serde_json` |
| `forge-stdlib/yaml` | `forge::yaml` — Format impl via `serde_yaml` |
| `forge-stdlib/toml` | `forge::toml` — Format impl via `toml` crate |
| `forge-stdlib/crypto` | `forge::crypto` — via `ring` crate |
| `forge-stdlib/process` | `forge::process` — via `tokio::process` |
| `forge-stdlib/time` | `forge::time` — via `chrono` crate |
| `forge-stdlib/log` | `forge::log` — via `tracing` crate |
| `forge-stdlib/test` | `forge::test` + `forge test` command |
| `forge-lang/hir` | Stdlib module resolution at HIR stage |
| `forge-lang/derive` | `#[derive(Serialize, Deserialize)]` macro |

### Dependencies

- Requires RFC-001 (ForgeScript syntax) — type system, `#[derive]` annotation
- Requires RFC-002 (evaluation pipeline) — stdlib resolved at HIR stage
- Requires RFC-004 (path & env model) — `forge::env` and `forge::fs` extend it
- Requires RFC-014 (release policy) — compatibility guarantee reference

### Milestones

1. Implement `forge::env` — extends RFC-004, needed earliest
2. Implement `forge::fs` — programmatic API over RFC-004 path primitives
3. Implement `forge::str` — string ops + RE2 regex via `regex` crate
4. Implement `forge::codec` — trait system + `Value` enum
5. Implement `forge::json` — Format impl + extras
6. Implement `forge::yaml` — Format impl
7. Implement `forge::toml` — Format impl
8. Implement `forge::process` — process spawning via `tokio::process`
9. Implement `forge::net` — HTTP via `reqwest`, TCP/UDP/DNS via `tokio::net`
10. Implement `forge::crypto` — via `ring` crate
11. Implement `forge::time` — via `chrono`
12. Implement `forge::log` — via `tracing`
13. Implement `forge::test` + `forge test` command
14. Implement `#[derive(Serialize, Deserialize)]` macro in `forge-lang/derive`
15. Integration tests for all modules on ubuntu-latest, macos-latest, windows-latest

---

## References

- [Go Standard Library](https://pkg.go.dev/std)
- [Go Compatibility Promise](https://go.dev/doc/go1compat)
- [Deno Standard Library](https://deno.land/std)
- [Rust `regex` crate — RE2-compatible](https://docs.rs/regex)
- [ring — Cryptographic library](https://docs.rs/ring)
- [Sigstore — used for Ed25519 in RFC-009](https://www.sigstore.dev/)
- [reqwest — HTTP client](https://docs.rs/reqwest)
- [tokio — Async runtime](https://tokio.rs/)
- [RFC-001 — ForgeScript Syntax](./RFC-001-forgescript-syntax.md)
- [RFC-004 — Path & Environment Model](./RFC-004-path-and-env.md)
- [RFC-014 — Release Policy & Versioning](./RFC-014-release-policy.md)