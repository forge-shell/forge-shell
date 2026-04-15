# RFC-012 — ForgeScript REPL & Interactive Shell

| Field          | Value                        |
|----------------|------------------------------|
| Status         | **Draft**                    |
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
completion, history, syntax highlighting, autosuggestions, and inline error
hints — without introducing a different language.

---

## Motivation

A shell is only as good as its interactive experience. bash and zsh are
tolerable because decades of plugins (Oh My Zsh, Prezto, Starship) have
papered over their rough edges. Fish Shell built quality interactive
experience into the shell itself. ForgeScript takes Fish's lesson seriously —
the interactive experience is first-class, not an afterthought.

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

### 2. Syntax — Unified with Scripts

All three invocation forms are valid at the prompt — identical to scripts:

```
# Positional
ls /home/user

# POSIX-style flags
ls /home/user --show_hidden --sort name

# Named typed arguments
ls path: p"/home/user", show_hidden: true, sort: "name"
```

Positional type inference rules (RFC-001 Section 17) apply at the prompt
identically to scripts.

### 3. Tab Completion

Context-aware completion — the REPL knows the type expected at each position:

```
ls /ho<TAB>
→ ls /home/

ls /home/user --so<TAB>
→ ls /home/user --sort

fetch https://api.<TAB>
→ [history-based URL completions]
```

**Completion sources:**

| Context | Completion source |
|---|---|
| First token | Built-in commands + plugin commands + functions |
| `path` parameter | File system — directories and files |
| `url` parameter | History-based URL completions |
| `--flag` | Command's declared flag names |
| Flag value | Flag's declared type — enum variants, booleans etc. |
| ForgeScript keywords | Language keywords |

Plugin completions are declared in the plugin manifest (RFC-005) and loaded
at shell startup.

### 4. Fuzzy Path Completion

Typo-tolerant path completion — similar to fzf:

```
ls /hom/us<TAB>
→ ls /home/user/     # fuzzy matched

cd proj/src<TAB>
→ cd ~/projects/src/  # expanded and completed
```

### 5. History

Persistent history across sessions — stored in
`~/.config/forge/history.db` (SQLite):

| Feature | Description |
|---|---|
| Persistent | Survives shell restarts |
| Searchable | `Ctrl+R` incremental search |
| Deduplication | Consecutive duplicates collapsed |
| Timestamped | Each entry carries timestamp and working directory |
| Filtered | `--no-history` flag suppresses recording for sensitive commands |

```forge
# History search — Ctrl+R
# Shows most recent matching commands inline as ghost text
```

### 6. Syntax Highlighting — Live

Live syntax highlighting as the user types — before pressing Enter:

| Token | Colour |
|---|---|
| Built-in commands | Green |
| Unknown commands | Red — immediate visual feedback |
| `path` literals | Yellow |
| `str` literals | Cyan |
| `int` / `float` literals | Magenta |
| `--flags` | Blue |
| Errors | Red underline |
| Comments | Grey |

Colours follow the terminal's theme — respects `NO_COLOR` environment
variable.

### 7. Autosuggestions

Ghost text suggestions based on history — Fish Shell style:

```
ls /home/user▌░░░░░░░░░░░░░░░░░░
             └─ ghost: --show_hidden (from history)

Press → to accept, Ctrl+F to accept word
```

- Suggestions sourced from command history
- Most recent matching history entry shown
- `→` accepts full suggestion
- `Ctrl+F` accepts next word only

### 8. Inline Error Hints

Type errors and unknown commands shown inline before Enter is pressed:

```
ls /home/user --sort 42▌
                     ^^
                     error: expected str, found int
```

This requires a lightweight incremental parser running on each keystroke —
distinct from the full evaluation pipeline.

### 9. Multi-line Editing

Blocks open naturally at the prompt:

```
$ for file in ls /home/user {
>     echo "{file}"
> }
```

- `{` opens a block — prompt continues with `>` until `}` closes it
- `Escape` cancels multi-line entry
- Multi-line history entries stored and recalled as a unit

### 10. Prompt — `[prompt]` Config

Prompt is configured in RFC-013 `config.toml`:

```toml
[prompt]
theme    = "default"
segments = ["cwd", "git", "execution_time", "exit_code"]
style    = "powerline"   # "powerline" | "plain" | "minimal"
```

**Built-in prompt segments:**

| Segment | Description |
|---|---|
| `cwd` | Current working directory — shortened intelligently |
| `git` | Git branch + status — clean/dirty/ahead/behind |
| `execution_time` | Last command duration — shown if > 2s |
| `exit_code` | Last exit code — shown if non-zero |
| `user` | Current user |
| `host` | Hostname |

**Plugin prompt segments:** `kubernetes`, `aws_profile`, `node_version`,
`python_env` etc. — declared in plugin manifests.

### 11. Built-in REPL Features — Always On

These features are built-in — not plugins:

```toml
[repl]
autosuggestions     = true   # ghost text from history
syntax_highlighting = true   # live highlighting as you type
fuzzy_completion    = true   # typo-tolerant path completion
inline_errors       = true   # type errors before Enter
```

### 12. Key Bindings — `[keybindings]` Config

```toml
[keybindings]
"ctrl+r"     = "history_search"
"ctrl+a"     = "line_start"
"ctrl+e"     = "line_end"
"ctrl+w"     = "delete_word"
"ctrl+l"     = "clear_screen"
"ctrl+c"     = "cancel"
"ctrl+d"     = "exit"
"→"          = "accept_suggestion"
"ctrl+f"     = "accept_suggestion_word"
```

### 13. Output Mode at the REPL

The REPL always knows it's interactive — `OutputMode::RichTerminal` is always
selected. No TTY detection needed at the prompt.

---

## Drawbacks

- **Incremental parser for inline errors adds complexity.** A lightweight
  parser must run on every keystroke — it must be fast enough to feel
  instantaneous. If it can't keep up, inline errors must be disabled or
  debounced.
- **Fuzzy completion can surprise users.** `ls /hom/us` completing to
  `/home/user` is helpful — but if the fuzzy match is wrong, it's
  disorienting. A clear visual indicator of fuzzy vs exact match is required.

---

## Alternatives Considered

### Alternative A — Separate interactive syntax

**Rejected:** Two syntaxes means two mental models. What works at the prompt
must work in a script — this is a core design principle of ForgeScript.

### Alternative B — POSIX-style readline integration

**Rejected:** readline provides history and basic completion but not
autosuggestions, syntax highlighting, or inline error hints. A native REPL
layer gives full control over the interactive experience.

---

## Unresolved Questions

- [ ] Which terminal emulators and colour depths should be officially
      supported?
- [ ] Should the incremental parser for inline errors share code with the
      full parser, or be a separate lightweight implementation?
- [ ] How are plugin completion definitions loaded at startup — eagerly or
      lazily?
- [ ] Should `forge` support a headless REPL mode for testing interactive
      behaviour?

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-repl` | REPL layer — main interactive shell loop |
| `forge-repl/completion` | Tab completion engine |
| `forge-repl/history` | Persistent history — SQLite backend |
| `forge-repl/highlight` | Live syntax highlighting |
| `forge-repl/suggest` | Autosuggestion engine |
| `forge-repl/prompt` | Prompt rendering — segments, themes |
| `forge-repl/parser` | Incremental parser for inline errors |

### Dependencies

- Requires RFC-001 (ForgeScript syntax) — unified syntax at prompt and in scripts
- Requires RFC-003 (built-in commands) — completion sources
- Requires RFC-005 (plugin system) — plugin completion manifests
- Requires RFC-013 (shell config) — `[repl]`, `[prompt]`, `[keybindings]` config

### Milestones

1. Implement basic REPL loop — input, parse, execute, output
2. Implement persistent history — SQLite backend, `Ctrl+R` search
3. Implement tab completion — built-ins, paths, flags
4. Implement fuzzy path completion
5. Implement live syntax highlighting
6. Implement autosuggestions — Fish-style ghost text
7. Implement incremental parser for inline error hints
8. Implement multi-line editing
9. Implement prompt rendering — built-in segments
10. Implement plugin completion loading
11. Integration tests — interactive behaviour on all three platforms

---

## References

- [Fish Shell Design](https://fishshell.com/docs/current/design.html)
- [Nushell REPL](https://github.com/nushell/nushell/tree/main/crates/nu-cli)
- [reedline — Rust REPL library](https://github.com/nushell/reedline)
- [fzf — Fuzzy finder](https://github.com/junegunn/fzf)
- [NO_COLOR standard](https://no-color.org/)
- [RFC-001 — ForgeScript Syntax](./RFC-001-forgescript-syntax.md)
- [RFC-005 — Plugin System](./RFC-005-plugin-system.md)
- [RFC-013 — Shell Configuration Model](./RFC-013-shell-config.md)