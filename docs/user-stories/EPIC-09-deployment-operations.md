# Epic 9 — Deployment and operations

Installing, running as a service, operating headless, and diagnosing problems.

[← Back to index](README.md)

---

## US-9.1 — Install from a package ⚠️

**As a** user on Debian or Ubuntu
**I want** to install from a package
**So that** I do not have to build from source.

**Acceptance criteria**
- `.deb` packages exist for the daemon, the CLI and the GTK GUI
- The daemon package can be installed alone, for headless servers
- systemd units are installed with the package

**Status**: **Partial.** DEB is available; RPM is in progress; AUR and Flatpak have not been
started. Everyone else builds from source, which needs only Rust for the daemon and CLI.

**Implementation**: `scripts/install.sh`, `docs/INSTALLATION.md`
**See**: [ROADMAP.md](../ROADMAP.md#packaging)

---

## US-9.2 — Run the daemon as a user service ✅

**As a** desktop user
**I want** the daemon to start when I log in

**Acceptance criteria**
- `systemctl --user enable --now ssh-tunnel-daemon` is all that is required
- The daemon uses the user's own runtime directory and config
- The keyring is available, because there is a desktop session
- Logs go to the journal: `journalctl --user -u ssh-tunnel-daemon -f`

**Implementation**: `docs/systemd/ssh-tunnel-daemon.user.service`, `docs/architecture/SYSTEMD.md`

---

## US-9.3 — Run the daemon as a system service ✅

**As an** operator
**I want** a system-wide daemon that starts at boot
**So that** tunnels are up before anyone logs in, including on privileged ports.

**Acceptance criteria**
- A templated unit runs the daemon as a chosen user: `ssh-tunnel-daemon@<user>`
- Privileged ports work via `CAP_NET_BIND_SERVICE`
- Group access can be enabled so several users can manage tunnels
- The keyring limitation is documented: a system service usually has no Secret Service, so profiles must use unencrypted keys or `SSH_TUNNEL_SKIP_KEYRING=1`

**Implementation**: `docs/systemd/ssh-tunnel-daemon@.service`, `docs/architecture/SYSTEMD.md`

---

## US-9.4 — Operate headless ✅

**As an** operator on a server or in a container
**I want** everything to work without a desktop session

**Acceptance criteria**
- Keyring unavailability is detected and handled, never fatal
- `SSH_TUNNEL_SKIP_KEYRING=1` disables the keyring explicitly for containers, CI and configuration management
- The CLI works fully over SSH, including interactive authentication prompts
- The GUI is not required and need not be installed

**Implementation**: `crates/common/src/keychain.rs`, `crates/cli/src/main.rs`
**See**: [EPIC-03 US-3.7](EPIC-03-authentication.md)

---

## US-9.5 — Diagnose a failure ✅

**As an** operator
**I want** enough information to work out what went wrong

**Acceptance criteria**
- Structured logging throughout via `tracing`; `RUST_LOG` controls verbosity
- HTTP connection errors are categorised — client disconnect, SSE stream close, network, protocol, server — with actionable hints
- Routine SSE disconnects log at debug rather than error, so the log is not full of noise
- Error detection is type-based rather than string-based, so it behaves identically under any locale
- `docs/INSTALLATION.md` has a troubleshooting section for the common failures

**Implementation**: `crates/daemon/src/main.rs` (`log_connection_error`, `categorize_connection_error`)

---

## US-9.6 — Configure daemon logging ❌

**As an** operator
**I want** a `--debug` flag and configurable log destinations

**Status**: **Not implemented.** Verbosity is controlled only through the `RUST_LOG`
environment variable. The daemon accepts no command-line arguments at all, so there is also
no way to point it at a different config file.

Log rotation is likewise unaddressed: under systemd the journal handles it, but a
file-based deployment would grow without bound.

**Workaround**: `RUST_LOG=debug`, and rely on journald rotation.
**See**: [ROADMAP.md](../ROADMAP.md#enhanced-logging)
