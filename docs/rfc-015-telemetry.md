# RFC-015 — Telemetry & Diagnostics

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

This RFC defines the telemetry and diagnostics model for Forge Shell —
covering what data is collected, what is never collected, opt-in vs opt-out,
data storage and transmission, the open-source telemetry pipeline, and how
telemetry informs the deprecation and release policy defined in RFC-014.

---

## Motivation

Telemetry serves two purposes for Forge Shell:

1. **Product decisions.** Which stdlib modules are used most? Which built-in
   commands are invoked most frequently? Which platforms are most common?
   Which deprecated APIs are still in active use? These questions directly
   inform RFC-014's deprecation process and 2.0 decision framework.

2. **Reliability.** Crash reports and error patterns surface bugs that
   never appear in issue trackers — silent failures, platform-specific
   edge cases, and compatibility regressions.

However, Forge Shell runs on developer machines with access to file systems,
environment variables, secrets, and production infrastructure. Telemetry that
captures any of this data would be a serious privacy and security violation.
The telemetry model must be designed around what is explicitly collected —
not what is filtered out.

---

## Design

### 1. Core Principle — Opt-in, Not Opt-out

Forge Shell telemetry is **opt-in**. No data is collected by default.

This is the opposite of most tools — but it's the right choice for a shell:

- A shell has broader system access than most tools
- Developers are privacy-conscious — opt-out creates distrust
- Opt-in data is higher quality — users who opt in are typically more engaged
- The developer community will scrutinise telemetry decisions closely

**First-run prompt:**

```
Forge Shell would like to collect anonymous usage data to improve the product.

This includes:
  ✓ Which built-in commands are used (no arguments)
  ✓ Which stdlib modules are imported (no values)
  ✓ Error codes (no messages or context)
  ✓ Platform and architecture
  ✓ Forge Shell version

This never includes:
  ✗ Command arguments or flags
  ✗ File paths, file names, or file contents
  ✗ Environment variable names or values
  ✗ Script contents
  ✗ Network addresses or URLs
  ✗ Any personally identifiable information

Would you like to enable telemetry? [y/N]
```

Default is **N** — no telemetry without explicit consent.

**Change at any time:**

```bash
forge telemetry enable     # opt in
forge telemetry disable    # opt out
forge telemetry status     # show current setting
forge telemetry show       # show what would be sent — always available
```

---

### 2. What Is Collected — Explicit Allowlist

Telemetry is defined by an allowlist — only these fields are ever transmitted.
Everything else is never collected, regardless of implementation.

| Field | Example | Purpose |
|---|---|---|
| `forge_version` | `"1.2.0"` | Version adoption tracking |
| `platform` | `"linux"` | Platform distribution |
| `arch` | `"x86_64"` | Architecture distribution |
| `event_type` | `"command"` \| `"error"` \| `"startup"` | Event classification |
| `command_name` | `"ls"` | Built-in command usage frequency |
| `stdlib_module` | `"forge::json"` | Stdlib module adoption |
| `error_code` | `"E042"` | Error frequency — code only, no message |
| `plugin_count` | `3` | Plugin ecosystem health |
| `deprecated_api` | `"forge::crypto::hash_file"` | Deprecation usage — critical for RFC-014 |
| `startup_ms` | `42` | Startup performance |
| `session_type` | `"interactive"` \| `"script"` \| `"mcp"` | Usage mode distribution |
| `install_id` | random UUID, rotated monthly | Session deduplication — never tied to identity |

**`install_id` rotation:** The install ID is a random UUID that is rotated
monthly. It cannot be used to track a user across months. It is used only
to deduplicate events within a reporting period.

---

### 3. What Is NEVER Collected — Explicit Denylist

These are hard guarantees — not filters applied to collected data, but things
the telemetry system is architecturally incapable of capturing:

| Category | Examples |
|---|---|
| Command arguments | `rm /home/user/secret.txt` — the `/home/user/secret.txt` part |
| File paths | Any path on the user's system |
| File contents | Any data read from disk |
| Environment variables | Names or values |
| Script contents | Source code of `.fgs` files |
| Network addresses | URLs, hostnames, IP addresses |
| Plugin names | Installed plugin identities |
| Error messages | Only error codes — never the message text |
| Output data | Any data produced by commands |
| Personal information | Name, email, username, machine name |
| Credentials | API keys, tokens, passwords |
| Git data | Repository names, commit messages, branch names |

**Architectural guarantee:** The telemetry event builder only has access to
the allowlist fields. It does not receive command arguments, file paths, or
any other sensitive data. This is enforced at the type level in Rust — not
by filtering.

---

### 4. Telemetry Events

**Startup event — emitted once per session:**

```json
{
  "event":        "startup",
  "forge_version": "1.2.0",
  "platform":     "linux",
  "arch":         "x86_64",
  "session_type": "interactive",
  "install_id":   "a1b2c3d4-...",
  "timestamp":    "2026-04-15T09:00:00Z"
}
```

**Command event — emitted per built-in command invocation:**

```json
{
  "event":        "command",
  "command_name": "ls",
  "session_type": "script",
  "install_id":   "a1b2c3d4-...",
  "timestamp":    "2026-04-15T09:00:01Z"
}
```

**Stdlib import event — emitted per module import:**

```json
{
  "event":         "stdlib_import",
  "stdlib_module": "forge::json",
  "install_id":    "a1b2c3d4-...",
  "timestamp":     "2026-04-15T09:00:01Z"
}
```

**Error event — emitted on compile-time or runtime errors:**

```json
{
  "event":      "error",
  "error_code": "E042",
  "install_id": "a1b2c3d4-...",
  "timestamp":  "2026-04-15T09:00:02Z"
}
```

**Deprecated API event — critical for RFC-014 decision making:**

```json
{
  "event":          "deprecated_api",
  "deprecated_api": "forge::crypto::hash_file",
  "install_id":     "a1b2c3d4-...",
  "timestamp":      "2026-04-15T09:00:01Z"
}
```

---

### 5. Data Transmission

**Batching:** Events are batched locally and transmitted once per day —
not in real time. This reduces network overhead and makes the transmission
pattern less observable.

**Transmission endpoint:** `telemetry.forge-shell.dev` — HTTPS only.

**Failure handling:** If transmission fails — network unavailable, endpoint
down — events are dropped silently. No retry queue, no local storage of
events beyond the current day's batch. Telemetry failure never affects shell
behaviour.

**No third-party analytics:** Telemetry data goes directly to
`telemetry.forge-shell.dev` — not to Google Analytics, Mixpanel, Segment,
or any third-party service. The pipeline is self-hosted and open source.

---

### 6. Open Source Telemetry Pipeline

The telemetry pipeline is fully open source — hosted at
`github.com/forge-shell/telemetry`:

- **Collector:** The server-side collector that receives events
- **Aggregator:** The aggregation layer that produces anonymised summaries
- **Dashboard:** The public-facing dashboard at `telemetry.forge-shell.dev/public`

**Public dashboard:** Aggregated, anonymised telemetry is published publicly
at `telemetry.forge-shell.dev/public`. Anyone can see:

- Command usage frequency across the ecosystem
- Platform distribution
- Stdlib module adoption rates
- Error code frequency
- Deprecated API usage rates

Raw events are never published — only aggregated summaries.

**Why open source:** Trust is built through transparency. Developers can
inspect exactly what the collector receives, how it processes events, and
what the aggregator produces. There is no "trust us" — the code is the proof.

---

### 7. Telemetry and the Deprecation Process

The `deprecated_api` event is the most important telemetry event for
RFC-014's 2.0 decision framework.

When a function is deprecated in `1.x.0`, the telemetry dashboard shows:

```
Deprecated API Usage — forge::crypto::hash_file
  Deprecated since: 1.3.0
  Sessions using this API: 2.3% of opted-in sessions
  Trend: ↓ declining (was 8.1% at deprecation)
  Recommendation: Safe to remove in 2.0.0
```

This gives the team objective data for the 2.0 decision:

- If usage is high and not declining → extend the deprecation window
- If usage is low and declining → safe to proceed with removal
- If usage is zero → can be removed in a patch release (exceptional case)

---

### 8. CI and Enterprise Environments

Telemetry is automatically disabled in:

- CI environments — detected via `CI=true`, `GITHUB_ACTIONS`, `JENKINS_URL` etc.
- Non-interactive sessions — `forge run script.fgs` without a TTY
- Environments where `FORGE_TELEMETRY=0` is set

Enterprise users can set `FORGE_TELEMETRY=0` in their base environment to
ensure telemetry is never enabled across their fleet — no per-machine
configuration required.

---

### 9. `forge telemetry show`

Available to all users regardless of opt-in status. Shows exactly what
would be transmitted if telemetry were enabled:

```bash
forge telemetry show

Telemetry status: disabled

Events that would be sent in the last session:
  startup      forge_version=1.2.0 platform=linux arch=x86_64 session_type=interactive
  command      command_name=ls
  command      command_name=grep
  stdlib_import stdlib_module=forge::json
  command      command_name=fetch

Note: command arguments, paths, and values are never included.
Enable telemetry: forge telemetry enable
```

This builds trust — users can see the exact data before deciding to opt in.

---

## Drawbacks

- **Opt-in yields smaller dataset.** Opt-out would produce more data. Accepted
  trade-off — user trust is more valuable than dataset size for a shell.
- **CI auto-disable means no CI usage data.** The team won't see which
  commands are most used in CI workflows. Accepted — CI environments should
  never transmit telemetry.
- **Self-hosted pipeline requires maintenance.** Running a telemetry
  collector is operational work. Mitigated by keeping the pipeline simple
  and the data volume low (opt-in).

---

## Alternatives Considered

### Alternative A — Opt-out telemetry

**Rejected:** A shell with opt-out telemetry will be scrutinised harshly by
the developer community. The trust cost outweighs the data volume benefit.
Go, Rust, and most developer tools that tried opt-out faced significant
backlash. Opt-in is the correct choice for developer tools.

### Alternative B — Third-party analytics (Mixpanel, Segment)

**Rejected:** Third-party analytics sends user data to an external party.
Even anonymised data sent to Mixpanel means Mixpanel has data about Forge
Shell users. Self-hosted, open-source pipeline is the only acceptable model.

### Alternative C — No telemetry

**Rejected:** Without deprecation usage data, the RFC-014 2.0 decision
framework has no objective foundation. The team would be guessing about
which deprecated APIs are safe to remove. Opt-in telemetry with a public
dashboard is a reasonable middle ground.

### Alternative D — Telemetry via GitHub Issues (manual)

**Rejected:** Manual reporting via issues captures only the most vocal users.
Passive telemetry — even opt-in — captures the silent majority who never
file issues but whose usage patterns matter.

---

## Unresolved Questions

- [ ] What is the retention period for raw telemetry events before aggregation?
- [ ] Should the public dashboard be updated daily or weekly?
- [ ] How are `install_id` UUIDs generated and stored — in `config.toml`?
- [ ] Should telemetry be disabled when `forge mcp serve` is running —
      an agent session may involve sensitive operations?
- [ ] What is the process for a telemetry security incident — if the
      collector is compromised, what is the disclosure and remediation plan?
- [ ] Should plugin authors have access to telemetry about their plugin's
      usage — command invocation counts only, no user data?
- [ ] Should `forge telemetry show` be available even in CI environments
      where telemetry is auto-disabled?

---

## Implementation Plan

### Affected Crates / Infrastructure

| Component | Responsibility |
|---|---|
| `forge-telemetry` | Event builder — allowlist enforced at type level |
| `forge-cli/telemetry` | `forge telemetry` subcommands |
| `forge-engine` | Emit command and error events |
| `forge-lang` | Emit stdlib import and deprecated API events |
| `telemetry-collector` | Server-side collector (separate repo) |
| `telemetry-aggregator` | Aggregation pipeline (separate repo) |
| `telemetry-dashboard` | Public dashboard (separate repo) |

### Dependencies

- Requires RFC-014 (release policy) — deprecated API tracking is core purpose
- Requires RFC-013 (shell config) — telemetry opt-in stored in `config.toml`

### Milestones

1. Design and implement `forge-telemetry` event builder — type-safe allowlist
2. Implement opt-in prompt on first run
3. Implement `forge telemetry enable/disable/status/show` subcommands
4. Implement CI environment auto-detection and disable
5. Implement event batching and daily transmission
6. Implement `FORGE_TELEMETRY=0` env var override
7. Build telemetry-collector — open source, self-hosted
8. Build telemetry-aggregator — anonymised summaries
9. Build public dashboard — `telemetry.forge-shell.dev/public`
10. Implement deprecated API usage tracking and dashboard view

---

## References

- [Go Telemetry Proposal](https://research.swtch.com/telemetry-opt-in)
- [Homebrew Analytics](https://docs.brew.sh/Analytics)
- [VS Code Telemetry](https://code.visualstudio.com/docs/getstarted/telemetry)
- [Next.js Telemetry](https://nextjs.org/telemetry)
- [RFC-014 — Release Policy & Versioning](./RFC-014-release-policy.md)
- [RFC-013 — Shell Configuration Model](./RFC-013-shell-config.md)