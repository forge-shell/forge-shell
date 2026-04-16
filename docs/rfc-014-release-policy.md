# RFC-014 — Release Policy & Versioning

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

This RFC defines the release policy and versioning model for Forge Shell —
covering the version scheme, release cadence, compatibility guarantees, the
deprecation process, the 2.0 decision framework, support windows, pre-release
labels, and the `forge::experimental` namespace. The model follows Go-style
compatibility: no breaking changes within a major version, ever.

---

## Motivation

A shell is infrastructure. Developers embed Forge Shell scripts in CI
pipelines, production deployment systems, and daily workflows. These scripts
must not break unexpectedly when Forge Shell is upgraded. A clear, public
release policy gives developers the confidence to build on Forge Shell and
operators the predictability to manage upgrades.

Without a formal policy, versioning decisions are made ad-hoc — breaking user
trust and creating ecosystem fragmentation.

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

**`forge --version` output:**

```bash
forge --version
forge 1.2.0 (stdlib 1.2.0, abi 1)
```

- `stdlib` version always matches the binary version — no separate stdlib versioning
- `abi` is the plugin ABI version — an independent integer (RFC-005)

---

### 2. Release Cadence

| Release type | Cadence | Contents |
|---|---|---|
| **Patch** (`1.0.x`) | As needed — within 48h for security | Security fixes, critical bug fixes only. Never new features. |
| **Minor** (`1.x.0`) | Quarterly — every 3 months | New stdlib modules, new built-ins, new MCP resources, performance improvements. Always backward compatible. |
| **Major** (`x.0.0`) | When breaking changes genuinely accumulate | Breaking changes only. High bar — see Section 5. |

---

### 3. Compatibility Guarantee

> Any `.fgs` script that compiles correctly with Forge Shell v1.x will compile
> and run correctly with any future Forge Shell v1.y release, where y > x.

This is the core promise. It covers:
- ForgeScript syntax — no keywords removed, no operator semantics changed
- Standard library API — no function signatures changed, no functions removed
- Built-in command behaviour — no flags removed, no output format changes
- Plugin ABI — ABI v1 plugins continue to work throughout 1.x

| Change type | Policy |
|---|---|
| Bug fixes | Any patch release — behaviour corrected, API unchanged |
| New stdlib functions | Any minor release — additive only |
| New built-in commands | Any minor release — additive only |
| New keywords | Any minor release — additive only |
| Function signature changes | Never within a major version |
| Function removal | Never within a major version |
| Built-in flag removal | Never within a major version |
| Security vulnerability fixes | May change behaviour — documented security advisory |

**The one exception:** Security vulnerabilities. If a stdlib function has a
security flaw, the behaviour may change in a minor release. This is the same
exception Go makes. Safety trumps compatibility in this case only.

---

### 4. Deprecation Process

Breaking changes within a major version are never allowed. Changes that would
break compatibility must go through the deprecation process:

```
1.x.0  → Feature works normally
1.y.0  → Feature deprecated — compile-time warning emitted
           Migration guide published
           forge migrate updated to handle mechanical migrations
           (minimum one minor release — ideally two)
2.0.0  → Feature removed — forge migrate handles ≥80% of migrations
           Full migration guide in release notes
```

**Deprecation annotation:**

```forge
#[deprecated(since = "1.3.0", replaced_by = "forge::crypto::sha256_file")]
fn forge::crypto::hash_file(p: path) -> Result<str>
```

- Deprecated functions compile with a **warning** — never an error within 1.x
- Deprecated functions are removed only in the next major version
- `forge check` can be run with `--warn-deprecated` to surface all deprecated usage

---

### 5. The 2.0 Decision Framework

Cutting a major version is a significant event — it breaks the compatibility
promise and requires every user to migrate. The bar is deliberately high.

**Wrong reasons to cut a 2.0:**

| Reason | Why it's wrong |
|---|---|
| "We've added a lot of features" | Features are 1.x — they don't require breaking changes |
| "The codebase needs a rewrite" | Internal rewrites don't break user-facing APIs |
| "It's been a year" | Time alone is not a reason |
| "We want a marketing moment" | Vanity versioning destroys trust |

**The three questions:**

Before cutting 2.0, all three must be answered yes:

1. **Is this genuinely impossible to ship in 1.x without breaking the compatibility guarantee?**
   Most changes can be done additively. Only proceed if the answer is truly no.

2. **Have the breaking changes completed the deprecation runway?**
   Every breaking change must have been deprecated for at least one minor release.
   If the deprecation runway hasn't completed, 2.0 is not ready.

3. **Can `forge migrate` handle ≥80% of migrations mechanically?**
   The remaining 20% must have a clear, documented manual migration path.
   If the migration story is incomplete, 2.0 is not ready.

**The right trigger — a cluster of related breaks:**

2.0 happens when multiple breaking changes accumulate that together justify a
migration event. A single breaking change is almost never worth a major version.
Announce breaking changes early, accumulate them, and ship them together so
users migrate once — not multiple times.

**Realistic timeline:**

```
1.0.0   → 2026 — first stable release
1.1-1.5 → 2026-2028 — grow ecosystem, accumulate learnings
2.0.0   → 2028-2029 at earliest — only if genuine breaks accumulate
```

Go has been at 1.x since 2012. That's a strength, not a failure.

---

### 6. Support Windows

| Version | Support |
|---|---|
| Current minor (`1.2.x`) | Active development + patches |
| Previous minor (`1.1.x`) | Security patches only |
| Older minors (`1.0.x`) | No support — upgrade encouraged |
| Previous major (`1.x.x`) after `2.0.0` | Security patches for 12 months |

---

### 7. Pre-release Labels

Before `1.0.0`, semver pre-release labels communicate stability:

```
0.1.0-alpha.1   → early development — APIs unstable
0.2.0-beta.1    → feature complete — stabilising
0.3.0-rc.1      → release candidate — final testing
1.0.0           → stable — compatibility guarantee begins
```

The compatibility guarantee does NOT apply to pre-release versions.

---

### 8. `forge::experimental` Namespace

Modules in `forge::experimental` are available for early use but are NOT
covered by the compatibility guarantee. They can change between minor releases.

```forge
import forge::experimental::net::websocket
```

**Graduation criteria — from experimental to stable:**

1. Module has been in `forge::experimental` for at least one minor release
2. No breaking API changes needed based on community feedback
3. Core team signs off on the API as stable
4. Moved to `forge::` proper in a minor release — with a compile-time notice

---

### 9. Plugin ABI Versioning

Plugin ABI versioning is independent from shell versioning (RFC-005):

- Plugin ABI is an integer — `abi = "1"`, `abi = "2"`
- ABI version incremented only on breaking changes to host function interface
- Multiple ABI versions supported simultaneously — 12-month deprecation window
- `forge plugin check` warns if an installed plugin targets a deprecated ABI

---

### 10. Release Channels — Post-v1

Not required for v1 launch. Planned for post-v1:

| Channel | Cadence | Audience |
|---|---|---|
| `stable` | Quarterly minor releases | Production use |
| `beta` | 4 weeks before stable | Early adopters |
| `nightly` | Daily builds | Plugin authors, contributors |

For v1: stable channel only.

---

## Drawbacks

- **High bar for 2.0 may accumulate technical debt.** If breaking changes are
  never made, ForgeScript may carry legacy decisions permanently. Mitigated by
  the deprecation process and the `forge::experimental` safety valve.
- **Quarterly cadence may feel slow.** Feature requests may wait up to 3 months
  for a minor release. Mitigated by the nightly channel (post-v1) and the
  `forge::experimental` namespace for early access.

---

## Alternatives Considered

### Alternative A — Go-style version numbering (1.21, 1.22)

**Rejected:** Go's numbering (`go1.21`) predates semver adoption. Semver is
universally understood, machine-parseable, and consistent with the plugin
ecosystem which already uses semver. No benefit to diverging from semver.

### Alternative B — Six-week Rust-style releases

**Rejected:** Six-week cadence is appropriate for a programming language with
a large contributor base. For a shell, quarterly cadence gives time for
thorough testing across three platforms and reduces upgrade fatigue for
operators.

### Alternative C — No formal compatibility guarantee

**Rejected:** A shell without a compatibility guarantee cannot be adopted in
production infrastructure. The Go compatibility promise is one of the most
powerful reasons Go succeeded in DevOps. ForgeScript needs the same foundation.

---

## Unresolved Questions

- [ ] What is the exact process for announcing a security vulnerability and
      shipping a patch — responsible disclosure timeline?
- [ ] Should patch releases require a full CI matrix run or an expedited path
      for security-critical fixes?
- [ ] How is the compatibility guarantee tested? A regression test suite of
      1.0.0 scripts run against every future release?
- [ ] Should there be a formal RFC process for deprecations — or is a
      `#[deprecated]` annotation in the code sufficient?
- [ ] What is the governance model for deciding when 2.0 is ready?

---

## Implementation Plan

### Affected Crates / Infrastructure

| Component | Responsibility |
|---|---|
| `forge-lang` | `#[deprecated]` annotation support, deprecation warnings |
| CI pipeline | Three-platform matrix on every release |
| `forge check` | `--warn-deprecated` flag |
| `forge migrate` | Updated for each breaking change in 2.x |
| Release tooling | Automated semver tagging, changelog generation |

### Milestones

1. Document compatibility guarantee in `ARCHITECTURE.md` and website
2. Implement `#[deprecated]` annotation and compile-time warning
3. Implement `forge check --warn-deprecated`
4. Establish CI release pipeline — three-platform matrix
5. Define responsible disclosure process for security vulnerabilities
6. Define governance model for 2.0 decision

---

## References

- [Go Compatibility Promise](https://go.dev/doc/go1compat)
- [Semantic Versioning 2.0.0](https://semver.org/)
- [Go Release Policy](https://go.dev/doc/devel/release)
- [Rust Release Channels](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html)
- [RFC-005 — Plugin System (ABI versioning)](./RFC-005-plugin-system.md)
- [RFC-010 — Standard Library (forge::experimental)](./RFC-010-standard-library.md)
- [RFC-011 — forge migrate](./RFC-011-forge-migrate.md)