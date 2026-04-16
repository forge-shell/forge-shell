# RFC-013 — Shell Configuration Model

| Field          | Value                        |
|----------------|------------------------------|
| Status         | **In Review**                |
| Author         | Ajitem Sahasrabuddhe         |
| Created        | 2026-04-15                   |
| Last Updated   | 2026-04-15                   |
| Supersedes     | —                            |
| Superseded By  | —                            |

---

## Summary

This RFC defines the Forge Shell configuration model — the `config.toml` file
that controls personal shell behaviour: environment setup, plugin loading,
prompt rendering, key bindings, aliases, and PATH management. Configuration
is user-level only — no project-level shell config exists. Per-directory
environment loading is handled by `.env` files (RFC-004). A separate minimal
`forge.toml` in the working directory controls formatter style only.

---

## Motivation

bash and zsh configuration is a patchwork of `.bashrc`, `.bash_profile`,
`.zshrc`, `.zprofile`, sourced files, and Oh My Zsh plugins. The loading
order is confusing, platform-specific, and poorly documented.

Forge Shell replaces this with a single `config.toml` — declarative, typed,
with a well-defined loading order. Configuration is personal shell preferences.
Project-level behaviour belongs in `.env` files and project tooling.

---

## Design

### 1. File Locations

**User config — personal shell preferences:**

| Platform | Path |
|---|---|
| Linux | `~/.config/forge/config.toml` |
| macOS | `~/.config/forge/config.toml` |
| Windows | `%APPDATA%\forge\config.toml` |

**Project formatter config — working directory only:**

```
./.forge.toml    # controls forge fmt style for this directory
```

No project-level shell config exists. The shell is a system utility — it
does not know about "projects". Per-directory env loading uses `.env` files
with `forge::env::load()` (RFC-004).

---

### 2. Full `config.toml` Specification

```toml
# ── Environment Variables ────────────────────
[env]
EDITOR    = "nvim"
GOPATH    = "~/go"
JAVA_HOME = "/usr/lib/jvm/java-17"

# Platform-specific env vars
[env.macos]
HOMEBREW_PREFIX = "/opt/homebrew"

[env.linux]
PACKAGE_MANAGER = "apt"

[env.windows]
USERPROFILE = "%USERPROFILE%"

# ── PATH Management ───────────────────────────
[path]
prepend = ["~/.local/bin", "/usr/local/go/bin", "~/.cargo/bin"]
append  = []

[path.macos]
prepend = ["/opt/homebrew/bin"]

# ── Aliases ───────────────────────────────────
[aliases]
ll  = "ls --show_hidden --long"
la  = "ls --show_hidden"
gs  = "git status"
gp  = "git push"

# Alias conflict resolution — explicit precedence
[aliases.precedence]
k  = "forge-kubectl"    # forge-kubectl's k wins over forge-k9s's k
gs = "forge-git"        # forge-git's gs wins over any other plugin

# ── Plugins ───────────────────────────────────
[plugins]
installed = ["forge-git", "forge-docker", "forge-kubectl"]
local     = ["~/.config/forge/plugins/"]   # local plugin directory

# ── Functions ─────────────────────────────────
[functions]
autoload = "~/.config/forge/functions/"   # .fgs files auto-loaded as shell functions

# ── Prompt ────────────────────────────────────
[prompt]
theme    = "default"
style    = "powerline"   # "powerline" | "plain" | "minimal"
segments = ["cwd", "git", "execution_time", "exit_code"]

# ── REPL ──────────────────────────────────────
[repl]
autosuggestions     = true
syntax_highlighting = true
fuzzy_completion    = true
inline_errors       = true
history_size        = 10000

# ── Key Bindings ──────────────────────────────
[keybindings]
"ctrl+r" = "history_search"
"ctrl+a" = "line_start"
"ctrl+e" = "line_end"
"ctrl+w" = "delete_word"
"ctrl+l" = "clear_screen"
"ctrl+c" = "cancel"
"ctrl+d" = "exit"
"→"      = "accept_suggestion"
"ctrl+f" = "accept_suggestion_word"

# ── Built-in Overrides ────────────────────────
# Interactive shell only — scripts are hermetic
[overrides]
ls   = "system"                  # use system ls
grep = "/usr/local/bin/rg"       # use ripgrep

# ── Updates ───────────────────────────────────
[updates]
auto    = true
channel = "stable"   # "stable" | "nightly"

# ── Telemetry ─────────────────────────────────
[telemetry]
enabled    = false                              # opt-in — default false
install_id = "a1b2c3d4-e5f6-7890-abcd-ef1234"  # generated on opt-in
rotated_at = "2026-04-01"                       # rotated monthly

# ── Includes ──────────────────────────────────
[includes]
optional = ["~/.config/forge/local.toml"]   # loaded if exists, no error if missing
```

---

### 3. `forge.toml` — Formatter Config Only

A minimal file in the working directory — controls `forge fmt` style for
this directory. Committed to version control. Nothing else.

```toml
# forge.toml — working directory
[fmt]
argument_style = "named"   # "preserve" | "named" | "positional"
```

**`forge fmt` argument styles:**

| Style | Behaviour |
|---|---|
| `"preserve"` | Fix whitespace only — never change invocation form. **System default.** |
| `"named"` | Normalise to named typed arguments |
| `"positional"` | Normalise to positional + `--flags` |

**Configuration cascade** (highest priority wins):

```
forge.toml in working directory
    ↑ overrides
~/.config/forge/config.toml [fmt]
    ↑ overrides
System default: "preserve"
```

**`forge fmt` is otherwise opinionated — one output, no other options.**
Analogous to `gofmt` — fixes whitespace, indentation, and structure.

```bash
forge fmt script.fgs           # format in place
forge fmt --check script.fgs   # exit non-zero if not formatted
forge fmt --print script.fgs   # print to stdout
```

---

### 4. Platform Sections

Every top-level section supports platform subsections:

```toml
[env]
EDITOR = "nvim"        # all platforms

[env.macos]
EDITOR = "nano"        # macOS only — overrides above

[env.linux]
EDITOR = "vim"         # Linux only

[env.windows]
EDITOR = "notepad"     # Windows only
```

Platform sections are merged — platform-specific section overrides the base
section for the current platform. Other platforms' sections are ignored.

---

### 5. Loading Order

```
1. User config          ~/.config/forge/config.toml
2. Optional includes    files in [includes.optional] — if they exist
```

No project-level config. No `.forge-env` script. Per-directory environment
loading via RFC-004's explicit `.env` file API.

For env vars and PATH — later entries override earlier ones.
For lists (plugins, aliases) — merged, not overridden.

---

### 6. Schema Versioning — Warn on Unknown Keys

```toml
forge_config_version = "1.1"   # optional — helps with diagnostics
```

| Scenario | Behaviour |
|---|---|
| Unknown key in config | `warning[C001]` — startup continues |
| Missing key | Silent — uses default |
| `forge_config_version` > binary version | Single startup warning |

Unknown keys are never hard errors. Typos are surfaced. Upgrades never break.

---

### 7. Alias System — Two-Tier

**Short aliases** — declared by plugins, conflict-managed:

```toml
# forge-kubectl plugin manifest
[aliases]
k    = "kubectl-get"
pods = "kubectl-pods"
```

**Namespaced aliases** — automatically registered, never conflict:

```
kubectl:k      → kubectl-get     (always available)
kubectl:pods   → kubectl-pods    (always available)
k9s:k          → k9s-start       (always available)
```

**Conflict resolution — install time:**

```
⚠️  Alias conflict: 'k' declared by forge-kubectl and forge-k9s
  [e]dit config  [s]kip alias  [q]uit
```

Resolved via `[aliases.precedence]` in `config.toml`.

**Precedence order:**

```
1. User-defined [aliases]         — always win
2. [aliases.precedence]           — explicit resolution
3. First-installed plugin         — fallback
4. Namespaced form (plugin:alias) — always available, never conflicts
```

---

### 8. Built-in Overrides

```toml
[overrides]
ls   = "system"               # delegate to OS $PATH
grep = "/usr/local/bin/rg"    # delegate to specific binary
```

**Hermeticity rule:** Config overrides apply to the interactive shell only.
`.fgs` scripts are hermetic — config overrides never affect script execution.

---

### 9. `forge config` CLI

```bash
forge config get env.EDITOR          # print a config value
forge config set env.EDITOR nvim     # modify config.toml in place
forge config list                    # print full resolved config
forge config validate                # validate config.toml syntax
forge config init                    # generate a default config.toml
forge config migrate                 # migrate from .bashrc/.zshrc
```

**`forge config set` modifies `config.toml` in place** — preserving comments
and key order. Consistent with Go's `go env -w` model.

---

## Drawbacks

- **TOML has limitations for complex logic.** Per-directory env loading
  requires ForgeScript's `forge::env::load()` API rather than config
  declarations. This is intentional — config is declarative, logic is code.
- **No project shell config.** Developers coming from direnv may expect
  directory-level shell configuration. The `.env` file approach covers the
  same use case more explicitly.

---

## Alternatives Considered

### Alternative A — Project-level shell config

**Rejected:** The shell is a system utility — no other shell has per-project
config. Per-directory env loading via `.env` files (RFC-004) covers the
legitimate use cases without making the shell project-aware.

### Alternative B — `.fgs` file as config

**Rejected:** A ForgeScript file as config allows arbitrary code execution at
startup — a security concern. Declarative TOML separates configuration from
code cleanly.

### Alternative C — JSON config

**Rejected:** TOML is more human-readable, supports comments, and is the
established convention in the Rust ecosystem (Cargo.toml).

### Alternative D — Configurable `forge fmt` styles (multiple options)

**Rejected in part:** `forge fmt` is opinionated about everything except
argument style. The `argument_style` option exists because three equivalent
invocation forms are all valid ForgeScript — the formatter must make a choice
about which to produce. The system default `"preserve"` means teams that
don't care get no surprises.

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | Project-level config sections | No project config — user config only + `forge.toml` for fmt |
| UQ-2 | Config schema versioning | Warn on unknown keys — never hard error |
| UQ-3 | `forge config set` — in-place or override | In-place — comments preserved |
| UQ-4 | Conflicting plugin aliases | Two-tier — short with conflict management + namespaced long form |

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-config` | Config file parsing, validation, platform section merging |
| `forge-config/schema` | Schema versioning, unknown key warnings |
| `forge-config/aliases` | Two-tier alias resolution, conflict detection |
| `forge-cli/config` | `forge config` subcommands |
| `forge-fmt` | `forge fmt` — argument style normalisation |

### Dependencies

- RFC-004 (path type, env model) — `[path]` and `[env]` sections
- RFC-005 (plugin system) — `[plugins]` section, alias conflict detection
- RFC-011 (forge migrate) — `forge config migrate` subcommand
- RFC-012 (REPL) — `[repl]`, `[prompt]`, `[keybindings]` sections

### Milestones

1. Define `config.toml` schema — all sections, types, defaults
2. Implement `forge-config` parser and validator
3. Implement platform section resolution — `[env.macos]` etc.
4. Implement schema versioning — unknown key warnings
5. Implement config loading order — user → optional includes
6. Implement `forge config` CLI — `get`, `set` (in-place), `list`, `validate`, `init`
7. Implement `forge config migrate` — extract from `.bashrc`/`.zshrc`
8. Implement two-tier alias system — short aliases + namespaced long form
9. Implement alias conflict detection at plugin install time
10. Implement `forge fmt` argument style normalisation
11. Implement `forge.toml` resolution — working directory formatter config
12. Integration tests on ubuntu-latest, macos-latest, windows-latest

---

## References

- [Cargo.toml reference](https://doc.rust-lang.org/cargo/reference/manifest.html)
- [direnv configuration](https://direnv.net/man/direnv.toml.1.html)
- [TOML specification](https://toml.io/en/)
- [gofmt](https://pkg.go.dev/cmd/gofmt)
- [RFC-004 — Path Type & Environment Variable Model](./RFC-004-path-and-env.md)
- [RFC-011 — forge migrate](./RFC-011-forge-migrate.md)
- [RFC-012 — REPL & Interactive Shell](./RFC-012-repl.md)