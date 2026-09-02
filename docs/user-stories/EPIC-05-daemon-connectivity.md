# Epic 5 — Daemon connectivity and client configuration

How clients find and talk to the daemon, how that channel is secured, and how a new user
gets configured in the first place.

[← Back to index](README.md)

---

## US-5.1 — Talk to a local daemon with no setup ✅

**As a** user on one machine
**I want** the client to find the daemon automatically

**Acceptance criteria**
- The default listener is a Unix socket in the user's runtime directory
- The client discovers the socket path without configuration
- The socket is created `0600` inside a `0700` directory, so no other user can reach it
- The daemon and every client derive that path from **one** implementation, so they cannot
  disagree about where the socket is — a disagreement is silent, and looks exactly like the
  daemon not running
- The PID file lives in the same directory as the socket, by construction
- With no `$XDG_RUNTIME_DIR` — a service account with no login session — the daemon uses
  `/run/ssh-tunnel-manager`, which is where clients already look and what the systemd unit
  creates

**Implementation**: `crates/common/src/runtime_paths.rs`, `crates/daemon/src/config.rs`
(`socket_path`), `crates/daemon/src/pidfile.rs`, `crates/common/src/daemon_client.rs`
**Tests**: `runtime_paths::tests` (socket and PID file always share a directory; a runtime
directory already named `ssh-tunnel-manager` is not nested again),
`daemon_api::unix_socket_daemon_serves_health`, `runtime_directory_and_socket_are_owner_only`

---

## US-5.2 — Run the daemon over TCP for local testing ✅

**As a** developer
**I want** an HTTP listener
**So that** I can inspect the API with ordinary tools.

**Acceptance criteria**
- `listener_mode = "tcp-http"` binds a TCP port
- **HTTP is restricted to loopback.** Binding to a network address is refused at startup with an explanation
- Authentication still applies

**Implementation**: `crates/daemon/src/config.rs` (validation), `crates/common/src/network.rs` (`is_loopback_address`)
**Tests**: `daemon_api::http_mode_is_refused_on_a_non_loopback_address`, `config::tests::test_validate_tcp_http_*`

---

## US-5.3 — Reach a daemon over the network securely ✅

**As an** operator
**I want** network access protected by TLS

**Acceptance criteria**
- `listener_mode = "tcp-https"` serves over TLS
- A self-signed certificate and key are generated on first start, `0600`
- The SHA-256 fingerprint is printed at startup so it can be pinned
- Any non-loopback bind requires this mode

**Implementation**: `crates/daemon/src/tls.rs`, `crates/common/src/tls.rs`
**Tests**: `scripts/test-network-modes.sh` (certificate generation, permissions)

---

## US-5.4 — Pin the daemon's certificate ✅

**As an** operator
**I want** to pin the exact certificate
**So that** a self-signed daemon is still trustworthy.

**Acceptance criteria**
- `tls_cert_fingerprint` in `cli.toml` pins the expected certificate
- A mismatched fingerprint refuses the connection
- **Without** a pinned fingerprint the client validates against public roots, so a self-signed daemon certificate is correctly **rejected** — pinning is what makes self-signed usable

**Implementation**: `crates/common/src/tls.rs` (`create_pinned_tls_config`, `FingerprintVerifier`)
**Tests**: `scripts/test-network-modes.sh` — pinned, unpinned and wrong-fingerprint cases

---

## US-5.5 — Require a token for every request ✅

**As an** operator
**I want** the API authenticated by default

**Acceptance criteria**
- `require_auth` defaults to **true** in every listener mode, including the Unix socket
- A token is generated on first start and written `0600`
- Requests without a token, or with the wrong token, receive 401
- The token is masked in logs

**Implementation**: `crates/daemon/src/auth.rs`, `crates/common/src/daemon_client.rs` (`add_auth_header`)
**Tests**: `daemon_api::request_without_token_is_rejected`, `request_with_wrong_token_is_rejected`, `auth_is_required_by_default`, `token_file_is_not_world_readable`

---

## US-5.6 — Be configured on first run without reading a manual ✅

**As a** new desktop user
**I want** the GUI to set itself up on first launch

**Acceptance criteria**
- A configuration wizard runs automatically when no `cli.toml` exists
- It detects the daemon's generated config snippet and offers to import it
- If no snippet is found, a manual dialog collects the connection mode, host, port and token
- For network modes it prompts for the address to connect to, since the daemon's bind address (for example `0.0.0.0`) is not a usable target
- An unreadable, malformed, or incomplete existing configuration is recoverable and is not
  silently replaced before the user saves
- The daemon runtime starts only after the client configuration validates

**Implementation**: `crates/gui-gtk/src/ui/config_wizard.rs`, `client_config.rs`;
`crates/gui-core/src/client_setup.rs`; `crates/gui-v2/src/setup_wizard.rs`, `window.rs`

GUI v2 uses a non-blocking, pre-runtime implementation. Its setup decisions come from local
file state, parsed `ConnectionMode`, and typed field-validation codes. Daemon-originating
text is not inspected, and the API token is redacted from setup debug state.

**Tests**: `client_setup::tests` covers discovery, typed validation, bind-all host completion,
atomic permissions, and token redaction; `setup_wizard::tests` covers mode-derived field
visibility, cancellation, and single runtime startup.

---

## US-5.7 — Be told what is wrong instead of getting a bare 401 ✅

**As a** CLI user whose client is misconfigured
**I want** an actionable message

**Acceptance criteria**
- The client validates its configuration *before* attempting to connect
- A missing `cli.toml` with an available snippet offers to copy it interactively
- With no snippet, the message gives step-by-step instructions
- Every daemon-touching command performs this check

**Implementation**: `crates/common/src/daemon_client.rs` (`validate_daemon_config`, `get_cli_config_snippet_path`), `crates/cli/src/main.rs` (`ensure_daemon_config`)

---

## US-5.8 — Inspect the running daemon ✅

**As an** operator
**I want** to query the daemon about itself

**Acceptance criteria**
- `GET /api/daemon/info` returns version, uptime, start time, listener mode, bind address or socket path, whether auth is required, group access, config and `known_hosts` paths, the SSH key directory, active tunnel count, PID and user
- `GET /api/health` is a simple liveness check
- The GUI daemon page renders this, reading from the running daemon over the API rather than from the config file

**Implementation**: `crates/daemon/src/api.rs` (`get_daemon_info`),
`crates/gui-gtk/src/ui/daemon_settings.rs`, `crates/gui-v2/src/daemon_view.rs`
**Tests**: `daemon_api::daemon_info_reports_its_own_configuration`

---

## US-5.9 — Control the daemon from the CLI ❌

**As a** CLI user
**I want** `ssh-tunnel daemon start|stop|status`

**Status**: **Not implemented.** The subcommands are parsed and then hit `TODO` stubs
(`crates/cli/src/main.rs`). The GUI can request a shutdown via
`POST /api/daemon/shutdown`, and the CLI cannot. GUI v2 deliberately presents daemon
start/restart/shutdown as WIP under its current capability scope and does not infer service
control from descriptive daemon information.

**Workaround**: `systemctl --user {start,stop,status} ssh-tunnel-daemon`.

---

## US-5.10 — Not be able to start a daemon nothing can reach ✅

**As a** user who ran the daemon as root to forward a privileged port

**I want** to be told immediately that this will not work

**so that** I am not left debugging a client that reports no daemon is running while one
plainly is.

**Acceptance criteria**
- The daemon checks its effective uid at startup and refuses to run as root
- It refuses **before writing anything to disk**, so a refused start leaves no root-owned PID
  file or runtime directory behind for the user's own daemon to trip over
- The message explains why root cannot work, and gives both routes to privileged ports: the
  shipped systemd unit, which already grants `CAP_NET_BIND_SERVICE`, or `setcap` on the binary
- No error message anywhere advises running the daemon with `sudo`

**Why root cannot work**: the socket goes in `$XDG_RUNTIME_DIR`, which for root in a login
shell is `/run/user/0` — a per-user directory that is mode `0700` *by design*. No other user
can traverse it, so the CLI and GUI report that no daemon is running. Loosening the socket
does not help: the daemon only chmods, so it stays owned by `root:root`, and `group_access`
then grants access to a group only root is in.

**Implementation**: `crates/daemon/src/main.rs` (`root_refusal`),
`crates/daemon/src/tunnel.rs` (privileged-port message)
**Tests**: `root_is_refused`, `an_ordinary_user_is_allowed`,
`nothing_advises_running_the_daemon_with_sudo` (verified to fail against the old message)

> The refusal has **no override**, so a container image running the daemon as root will not
> start. Nothing in this repository ships such an image. The live refusal has not yet been
> observed on a real host — see `.plan/ARCH-02_manual-client-validation.md`.

