# RFC-011 — `forge migrate` — Bash to ForgeScript Migration Tooling

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

`forge migrate` is a first-class CLI tool that mechanically translates bash
and zsh scripts — including `.bashrc`, `.zshrc`, and Oh My Zsh configurations
— into ForgeScript equivalents. It targets bash 4+ as its primary dialect,
handles common zsh patterns and Oh My Zsh constructs, supports both batch
and interactive migration modes, and follows sourced files with developer
confirmation. ForgeScript is not a superset of bash — migration is a
translation, not a rename.

---

## Motivation

ForgeScript is a clean-slate language. It intentionally breaks from bash
conventions — typed variables, named arguments, `Result` error handling. This
means existing bash scripts cannot simply be renamed to `.fgs`. The migration
cost is real and must be addressed head-on.

`forge migrate` is the answer. Rather than pretending compatibility exists
where it doesn't, it provides an honest, high-quality translation tool that
does the mechanical work and clearly flags what requires human judgement.

The precedent is strong: Go's `go fix`, Python's `2to3`, and TypeScript's
migration guide all take this approach. Honest tooling beats leaky
compatibility layers.

---

## Design

### 1. Target Dialects

**Primary target: bash 4+**

bash 4+ covers the constructs developers actually use:
- Associative arrays (`declare -A`)
- `[[ ]]` conditionals
- Process substitution `<(cmd)`
- `mapfile` / `readarray`
- String manipulation operators
- All standard POSIX constructs

**POSIX sh awareness:** If a script uses only POSIX syntax, `forge migrate`
recognises it and notes it's portable. bash-isms are only flagged when
present.

**bash 5+:** Features migrated where a ForgeScript equivalent exists, flagged
where not. Not a primary target.

**zsh:** Oh My Zsh constructs fully supported. Common zsh patterns handled.
Raw zsh-specific syntax flagged with suggestions — see Section 6.

---

### 2. Invocation

```bash
forge migrate script.sh                    # migrate single script
forge migrate ~/.bashrc                    # migrate shell config
forge migrate ~/.zshrc --oh-my-zsh        # zsh with Oh My Zsh awareness
forge migrate --dir ./scripts/             # migrate directory
forge migrate --dry-run script.sh          # show changes, write nothing
forge migrate script.sh --diff            # show diff after migration
forge migrate script.sh --undo            # revert to backup
forge migrate ~/.bashrc --interactive     # step-by-step review
forge migrate script.sh --output out.fgs  # specify output path
```

---

### 3. Migration Modes

#### Batch Mode (default)

Writes migrated files immediately. Original always backed up as `filename.bak`.

```bash
forge migrate deploy.sh

✓ deploy.fgs written
✓ .gitignore updated
✓ deploy.sh.bak created (backup)

forge migrate deploy.sh --diff    # show what changed
forge migrate deploy.sh --undo    # revert to original
forge migrate deploy.sh --dry-run # show changes, write nothing
```

#### Interactive Mode (`--interactive`)

Shows each change one at a time — developer confirms, skips, or edits.
Recommended for complex configs where developers want to understand each change.

```bash
forge migrate ~/.bashrc --interactive

Change 1 of 23 — Variable syntax
  - NAME="forge"
  + let name = "forge"
  [a]ccept  [s]kip  [e]dit  [q]uit
```

Per-change options:
- `a` — accept and apply
- `s` — skip — leave original construct
- `e` — open in `$EDITOR` for manual edit
- `q` — quit — write accepted changes so far, stop

---

### 4. What `forge migrate` Translates Automatically

#### Variables

```bash
NAME="forge"
echo $NAME
```
```forge
let name = "forge"
echo "{name}"
```

#### PATH Manipulation

```bash
export PATH="$HOME/.local/bin:$PATH"
```
```toml
[path]
prepend = ["~/.local/bin"]
```

#### Conditionals

```bash
if [ $count -gt 0 ]; then
    echo "non-zero"
fi
```
```forge
if count > 0 {
    echo "non-zero"
}
```

#### Functions

```bash
mkcd() {
    mkdir -p "$1"
    cd "$1"
}
```
```forge
# ~/.config/forge/functions/mkcd.fgs
fn mkcd(path: path) -> Result<(), CommandError> {
    mkdir path, parents: true
    cd path
}
```

#### Aliases

```bash
alias ll="ls -la"
```
```toml
[aliases]
ll = "ls --show_hidden --long"
```

#### Environment Variables

```bash
export EDITOR="nvim"
export GOPATH="$HOME/go"
```
```toml
[env]
EDITOR = "nvim"
GOPATH = "~/go"
```

#### Loops

```bash
for file in /etc/forge/*.conf; do
    echo "$file"
done
```
```forge
for file in find(p"/etc/forge", pattern: "*.conf") {
    echo "{file}"
}
```

#### Error Handling

```bash
cp file.txt /dest || echo "failed"
```
```forge
match cp(p"file.txt", p"/dest") {
    Ok(_)  => {}
    Err(e) => echo "failed: {e}"
}
```

#### curl / wget

```bash
curl -X POST https://api.example.com/data
wget https://example.com/file.tar.gz
```
```forge
fetch url: u"https://api.example.com/data", method: "POST"
fetch url: u"https://example.com/file.tar.gz", output: p"./file.tar.gz"
```

#### Associative Arrays (bash 4+)

```bash
declare -A map
map["key"]="value"
```
```forge
let map = { "key": "value" }
echo "{map["key"]}"
```

---

### 5. Sourced Scripts — Follow with Confirmation

When `forge migrate` encounters `source` statements, it detects the sourced
files and asks the developer which to migrate.

```bash
forge migrate ~/.bashrc

Detected sourced files:
  ~/.aliases              (user file — migrate recommended)
  ~/.functions            (user file — migrate recommended)
  /etc/profile.d/nvm.sh  (system file — skip recommended)
  ~/.fzf.bash             (user file — migrate recommended)

  [y] migrate all user-owned files
  [n] skip all — flag source statements for manual review
  [s] select individually
```

**System file detection heuristics:**

| Path pattern | Recommendation |
|---|---|
| `/etc/`, `/usr/`, `/opt/` | Skip — system file |
| `~/`, `./`, relative path | Migrate — user-owned |
| File not found | Flag with note |

**Output — correct ForgeScript imports generated:**

```forge
import ./aliases     # was: source ~/.aliases
import ./functions   # was: source ~/.functions
# source /etc/profile.d/nvm.sh — skipped (system file, manual review required)
```

---

### 6. zsh-Specific Handling

#### Oh My Zsh Constructs (fully supported)

| Construct | Migration |
|---|---|
| `plugins=(git docker...)` | `config.toml [plugins]` + install suggestion |
| `ZSH_THEME="..."` | `config.toml [prompt] theme` |
| `ZSH_CUSTOM` directory | `config.toml [plugins] local` |
| Powerlevel10k segments | `config.toml [prompt] segments` |
| `zsh-autosuggestions` | `config.toml [repl] autosuggestions = true` |
| `zsh-syntax-highlighting` | `config.toml [repl] syntax_highlighting = true` |

#### Common zsh Patterns (handled)

| Construct | Migration |
|---|---|
| `setopt` / `unsetopt` | → `config.toml [repl]` settings |
| `autoload -Uz compinit` | → `config.toml [completions]` |
| `bindkey` | → `config.toml [keybindings]` |
| `zstyle` completion config | → `config.toml [completions]` where possible, flag otherwise |

#### zsh-Specific Syntax (flagged with suggestions)

| Construct | Suggestion |
|---|---|
| `precmd` / `preexec` hooks | No direct equivalent — REPL hooks planned post-v1 |
| Glob qualifiers `*(.)` | Use `find` with typed arguments |
| `zparseopts` | Use ForgeScript named arguments |
| `zmodload` | Check forge-shell.dev/plugins for equivalent |

---

### 7. What `forge migrate` Flags for Human Review

#### `eval` patterns

```
# WARNING: eval pattern detected — manual migration required
# Original: eval "$(rbenv init -)"
# Suggestion: check forge-shell.dev/plugins for forge-rbenv
# Fallback: use forge::process::capture() to run rbenv and parse output
```

#### Process substitution

```
# WARNING: process substitution detected — manual migration required
# Original: diff <(ls dir1) <(ls dir2)
# Suggestion: use spawn { } and capture output, then pass to diff
```

#### Here-documents

```
# WARNING: heredoc detected — manual migration required
# Suggestion: use multiline string literal or forge::fs::write()
```

---

### 8. Generated `.gitignore`

```gitignore
# ForgeScript — generated by forge migrate
.env.local
.env.*.local
.forge-env.local
*.fgs.bak
```

---

### 9. Migration Report

```
forge migrate report
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 Translated automatically    42 constructs
 Flagged for review           5 constructs
 Sourced files migrated       3 files
 Output files                 4 .fgs files
                              1 config.toml
                              1 .gitignore
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 Run: forge check ./scripts/ to validate
```

---

## Drawbacks

- **Translation is never perfect.** bash has too many edge cases to handle
  exhaustively. `forge migrate` covers the common 80% — the remaining 20%
  requires human review.
- **Oh My Zsh plugin equivalents may not exist at launch.** `forge migrate`
  can suggest `forge-git` but if the plugin isn't in the registry yet, the
  developer hits a wall.
- **Sourced system files are skipped.** Scripts that depend heavily on
  `/etc/profile.d/` will need more manual work.

---

## Alternatives Considered

### Alternative A — Bash compatibility layer (superset model)
**Rejected:** ForgeScript is not a superset of bash. Honest migration tooling
is better than fake compatibility.

### Alternative B — No migration tooling
**Rejected:** Migration friction is a real adoption barrier.

### Alternative C — Interactive mode only
**Rejected:** Interactive mode for a 500-line `.bashrc` with 80 mechanical
changes is tedious. Batch default with interactive opt-in serves both use
cases correctly.

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | Target bash dialect | bash 4+ primary, POSIX sh awareness |
| UQ-2 | zsh syntax beyond Oh My Zsh | Common patterns + Oh My Zsh — flag the rest |
| UQ-3 | Interactive vs batch | Batch default, `--interactive` opt-in |
| UQ-4 | Sourced scripts | Follow with confirmation — system files skipped |

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-migrate` | Main CLI — modes, report, gitignore |
| `forge-migrate/parser` | bash 4+ / POSIX sh / zsh AST parser |
| `forge-migrate/translator` | AST → ForgeScript translation rules |
| `forge-migrate/omz` | Oh My Zsh awareness layer |
| `forge-migrate/source` | Sourced file detection and confirmation |
| `forge-migrate/interactive` | Interactive mode UI |
| `forge-migrate/report` | Migration report generation |

### Dependencies

- Requires RFC-001 (ForgeScript syntax) — translation target
- Requires RFC-013 (Shell Configuration Model) — `config.toml` output format
- RFC-009 (Plugin Registry) — plugin suggestions in migration output

### Milestones

1. Implement bash 4+ AST parser — variables, functions, conditionals, loops
2. Implement POSIX sh detection and awareness
3. Implement translation rules — variables, PATH, aliases, functions, curl/wget
4. Implement associative array migration
5. Implement `source` detection and follow-with-confirmation flow
6. Implement `eval` and process substitution flagging
7. Implement Oh My Zsh awareness layer
8. Implement common zsh pattern handling — setopt, bindkey, autoload
9. Implement zsh-specific flagging — zparseopts, glob qualifiers, hooks
10. Implement batch mode — backup, diff, undo, dry-run
11. Implement interactive mode — per-change accept/skip/edit/quit
12. Implement `.gitignore` generation
13. Implement migration report
14. Integration tests — real-world `.bashrc` and `.zshrc` migration

---

## References

- [Go fix tool](https://pkg.go.dev/cmd/fix)
- [Python 2to3](https://docs.python.org/3/library/2to3.html)
- [Oh My Zsh](https://ohmyz.sh/)
- [bash 4 manual](https://www.gnu.org/software/bash/manual/)
- [zsh documentation](https://zsh.sourceforge.io/Doc/)
- [RFC-001 — ForgeScript Syntax](./RFC-001-forgescript-syntax.md)
- [RFC-013 — Shell Configuration Model](./RFC-013-shell-config.md)
- [RFC-009 — Plugin Registry](./RFC-009-plugin-registry.md)