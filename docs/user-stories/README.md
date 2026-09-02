# User Stories

What SSH Tunnel Manager does today, described from the point of view of the people using it.

These document the **current implementation**, not intentions. Planned work lives in
[../ROADMAP.md](../ROADMAP.md). Where a story is unimplemented or partial it is recorded as
such rather than omitted, so this set can be read as an honest specification of the product.

## Numbering

Stories are identified as **`US-<epic>.<n>`** — for example `US-3.4` is the fourth story in
epic 3. Numbers are stable: a story that is removed leaves its number retired rather than
reused, so references from commits, issues and tests stay valid.

## Status markers

| Marker | Meaning |
|---|---|
| ✅ | Implemented and working |
| ⚠️ | Partially implemented — the story text says what is missing |
| ❌ | Not implemented; the interface may exist but does nothing useful |

## Epics

| # | Epic | Stories | Summary |
|---|---|---|---|
| 1 | [Profile management](EPIC-01-profile-management.md) | 8 | Creating, listing, inspecting, editing and deleting connection profiles |
| 2 | [Tunnel lifecycle](EPIC-02-tunnel-lifecycle.md) | 9 | Starting, stopping, restarting and inspecting tunnels; port forwarding |
| 3 | [Authentication](EPIC-03-authentication.md) | 9 | Keys, passphrases, passwords, two-factor, credential storage |
| 4 | [Host key verification](EPIC-04-host-key-verification.md) | 5 | Trusting a server on first connect, and detecting a changed key |
| 5 | [Daemon connectivity](EPIC-05-daemon-connectivity.md) | 10 | Listener modes, token authentication, TLS pinning, client configuration, first-run setup |
| 6 | [Real-time status](EPIC-06-realtime-status.md) | 7 | The event stream, live indicators, heartbeat, reconnection and prompt reconciliation |
| 7 | [Remote daemon](EPIC-07-remote-daemon.md) | 5 | Managing a daemon on another machine without moving private keys |
| 8 | [Security hardening](EPIC-08-security-hardening.md) | 9 | Permissions, authentication defaults, network restrictions |
| 9 | [Deployment and operations](EPIC-09-deployment-operations.md) | 6 | Service setup, headless operation, packaging, diagnostics |
| 10 | [Development and quality](EPIC-10-development-quality.md) | 14 | Testing, sandboxing and CI — stories for contributors |
| 11 | [Second-generation desktop GUI](EPIC-11-desktop-gui-v2.md) | 7 | Adaptive profile UI, safe editing/authentication, accessibility and production rollout |

**89 stories**: 76 ✅ implemented, 7 ⚠️ partial, 6 ❌ not implemented.

The six unimplemented stories are worth knowing up front: no `ssh-tunnel edit` command
([US-1.8](EPIC-01-profile-management.md)), no remote or dynamic forwarding
([US-2.8](EPIC-02-tunnel-lifecycle.md)), no tunnel auto-reconnect
([US-2.9](EPIC-02-tunnel-lifecycle.md)), `ssh-tunnel daemon start|stop|status` are stubs
([US-5.9](EPIC-05-daemon-connectivity.md)), and no configurable daemon logging
([US-9.6](EPIC-09-deployment-operations.md)). GUI v2 is not yet installed as the production
desktop application ([US-11.7](EPIC-11-desktop-gui-v2.md)); its other six stories are partial
until runtime validation and cutover are complete.

## Personas

| Persona | Who they are |
|---|---|
| **Desktop user** | Runs the GTK GUI on a workstation. Wants tunnels to start with as little ceremony as possible. |
| **CLI user** | Works in a terminal, often over SSH. Wants scriptable commands and clear output. |
| **Operator** | Deploys the daemon on a server, possibly headless, possibly as a system service. Cares about permissions, service management and unattended behaviour. |
| **Contributor** | Works on the codebase. Wants to change things without breaking them or damaging their own machine. |

## Keeping these current

A change that alters observable behaviour needs the affected story updated in the same
commit — including flipping a ❌ to a ✅. A change to *how* something is built, with no
behavioural effect, belongs in [../architecture/](../architecture/) instead.
