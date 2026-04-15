# RFC-007 — AI Agent Layer & MCP Protocol Integration

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

This RFC defines the Forge Shell AI agent layer — a full implementation of
the Model Context Protocol (MCP) specification version 2025-11-25 that makes
Forge Shell the most capable local MCP server available to AI agents. Forge
Shell exposes built-in commands as MCP Tools, file system and shell state as
MCP Resources, shell-level diagnostic workflows as MCP Prompts, and supports
MCP Sampling, Roots, Elicitation, and Tasks. Plugin-owned tools, resources,
and prompts are aggregated automatically. The official Rust MCP SDK (`rmcp`)
is used throughout.

---

## Motivation

AI agents increasingly need to interact with the file system, run processes,
query system state, and chain shell operations. Today, agents do this by
generating bash scripts — fragile, platform-specific, opaque, and unsafe.

Forge Shell solves all four problems simultaneously:
- **Typed, structured output** — agents consume `StructuredOutput` JSON, not text
- **Cross-platform execution** — scripts run identically on Linux, macOS, Windows
- **Rich resource access** — agents read system state without tool calls
- **Safety model** — confirmation, audit log, capability scoping, roots enforcement

The MCP integration makes this available to any MCP-compatible AI system —
Claude, GPT, Gemini, and any other MCP client — without custom integration code.

---

## Design

### 1. MCP Server Architecture

```
AI Agent (MCP Client)
        ↓  JSON-RPC 2.0 over stdio or streamable HTTP
forge mcp serve (MCP Server)
        ↓
forge-agent crate
    ├── Tools layer        — built-in commands + #[tool] .fgs functions
    ├── Resources layer    — shell state, file system, audit log + plugin resources
    ├── Prompts layer      — shell diagnostics + plugin prompts
    ├── Tasks manager      — long-running command lifecycle
    ├── Sampling client    — requests model completions from connected client
    ├── Roots enforcer     — file system boundary enforcement
    ├── Elicitation layer  — missing args + destructive confirmations
    └── Audit logger       — all agent actions logged
        ↓
forge-engine (existing evaluation pipeline)
```

**MCP SDK:** `rmcp` v0.16.0 — official Rust MCP SDK, tokio async runtime.

**Transports:**

```bash
forge mcp serve                          # stdio — default, local agents
forge mcp serve --transport http --port 8765  # streamable HTTP — remote agents
```

---

### 2. MCP Tools — Built-in Commands

#### 2.1 Default Tool Exposure

The safe read-only subset is exposed by default. Destructive commands require
explicit opt-in.

**Exposed by default:**

| Group | Commands |
|---|---|
| File System (read) | `ls`, `tree`, `find`, `stat`, `du`, `df`, `cat` |
| Text & Streams | `grep`, `head`, `tail`, `sort`, `uniq`, `wc`, `jq`, `yq`, `tq`, `diff` |
| Environment | `env`, `pwd`, `which` |
| Network (GET only) | `ping`, `fetch` (GET only) |
| Forge-specific | `forge check` |

**Requires explicit opt-in:**

| Group | Commands |
|---|---|
| File System (destructive) | `cp`, `mv`, `rm`, `mkdir`, `rmdir`, `touch`, `hash` |
| Environment (mutating) | `set`, `unset`, `cd` |
| Network (mutating) | `fetch` (POST/PUT/DELETE/PATCH) |
| Forge-specific | `forge run`, `forge migrate`, `bench`, `watch` |

**Hard exclusions — never exposed regardless of flags:**

```
forge plugin install
forge plugin remove
forge plugin update
```

Plugin management is a human configuration action. It never appears in an
agent session under any circumstances — no flag overrides this.

**CLI flags:**

```bash
forge mcp serve                          # safe subset — default
forge mcp serve --allow-tools rm,cp,mv  # add specific destructive tools
forge mcp serve --allow-all             # all commands except plugin management
forge mcp serve --read-only             # read-only subset only — stricter than default
```

#### 2.2 `#[tool]` Annotation — ForgeScript Functions as MCP Tools

Any `.fgs` function annotated with `#[tool]` is exposed as an MCP tool when
`forge mcp serve --script script.fgs` is specified.

```forge
#!/usr/bin/env forge

#[tool(
    description = "Deploy the application to the specified environment",
    confirm = true    # requires elicitation confirmation before execution
)]
fn deploy(
    env:     str,    # "staging" | "production"
    dry_run: bool,   # if true, show plan without executing
) -> Result<DeployOutput, CommandError> {
    if dry_run {
        echo "Dry run: would deploy to {env}"
        return Ok(DeployOutput::dry_run(env))
    }
    run("kubectl apply -f deploy/{env}.yaml")?
    Ok(DeployOutput::success(env))
}
```

JSON Schema is generated automatically from the ForgeScript function signature
— no manual schema authoring required.

#### 2.3 Plugin Tools

Plugins declare their MCP tools in `forge-plugin.toml`. The Forge Shell MCP
server aggregates all installed plugin tools automatically — the agent sees
one unified `tools/list` response.

```toml
[[mcp.tools]]
command     = "git-status"
description = "Rich git status viewer"
```

#### 2.4 StructuredOutput → MCP Tool Response

The host converts postcard → `StructuredOutput` → JSON at the MCP boundary.
Every tool response contains two content blocks:

```json
{
  "content": [
    {
      "type": "text",
      "text": "3 files found in /home/user/projects"
    },
    {
      "type": "resource",
      "resource": {
        "uri":      "forge://output/ls",
        "mimeType": "application/json",
        "text": "{\"command\":\"ls\",\"version\":1,\"status\":\"Success\",\"payload\":{\"Ls\":[...]}}"
      }
    }
  ],
  "isError": false
}
```

- **Text block** — human-readable summary for agent reasoning
- **Resource block** — full `StructuredOutput` JSON for programmatic consumption

Conversion pipeline:

```
postcard (built-in or plugin)
    ↓ host deserialises
StructuredOutput
    ↓ OutputMode::Structured
    ↓ serde_json::to_value()
MCP Tool Response JSON
```

---

### 3. MCP Tasks — Long-Running Commands

Commands are classified by expected duration:

| Command type | Response model |
|---|---|
| Fast (`ls`, `grep`, `cat`) | Immediate MCP Tool response |
| Bounded long-running (`bench` N runs, `tail` N lines) | Immediate — result when done |
| Unbounded (`tail -f`, `watch`) | MCP Task — agent polls, can cancel |
| `forge run` (unknown duration) | MCP Task — agent polls, can cancel |

**Task lifecycle:**

```
Agent invokes tool: tail --follow /var/log/forge.log
         ↓
Forge Shell returns immediately:
  Task { id: "task-001", status: "running" }
         ↓
Agent polls: tasks/status { id: "task-001" }
  → Task { id: "task-001", status: "running", partial_output: [...] }
         ↓
Agent cancels: tasks/cancel { id: "task-001" }
  → Task { id: "task-001", status: "cancelled" }
```

Active Tasks always extend the session timeout — a connection with a running
Task is never considered idle.

---

### 4. MCP Resources

#### 4.1 Core Resources — Forge Shell Owns

```
# File System
forge://fs/{path}              — file contents
forge://fs/tree/{path}         — directory tree as structured JSON
forge://fs/stat/{path}         — file metadata

# Environment
forge://env                    — all environment variables
forge://env/{key}              — single environment variable
forge://path                   — PATH as list<path> JSON
forge://cwd                    — current working directory

# Shell State
forge://jobs                   — background jobs
forge://history                — session command history

# System
forge://system/platform        — OS, architecture, forge version
forge://system/disk            — disk usage summary
forge://system/ports           — listening ports with process names
forge://system/processes       — running processes

# Config
forge://config/forge           — resolved forge config
forge://config/hosts           — /etc/hosts

# Plugins
forge://plugins                — installed plugins — name, version, capabilities
forge://plugins/{name}         — specific plugin manifest

# Audit
forge://audit/current          — current session audit log — subscribable
forge://audit/sessions         — list of past sessions
forge://audit/sessions/{id}    — specific session log

# Script Output Cache
forge://output/{script}        — last StructuredOutput of a named script
```

#### 4.2 Plugin Resources

Plugins declare their MCP resources in `forge-plugin.toml`:

```toml
[[mcp.resources]]
uri_pattern  = "forge://git/status"
description  = "Git working tree status"
subscribable = true

[[mcp.resources]]
uri_pattern  = "forge://git/diff"
description  = "Current unstaged diff"
subscribable = true
```

**Example plugin-owned resource namespaces:**

| Plugin | Resource namespace |
|---|---|
| `forge-git` | `forge://git/*` |
| `forge-docker` | `forge://docker/*` |
| `forge-kubectl` | `forge://kubernetes/*` |
| `forge-aws` | `forge://aws/*` |
| `forge-rust` | `forge://project/rust/*` |
| `forge-node` | `forge://project/node/*` |

#### 4.3 Subscriptions

| Resource | Subscribable |
|---|---|
| `forge://jobs` | ✅ |
| `forge://audit/current` | ✅ |
| `forge://env` | ✅ |
| `forge://cwd` | ✅ |
| `forge://fs/{path}` | ✅ |
| `forge://system/disk` | ✅ |
| `forge://system/ports` | ✅ |
| `forge://plugins` | ✅ |
| `forge://system/platform` | ❌ static |
| `forge://config/hosts` | ❌ static |

---

### 5. MCP Prompts

#### 5.1 Core Prompts — Forge Shell Owns

Shell-level diagnostic prompts only. Project and tool-specific prompts belong
to plugins.

```
forge://prompts/diagnose-error    — given an error, gather system context
forge://prompts/debug-port        — given a port, find what's using it
forge://prompts/explain-failure   — given a failed command, explain why
forge://prompts/system-health     — snapshot of system state
forge://prompts/env-audit         — review environment variables for issues
```

**Example — `diagnose-error` assembled by Forge Shell:**

When invoked with `{ "error": "EADDRINUSE: port 8080 already in use" }`,
Forge Shell assembles a message containing:
- The error text
- `forge://system/platform`
- `forge://system/ports` — what's on port 8080
- `forge://system/processes` — relevant processes
- Recent `forge://audit/current` entries
- `forge://cwd`

The agent receives rich pre-assembled context — no separate resource reads needed.

#### 5.2 Plugin Prompts

```toml
[[mcp.prompts]]
name        = "git-commit"
description = "Generate a commit message from current diff"
arguments   = []
```

**Example plugin-owned prompts:**

| Plugin | Prompts |
|---|---|
| `forge-git` | `git-commit`, `code-review`, `branch-summary` |
| `forge-rust` | `cargo-test`, `clippy-review`, `build-diagnosis` |
| `forge-node` | `npm-test`, `lint-review`, `dependency-audit` |
| `forge-docker` | `container-diagnosis`, `image-cleanup` |

---

### 6. MCP Sampling

Forge Shell uses Sampling for enhancement use cases only — never for
decisions with side effects.

**Approved sampling use cases:**

| Use case | Trigger | Forge asks client |
|---|---|---|
| Error explanation | Command fails with cryptic error | "Explain this error given this system context" |
| Script improvement | `forge-ai fmt script.fgs` | "Suggest improvements to this ForgeScript" |
| Diagnostic summary | `diagnose-error` prompt invoked | "Summarise these diagnostic findings" |

**Hard rules — sampling never used for:**
- Destructive decisions — `rm`, `deploy`, `forge plugin install`
- Confirmation bypass — human approval always takes precedence
- Core shell functionality — sampling is enhancement, not foundation
- Any action with side effects

---

### 7. MCP Roots

Roots are file system boundaries declared by the MCP client. Forge Shell
enforces them strictly.

```json
{
  "roots": [
    { "uri": "file:///home/user/projects/forge-shell", "name": "Project Root" }
  ]
}
```

**Enforcement rules:**
- Roots declared → all file operations and resource reads enforced within roots
- No roots declared → full file system access
- No override flag — roots are a hard security boundary, not a preference
- Violations rejected and logged to audit log — never executed

**Audit log entry for root violation:**

```json
{
  "timestamp": "2026-04-15T09:00:01Z",
  "tool":      "cat",
  "arguments": { "path": "/etc/passwd" },
  "result":    "rejected",
  "reason":    "outside_declared_roots"
}
```

---

### 8. MCP Elicitation

Forge Shell uses elicitation for two cases: missing required arguments and
destructive operation confirmation.

#### 8.1 Missing Argument Elicitation

```
Agent calls: rm (no path argument)
         ↓
Forge Shell elicits: "Which path should be removed?"
         ↓
Client responds: "/tmp/old-build"
         ↓
Forge Shell proceeds with rm /tmp/old-build
```

#### 8.2 Destructive Confirmation — Type-to-Confirm

Destructive operations require the human to type the target exactly — not
a simple `[y/N]` prompt.

```
Agent calls: rm path: p"/home/user/projects" --recursive
         ↓
Forge Shell elicits:
  "⚠️ This will permanently delete /home/user/projects and all contents.
   Type the path to confirm:"
         ↓
Human types: /home/user/projects
         ↓
Forge Shell proceeds only if input matches exactly
```

**Commands requiring type-to-confirm elicitation:**

| Command | Must type |
|---|---|
| `rm --recursive` | Full path |
| `rm` on non-empty directory | Full path |
| `fetch` with DELETE | Full URL |
| `forge run` in agent context | Script name |
| `#[tool]` with `confirm = true` | Declared in annotation |

---

### 9. Session Management

**Session timeout:**

```toml
[mcp]
session_timeout = "30m"    # default — configurable
```

```bash
forge mcp serve --timeout 60m     # override per session
forge mcp serve --timeout none    # explicit infinite
```

- Default: 30 minutes of inactivity
- Active Tasks always extend timeout — running tasks are never idle
- On timeout: `notifications/session_timeout` sent before connection closes
- Agent can reconnect cleanly after timeout

---

### 10. Audit Log

All agent-invoked actions are logged — including rejected operations.

**Disk location:**

| Platform | Path |
|---|---|
| Linux / macOS | `~/.config/forge/audit/sessions/` |
| Windows | `%APPDATA%\forge\audit\sessions\` |

**Log entry format:**

```json
{
  "session_id":  "sess-001",
  "timestamp":   "2026-04-15T09:00:01Z",
  "tool":        "rm",
  "arguments":   { "path": "/tmp/old.log" },
  "confirmed":   true,
  "result":      "ok",
  "duration_ms": 8
}
```

**Audit log as MCP Resource:** `forge://audit/current` — subscribable,
real-time updates. `forge://audit/sessions/{id}` — past sessions read-only.

**Access control:** Audit log is always read-only via MCP. No tool can
modify or delete audit entries.

---

### 11. Capability Scoping Summary

| Capability | Default | Override |
|---|---|---|
| Read-only built-ins | ✅ Exposed | — |
| Destructive built-ins | ❌ Hidden | `--allow-tools` or `--allow-all` |
| Plugin management | ❌ Never | No override — hard rule |
| `#[tool]` functions | ❌ Hidden | `--script script.fgs` |
| Roots enforcement | ✅ If declared | None — roots are hard boundary |
| Session timeout | 30 min | `--timeout` flag or `config.toml` |
| Sampling | ✅ Enhancement only | — |
| Elicitation | ✅ Always | — |

---

## Drawbacks

- **MCP spec evolves rapidly.** The 2025-11-25 spec is the current version —
  future spec changes may require updates to `forge-agent`. Using `rmcp`
  (official SDK) mitigates this — SDK updates handle spec changes.
- **Plugin resource aggregation adds complexity.** The MCP server must
  dynamically aggregate tools, resources, and prompts from all installed
  plugins. Plugin install/remove mid-session is not supported — requires
  server restart.
- **Type-to-confirm elicitation adds friction.** Deliberate — destructive
  operations should be friction-ful. But poorly designed agent workflows
  that require frequent destructive operations will feel slow.

---

## Alternatives Considered

### Alternative A — Bash Script Generation

**Rejected:** Makes Forge Shell a dumb executor. Structured output, the
confirmation model, and typed tools all require native integration. Reintroduces
platform-specific problems ForgeScript is designed to solve.

### Alternative B — Bespoke REST API

**Rejected:** MCP is the de-facto standard for AI agent tool integration.
A bespoke REST API requires custom integration in every AI framework. MCP
gives compatibility with Claude, GPT, Gemini, and any MCP-compatible system.

### Alternative C — Plugin-Based Agent Integration

**Rejected:** The agent layer requires deep integration with the evaluation
pipeline — structured output, audit logging, elicitation, roots enforcement.
This cannot be safely implemented as a plugin.

### Alternative D — Expose All Built-ins by Default

**Rejected:** An agent with automatic access to `rm`, `mv`, and `fetch POST`
is a significant blast radius. The MCP spec requires explicit user consent
before invoking tools. Safe-subset-by-default honours this principle.

### Alternative E — Allow Plugin Installation via MCP

**Rejected:** An agent that can install plugins can escalate its own
capabilities by installing a plugin with `exec["*"]` and `filesystem:read-write`.
This is a privilege escalation vector that must be closed unconditionally.

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | Which built-ins as MCP Tools? | Safe read-only subset by default — opt-in for destructive |
| UQ-2 | Long-running commands | MCP Tasks — agent polls, can cancel |
| UQ-3 | Audit log as MCP Resource | Both disk and MCP Resource — subscribable |
| UQ-4 | Session timeout | 30 min default — configurable, Tasks extend |
| UQ-5 | MCP Resources | Core shell resources — tool/project resources via plugins |
| UQ-6 | MCP Prompts | Shell diagnostics only — project prompts via plugins |
| UQ-7 | MCP Sampling | Yes — error explanation and script enhancement only |
| UQ-8 | MCP Roots | Strictly enforced — no override, no roots = full access |
| UQ-9 | MCP Elicitation | Yes — missing args + type-to-confirm for destructive |
| UQ-10 | StructuredOutput → MCP | Host converts at boundary — text + resource JSON |
| UQ-11 | Agents install plugins? | Never — hard rule, no override |

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-agent` | MCP server — tools, resources, prompts, tasks, sampling, roots, elicitation |
| `forge-agent/tools` | Tool registry — built-in + plugin + `#[tool]` function tools |
| `forge-agent/resources` | Resource registry — core + plugin resources, subscriptions |
| `forge-agent/prompts` | Prompt registry — core + plugin prompts |
| `forge-agent/tasks` | Long-running task lifecycle manager |
| `forge-agent/audit` | Audit logger — disk + MCP resource |
| `forge-agent/elicitation` | Missing arg + type-to-confirm elicitation |
| `forge-lang` | `#[tool]` annotation, JSON Schema generation from type signatures |
| `forge-cli` | `forge mcp serve` subcommand |

**External dependency:** `rmcp` v0.16.0 — official Rust MCP SDK.

### Dependencies

- Requires RFC-001 (ForgeScript syntax) — `#[tool]` annotation
- Requires RFC-002 (evaluation pipeline) — structured output integration
- Requires RFC-003 (built-in commands) — all built-ins with `StructuredOutput`
- Requires RFC-005 (plugin system) — plugin tool/resource/prompt aggregation
- Requires RFC-006 (job control) — `forge://jobs` resource

### Milestones

1. Implement `forge mcp serve` — stdio and streamable HTTP transports
2. Implement tool registry — built-in command exposure, capability scoping
3. Implement `#[tool]` annotation and JSON Schema generation
4. Implement MCP Tasks — long-running command lifecycle
5. Implement core resource registry — all `forge://` URIs
6. Implement resource subscriptions — jobs, audit, env, cwd, fs
7. Implement core prompt registry — shell diagnostic prompts
8. Implement plugin aggregation — tools, resources, prompts from installed plugins
9. Implement roots enforcement
10. Implement elicitation — missing args + type-to-confirm
11. Implement sampling — error explanation + script improvement
12. Implement audit logger — disk + MCP resource
13. Implement session timeout management
14. Integration tests — full MCP session on ubuntu-latest, macos-latest, windows-latest

---

## References

- [Model Context Protocol Specification 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25)
- [rmcp — Official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [MCP Tools](https://modelcontextprotocol.io/docs/concepts/tools)
- [MCP Resources](https://modelcontextprotocol.io/docs/concepts/resources)
- [MCP Prompts](https://modelcontextprotocol.io/docs/concepts/prompts)
- [MCP Tasks — November 2025 spec](https://blog.modelcontextprotocol.io/posts/2025-11-25-first-mcp-anniversary/)
- [RFC-001 — ForgeScript Syntax](./RFC-001-forgescript-syntax.md)
- [RFC-002 — Evaluation Pipeline](./RFC-002-evaluation-pipeline.md)
- [RFC-003 — Built-in Commands](./RFC-003-builtin-commands.md)
- [RFC-005 — Plugin System](./RFC-005-plugin-system.md)
- [RFC-006 — Job Control](./RFC-006-job-control.md)