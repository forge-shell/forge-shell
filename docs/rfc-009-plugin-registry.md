# RFC-009 — Plugin Registry & Distribution

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

This RFC defines the Forge Shell plugin registry — the infrastructure for
discovering, publishing, installing, and verifying third-party plugins. The
registry uses a Go-style decentralised model: no hosted binaries, a static
index at `plugins.forge-shell.dev`, Sigstore keyless signing via GitHub OIDC,
and a transparency log at `sum.forge-shell.dev`. Plugin discovery uses a
locally cached index with background refresh. Publisher namespaces prevent
typosquatting. This RFC formalises the distribution model established in
RFC-005.

---

## Motivation

The plugin system defined in RFC-005 establishes how plugins work internally.
This RFC addresses how users discover and install them, how plugin authors
publish them, and how the ecosystem maintains security and trust without
requiring Forge Shell to host and maintain a centralised registry.

The registry must be:
- **Simple** — easy to publish to, easy to install from
- **Secure** — plugins verified before installation, no long-lived keys to steal
- **Decentralised-friendly** — no hosted binaries, users can host private registries
- **Discoverable** — full-text search, categories, tags

---

## Design

### 1. Registry Architecture — Decentralised, Static

The official registry hosts no plugin binaries. It is a static site — a set
of JSON files served from a CDN. No dynamic API. No server-side compute.

| Component | What it is | Hosting cost |
|---|---|---|
| `plugins.forge-shell.dev` | Static discovery index — JSON files | Minimal — CDN |
| `sum.forge-shell.dev` | Append-only content hash log | Minimal — static |
| `github.com/forge-shell/plugin-index` | Index source — TOML files, PR-based | Zero |
| Official plugins (`forge-shell/forge-*`) | GitHub repos — source + releases | Zero |

Forge Shell never hosts plugin binaries. All binaries are fetched directly
from plugin authors' GitHub release pages or other hosting.

---

### 2. Plugin Index Format

The index is a single JSON file at `plugins.forge-shell.dev/index.json`.

```json
{
  "version": 1,
  "generated_at": "2026-04-15T09:00:00Z",
  "plugins": [
    {
      "name":           "forge-git",
      "vanity":         "forge-git",
      "source":         "github.com/forge-shell/forge-git",
      "description":    "Git integration for Forge Shell",
      "category":       "version-control",
      "tags":           ["git", "github", "version-control"],
      "official":       true,
      "verified":       true,
      "signing_method": "sigstore",
      "github":         "forge-shell",
      "latest":         "1.2.0",
      "versions": [
        {
          "version":    "1.2.0",
          "wasm_url":   "https://github.com/forge-shell/forge-git/releases/download/v1.2.0/forge-git.wasm",
          "sha256":     "abc123...",
          "rekor_uuid": "uuid-of-rekor-transparency-log-entry",
          "yanked":     false
        },
        {
          "version":    "1.1.0",
          "wasm_url":   "https://github.com/forge-shell/forge-git/releases/download/v1.1.0/forge-git.wasm",
          "sha256":     "def456...",
          "rekor_uuid": "uuid-of-rekor-transparency-log-entry",
          "yanked":     false
        }
      ]
    },
    {
      "name":           "forge-kubectl",
      "vanity":         "ajitem/forge-kubectl",
      "source":         "github.com/ajitem/forge-kubectl",
      "description":    "Kubernetes and Helm integration",
      "category":       "orchestration",
      "tags":           ["kubernetes", "helm", "k8s", "containers"],
      "official":       false,
      "verified":       true,
      "signing_method": "sigstore",
      "github":         "ajitem-sahasrabuddhe",
      "latest":         "2.1.0",
      "versions": [...]
    }
  ]
}
```

**Index source — `plugin-index/plugins.toml`:**

```toml
[[plugin]]
name        = "forge-git"
vanity      = "forge-git"
source      = "github.com/forge-shell/forge-git"
description = "Git integration for Forge Shell"
category    = "version-control"
tags        = ["git", "github", "version-control"]
official    = true
verified    = true
github      = "forge-shell"
```

---

### 3. Publisher Namespaces

Publisher namespaces prevent typosquatting and establish identity ownership.

**Registration — PR to `plugin-index/publishers.toml`:**

```toml
[[publisher]]
namespace      = "ajitem"
github         = "ajitem-sahasrabuddhe"
signing_method = "sigstore"
verified       = true
```

**Rules:**
- Namespace claimed via PR from the author's authenticated GitHub account
- Once claimed, only that publisher can register `namespace/*` vanity URLs
- Namespace transfer requires team review — not self-service
- Squatting policy: inactive namespaces (no plugins, >12 months) can be reclaimed

---

### 4. Signing — Sigstore Keyless

Forge Shell uses Sigstore keyless signing — no long-lived private keys.

**Why Sigstore:**
- No private keys to steal, rotate, or revoke
- GitHub OIDC is the trust anchor — same identity used for namespace registration
- Short-lived certificates (10 minutes) — ephemeral, no key management
- Rekor transparency log provides public auditability
- `cosign` GitHub Action makes CI signing zero-configuration

**How it works:**

```
Author pushes release tag on GitHub
         ↓
GitHub Actions workflow runs
         ↓
cosign signs forge-kubectl.wasm using GitHub OIDC
         ↓
Sigstore issues ephemeral certificate tied to github.com/ajitem-sahasrabuddhe
         ↓
Signature + certificate recorded in Rekor transparency log
         ↓
Author submits PR to plugin-index with wasm_url, sha256, rekor_uuid
```

**Verification on install:**

```
forge plugin install ajitem/forge-kubectl
         ↓
Fetch forge-kubectl.wasm from wasm_url
         ↓
Verify SHA-256 matches index entry
         ↓
Fetch Rekor entry by rekor_uuid
         ↓
Verify: signature valid + certificate from Sigstore + OIDC identity matches
        registered publisher (ajitem-sahasrabuddhe)
         ↓
Install to ~/.config/forge/plugins/forge-kubectl/
```

**Plugin manifest signing in CI:**

```yaml
# .github/workflows/release.yml
- name: Sign plugin
  uses: sigstore/cosign-action@v3
  with:
    files: forge-kubectl.wasm
# cosign uses GitHub OIDC automatically — no keys configured
```

**Revocation:** GitHub account revocation handled by GitHub. No revocation
list maintained by Forge Shell. If a publisher's GitHub account is compromised,
GitHub revokes their OIDC token — new signatures cannot be issued for that
identity. Existing verified signatures remain valid.

---

### 5. Three-Tier Install Resolution

```
Tier 1 — Canonical short name (official + verified only)
  forge-git → plugins.forge-shell.dev → github.com/forge-shell/forge-git

Tier 2 — Publisher namespace (registered publishers)
  ajitem/forge-kubectl → plugins.forge-shell.dev → github.com/ajitem/forge-kubectl

Tier 3 — Direct source URL (no index needed)
  github.com/user/plugin@v1.0.0 → fetched directly
```

```bash
forge plugin install forge-git                      # Tier 1
forge plugin install forge-git@v1.2.0              # Tier 1 — pinned
forge plugin install ajitem/forge-kubectl           # Tier 2
forge plugin install ajitem/forge-kubectl@v2.0.0   # Tier 2 — pinned
forge plugin install github.com/user/plugin         # Tier 3
forge plugin install github.com/user/plugin@v1.0.0 # Tier 3 — pinned
```

---

### 6. Install Flow

```bash
forge plugin install ajitem/forge-kubectl
```

```
Resolve: ajitem/forge-kubectl → github.com/ajitem/forge-kubectl v2.1.0
Fetch:   forge-kubectl.wasm (1.2 MB)
Verify:  SHA-256 ✅
Verify:  Sigstore signature ✅ — github.com/ajitem-sahasrabuddhe

Installing: forge-kubectl v2.1.0
Source:      github.com/ajitem/forge-kubectl
Verified:    ✅ Reviewed by Forge Shell team
Capabilities: exec["kubectl", "helm"], filesystem:read, env:read
Limits:      memory: 64MB (↑ above default 32MB), cpu: 15s (↑ above default 5s)
             ⚠️  This plugin requests above-default resource limits
Proceed? [y/N] y

Installed: forge-kubectl v2.1.0
  → ~/.config/forge/plugins/forge-kubectl/
```

**Unverified plugin install:**

```
Installing: community-tool v1.0.0
Source:      github.com/unknown/community-tool
Verified:    ❌ Not reviewed by Forge Shell team
Capabilities: exec["*"], filesystem:read-write, network
             ⚠️  Broad capabilities — review source before installing
Proceed? [y/N]
```

---

### 7. Plugin Discovery — Local Index Search

```bash
# Search — downloads and caches index on first run
forge plugin search kubectl

  forge-kubectl           orchestration  ✅ verified  ★ official
  Kubernetes and Helm integration for Forge Shell
  forge plugin install forge-kubectl

  ajitem/forge-k8s-tools  orchestration  ✅ verified
  Extended Kubernetes tooling — kustomize, stern, k9s
  forge plugin install ajitem/forge-k8s-tools

# Browse by category
forge plugin search --category orchestration

# Filter by tag
forge plugin search --tag kubernetes

# Force index refresh
forge plugin search --refresh kubectl
```

**Index cache location:**

```
~/.cache/forge/plugin-index/
    index.json          # full plugin index
    index.json.etag     # ETag for conditional HTTP requests
    index.json.age      # last fetch timestamp
```

**Cache refresh policy:**
- Age < 1 hour → search cached index, no network request
- Age > 1 hour → background refresh, search cached version immediately
- `--refresh` → force fresh fetch before search

**Search field ranking:**

| Field | Weight |
|---|---|
| Name exact match | Highest |
| Name prefix match | High |
| Tag exact match | High |
| Category match | Medium |
| Description substring | Low |

---

### 8. Fixed Categories

```
version-control    # git, svn, mercurial
containers         # docker, podman, containerd
orchestration      # kubernetes, helm, nomad
cloud              # aws, gcp, azure, digitalocean
languages          # node, python, rust, go, java
databases          # postgres, mysql, redis, mongodb
networking         # ssh, http, dns, vpn
security           # signing, secrets, scanning
productivity       # aliases, completions, prompt themes
monitoring         # logging, metrics, alerting
devtools           # linting, formatting, testing
```

Each plugin declares exactly one category. Free-form tags supplement for
more specific discovery.

---

### 9. Yanking

A yanked version is not installed by default but remains downloadable.

```bash
# Yank via PR to plugin-index
# plugin-index maintainers can also yank for security reasons

# Installing a yanked version requires acknowledgement
forge plugin install ajitem/forge-kubectl@2.0.0
# Warning: v2.0.0 has been yanked: "Critical security vulnerability"
# Install anyway? [y/N]
```

Yanking is not deletion:
- Existing installations unaffected — warning shown on `forge plugin list`
- New installs require explicit confirmation
- Yank reason always displayed

---

### 10. Plugin Lifecycle Commands

```bash
# Discovery
forge plugin search kubectl
forge plugin search --category orchestration
forge plugin search --tag kubernetes
forge plugin search --refresh docker

# Installation
forge plugin install forge-git
forge plugin install forge-git@v1.2.0
forge plugin install ajitem/forge-kubectl
forge plugin install github.com/user/plugin@v1.0.0

# Management
forge plugin list                    # list installed plugins
forge plugin info forge-git          # show manifest + capabilities
forge plugin update forge-git        # update to latest
forge plugin update --all            # update all installed plugins
forge plugin remove forge-git        # remove plugin
forge plugin check                   # check for updates + deprecated ABIs

# Index management
forge plugin index refresh           # force index refresh
forge plugin index stats             # show index cache info
```

---

### 11. Private Registries

Private registries follow the same static JSON index format as the official
registry. Forge Shell ships `forge registry serve` to host a local registry
from a directory.

**Configuration in `config.toml`:**

```toml
[plugins.registries]
default  = "https://plugins.forge-shell.dev"
internal = "https://plugins.mycompany.internal"
```

```bash
# Install from private registry
forge plugin install mycompany-tools --registry internal

# Install from direct URL — bypasses registry, still verifies SHA-256
forge plugin install https://releases.mycompany.com/forge-tools-1.0.0.wasm \
  --sha256 abc123...
```

**Host a private registry:**

```bash
forge registry serve --dir ./my-plugins --port 8080
```

Private registries do not need Sigstore signing — SHA-256 verification is
always performed. For private use, content integrity is the primary concern.

---

## Drawbacks

- **Sigstore dependency.** Forge Shell's verification depends on Sigstore's
  Rekor infrastructure. If Rekor is unavailable, verification of new plugin
  installs is degraded. Mitigation: SHA-256 verification always runs
  independently — content integrity is never compromised.
- **PR-based publishing is slow.** Submitting a PR and waiting for merge is
  slower than `cargo publish`. Acceptable for v1 — the ecosystem is small
  enough that manual review adds value. A future automated publishing pipeline
  can be added when volume demands it.
- **Static index has no real-time updates.** The index is regenerated from
  `plugin-index` on merge — not instantly. A plugin published at 09:00 may
  not appear in search until the next index regeneration. Acceptable given
  the background refresh model.

---

## Alternatives Considered

### Alternative A — Centralised Registry (crates.io style)

**Rejected:** Hosting, maintaining, and securing a global registry is
expensive and operationally complex. crates.io outages have broken the Rust
ecosystem. A static index with no hosted binaries is simpler and more resilient.

### Alternative B — Long-lived Ed25519 Author Keys

**Rejected:** Long-lived keys must be managed, rotated, and revoked.
Sigstore keyless signing eliminates this entirely — no keys to steal, no
revocation infrastructure to build. The industry is converging on keyless
signing for software supply chain security.

### Alternative C — Remote Search API

**Rejected:** Contradicts the static site model. Requires server-side compute.
Breaks offline use. The plugin index will remain small enough for local search
for the foreseeable future.

### Alternative D — Free-form Tags Only (no categories)

**Rejected:** Free-form tags from many authors become inconsistent. `git`,
`Git`, `github`, `version-control` all mean the same thing — fixed categories
provide consistent browse structure. Tags supplement for specifics.

---

## Unresolved Questions

All previously unresolved questions have been resolved.

| ID | Question | Resolution |
|---|---|---|
| UQ-1 | Author key registration and revocation | Sigstore keyless — GitHub OIDC, no long-lived keys |
| UQ-2 | Categories and tags | Fixed categories + free-form tags |
| UQ-3 | Governance model | PR-based submissions — resolved in RFC-005 |
| UQ-4 | Full-text search | Local cached index — background refresh |
| UQ-5 | Name conflict resolution | Namespace ownership — resolved in RFC-005 |
| UQ-6 | Plugin namespacing | Three-tier system — resolved in RFC-005 |

---

## Implementation Plan

### Affected Crates

| Crate | Responsibility |
|---|---|
| `forge-plugin/index` | Index fetch, cache, search — local full-text search engine |
| `forge-plugin/install` | Three-tier resolution, download, SHA-256, Sigstore verification |
| `forge-plugin/registry` | Private registry client, `forge registry serve` |
| `forge-cli/plugin` | All `forge plugin` subcommands |

**External dependencies:**
- `cosign` / Sigstore Rust client — for signature verification
- `rekor-rs` — Rekor transparency log client

### Dependencies

- Requires RFC-005 (plugin system) — install flow, capability display, manifest format
- Requires RFC-013 (shell config) — `[plugins.registries]` config section

### Milestones

1. Define index JSON schema — `plugins.forge-shell.dev/index.json`
2. Implement index fetch and local cache — ETag, age-based refresh
3. Implement local search — full-text, category filter, tag filter
4. Implement three-tier install resolution — Tier 1, 2, 3
5. Implement SHA-256 verification — all installs
6. Implement Sigstore signature verification — verified/official plugins
7. Implement capability display and confirmation on install
8. Implement yanking — install warning, explicit confirmation
9. Implement `forge plugin` subcommands — search, install, list, info, update, remove, check
10. Implement private registry support — config + `forge registry serve`
11. Build `plugins.forge-shell.dev` static site — index generation from `plugin-index` TOML
12. Build `sum.forge-shell.dev` — content hash transparency log
13. Integration tests on ubuntu-latest, macos-latest, windows-latest

---

## References

- [Sigstore — Software Supply Chain Security](https://www.sigstore.dev/)
- [Rekor — Transparency Log](https://github.com/sigstore/rekor)
- [cosign — Container Signing](https://github.com/sigstore/cosign)
- [Go module proxy protocol](https://go.dev/ref/mod#goproxy-protocol)
- [sum.golang.org — Go transparency log](https://sum.golang.org/)
- [Cargo registry index format](https://doc.rust-lang.org/cargo/reference/registry-index.html)
- [RFC-005 — Plugin System](./RFC-005-plugin-system.md)
- [RFC-013 — Shell Configuration Model](./RFC-013-shell-config.md)