# RFC-007 — AI Agent Layer & MCP Protocol Integration

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

This RFC defines the Forge Shell AI agent layer — a set of capabilities that
make Forge Shell a first-class execution environment for AI agents. This
includes structured I/O for all built-in commands, a tool registration API
for exposing `.fgs` functions as AI tools, and an implementation of the
Model Context Protocol (MCP) that allows any MCP-compatible AI agent or
framework to invoke Forge Shell commands.

---

## Motivation

AI agents increasingly need to interact with the filesystem, run processes,
query cloud infrastructure, and chain together shell operations. Today, agents
do this by generating bash scripts, which are:

- **Fragile** — exit codes are ignored, errors silently swallowed
- **Platform-specific** — bash scripts break on Windows
- **Opaque** — no structured output, agents must parse text
- **Unsafe** — no confirmation model, no audit trail

Forge Shell can solve all four problems simultaneously. Its structured output,
cross-platform execution, and typed error model make it an ideal agent
execution environment. The MCP integration makes this available to any
MCP-compatible AI system without agent-specific integration code.

---

## Design

### 1. Structured I/O

Every built-in command emits structured data. The output format is controlled
by a global flag or per-command flag:

```bash
# Default — human-readable table
ls

# JSON — for agent consumption
forge --output json ls ./src

# NDJSON — for streaming pipelines
forge --output ndjson ps

# Silent — no output, only exit code
forge --output none rm ./temp
```

#### Output Schema

Every structured output record includes:

```json
{
  "type":    "record",
  "command": "ls",
  "data":    [ ... ],
  "meta": {
    "platform":   "linux",
    "duration_ms": 12,
    "exit_code":   0
  }
}
```

#### Error Schema

Errors are structured too — agents can parse failure reasons without
text matching:

```json
{
  "type":    "error",
  "command": "cp",
  "error": {
    "kind":    "PermissionDenied",
    "message": "Permission denied: /etc/hosts",
    "path":    "/etc/hosts",
    "code":    13
  },
  "meta": {
    "platform":   "linux",
    "exit_code":   1
  }
}
```

---

### 2. Tool Registration API

Any `.fgs` function annotated with `#[tool]` is automatically exposed as an
AI tool with a JSON Schema derived from its type signature.

```forge
#[tool(
  description = "Deploy the application to the specified environment",
  confirm = true    # requires user confirmation before execution
)]
fn deploy(
  env:     string,           # "staging" | "production"
  dry_run: bool = false,     # show what would happen without executing
  region:  string = "eu-west-1"
) -> Result<DeployOutput, DeployError> {
  # ...
}
```

Forge Shell generates the following JSON Schema automatically:

```json
{
  "name": "deploy",
  "description": "Deploy the application to the specified environment",
  "input_schema": {
    "type": "object",
    "properties": {
      "env":     { "type": "string" },
      "dry_run": { "type": "boolean", "default": false },
      "region":  { "type": "string", "default": "eu-west-1" }
    },
    "required": ["env"]
  }
}
```

---

### 3. Model Context Protocol (MCP) Implementation

Forge Shell implements MCP as a built-in server. Any MCP-compatible AI agent
can connect to Forge Shell and invoke registered tools.

#### Starting the MCP Server

```bash
# Start MCP server on stdio (for local agent integration)
forge mcp serve --transport stdio

# Start MCP server on a TCP socket
forge mcp serve --transport tcp --port 8765

# Start with a specific script — only tools from this script are exposed
forge mcp serve --script ./tools/deploy.fgs
```

#### MCP Tool Discovery

When an agent connects, it can discover all registered tools:

```json
{
  "method": "tools/list",
  "result": {
    "tools": [
      {
        "name":        "deploy",
        "description": "Deploy the application to the specified environment",
        "inputSchema": { ... }
      },
      {
        "name":        "ls",
        "description": "List directory contents",
        "inputSchema": { ... }
      }
    ]
  }
}
```

All built-in commands are automatically available as MCP tools with their
structured output schemas. Script tools annotated with `#[tool]` are also
exposed.

#### MCP Tool Invocation

```json
{
  "method": "tools/call",
  "params": {
    "name": "deploy",
    "arguments": {
      "env":     "staging",
      "dry_run": true
    }
  }
}
```

```json
{
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Dry run: would deploy to staging in eu-west-1\nChanges: 3 services updated"
      }
    ],
    "isError": false
  }
}
```

---

### 4. Safety Model

The agent layer enforces a safety model that prevents runaway agents from
causing irreversible damage.

#### Confirmation Mode

Commands marked `confirm = true` in their `#[tool]` annotation pause and
request user approval before execution. In non-interactive (agent) sessions,
confirmation is handled via MCP:

```json
{
  "method": "tools/confirm",
  "params": {
    "tool":    "deploy",
    "args":    { "env": "production" },
    "warning": "This will deploy to PRODUCTION. Are you sure?"
  }
}
```

The agent framework must reply with `confirmed: true` before the command
executes. If the framework does not support confirmation, the command is
rejected.

#### Dry-Run Mode

All destructive commands support `--dry-run`. When `--dry-run` is passed,
the execution plan is generated and returned as structured output but never
executed:

```json
{
  "type": "dry_run",
  "plan": [
    { "op": "exec",   "cmd": "kubectl", "args": ["apply", "-f", "manifest.yaml"] },
    { "op": "exec",   "cmd": "kubectl", "args": ["rollout", "status", "deployment/app"] }
  ]
}
```

#### Audit Log

All agent-invoked commands are logged to a structured audit log:

```
~/.forge/logs/agent-audit.ndjson
```

```json
{"timestamp":"2026-04-09T10:23:45Z","session":"sess_abc123","tool":"deploy","args":{"env":"staging"},"exit_code":0,"duration_ms":4521}
{"timestamp":"2026-04-09T10:24:01Z","session":"sess_abc123","tool":"ls","args":{"path":"./src"},"exit_code":0,"duration_ms":8}
```

#### Capability Scoping for Agent Sessions

When starting an MCP server, the exposed tool set can be restricted:

```bash
# Only expose specific tools
forge mcp serve --allow-tools deploy,ls,ps

# Deny destructive tools
forge mcp serve --deny-tools rm,kill,chmod

# Read-only mode — only non-mutating built-ins
forge mcp serve --read-only
```

---

### 5. Agent Session Protocol

A full agent session is stateful. Forge Shell maintains session context
across multiple tool calls:

```
Agent                          Forge Shell (MCP Server)
  │                                      │
  │── initialize ────────────────────────▶│  negotiate capabilities
  │◀─ initialized ───────────────────────│
  │                                      │
  │── tools/list ────────────────────────▶│  discover available tools
  │◀─ tools (list) ──────────────────────│
  │                                      │
  │── tools/call (ls ./src) ────────────▶│  execute
  │◀─ result (structured JSON) ──────────│
  │                                      │
  │── tools/call (deploy staging) ──────▶│  requires confirmation
  │◀─ tools/confirm (warning message) ───│
  │── tools/confirm (confirmed: true) ──▶│
  │◀─ result ────────────────────────────│
  │                                      │
  │── shutdown ──────────────────────────▶│
```

---

### 6. forge-agent Crate Structure

```
forge-agent/
├── src/
│   ├── mcp/
│   │   ├── server.rs      # MCP server (stdio + TCP transports)
│   │   ├── protocol.rs    # MCP message types
│   │   ├── handler.rs     # Tool call dispatch
│   │   └── session.rs     # Session state management
│   ├── tool/
│   │   ├── registry.rs    # Tool registration and discovery
│   │   ├── schema.rs      # JSON Schema generation from ForgeScript types
│   │   └── confirm.rs     # Confirmation model
│   ├── output/
│   │   ├── json.rs        # JSON output formatter
│   │   ├── ndjson.rs      # NDJSON output formatter
│   │   └── schema.rs      # Output type definitions
│   └── audit/
│       └── log.rs         # Audit log writer
```

---

## Drawbacks

- **MCP spec is evolving** — the Model Context Protocol is relatively new.
  Breaking changes in the spec would require updates to `forge-agent`.
- **Structured output adds overhead** — serialising every command output to
  JSON has a measurable cost. For interactive use, this is opt-in. For agent
  sessions, it is always on.
- **Confirmation UX in agent sessions** — the confirmation model requires
  agent framework support. Frameworks that do not implement the confirmation
  protocol cannot safely run destructive Forge tools.
- **Schema generation complexity** — generating accurate JSON Schema from
  ForgeScript type annotations requires deep integration with the type system.

---

## Alternatives Considered

### Alternative A — Bash Script Generation

**Approach:** The agent generates bash scripts which Forge Shell executes.
**Rejected because:** This makes Forge Shell a dumb executor. Structured
output, the confirmation model, and the type-safe tool API all require
native integration. Bash script generation also reintroduces all the
platform-specific problems Forge Shell is designed to solve.

### Alternative B — HTTP/REST API

**Approach:** Forge Shell exposes a REST API instead of implementing MCP.
**Rejected because:** MCP is rapidly becoming the standard protocol for
AI agent tool integration. Implementing a bespoke REST API would require
custom integration code in every AI framework. MCP gives Forge Shell
compatibility with Claude, GPT, and any other MCP-compatible system for free.

### Alternative C — Plugin-Based Agent Integration

**Approach:** Agent integration is a plugin, not a core feature.
**Rejected because:** The agent layer requires deep integration with the
execution pipeline (structured output, audit logging, confirmation model).
This cannot be safely or cleanly implemented as a plugin.

---

## Unresolved Questions

- [ ] Should all built-in commands be exposed as MCP tools by default, or
      should agent-accessible tools be opt-in?
- [ ] How should long-running tool calls (e.g. `tail -f`) be handled in MCP?
      MCP does not natively support streaming results.
- [ ] Should the audit log be queryable via a built-in command or MCP tool?
- [ ] What is the session timeout policy for idle MCP connections?
- [ ] Should Forge Shell support MCP resources (not just tools)?

---

## Implementation Plan

### Affected Crates

- `forge-agent` — new crate, all agent layer code
- `forge-builtins` — add structured output layer to all built-ins
- `forge-lang` — `#[tool]` annotation, JSON Schema generation from types
- `forge-cli` — `forge mcp serve` subcommand

### Dependencies

- Requires RFC-001 (ForgeScript Syntax) — `#[tool]` annotation syntax
- Requires RFC-002 (Evaluation Pipeline) — structured output integrates
  with execution engine
- Requires RFC-003 (Built-in Commands) — all built-ins need structured
  output before MCP exposure

### Milestones

1. Implement structured JSON/NDJSON output layer for all built-ins
2. Define tool registration API and `#[tool]` annotation
3. Implement JSON Schema generation from ForgeScript type signatures
4. Implement MCP server (stdio transport) in `forge-agent`
5. Implement MCP server (TCP transport)
6. Implement confirmation model
7. Implement audit log
8. Implement capability scoping (`--allow-tools`, `--deny-tools`, `--read-only`)
9. Integration tests — MCP tool discovery and invocation

---

## References

- [Model Context Protocol Specification](https://modelcontextprotocol.io/specification)
- [MCP TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk)
- [JSON Schema Specification](https://json-schema.org/specification)
- [Anthropic Claude Tool Use](https://docs.anthropic.com/en/docs/build-with-claude/tool-use)
