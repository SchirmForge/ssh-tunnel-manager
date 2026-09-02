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

GUI v2 also treats a remote key path as a path on the daemon host: it does not expand,
validate or offer to upload that path from the client. Its runtime/manual remote-daemon
validation remains part of Epic 11.

---

## US-7.5 — Have my saved credentials work with a remote daemon ✅

**As a** user running the GUI on my desktop and the daemon on a machine on my LAN
**I want** "remember this password" to actually remember it
**So that** I am not asked for it on every connection despite having saved it.

**Acceptance criteria**
- A credential saved on the client is used when the daemon asks, whether the daemon is local
  or on the network
- The profile records **where** the credential is (`client`), not merely that one exists
- `prepare_profile_for_remote` carries that decision rather than discarding it
- The daemon is not told the client holds a credential — it raises its usual prompt, and a
  stored answer arrives instead of a typed one, over the same authenticated channel
- Credentials travel only over the existing client↔daemon transport: a Unix socket locally,
  TLS with certificate pinning remotely

**Implementation**: `crates/common/src/daemon_client.rs` (`ClientHeldCredential`),
`crates/common/src/profile_manager.rs` (`prepare_profile_for_remote`),
`crates/common/src/config.rs` (`PasswordStorage::resolved`),
`crates/gui-core/src/runtime.rs` (structured client-held credential resolution)

> **This was broken from the introduction of remote daemon support until v0.3.0**, and it
> failed silently. `password_storage = "keychain"` said only that the credential was in *a*
> keychain. The client wrote it to its own; the daemon looked in its own; with the daemon on
> another machine those are different stores. Nothing was found, so the user was prompted —
> no error, no warning, just the prompt they thought they had avoided.
>
> The fix is that the setting now says *where*, and that a credential which lives on the
> client is answered by the client. Private keys are unaffected: they remain on the daemon
> host and are never transmitted (see US-7.3).
