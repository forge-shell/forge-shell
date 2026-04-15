# RFC-011 — `forge migrate` — Bash to ForgeScript Migration Tooling

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

`forge migrate` is a first-class CLI tool that mechanically translates bash
and zsh scripts — including `.bashrc`, `.zshrc`, and Oh My Zsh configurations
— into ForgeScript equivalents. It is honest about what it can and cannot
migrate automatically, flags constructs requiring human review, and generates
`.gitignore` recommendations. ForgeScript is not a superset of bash —
migration is a translation, not a rename.

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

### 1. Invocation

```bash
# Migrate a single script
forge migrate script.sh

# Migrate a shell config file
forge migrate ~/.bashrc
forge migrate ~/.zshrc

# Migrate an entire directory
forge migrate --dir ./scripts/

# Dry run — show what would change without writing files
forge migrate --dry-run script.sh

# Output to specific path
forge migrate script.sh --output script.fgs

# Oh My Zsh aware migration
forge migrate --zsh ~/.zshrc --oh-my-zsh
```

### 2. What `forge migrate` Translates Automatically

#### Variables

```bash
# bash
NAME="forge"
echo $NAME
echo ${NAME}
```

```forge
# ForgeScript
let name = "forge"
echo "{name}"
```

#### PATH Manipulation

```bash
export PATH="$HOME/.local/bin:$PATH"
export PATH="/usr/local/go/bin:$PATH"
```

```toml
# config.toml [path] — RFC-013
[path]
prepend = ["~/.local/bin", "/usr/local/go/bin"]
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
alias gs="git status"
```

```toml
# config.toml [aliases]
[aliases]
ll = "ls --show_hidden --long"
gs = "git status"
```

#### Environment Variables

```bash
export EDITOR="nvim"
export GOPATH="$HOME/go"
```

```toml
# config.toml [env]
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

### 3. Oh My Zsh Awareness

When `--oh-my-zsh` flag is passed:

| Oh My Zsh construct | Migration output |
|---|---|
| `plugins=(git docker...)` | `config.toml [plugins]` + install suggestion |
| `ZSH_THEME="..."` | `config.toml [prompt] theme` |
| `ZSH_CUSTOM` directory | `config.toml [plugins] local` |
| Powerlevel10k segments | `config.toml [prompt] segments` |
| `zsh-autosuggestions` | `config.toml [repl] autosuggestions = true` |
| `zsh-syntax-highlighting` | `config.toml [repl] syntax_highlighting = true` |

### 4. What `forge migrate` Flags for Human Review

Some constructs cannot be automatically migrated. `forge migrate` emits clear
warnings with suggestions:

#### `eval` patterns

```bash
eval "$(rbenv init -)"
eval "$(pyenv init -)"
```

```
# WARNING: eval pattern detected — manual migration required
# Original: eval "$(rbenv init -)"
# Suggestion: check forge-shell.dev/plugins for forge-rbenv
# Fallback: use forge::process::capture() to run rbenv and parse output
```

#### Process substitution

```bash
diff <(ls dir1) <(ls dir2)
```

```
# WARNING: process substitution detected — manual migration required
# Original: diff <(ls dir1) <(ls dir2)
# Suggestion: use spawn { } and capture output, then pass to diff
```

#### Here-documents

```bash
cat <<EOF
Hello World
EOF
```

```
# WARNING: heredoc detected — manual migration required
# Suggestion: use multiline string literal or forge::fs::write()
```

#### Associative arrays

```bash
declare -A map
map["key"]="value"
```

```
# WARNING: associative array detected — no direct equivalent
# Suggestion: use a struct or forge::json for key-value storage
```

### 5. Generated `.gitignore`

`forge migrate` always generates `.gitignore` recommendations:

```gitignore
# ForgeScript — generated by forge migrate
.env.local
.env.*.local
.forge-env.local
*.fgs.bak
```

### 6. Migration Report

After migration, `forge migrate` prints a summary report:

```
forge migrate report
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 Translated automatically    42 constructs
 Flagged for review           5 constructs
 Output files                 3 .fgs files
                              1 config.toml
                              1 .gitignore
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 Run: forge check ./scripts/ to validate
```

---

## Drawbacks

- **Translation is never perfect.** Bash has too many edge cases and
  platform-specific behaviours to handle exhaustively. `forge migrate` covers
  the common 80% — the remaining 20% requires human review.
- **Oh My Zsh plugin equivalents may not exist at launch.** `forge migrate`
  can suggest `forge-git` but if the plugin isn't in the registry yet, the
  developer hits a wall.

---

## Alternatives Considered

### Alternative A — Bash compatibility layer (superset model)

**Rejected:** ForgeScript is not a superset of bash. Pretending compatibility
exists where it doesn't produces a leaky abstraction that compromises every
design decision. Honest migration tooling is better than fake compatibility.

### Alternative B — No migration tooling — rewrite manually

**Rejected:** Migration friction is a real adoption barrier. First-class
tooling communicates that the project takes migration seriously.

---

## Unresolved Questions

- [ ] Which bash dialect versions should `forge migrate` target? POSIX sh only,
      bash 4+, or bash 5+?
- [ ] Should `forge migrate` attempt to migrate zsh-specific syntax
      (zparseopts, zstyle) beyond Oh My Zsh?
- [ ] Should migration output be interactive (review each change) or
      batch (write all files, review after)?
- [ ] How should `forge migrate` handle scripts that source other scripts?

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-migrate` | Main migration CLI tool |
| `forge-migrate/parser` | Bash/zsh AST parser |
| `forge-migrate/translator` | AST → ForgeScript translation rules |
| `forge-migrate/omz` | Oh My Zsh awareness layer |
| `forge-migrate/report` | Migration report generation |

### Dependencies

- Requires RFC-001 (ForgeScript syntax) to be accepted — translation target
- Requires RFC-013 (Shell Configuration Model) — `config.toml` output format
- RFC-009 (Plugin Registry) — plugin suggestions in migration output

### Milestones

1. Implement bash AST parser — variables, functions, conditionals, loops
2. Implement translation rules — variables, PATH, aliases, functions
3. Implement `eval` and process substitution detection and flagging
4. Implement Oh My Zsh awareness layer
5. Implement `.gitignore` generation
6. Implement migration report
7. Integration tests — real-world `.bashrc` and `.zshrc` migration

---

## References

- [Go fix tool](https://pkg.go.dev/cmd/fix)
- [Python 2to3](https://docs.python.org/3/library/2to3.html)
- [Oh My Zsh](https://ohmyz.sh/)
- [bash manual](https://www.gnu.org/software/bash/manual/)
- [RFC-001 — ForgeScript Syntax](./RFC-001-forgescript-syntax.md)
- [RFC-013 — Shell Configuration Model](./RFC-013-shell-config.md)