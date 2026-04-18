# RFC-016 — Script Directives

| Field         | Value                |
|---------------|----------------------|
| Status        | Draft                |
| Author        | Ajitem Sahasrabuddhe |
| Created       | 2026-04-17           |
| Last Updated  | 2026-04-17           |
| Supersedes    | —                    |
| Superseded By | —                    |

---

## Summary

ForgeScript files may begin with a Unix shebang (`#!/usr/bin/env forge`) for
direct execution, followed by zero or more ForgeScript directives
(`#!forge:key = "value"`) that configure per-file runtime behaviour. This RFC
defines the complete directive syntax, the v1 directive set, lexer and parser
representation, runtime enforcement semantics, and the reserved namespace for
future directives. It supersedes and replaces the informal directive
description in RFC-001 §2.

---

## Motivation

During implementation of `forge-lexer` (Issue #8), the `#!` line handling
surfaced as an unspecified design area. RFC-001 §2 listed a partial directive
set without defining enforcement semantics, the lexer representation, or the
boundary between script directives and plugin manifest concerns. This RFC
closes that gap with a complete, implementable specification.

Key decisions resolved here:

- Script directives use `#!forge:key = "value"` — namespaced to avoid
  conflicts with Unix shebangs and future tooling.
- ABI versioning belongs in `forge-plugin.toml`, not in script directives.
- Plugin entry point declaration belongs in `forge-plugin.toml`, not in
  script directives.
- `version` as a script metadata field is dropped — git tags are the source
  of truth for versioning.
- `sandbox` is deferred — cross-platform enforcement is not achievable in v1.

---

## Design

### 1. Syntax

```
file         ::= [shebang_line] {directive_line} {statement}
shebang_line ::= "#!/" <anything> <newline>        -- line 1 only
directive_line::= "#!forge:" key " = " value <newline>
key          ::= [a-z][a-z0-9-]*
value        ::= quoted_string | unquoted_value
quoted_string::= '"' <utf8 chars excluding newline> '"'
unquoted_value::= <non-whitespace chars excluding newline>
```

**Rules:**
- The Unix shebang (`#!/...`) must be on line 1 if present. It is the only
  `#!` form that does not start with `#!forge:`.
- `#!forge:` directives must appear before any statements. A directive after
  a statement is a parse error.
- Unknown `#!forge:` keys produce a compile-time warning and are otherwise
  ignored — forward compatibility.
- Any `#!` line that is neither a Unix shebang nor a `#!forge:` directive
  is a compile-time warning and is ignored.

### 2. Example

```forge
#!/usr/bin/env forge
#!forge:description = "Deploy script for the production environment"
#!forge:author      = "Ajitem Sahasrabuddhe"
#!forge:min-version = "0.3.0"
#!forge:platform    = "unix"
#!forge:overflow    = "saturate"
#!forge:strict      = true
#!forge:timeout     = "5m"
#!forge:require-env = "DATABASE_URL,API_KEY"

# Script body begins here
let target = $TARGET_ENV
```

### 3. Lexer Representation

The lexer emits `#!` lines as a dedicated token variant:

```rust
/// A line beginning with `#!`. The inner string is the full content
/// after `#!`, trimmed of leading and trailing whitespace.
///
/// Examples:
///   `#!/usr/bin/env forge`     →  ShebangDirective("/usr/bin/env forge")
///   `#!forge:overflow = "saturate"` →  ShebangDirective("forge:overflow = \"saturate\"")
ShebangDirective(String),
```

The lexer does not parse the directive structure — it emits the raw content.
Structural parsing (key/value splitting, validation) is the parser's
responsibility.

### 4. Parser Representation

```rust
pub struct Program {
    pub directives: Vec<Directive>,
    pub stmts: Vec<Stmt>,
}

pub struct Directive {
    pub kind: DirectiveKind,
    pub span: Span,
}

pub enum DirectiveKind {
    /// `#!/path/to/interpreter` — Unix shebang.
    /// Ignored by the executor on all platforms.
    UnixShebang(String),

    /// `#!forge:description = "..."` — human-readable script description.
    Description(String),

    /// `#!forge:author = "..."` — script author.
    Author(String),

    /// `#!forge:min-version = "1.2.0"` — minimum Forge Shell version required.
    MinVersion(semver::Version),

    /// `#!forge:platform = "unix"` — supported platforms.
    Platform(Vec<Platform>),

    /// `#!forge:overflow = "saturate"` — integer overflow behaviour.
    Overflow(OverflowMode),

    /// `#!forge:strict = true` — fail on first error.
    Strict(bool),

    /// `#!forge:timeout = "30s"` — maximum wall-clock execution time.
    Timeout(Duration),

    /// `#!forge:jobs = "4"` — maximum parallel jobs.
    Jobs(JobLimit),

    /// `#!forge:env-file = ".env"` — env file to load before execution.
    EnvFile(String),

    /// `#!forge:require-env = "VAR1,VAR2"` — required environment variables.
    RequireEnv(Vec<String>),

    /// `#!forge:override = "ls"` — built-in command this script overrides.
    Override(String),

    /// An unrecognised `#!forge:` directive — preserved for diagnostics.
    /// Produces a compile-time warning. Does not affect execution.
    Unknown { key: String, value: String },
}

pub enum Platform {
    All,
    Unix,    // Linux + macOS
    Linux,
    MacOs,
    Windows,
}

pub enum OverflowMode {
    Panic,
    Saturate,
    Wrap,
}

pub enum JobLimit {
    Count(u32),
    Auto,  // number of logical CPU cores
}
```

### 5. v1 Directive Reference

| Directive | Type | Default | Runtime enforcement |
|---|---|---|---|
| `description` | string | — | Metadata only — displayed by `forge script info` |
| `author` | string | — | Metadata only |
| `min-version` | semver | — | Hard error if Forge runtime is older |
| `platform` | `all` \| `unix` \| `linux` \| `macos` \| `windows` | `all` | Hard error if current platform not in list |
| `overflow` | `panic` \| `saturate` \| `wrap` | `panic` | Enforced by executor for all integer operations |
| `strict` | `true` \| `false` | `false` | Executor terminates on first non-zero exit code |
| `timeout` | duration (`30s`, `5m`, `1h`) | — | Executor kills script after elapsed wall time |
| `jobs` | integer \| `auto` | `auto` | Executor caps concurrent job spawns |
| `env-file` | path string | — | Loaded and merged into env before first statement |
| `require-env` | comma-separated names | — | Hard error before first statement if any var is unset |
| `override` | built-in command name | — | Resolver substitutes this script for the named built-in |

### 6. Runtime Enforcement Detail

**`min-version`**
```
forge run script.fgs
Error: script requires Forge Shell >= 0.3.0, but this is 0.2.1
       Update with: forge self-update
```

**`platform`**
```
forge run script.fgs   # on Windows
Error: script declares platform = "unix" and cannot run on windows
```

**`require-env`**
```
forge run deploy.fgs
Error: required environment variable DATABASE_URL is not set
       required environment variable API_KEY is not set
       Declared by: #!forge:require-env
```

**`strict`**
When `strict = true`, any command that exits with a non-zero status
immediately terminates the script. Equivalent to `set -e` in bash.

**`timeout`**
The executor starts a timer at the first statement. If wall time exceeds the
declared timeout, the process group is killed and the script exits with
`ExecError::Timeout`.

**`env-file`**
Loaded using the same semantics as RFC-004's `.env` loading. Variables
already in the environment are not overwritten — the file provides defaults
only.

**`override`**
The built-in resolver checks for an `override` directive before dispatching.
A script declaring `#!forge:override = "ls"` shadows the built-in `ls` for
the duration of the session when the script is on `$PATH`.

### 7. Reserved Namespace

The following keys are reserved for future RFCs and must not be used by
plugins or tooling for other purposes:

| Key | Reserved for |
|---|---|
| `agent` | RFC-007 — AI agent layer directives |
| `sandbox` | Future RFC — sandboxed execution model |
| `abi` | Not used in scripts — belongs in `forge-plugin.toml` |
| `plugin` | Not used in scripts — belongs in `forge-plugin.toml` |

### 8. What Does Not Belong in Script Directives

These concerns are explicitly out of scope for `#!forge:` directives:

**ABI versioning** — plugin ABI is declared in `forge-plugin.toml` under the
`[plugin] abi` field. Scripts are not plugins and do not have an ABI.

**Plugin entry point** — declared in `forge-plugin.toml` under
`[plugin] entrypoint`. The `#!plugin:` directive form is not used.

**Script version** — versioned via git tags. A version number in the file
creates two sources of truth that will inevitably drift.

---

## Drawbacks

- `#!forge:` prefix adds verbosity compared to a simpler `#!key = value`
  form. The namespace is necessary to avoid conflicts with other tools that
  parse `#!` lines (e.g. GitHub's Linguist).
- Directives must precede all statements, which means a developer cannot
  conditionally set overflow mode mid-script. This is intentional — directives
  are static metadata, not runtime configuration.

---

## Alternatives Considered

### Alternative A — Frontmatter block (YAML/TOML header)

```forge
---
description: Deploy script
min-version: 0.3.0
---
let x = 42
```

**Rejected because:** Requires a separate parser for the frontmatter format,
adds visual noise, and is unfamiliar in a shell context. `#!forge:` lines are
parseable by the existing lexer with minimal additions.

### Alternative B — No namespace (`#!key = value`)

```forge
#!min-version = "0.3.0"
```

**Rejected because:** Collides with Unix shebangs (`#!/usr/bin/env forge`
starts with `#!/`) and creates ambiguity for tools that inspect `#!` lines.
The `forge:` namespace makes ForgeScript directives unambiguous.

### Alternative C — `version` as a script metadata field

**Rejected because:** Git tags are the source of truth for versioning.
Embedding a version in the script file creates two sources of truth. When
they diverge — and they will — the result is confusion, not clarity.

### Alternative D — `sandbox` directive for restricting script capabilities

**Rejected because:** Cross-platform sandbox enforcement requires three
different OS mechanisms (seccomp+namespaces on Linux, Sandbox.framework on
macOS, AppContainer on Windows). No consistent behaviour is achievable in v1.
Deferred to a future RFC.

---

## Unresolved Questions

- **UQ-1 — RESOLVED:** Unknown directives are always a warning, never a hard
  error. No `--strict` flag for directive validation.
- **UQ-2 — RESOLVED:** ABI versioning belongs in `forge-plugin.toml`, not in
  script directives. `#!abi:` is not a valid directive form.
- **UQ-3 — RESOLVED:** `agent` key reserved in the directive namespace.
  Formal definition deferred to RFC-007.

---

## Implementation Plan

### Affected Crates

- `forge-lexer` — `ShebangDirective(String)` token variant — **Issue #32**
- `forge-ast` — `Directive`, `DirectiveKind`, and supporting enums; `directives:
  Vec<Directive>` on `Program` — **Issue #33**
- `forge-parser` — collect leading directives, validate structure, emit
  `ParseError::DirectiveAfterStatement` — **Issue #34**
- `forge-exec` — enforce `min-version`, `platform`, `strict`, `timeout`,
  `jobs`, `env-file`, `require-env`, `override` — **Issue #35** (v0.3)

### Dependencies

- RFC-001 §2 — superseded by this RFC for directive specification
- RFC-004 — `env-file` loading semantics follow RFC-004's `.env` model
- RFC-005 — confirms `forge-plugin.toml` owns `abi` and `entrypoint`
- RFC-007 — `agent` key definition deferred here
- RFC-014 — `min-version` enforcement ties to the release versioning model

### Milestones

Issues #32, #33, #34 target **v0.2 — Pipeline**.
Issue #35 targets **v0.3 — Execution**.

---

## References

- [Unix shebang — Wikipedia](https://en.wikipedia.org/wiki/Shebang_(Unix))
- [Rust edition system](https://doc.rust-lang.org/edition-guide/)
- [Go minimum version selection](https://research.swtch.com/vgo-mvs)
- [RFC-001 — ForgeScript Language Syntax & Type System](https://github.com/forge-shell/forge-shell/blob/main/docs/rfc-001-forgescript-syntax.md)
- [RFC-004 — Path & Environment Variable Model](https://github.com/forge-shell/forge-shell/blob/main/docs/rfc-004-path-and-env.md)
- [RFC-005 — Plugin System & WASM Capability Model](https://github.com/forge-shell/forge-shell/blob/main/docs/rfc-005-plugin-system.md)
- [RFC-007 — AI Agent Layer & MCP Protocol Integration](https://github.com/forge-shell/forge-shell/blob/main/docs/rfc-007-ai-agent-layer.md)
- [RFC-014 — Release Policy & Versioning](https://github.com/forge-shell/forge-shell/blob/main/docs/rfc-014-release-policy.md)
