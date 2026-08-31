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
- A stored password is used automatically when `password_storage = "keychain"`
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

---

## US-3.4 — Complete two-factor authentication ✅

**As a** user on a server requiring 2FA
**I want** to be asked for my code at the right moment

**Acceptance criteria**
- Keyboard-interactive authentication is supported, including publickey + keyboard-interactive combinations
- The server's own prompt text is shown verbatim, so non-English servers read correctly
- Multiple prompts within one keyboard-interactive session are handled in order

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

**Implementation**: `crates/daemon/src/tunnel.rs` (two-loop retry structure), `crates/gui-gtk/src/ui/auth_dialog.rs`
**Tests**: `live_ssh::a_wrong_2fa_code_is_re_prompted_not_fatal`

> Fixed in v0.1.10; nothing guarded it until the live test suite arrived in v0.1.11.
> Password authentication gained the same behaviour in v0.2.0 — see US-3.3.

---

## US-3.6 — Store credentials in the system keychain ✅

**As a** desktop user
**I want** my passphrase or password remembered
**So that** I am not retyping it constantly.

**Acceptance criteria**
- Setting `password_storage = "keychain"` stores the secret under service `ssh-tunnel-manager`, keyed by the profile UUID
- The daemon retrieves both key passphrases **and** passwords automatically
- If retrieval fails, the user is prompted interactively rather than the connection failing
- The GUI puts the "Store in Keychain" switch *before* the password field, and only shows the field when storing

**Implementation**: `crates/common/src/keychain.rs`, `crates/daemon/src/security.rs`
**Note**: A TOTP second factor is inherently single-use and cannot be stored.

---

## US-3.7 — Work where there is no keychain ✅

**As an** operator on a headless server
**I want** everything to work without a Secret Service
**So that** the absence of a desktop session is not a blocker.

**Acceptance criteria**
- Keyring availability is determined by attempting a real operation, not by guessing from environment variables
- Profile creation succeeds when no keyring is present, with a clear warning that the credential will not be stored
- `SSH_TUNNEL_SKIP_KEYRING=1` forces the keyring off for containers, CI and configuration management
- The daemon prompts interactively for anything it cannot retrieve

**Implementation**: `crates/common/src/keychain.rs` (`is_keychain_available`, `should_skip_keyring`)

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

**Implementation**: `crates/daemon/src/tunnel.rs` (`AuthContext::request_input`), `crates/daemon/src/api.rs` (`submit_auth`)
**Tests**: `daemon_api::*` for the endpoint behaviour; `live_ssh::stopping_during_authentication_returns_promptly`
