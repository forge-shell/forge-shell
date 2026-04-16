# RFC-014 — Release Policy & Versioning

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

This RFC defines the release policy and versioning model for Forge Shell —
covering the version scheme, release cadence, compatibility guarantees, the
deprecation process, the 2.0 decision framework, security vulnerability
disclosure, CI requirements, compatibility testing, and governance. The model
follows Go-style compatibility: no breaking changes within a major version,
ever.

---

## Motivation

A shell is infrastructure. Developers embed Forge Shell scripts in CI
pipelines, production deployment systems, and daily workflows. These scripts
must not break unexpectedly when Forge Shell is upgraded. A clear, public
release policy gives developers the confidence to build on Forge Shell and
operators the predictability to manage upgrades.

---

## Design

### 1. Version Scheme — Semantic Versioning

Forge Shell uses semantic versioning (`MAJOR.MINOR.PATCH`):

```
1.0.0   → first stable release
1.0.1   → patch — security fix or critical bug
1.1.0   → minor — new features, backward compatible
2.0.0   → major — breaking changes, migration required
```

```bash
forge --version
forge 1.2.0 (stdlib 1.2.0, abi 1)
```

- `stdlib` version always matches the binary version
- `abi` is the plugin ABI version — an independent integer (RFC-005)

---

### 2. Release Cadence

| Release type | Cadence | Contents |
|---|---|---|
| **Patch** (`1.0.x`) | As needed — within 48h for critical security | Security fixes, critical bugs only. Never new features. |
| **Minor** (`1.x.0`) | Quarterly | New stdlib, built-ins, MCP resources, performance. Always backward compatible. |
| **Major** (`x.0.0`) | When breaking changes genuinely accumulate | Breaking changes only. High bar — see Section 5. |

---

### 3. Compatibility Guarantee

> Any `.fgs` script that compiles correctly with Forge Shell v1.x will compile
> and run correctly with any future Forge Shell v1.y release, where y > x.

| Change type | Policy |
|---|---|
| Bug fixes | Any patch — API unchanged |
| New stdlib functions | Any minor — additive only |
| New built-in commands | Any minor — additive only |
| Function signature changes | Never within a major version |
| Function removal | Never within a major version |
| Security vulnerability fixes | May change behaviour — security advisory published |

---

### 4. Deprecation Process

**Tiered by scope:**

| Scope | Process |
|---|---|
| Single function, flag, or argument | `#[deprecated]` annotation + CHANGELOG entry |
| Module, syntax, built-in, ABI version | Full RFC required |

**Threshold:** If `forge migrate` handles the migration in < 50 lines of translation rules → annotation only. Otherwise → RFC.

**Annotation format:**

```forge
#[deprecated(since = "1.2.0", replaced_by = "forge::crypto::sha256_file")]
fn forge::crypto::hash_file(p: path) -> Result<str>
```

Deprecated functions compile with a **warning** — never an error within 1.x.
Removed only in the next major version.

**Process timeline:**

```
1.x.0  → feature works normally
1.y.0  → deprecated — warning emitted, migration guide published
           (minimum one minor release — ideally two)
2.0.0  → removed — forge migrate handles ≥80% mechanically
```

---

### 5. The 2.0 Decision Framework

**Wrong reasons to cut a 2.0:**

| Reason | Why it's wrong |
|---|---|
| "We've added lots of features" | Features are 1.x — additive |
| "Codebase needs a rewrite" | Internal rewrites don't break user APIs |
| "It's been a year" | Time alone is not a reason |
| "Marketing moment" | Vanity versioning destroys trust |

**The three questions — all must be yes:**

1. **Impossible to ship in 1.x without breaking the compatibility guarantee?**
2. **Have the breaking changes completed the deprecation runway?**
3. **Can `forge migrate` handle ≥80% of migrations mechanically?**

**The 2.0 RFC process:**

```
RFC opened → 60-day public comment period
    ↓
Core team ratification
    ↓
2.0 development (6 months) → beta → release
```

The 2.0 RFC lists all breaking changes, deprecation status, `forge migrate`
coverage, and telemetry data on deprecated API usage. The 60-day comment
period gives the community — including plugin authors and enterprise users —
visibility and input before the decision is locked.

**Realistic timeline:**

```
1.0.0   → 2026 — first stable release
1.1-1.5 → 2026-2028 — grow ecosystem, accumulate learnings
2.0.0   → 2028-2029 at earliest
```

Go has been at 1.x since 2012. That is a strength, not a failure.

---

### 6. Support Windows

| Version | Support |
|---|---|
| Current minor (`1.2.x`) | Active development + patches |
| Previous minor (`1.1.x`) | Security patches only |
| Older minors (`1.0.x`) | No support — upgrade encouraged |
| Previous major (`1.x.x`) after `2.0.0` | Security patches for 12 months |

---

### 7. Security Vulnerability Disclosure

**Contact:** `security@forge-shell.dev` — acknowledged within 24 hours.

**Severity-based timelines:**

| Severity | Patch target | Disclosure deadline |
|---|---|---|
| Critical (RCE, credential leak) | 48 hours | 7 days |
| High (privilege escalation) | 7 days | 30 days |
| Medium (DoS, info disclosure) | 30 days | 90 days |
| Low | Next minor release | 90 days |

Day 90 is a hard deadline — disclose with mitigations if fix not ready.
CVE assigned via GitHub Security Advisory for critical and high severity.

---

### 8. CI Requirements — Full Matrix Always

Every release — including security patches — runs the full CI matrix.
No expedited path that skips platforms.

**CI pipeline targets:**

```
Phase 1 (5 min)   — syntax check + type check + unit tests (all platforms parallel)
Phase 2 (10 min)  — integration tests (all platforms parallel)
Phase 3 (5 min)   — release artefact build + smoke test

Total target: < 20 minutes
```

Speed comes from parallelism and caching — not from skipping platforms.

---

### 9. Compatibility Testing — `forge-compat` Suite

A dedicated compatibility test suite at `github.com/forge-shell/forge-compat`,
grouped by version. Run on every CI build.

**Repository structure:**

```
forge-compat/
    v1.0/
        scripts/
            ls-basic.fgs
            json-parse.fgs
            env-access.fgs
        expected/
            ls-basic.txt
            json-parse.txt
    v1.1/
        scripts/
            net-lookup.fgs
            yaml-parse.fgs
        expected/
            ...
    run.sh    # runs ALL version groups against current binary
```

**CI output:**

```bash
v1.0 scripts: 42 passed, 0 failed ✅
v1.1 scripts: 38 passed, 0 failed ✅
```

A failure in any version group is a **compatibility guarantee violation** —
treated as a blocker, never a warning.

**Suite growth:**
- Every bug report involving behavioural regression → new test added
- Every new stdlib function → basic usage test added
- Every deprecated API → test that it still works until removal
- Every version release → new version group added

---

### 10. Pre-release Labels

```
0.1.0-alpha.1   → early development — APIs unstable
0.2.0-beta.1    → feature complete — stabilising
0.3.0-rc.1      → release candidate — final testing
1.0.0           → stable — compatibility guarantee begins
```

Compatibility guarantee does NOT apply to pre-release versions.

---

### 11. `forge::experimental` Namespace

Modules not yet covered by the compatibility guarantee — can change between
minor releases.

```forge
import forge::experimental::net::websocket
```

**Graduation criteria:**
1. In `forge::experimental` for at least one minor release
2. No breaking API changes needed based on community feedback
3. Core team sign-off

Graduated to `forge::` proper in a minor release — compile-time notice issued.

---

### 12. Plugin ABI Versioning

Independent from shell versioning (RFC-005):
- Plugin ABI is an integer — `abi = "1"`, `abi = "2"`
- ABI version incremented only on breaking changes to host function interface
- Multiple ABI versions supported simultaneously — 12-month deprecation window
- `forge plugin check` warns if installed plugin targets a deprecated ABI

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | Responsible disclosure timeline | 90-day coordinated, severity-based |
| UQ-2 | CI matrix for patches | Full matrix always — < 20 minute target |
| UQ-3 | Compatibility testing | `forge-compat` suite, grouped by version |
| UQ-4 | Deprecation process | Tiered — annotation for minor, RFC for major |
| UQ-5 | 2.0 governance | RFC + 60-day comment + core team ratification |

---

## Implementation Plan

### Affected Components

| Component | Responsibility |
|---|---|
| `forge-lang` | `#[deprecated]` annotation, deprecation warnings |
| CI pipeline | Three-platform matrix, < 20 min target |
| `forge check` | `--warn-deprecated` flag |
| `forge migrate` | Updated for each breaking change in 2.x |
| `forge-compat` | Compatibility test suite — separate repository |
| Release tooling | Semver tagging, changelog generation |

### Milestones

1. Document compatibility guarantee in `ARCHITECTURE.md` and website
2. Implement `#[deprecated]` annotation and compile-time warning
3. Implement `forge check --warn-deprecated`
4. Establish CI release pipeline — three-platform matrix, < 20 min
5. Create `forge-compat` repository — initial v1.0 test scripts
6. Define responsible disclosure process — `security@forge-shell.dev`
7. Implement CVE advisory workflow via GitHub Security Advisory

---

## References

- [Go Compatibility Promise](https://go.dev/doc/go1compat)
- [Semantic Versioning 2.0.0](https://semver.org/)
- [Go Release Policy](https://go.dev/doc/devel/release)
- [GitHub Security Advisory](https://docs.github.com/en/code-security/security-advisories)
- [RFC-005 — Plugin System (ABI versioning)](./RFC-005-plugin-system.md)
- [RFC-010 — Standard Library (forge::experimental)](./RFC-010-standard-library.md)
- [RFC-011 — forge migrate](./RFC-011-forge-migrate.md)
- [RFC-015 — Telemetry & Diagnostics](./RFC-015-telemetry.md)