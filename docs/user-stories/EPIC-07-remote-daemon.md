# Epic 7 — Remote daemon operation

Managing a daemon running on another machine, without private keys ever crossing the
network.

[← Back to index](README.md)

---

## US-7.1 — Manage a daemon on another machine ✅

**As an** operator
**I want** to drive a daemon on a server from my workstation
**So that** tunnels run where they are needed rather than where I happen to be sitting.

**Acceptance criteria**
- Pointing `cli.toml` at an HTTPS daemon works for both the CLI and the GUI
- Every tunnel operation behaves identically to the local case, including interactive authentication over SSE
- The connection requires HTTPS and a token; a self-signed daemon needs its fingerprint pinned

**Implementation**: `crates/common/src/daemon_client.rs`, `crates/gui-core/src/daemon/client.rs`

---

## US-7.2 — Use my local profiles with a remote daemon ✅

**As an** operator
**I want** my profiles to work against a remote daemon
**So that** I do not have to maintain a second copy on the server.

**Acceptance criteria**
- Hybrid mode sends the profile with the start request, so the daemon does not need it on disk
- `ProfileSourceMode` selects the behaviour: `Local` for a socket daemon, `Hybrid` for HTTP/HTTPS
- The client picks the mode automatically from its connection mode

**Implementation**: `crates/common/src/types.rs` (`ProfileSourceMode`, `StartTunnelRequest`), `crates/daemon/src/api.rs` (`start_tunnel`)

---

## US-7.3 — Keep my private keys off the network ✅

**As a** security-conscious operator
**I want** my SSH private key never transmitted
**So that** using a remote daemon does not widen my exposure.

**Acceptance criteria**
- Only the key's **filename** is sent, never its path or contents
- The daemon resolves that filename against its own SSH directory
- Keys are copied to the daemon host once, deliberately, by the operator
- There is no key synchronisation, upload endpoint or encrypted transfer path — by design

**Implementation**: `crates/common/src/profile_manager.rs` (`prepare_profile_for_remote`)
**Tests**: `profile_manager::tests::test_prepare_profile_for_remote_reduces_key_to_filename`

---

## US-7.4 — Be told exactly where to put the key ✅

**As an** operator setting up a remote daemon
**I want** guidance naming the real directory
**So that** I am not guessing at `~/.ssh` when the daemon runs as another user.

**Acceptance criteria**
- The daemon reports its actual SSH directory through `DaemonInfo.ssh_key_dir`
- Setup messages and key-not-found errors use that path, not a generic `~/.ssh`
- The GUI shows a one-time warning explaining that keys must exist on the daemon host, with a "Don't show this again" checkbox persisted as `skip_ssh_setup_warning` in `cli.toml`
- Operations that are meaningless for a remote daemon are hidden — the Restart Daemon button only appears in unix-socket mode

**Implementation**: `crates/common/src/profile_manager.rs` (`get_remote_key_setup_message`), `crates/daemon/src/api.rs` (`DaemonInfo`), `crates/gui-gtk/src/ui/daemon_settings.rs`
