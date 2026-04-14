# RFC-009 — Plugin Registry & Distribution

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

This RFC defines the Forge Shell plugin registry — the infrastructure for
discovering, publishing, installing, and verifying third-party plugins.
The registry is a static, content-addressed store hosted at
`plugins.forge-shell.dev`. Plugins are distributed as signed WASM modules
with verified SHA-256 hashes.

---

## Motivation

The plugin system defined in RFC-005 establishes how plugins work internally.
This RFC addresses how users discover and install them, and how plugin authors
publish them. Without a registry, the plugin ecosystem cannot grow beyond
first-party plugins.

The registry must be:
- **Simple** — easy to publish to, easy to install from
- **Secure** — plugins are verified before installation
- **Decentralised-friendly** — users can host their own registries

---

## Design

### 1. Registry Architecture

The official registry is a **static site** — a set of JSON files served from
a CDN. There is no dynamic API. This makes it simple to operate, easy to
mirror, and resistant to downtime.

```
plugins.forge-shell.dev/
├── index.json                          # full plugin index
├── plugins/
│   ├── forge-git/
│   │   ├── meta.json                   # plugin metadata
│   │   ├── forge-git-1.0.0.wasm
│   │   ├── forge-git-1.0.0.wasm.sig   # detached signature
│   │   ├── forge-git-1.1.0.wasm
│   │   └── forge-git-1.1.0.wasm.sig
│   └── forge-kubectl/
│       └── ...
└── keys/
    └── forge-shell.pub                 # Forge Shell's signing public key
```

---

### 2. Index Format

```json
{
  "version": "1",
  "updated": "2026-04-09T10:00:00Z",
  "plugins": [
    {
      "name":        "forge-git",
      "description": "First-class Git integration for Forge Shell",
      "author":      "forge-shell",
      "homepage":    "https://github.com/forge-shell/forge-git",
      "license":     "MIT",
      "latest":      "1.2.0",
      "versions": [
        {
          "version":    "1.2.0",
          "url":        "https://plugins.forge-shell.dev/plugins/forge-git/forge-git-1.2.0.wasm",
          "sha256":     "abc123...",
          "signature":  "https://plugins.forge-shell.dev/plugins/forge-git/forge-git-1.2.0.wasm.sig",
          "min_forge":  "0.1.0",
          "published":  "2026-04-09T09:00:00Z",
          "yanked":     false
        }
      ]
    }
  ]
}
```

---

### 3. Plugin Installation Flow

```
forge plugin install forge-git
    │
    ├─ fetch index.json from registry
    │
    ├─ resolve "forge-git" → latest version → download URL + sha256 + sig URL
    │
    ├─ download forge-git-1.2.0.wasm
    │
    ├─ verify SHA-256 hash matches index entry
    │
    ├─ fetch signature file
    │
    ├─ verify signature against forge-shell.pub
    │
    ├─ check min_forge compatibility
    │
    ├─ extract forge-plugin.toml from WASM module
    │
    ├─ display capabilities to user:
    │     Plugin 'forge-git' v1.2.0 requests:
    │       exec: ["git"]
    │       filesystem: ["read"]
    │     Install? [y/N]
    │
    ├─ user confirms
    │
    └─ install to ~/.forge/plugins/forge-git/
```

Capability display and user confirmation are required for all plugin
installations — including updates. This ensures users are always aware of
what permissions a plugin holds.

---

### 4. Plugin Signing

All plugins in the official registry are signed. Forge Shell verifies
signatures before installation.

**Signing algorithm:** Ed25519

**Key management:**
- The `forge-shell` organisation holds the signing key
- First-party plugins (`forge-git`, `forge-kubectl`) are signed by
  `forge-shell` directly
- Third-party plugins are signed by the plugin author's key, which must be
  registered in the registry

**Signature verification flow:**

```
1. Download plugin.wasm and plugin.wasm.sig
2. Fetch the author's registered public key from the registry
3. Verify: Ed25519.verify(plugin.wasm bytes, sig, author_pubkey)
4. If verification fails → refuse installation with clear error
```

---

### 5. Publishing a Plugin

Plugin authors publish via the Forge Shell CLI:

```bash
# Authenticate with the registry (one-time)
forge plugin auth

# Publish a new version
forge plugin publish ./forge-myplugin.wasm

# Publish with explicit manifest
forge plugin publish ./forge-myplugin.wasm --manifest ./forge-plugin.toml

# Yank a broken version
forge plugin yank forge-myplugin@1.0.1 --reason "Critical bug in deploy command"
```

Publishing flow:
1. CLI authenticates with the registry API (GitHub OAuth)
2. CLI uploads the `.wasm` file and `forge-plugin.toml`
3. Registry validates the manifest, checks WASM module integrity
4. Registry signs the plugin with the author's registered key
5. Registry updates `index.json`

---

### 6. Yanking

A yanked plugin version is still downloadable but is not installed by default:

```bash
# Installing a yanked version requires explicit acknowledgement
forge plugin install forge-myplugin@1.0.1
# Warning: v1.0.1 has been yanked: "Critical bug in deploy command"
# Install anyway? [y/N]
```

Yanking is not deletion. Existing installations are not affected. Users who
have already installed the yanked version see a warning on next update check.

---

### 7. Alternative / Private Registries

Users can configure alternative registries in `~/.forge/config.fgs`:

```forge
[registries]
default  = "https://plugins.forge-shell.dev"
internal = "https://plugins.mycompany.internal"
```

```bash
# Install from alternative registry
forge plugin install mycompany-tools --registry internal

# Install from direct URL (bypasses registry, still verifies hash)
forge plugin install https://releases.mycompany.com/forge-tools-1.0.0.wasm \
  --sha256 abc123...
```

Private registries follow the same static JSON format as the official registry.
Forge Shell ships with a `forge registry serve` command to host a local
registry from a directory.

---

### 8. Update Checking

Forge Shell checks for plugin updates in the background (opt-in):

```toml
[updates]
check_interval_hours = 24
auto_update          = false   # never auto-update without user confirmation
notify               = true    # show notification when updates are available
```

```bash
# Check for updates manually
forge plugin update --check

# Update a specific plugin
forge plugin update forge-git

# Update all plugins
forge plugin update --all
```

---

## Drawbacks

- **Static registry is simple but limited** — search, filtering, and
  dependency resolution require additional infrastructure if the registry
  grows large.
- **Capability confirmation UX** — showing capabilities on every install is
  correct but adds friction for power users. There is no way to pre-approve
  known capability sets.
- **Signing infrastructure overhead** — managing Ed25519 keys, rotation, and
  revocation requires ongoing operational attention.
- **No dependency resolution** — plugins cannot declare dependencies on other
  plugins. If needed, this is a significant future complexity.

---

## Alternatives Considered

### Alternative A — Dynamic API Registry (like crates.io)

**Approach:** A full API backend with a database, search, and dynamic responses.
**Rejected because:** Significantly more infrastructure to operate and maintain.
The static approach works well for npm, Homebrew Formulae, and Cargo's index.
It can always be upgraded later.

### Alternative B — GitHub Releases as the Registry

**Approach:** Plugins are distributed via GitHub Releases. `forge plugin install`
fetches from GitHub.
**Rejected because:** Couples the registry to GitHub. Breaks for private plugins,
on-premise deployments, and users in regions where GitHub is restricted.

### Alternative C — No Official Registry

**Approach:** Users install plugins by URL only. No central discovery.
**Rejected because:** Discoverability is essential for ecosystem growth. A
shell without a plugin registry is a shell with no plugins in practice.

---

## Unresolved Questions

- [ ] How are author keys registered and revoked?
- [ ] Should the registry support plugin categories or tags for discovery?
- [ ] What is the governance model for the official registry? Who reviews
      submissions?
- [ ] Should there be a `forge plugin search` command for full-text search?
- [ ] How are plugin name conflicts resolved? (first-come-first-served? namespaced?)
- [ ] Should plugins be namespaced (e.g. `forge-shell/forge-git` vs `forge-git`)?

---

## Implementation Plan

### Affected Crates

- `forge-plugin` — registry client, installation, verification, update checking
- `forge-cli` — `forge plugin install/publish/update/yank/auth/list/remove` subcommands
- New: `forge-registry` — static registry server (for self-hosted registries)

### Dependencies

- Requires RFC-005 (Plugin System) — this RFC extends the plugin system
  with distribution infrastructure

### Milestones

1. Define registry index JSON schema
2. Implement registry client — fetch index, resolve versions
3. Implement SHA-256 verification
4. Implement Ed25519 signature verification
5. Implement capability display and confirmation on install
6. Implement `forge plugin install/list/remove/update` commands
7. Build the official registry static site
8. Implement `forge plugin publish/yank/auth` commands
9. Implement alternative registry configuration
10. Implement `forge registry serve` for self-hosted registries

---

## References

- [Cargo Registry Protocol](https://doc.rust-lang.org/cargo/reference/registry-index.html)
- [Homebrew Formula Repository](https://github.com/Homebrew/homebrew-core)
- [Ed25519 — IETF RFC 8032](https://tools.ietf.org/html/rfc8032)
- [npm Registry API](https://github.com/npm/registry/blob/main/docs/REGISTRY.md)
- [sigstore — Software Supply Chain Security](https://www.sigstore.dev)
