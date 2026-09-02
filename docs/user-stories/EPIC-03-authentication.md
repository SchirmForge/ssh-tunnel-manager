# Epic 3 — Authentication

How the daemon proves who you are to the SSH server, and how credentials are asked for and
stored. This is the most intricate part of the product and the part most protected by tests.

[← Back to index](README.md)

---

## US-3.1 — Connect with an SSH key ✅

**As a** user
**I want** to authenticate with a private key
**So that** connecting needs no typing.

**Acceptance criteria**
- `auth_type = "key"` with `key_path` authenticates by public key
- Key file permissions are checked before use; an over-permissive key is reported
- An unencrypted key connects with no prompt at all
- The key is referenced by path and never copied or transmitted

**Implementation**: `crates/daemon/src/tunnel.rs` (`authenticate_with_key`)
**Tests**: `live_ssh::key_authentication_connects_and_forwards_traffic`

---

## US-3.2 — Be prompted for a key passphrase ✅

**As a** user with an encrypted key
**I want** to be asked for the passphrase when it is needed

**Acceptance criteria**
- An encrypted key raises an `AuthRequired` event of type `KeyPassphrase`
- The prompt names the key file
- A wrong passphrase re-prompts rather than failing the tunnel outright
- Encrypted-key detection uses the `russh::keys::Error` variant, not error-message text, so it works on non-English systems

**Implementation**: `crates/daemon/src/tunnel.rs` (`request_passphrase_and_load`, `build_key_load_error_message`)
**Tests**: `live_ssh::an_encrypted_key_prompts_for_its_passphrase`

---

## US-3.3 — Connect with a password ✅

**As a** user on a server without key access
**I want** to authenticate with a password

**Acceptance criteria**
- `auth_type = "password"` raises an `AuthRequired` event of type `Password`
- Input is hidden in both the terminal and the GUI dialog
- A daemon-held password is used automatically; an attached client may answer a structured
  password request from `password_storage = "client"`
- A wrong password re-prompts while the server still permits attempts
- A password loaded from the keychain is tried **once**: if it is stale, the user is
  prompted rather than the stored value being replayed until the server cuts the connection

**Implementation**: `crates/daemon/src/tunnel.rs` (`authenticate_with_password`)
**Tests**: `live_ssh::password_authentication_connects`, `live_ssh::a_wrong_password_is_re_prompted_not_fatal`

> **True only from v0.2.0.** This story listed the re-prompt as an acceptance criterion and
> named the test that covers it, but `authenticate_with_password` prompted once and gave up,
> and the test had never actually been run against a server. The v0.1.10 "re-prompt, don't
> fail" work had been applied to keyboard-interactive alone. A documented criterion, a named
> test and working behaviour are three different things.

GUI v2 selects its password entry only from `AuthRequestType::Password` and the structured
`hidden` flag; prompt text cannot select or reveal the control.

---

## US-3.4 — Complete two-factor authentication ✅

**As a** user on a server requiring 2FA
**I want** to be asked for my code at the right moment

**Acceptance criteria**
- Keyboard-interactive authentication is supported, including publickey + keyboard-interactive combinations
- The server's own prompt text is shown verbatim, so non-English servers read correctly
- Multiple prompts within one keyboard-interactive session are handled in order
- Clients do not call every keyboard-interactive challenge “2FA” unless the daemon supplies
  the structured two-factor request type

**Implementation**: `crates/daemon/src/tunnel.rs` (`authenticate_keyboard_interactive`)
**Tests**: `live_ssh::two_factor_authentication_connects_with_a_valid_code`

---

## US-3.5 — Retry a rejected code instead of starting over ✅

**As a** user who mistyped a 2FA code
**I want** to be asked again
**So that** one typo does not cost me the whole connection.

**Acceptance criteria**
- After a rejected attempt the daemon checks whether `keyboard-interactive` is still in the server's `remaining_methods`
- If it is, a new keyboard-interactive session starts and the user is re-prompted
- The server's retry policy is respected — no artificial client-side attempt limit
- When the server finally refuses, the tunnel fails with a clear reason
- In the GUI the dialog stays open showing "Verifying…" until an SSE event confirms the next state, so there is no race between the submit and the result

**Implementation**: `crates/daemon/src/tunnel.rs` (two-loop retry structure),
`crates/gui-gtk/src/ui/auth_dialog.rs`, `crates/gui-core/src/auth.rs`,
`crates/gui-v2/src/auth_dialog.rs`
**Tests**: `live_ssh::a_wrong_2fa_code_is_re_prompted_not_fatal`

> Fixed in v0.1.10; nothing guarded it until the live test suite arrived in v0.1.11.
> Password authentication gained the same behaviour in v0.2.0 — see US-3.3.

---

## US-3.6 — Store credentials so they are remembered ✅

**As a** user
**I want** my passphrase or password remembered
**So that** I am not retyping it constantly — **including when the daemon runs elsewhere.**

**Acceptance criteria**
- The secret is stored under service `ssh-tunnel-manager`, keyed by the profile UUID
- `password_storage` records **where** it is, not merely that it exists:
  - `client` — kept by the client and sent when the daemon asks. Works the same for a local
    or a remote daemon
  - `daemon-host` — kept on the daemon host and read there by the daemon
- A stored credential is offered **once** per connection attempt. If it is rejected the user
  is prompted, rather than the stale value being replayed until the server's `MaxAuthTries`
  is exhausted
- If retrieval fails for any reason, the user is prompted rather than the connection failing
- The packaged GUI puts the "Store in Keychain" switch before the password field. GUI v2
  exposes explicit client/daemon-host storage, reveals the new-secret field only for a real
  Store operation, and never loads an existing secret into editor state

**Implementation**: `crates/common/src/keychain.rs`, `crates/common/src/daemon_client.rs`
(`ClientHeldCredential`), `crates/daemon/src/security.rs`, `crates/gui-core/src/editor.rs`,
`runtime.rs`

**Note**: A TOTP second factor is inherently single-use and is never stored.

> **This did not work with a remote daemon until v0.3.0.** `password_storage = "keychain"`
> recorded only that the credential was in *a* keychain, never whose. The client saved it
> locally and the daemon looked on its own host; against a LAN daemon those are different
> machines, so nothing was found and the user was prompted anyway — silently, every time.
> The legacy value is still read and resolves by where the daemon is.
>
> The packaged `gui-gtk` still records the legacy value. GUI v2 records the explicit location
> and uses the common Secret Service/keyutils facade, but does not become the packaged GUI
> until its manual validation and cutover gates pass.

---

## US-3.7 — Store credentials without a desktop session ✅

**As an** operator on a headless server
**I want** credential storage to work without a Secret Service
**So that** the absence of a desktop session does not mean retyping a password every time.

**Acceptance criteria**
- With no D-Bus session the daemon falls back to the Linux kernel keyutils keyring, which
  needs neither a desktop nor a session bus
- It says which store it opened, and warns that credentials in the kernel keyring **do not
  survive a reboot**
- `credential_store` in `daemon.toml` forces the choice: `auto`, `secret-service`, `keyutils`
  or `none`
- Store availability is determined by opening one, not by guessing from environment variables
- Profile creation succeeds when no store is available, with a clear warning that the
  credential will not be kept
- `SSH_TUNNEL_SKIP_KEYRING=1` disables storage, as does `credential_store = "none"`
- The daemon prompts interactively for anything it cannot retrieve

**Implementation**: `crates/common/src/keychain.rs` (`StoreKind`, `build_store`,
`is_keychain_available`), `crates/daemon/src/config.rs`

> Before v0.3.0 this story read "work where there is *no* keychain", and the answer was to do
> without one. A headless daemon now has a real store; the caveat is that the kernel keyring
> is cleared on reboot, so a credential saved there is gone after a restart. Genuinely
> unattended operation needs the work planned in `AUTH-02`.

---

## US-3.8 — Understand why authentication failed ✅

**As a** user
**I want** to be told what the server actually wanted

**Acceptance criteria**
- A failure names the methods the server will still accept
- Error categorisation is type-based — `russh` error variants, `std::io::ErrorKind`, `hyper` predicates — never string matching on messages, so behaviour is identical under any system locale
- Key loading failures distinguish "file missing", "wrong permissions" and "encrypted, passphrase needed"

**Implementation**: `crates/daemon/src/tunnel.rs`, `crates/daemon/src/main.rs` (`categorize_connection_error`)

---

## US-3.9 — Answer a prompt without a terminal ✅

**As an** operator running the daemon as a service
**I want** the daemon never to block on a console
**So that** it can run headless and still support interactive authentication.

**Acceptance criteria**
- The daemon emits `AuthRequired` over SSE and waits for an answer over the API; it never reads a terminal
- Any client — GUI, CLI or a script — can answer, and the daemon does not know which
- The answer must carry the matching request id, so a stale response cannot be replayed
- If nothing answers within `AUTH_RESPONSE_TIMEOUT` (60s) the attempt fails; cancelling explicitly returns immediately

**Implementation**: `crates/daemon/src/tunnel.rs` (`AuthContext::request_input`),
`crates/daemon/src/api.rs` (`submit_auth`), `crates/gui-core/src/auth.rs`, `controller.rs`,
`crates/gui-v2/src/auth_dialog.rs`
**Tests**: `daemon_api::*` for the endpoint behaviour; `live_ssh::stopping_during_authentication_returns_promptly`
