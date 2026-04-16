# RFC-015 — Telemetry & Diagnostics

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

This RFC defines the telemetry and diagnostics model for Forge Shell — opt-in
only, type-driven allowlist enforced at compile time, self-hosted open-source
pipeline, public dashboard sourced to the plugin registry, and 30-day raw
event retention. Telemetry is enabled during MCP sessions with a distinct
`session_type = "mcp"` field — capturing AI agent vs human usage patterns.
Plugin names are inferred from namespaced command names — no separate
`plugin_name` field required.

---

## Motivation

Telemetry serves two purposes:

1. **Product decisions.** Which stdlib modules are used most? Which deprecated
   APIs are still in active use? Which commands do AI agents invoke vs humans?
   These questions directly inform RFC-014's deprecation process and 2.0
   decision framework.

2. **Reliability.** Crash reports and error patterns surface bugs that never
   appear in issue trackers — silent failures, platform-specific edge cases,
   and compatibility regressions.

Forge Shell runs on developer machines with access to file systems, environment
variables, secrets, and production infrastructure. The telemetry model is
designed around what is explicitly collected — not what is filtered out.

---

## Design

### 1. Core Principle — Opt-in, Not Opt-out

Forge Shell telemetry is **opt-in**. No data is collected by default.

**First-run prompt:**

```
Forge Shell would like to collect anonymous usage data to improve the product.

This includes:
  ✓ Which built-in commands are used (no arguments)
  ✓ Which stdlib modules are imported (no values)
  ✓ Error codes (no messages or context)
  ✓ Platform and architecture
  ✓ Forge Shell version
  ✓ Session type (interactive / script / mcp agent)

This never includes:
  ✗ Command arguments or flags
  ✗ File paths, file names, or file contents
  ✗ Environment variable names or values
  ✗ Script contents
  ✗ Network addresses or URLs
  ✗ Any personally identifiable information
  ✗ Credentials

Would you like to enable telemetry? [y/N]
```

Default is **N**. Change at any time:

```bash
forge telemetry enable
forge telemetry disable
forge telemetry status
forge telemetry show      # show what would be sent — always available
```

---

### 2. Type-Driven Allowlist

The allowlist is the shape of the `TelemetryEvent` struct itself — enforced
at compile time by Rust's type system. The struct IS the allowlist.

```rust
#[derive(Serialize)]
pub struct TelemetryEvent {
    pub forge_version:  String,
    pub platform:       Platform,
    pub arch:           Arch,
    pub event_type:     EventType,
    pub command_name:   Option<String>,   // handles all plugins — namespace infers plugin
    pub stdlib_module:  Option<String>,
    pub error_code:     Option<ErrorCode>,
    pub deprecated_api: Option<String>,
    pub startup_ms:     Option<u64>,
    pub session_type:   SessionType,      // Interactive | Script | Mcp
    pub install_id:     String,
}
```

**Why type-driven, not a string list:**

| String allowlist | Type-driven struct |
|---|---|
| Removal is silent | Removal breaks compilation immediately |
| Adding requires updating two places | One place — the struct |
| Runtime check — bypassable | Compile-time — cannot be bypassed |
| No enforcement all call sites provide all fields | All call sites must provide all fields |

**Adding a field** requires a PR that: updates the struct, updates every call
site (compiler forces this), updates `forge telemetry show`, updates the
collector, updates the public documentation, and updates RFC-015.

**Plugin scalability:** `command_name: Option<String>` handles all plugins
past, present, and future. No per-plugin allowlist entries. No struct changes
as new plugins are installed.

**Plugin name inference:** Namespaced command names (`kubectl:pods`) allow
the aggregator to infer the plugin (`forge-kubectl`) without a separate
`plugin_name` field.

**Plugins never touch `TelemetryEvent`** — it is entirely internal to the
Forge Shell host binary. Plugin ABI changes cannot affect telemetry struct.

---

### 3. What Is NEVER Collected

Hard guarantees — not filters, but architectural constraints:

| Category | Examples |
|---|---|
| Command arguments | File paths, flag values |
| File paths | Any path on the user's system |
| File contents | Any data read from disk |
| Environment variables | Names or values |
| Script contents | Source code of `.fgs` files |
| Network addresses | URLs, hostnames, IP addresses |
| Plugin names | Inferred from command namespace — never directly collected |
| Error messages | Error codes only — never message text |
| Output data | Any data produced by commands |
| Personal information | Name, email, username, machine name |
| Credentials | API keys, tokens, passwords |

---

### 4. Telemetry Events

**Startup:**

```json
{
  "event": "startup",
  "forge_version": "1.2.0",
  "platform": "linux",
  "arch": "x86_64",
  "session_type": "interactive",
  "install_id": "a1b2c3d4-...",
  "timestamp": "2026-04-15T09:00:00Z"
}
```

**Command:**

```json
{
  "event": "command",
  "command_name": "kubectl:pods",
  "session_type": "mcp",
  "install_id": "a1b2c3d4-...",
  "timestamp": "2026-04-15T09:00:01Z"
}
```

**Stdlib import:**

```json
{
  "event": "stdlib_import",
  "stdlib_module": "forge::json",
  "session_type": "script",
  "install_id": "a1b2c3d4-...",
  "timestamp": "2026-04-15T09:00:01Z"
}
```

**Error:**

```json
{
  "event": "error",
  "error_code": "E042",
  "session_type": "script",
  "install_id": "a1b2c3d4-...",
  "timestamp": "2026-04-15T09:00:02Z"
}
```

**Deprecated API — critical for RFC-014:**

```json
{
  "event": "deprecated_api",
  "deprecated_api": "forge::crypto::hash_file",
  "install_id": "a1b2c3d4-...",
  "timestamp": "2026-04-15T09:00:01Z"
}
```

---

### 5. Session Types — AI vs Human Usage

`session_type` captures the usage mode:

| Value | Context |
|---|---|
| `"interactive"` | Human at the REPL |
| `"script"` | `forge run script.fgs` |
| `"mcp"` | AI agent via `forge mcp serve` |

Telemetry is enabled during MCP sessions — the `session_type = "mcp"` field
captures AI agent vs human usage patterns. This validates Forge Shell's
AI-native design and informs RFC priorities.

**Public dashboard session type breakdown:**

```
Command usage by session type
  ls      interactive: 62%   script: 28%   mcp: 10%
  fetch   interactive: 31%   script: 35%   mcp: 34%   ← agents use fetch heavily
  jq      interactive: 28%   script: 29%   mcp: 43%   ← agents use structured data
```

---

### 6. `install_id` — UUID v4, Monthly Rotation

Stored in `config.toml`:

```toml
[telemetry]
enabled    = true
install_id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
rotated_at = "2026-04-01"   # rotated on 1st of each month
```

- Generated at first opt-in using cryptographically secure UUID v4
- Rotated automatically on the 1st of each month — new UUID generated
- Deleted with `config.toml` — new UUID on next startup
- Per-user — never shared across users
- Cannot track users across months

---

### 7. Data Transmission

- **Batching:** Events batched locally, transmitted once per day
- **Endpoint:** `telemetry.forge-shell.dev` — HTTPS only
- **Failure handling:** Transmission failures drop events silently — no retry
  queue, no persistent event storage beyond current day's batch
- **No third-party analytics:** Self-hosted pipeline — not Google Analytics,
  Mixpanel, Segment, or any third party

---

### 8. Raw Event Retention

```
Day 0-30  → raw events retained — debugging and anomaly investigation
Day 30    → raw events deleted permanently
Day 30+   → aggregated summaries only — retained permanently
```

---

### 9. Open Source Telemetry Pipeline

Fully open source at `github.com/forge-shell/telemetry`:

- **Collector:** Server-side event receiver
- **Aggregator:** Anonymised summary producer
- **Dashboard:** Public-facing UI

Trust is built through transparency — the code is the proof, not a policy statement.

---

### 10. Public Dashboard — `telemetry.forge-shell.dev/public`

Updated daily — nightly aggregation, morning refresh.

**Shows:**

```
Version distribution
  1.2.0  ████████████████░░░░  62%
  1.1.0  ████████████░░░░░░░░  31%

Platform distribution
  Linux    54%   macOS  41%   Windows  5%

Most used commands
  ls       98%   cat  79%   grep  68%   fetch  55%

Command usage by session type
  fetch   interactive: 31%   script: 35%   mcp: 34%

Deprecated API usage
  forge::crypto::hash_file  8%  ↓ declining (was 21% at deprecation)
```

**Never shows:** individual user data, machine identifiers, raw events,
geographic data finer than continent level.

---

### 11. Plugin Registry Integration

Aggregated telemetry is sourced nightly to `plugins.forge-shell.dev`:

```
forge-kubectl v2.1.0  ✅ verified  ★ official
  📊 Active installs   ~12,000 opted-in sessions
     Platforms         Linux 71%  macOS 26%  Windows 3%
     Weekly trend      ↑ +8% this month
  🔧 Most used commands
     kubectl:pods      68%
     kubectl:deploy    31%
```

Plugin authors see their plugin's analytics on the public registry page —
no private dashboard, no account required.

**Plugin name inference from namespace:** `kubectl:pods` → `forge-kubectl`.
The RFC-013 namespaced alias system makes this inference trivial. No new
allowlist field required.

---

### 12. CI and Enterprise Environments

Telemetry is automatically disabled in:

- CI environments — `CI=true`, `GITHUB_ACTIONS`, `JENKINS_URL` etc.
- Non-interactive sessions without explicit opt-in
- `FORGE_TELEMETRY=0` environment variable — enterprise fleet override

```bash
# Enterprise base environment
export FORGE_TELEMETRY=0    # disables across entire fleet
```

---

### 13. `forge telemetry show` — Always Available

Available regardless of opt-in status, including CI environments.

```bash
forge telemetry show

Telemetry status: disabled (CI environment — GITHUB_ACTIONS=true)

Events that would be sent if enabled:
  startup       forge_version=1.2.0 platform=linux session_type=script
  command       command_name=kubectl:pods
  stdlib_import stdlib_module=forge::json

No data was transmitted.
```

The `No data was transmitted` line provides an explicit audit trail.

---

### 14. Security Incident Process

```
Hour 0   → Incident detected → collector taken offline immediately
Hour 2   → Public notice at forge-shell.dev/security
Hour 24  → Full incident report — what was accessed, what was not
Hour 24  → New collector endpoint deployed if needed
```

**Why damage ceiling is low by design:** No PII, no credentials, no paths,
no arguments — ever collected. A breach exposes command usage statistics.
Nothing more. The architecture makes this a guarantee, not a promise.

---

## Drawbacks

- **Opt-in yields smaller dataset.** Accepted — user trust outweighs data
  volume for a shell.
- **CI auto-disable means no CI usage data.** Accepted — CI environments
  should never transmit telemetry.
- **Self-hosted pipeline requires maintenance.** Mitigated by keeping the
  pipeline simple and data volume low (opt-in).

---

## Alternatives Considered

### Alternative A — Opt-out telemetry

**Rejected:** Developer community scrutiny is high for shells. Trust cost
outweighs data volume benefit. Go, Rust, and most developer tools that tried
opt-out faced significant backlash.

### Alternative B — Third-party analytics

**Rejected:** Third-party analytics sends data to an external party.
Self-hosted, open-source pipeline is the only acceptable model for a shell.

### Alternative C — No telemetry

**Rejected:** Without deprecated API usage data, RFC-014's 2.0 decision
framework has no objective foundation. Opt-in telemetry with a public
dashboard is a reasonable middle ground.

### Alternative D — String-based allowlist

**Rejected:** String allowlists can drift from the code, be accidentally
modified, and are bypassable at runtime. The type-driven struct approach
makes the allowlist a compile-time guarantee.

### Alternative E — Disable telemetry during MCP sessions

**Rejected:** MCP session telemetry with `session_type = "mcp"` provides
valuable insight into AI agent vs human usage patterns — validating the
RFC-007 investment and informing future RFC priorities.

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | Raw event retention | 30 days — then deleted, aggregated summaries permanent |
| UQ-2 | Dashboard update frequency | Daily — nightly aggregation, morning refresh |
| UQ-3 | `install_id` storage | `config.toml` — UUID v4, monthly rotation |
| UQ-4 | Telemetry during MCP sessions | Enabled — `session_type = "mcp"` captures AI vs human |
| UQ-5 | Security incident process | Standard response — low damage ceiling by design |
| UQ-6 | Plugin author usage data | Public registry page sourced from telemetry nightly |
| UQ-7 | `forge telemetry show` in CI | Always available — transparency tool |

---

## Implementation Plan

### Affected Crates / Infrastructure

| Component | Responsibility |
|---|---|
| `forge-telemetry` | `TelemetryEvent` struct — type-driven allowlist |
| `forge-cli/telemetry` | `forge telemetry` subcommands |
| `forge-engine` | Emit command and error events |
| `forge-lang` | Emit stdlib import and deprecated API events |
| `telemetry-collector` | Server-side collector (separate repo) |
| `telemetry-aggregator` | Anonymised summary pipeline (separate repo) |
| `telemetry-dashboard` | Public dashboard (separate repo) |

### Dependencies

- Requires RFC-014 (release policy) — deprecated API tracking is core purpose
- Requires RFC-013 (shell config) — `[telemetry]` section in `config.toml`
- Requires RFC-009 (plugin registry) — registry page sourced from telemetry

### Milestones

1. Implement `forge-telemetry` — `TelemetryEvent` struct, type-driven allowlist
2. Implement opt-in prompt on first run
3. Implement `forge telemetry enable/disable/status/show`
4. Implement CI environment auto-detection and disable
5. Implement `FORGE_TELEMETRY=0` env var override
6. Implement event batching and daily transmission
7. Implement `install_id` UUID v4 generation and monthly rotation
8. Build `telemetry-collector` — open source, self-hosted
9. Build `telemetry-aggregator` — session type breakdown, plugin inference
10. Build public dashboard — daily updates, session type breakdown
11. Integrate telemetry data into plugin registry page — nightly pipeline
12. Implement deprecated API usage dashboard view — feeds RFC-014 decisions

---

## References

- [Go Telemetry Design](https://research.swtch.com/telemetry-opt-in)
- [Homebrew Analytics](https://docs.brew.sh/Analytics)
- [Next.js Telemetry](https://nextjs.org/telemetry)
- [NO_COLOR standard](https://no-color.org/)
- [RFC-014 — Release Policy & Versioning](./RFC-014-release-policy.md)
- [RFC-013 — Shell Configuration Model](./RFC-013-shell-config.md)
- [RFC-009 — Plugin Registry](./RFC-009-plugin-registry.md)
- [RFC-007 — AI Agent Layer](./RFC-007-ai-agent-layer.md)