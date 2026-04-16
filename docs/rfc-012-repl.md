# RFC-012 — ForgeScript REPL & Interactive Shell

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

This RFC defines the ForgeScript interactive shell — the REPL layer that sits
between user keystrokes and the ForgeScript parser. The REPL uses identical
ForgeScript syntax to `.fgs` scripts — there is no separate interactive mode.
The REPL layer adds quality-of-life features: tab completion, fuzzy path
completion, history, syntax highlighting, autosuggestions, inline error hints,
and a headless mode for testing. Plugin completions load in the background
after the shell becomes interactive, with a disk cache for subsequent starts.

---

## Motivation

A shell is only as good as its interactive experience. bash and zsh are
tolerable because decades of plugins have papered over their rough edges.
Fish Shell built quality interactive experience into the shell itself.
ForgeScript takes Fish's lesson seriously — the interactive experience is
first-class, not an afterthought.

The key design decision: **one syntax, everywhere**. What works at the prompt
works in scripts. The REPL layer is purely additive — it enhances the
interactive experience without introducing a parallel language.

---

## Design

### 1. Architecture

```
User keystrokes
      ↓
 REPL Layer         — completion, history, highlighting, suggestions
      ↓
ForgeScript Parser  — identical to script parsing
      ↓
Evaluation Pipeline — identical to script execution
```

The REPL layer is not a separate parser. It is a quality-of-life wrapper
around the standard ForgeScript evaluation pipeline.

---

### 2. Syntax — Unified with Scripts

All three invocation forms are valid at the prompt — identical to scripts:

```forge
ls /home/user
ls /home/user --show_hidden --sort name
ls path: p"/home/user", show_hidden: true, sort: "name"
```

Positional type inference rules (RFC-001 Section 17) apply at the prompt
identically to scripts. Output mode at the REPL is always `RichTerminal`.

---

### 3. Terminal Support

**Colour depth detection — progressive enhancement:**

```
COLORTERM=truecolor or 24bit  →  true colour — full rich rendering
TERM=xterm-256color            →  256 colour — good rendering
TERM=xterm or similar          →  16 colour — basic rendering
NO_COLOR set                   →  no colour — plain text only
TERM=dumb or no TTY            →  plain text only — CI environments
```

`NO_COLOR` is respected unconditionally — no exceptions.

**Tier 1 — must work perfectly:**

| Terminal | Platform |
|---|---|
| Ghostty | macOS, Linux |
| WezTerm | All three |
| Windows Terminal | Windows |
| Alacritty | All three |
| VS Code integrated terminal | All three |
| IntelliJ integrated terminal | All three |
| iTerm2 | macOS |
| GNOME Terminal | Linux |
| Warp | macOS, Linux |
| SSH sessions | All three |

**Tier 2 — best effort, community-tested:**

| Terminal | Platform |
|---|---|
| Kitty | macOS, Linux |
| tmux + any Tier 1 terminal | All three |
| Zellij + any Tier 1 terminal | All three |
| foot | Linux (Wayland) |

---

### 4. Tab Completion

Context-aware completion — the REPL knows the type expected at each position.

**Completion sources:**

| Context | Source |
|---|---|
| First token | Built-in commands + plugin commands + autoloaded functions |
| `path` parameter | File system — directories and files |
| `url` parameter | History-based URL completions |
| `--flag` | Command's declared flag names |
| Flag value | Flag's declared type — enum variants, booleans |
| Plugin commands | Plugin completion definitions |

---

### 5. Fuzzy Path Completion

Typo-tolerant path completion:

```
ls /hom/us<TAB>  →  ls /home/user/
cd proj/src<TAB> →  cd ~/projects/src/
```

---

### 6. Plugin Completion Loading — Background Eager with Cache

Plugin completions load in the background after the shell becomes interactive.

**Startup sequence:**

```
forge starts → shell interactive immediately
    ↓ background thread
Plugin completions load asynchronously (~50-100ms)
    ↓
Cache written to disk
```

**Cache location:**

| Platform | Path |
|---|---|
| Linux | `~/.cache/forge/completions/` |
| macOS | `~/Library/Caches/forge/completions/` |
| Windows | `%LOCALAPPDATA%\forge\cache\completions\` |

**Cache invalidation:** Plugin version change → refresh. No change → instant load from cache.

**Graceful fallback:**

```
$ forge-kubectl <TAB>
  Loading completions... (done in 80ms)
  pods    services    deployments    namespaces
```

---

### 7. History

Persistent history — SQLite backend.

| Platform | Path |
|---|---|
| Linux | `~/.local/share/forge/history.db` |
| macOS | `~/Library/Application Support/forge/history.db` |
| Windows | `%APPDATA%\forge\history.db` |

Features: persistent across restarts, `Ctrl+R` incremental search,
consecutive duplicate deduplication, timestamps + working directory per entry,
`--no-history` flag to suppress recording.

```toml
[repl]
history_size = 10000
```

---

### 8. Syntax Highlighting — Live

| Token | Colour |
|---|---|
| Built-in commands | Green |
| Unknown commands | Red |
| `path` literals | Yellow |
| `str` literals | Cyan |
| `int` / `float` literals | Magenta |
| `--flags` | Blue |
| Errors | Red underline |
| Comments | Grey |

`NO_COLOR` suppresses all highlighting.

---

### 9. Autosuggestions

Ghost text suggestions from history — Fish Shell style:

```
ls /home/user▌░░░░░░░░░░░░░
             └─ ghost: --show_hidden

→  accept full    Ctrl+F  accept word
```

---

### 10. Inline Error Hints — Incremental Parser

**Implementation:** `forge-lang/parser` in incremental mode — shared with
the full pipeline. No separate lightweight parser.

```forge
ls /home/user --sort 42▌
                     ^^ error: expected str, found int
```

- Shared parser with error recovery — produces partial AST from incomplete input
- Runs in a separate background thread — never blocks keystrokes
- Debounced — 150ms after last keystroke
- Precedent: rust-analyzer, TypeScript LS, clangd

---

### 11. Multi-line Editing

```
$ for file in ls /home/user {
>     echo "{file}"
> }
```

`{` opens block → `>` prompt → `}` closes. `Escape` cancels. Multi-line
entries stored and recalled as a unit from history.

---

### 12. Prompt Config

```toml
[prompt]
theme    = "default"
segments = ["cwd", "git", "execution_time", "exit_code"]
style    = "powerline"   # "powerline" | "plain" | "minimal"
```

**Built-in segments:** `cwd`, `git`, `execution_time`, `exit_code`, `user`, `host`

Plugin segments — `kubernetes`, `aws_profile`, `node_version` etc. — declared
in plugin manifests (RFC-005).

---

### 13. Key Bindings Config

```toml
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
```

---

### 14. Headless REPL Mode

For testing REPL behaviour in CI without a real TTY.

```bash
echo "ls /home/user\nexit" | forge repl --headless
forge repl --headless < test-session.txt
echo "ls /home/<TAB>\nexit" | forge repl --headless
```

- `<TAB>` literal triggers tab-completion simulation
- No ANSI codes — plain text output always
- Deterministic — suitable for CI assertions
- Exit code reflects last command result

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | Terminal support matrix | Progressive enhancement + `NO_COLOR` + 10 Tier 1 terminals |
| UQ-2 | Incremental parser | Shared parser with error recovery — 150ms debounce |
| UQ-3 | Plugin completion loading | Background eager + disk cache |
| UQ-4 | Headless REPL mode | `forge repl --headless` |

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-repl` | Main interactive shell loop |
| `forge-repl/completion` | Tab completion engine |
| `forge-repl/history` | SQLite history backend |
| `forge-repl/highlight` | Live syntax highlighting |
| `forge-repl/suggest` | Ghost text autosuggestions |
| `forge-repl/prompt` | Prompt rendering |
| `forge-lang/parser` | Incremental mode for inline errors |

### Dependencies

- RFC-001 (ForgeScript syntax), RFC-003 (built-ins), RFC-005 (plugins), RFC-013 (config)

### Milestones

1. Basic REPL loop — input, parse, execute, output
2. Persistent history — SQLite, `Ctrl+R`
3. Tab completion — built-ins, paths, flags
4. Fuzzy path completion
5. Live syntax highlighting
6. Autosuggestions — ghost text
7. Incremental parser — 150ms debounce, background thread
8. Inline error hints
9. Multi-line editing
10. Prompt rendering — built-in segments
11. Background plugin completion loading + disk cache
12. Headless mode — `forge repl --headless`
13. Integration tests — all three platforms
14. Headless CI tests

---

## References

- [Fish Shell Design](https://fishshell.com/docs/current/design.html)
- [reedline — Rust REPL library](https://github.com/nushell/reedline)
- [NO_COLOR standard](https://no-color.org/)
- [Ghostty terminal](https://ghostty.org/)
- [WezTerm](https://wezfurlong.org/wezterm/)
- [RFC-001 — ForgeScript Syntax](./RFC-001-forgescript-syntax.md)
- [RFC-005 — Plugin System](./RFC-005-plugin-system.md)
- [RFC-013 — Shell Configuration Model](./RFC-013-shell-config.md)