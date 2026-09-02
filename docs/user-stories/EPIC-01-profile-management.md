# Epic 1 — Profile management

Creating, listing, inspecting, editing and deleting the connection profiles that describe a
tunnel. A profile is a TOML file under `~/.config/ssh-tunnel-manager/profiles/`, named by
its UUID.

[← Back to index](README.md)

---

## US-1.1 — Create a profile interactively ✅

**As a** CLI user
**I want** to be walked through creating a profile
**So that** I do not have to remember every field or hand-write TOML.

**Acceptance criteria**
- `ssh-tunnel add <name>` prompts for host, port, user, authentication type, key path and the port forwarding details
- Defaults are offered where sensible (port 22, bind address `127.0.0.1`, ed25519 key path)
- The profile is validated before it is written; validation failures explain which field is wrong
- A UUID is generated and the file is written `0600`

**Implementation**: `crates/cli/src/main.rs` (`Commands::Add`, `add_profile`)
**Tests**: `profile_manager::tests::test_profile_save_load_delete_round_trip`

---

## US-1.2 — Create a profile non-interactively ✅

**As a** CLI user automating a deployment
**I want** to create a profile entirely from arguments
**So that** it works in a script with no terminal attached.

**Acceptance criteria**
- `--non-interactive` (`-y`) suppresses every prompt
- Host, port, user, key path, bind address, local port, forward host and forward port are all settable by flag
- Advanced options are settable too: `--compression`, `--keepalive-interval`, `--auto-reconnect`, `--reconnect-attempts`, `--reconnect-delay`, `--tcp-keepalive`, `--max-packet-size`, `--window-size`
- Missing required values cause a clear error rather than a hang waiting for input

**Implementation**: `crates/cli/src/main.rs` (`Commands::Add`, `non_interactive`)

---

## US-1.3 — List profiles ✅

**As a** CLI user
**I want** to see my profiles at a glance
**So that** I can find the one I need.

**Acceptance criteria**
- `ssh-tunnel list` prints a table of name, remote target, tunnel description and tags
- The tunnel description reads `local: 127.0.0.1:8080 → remote: host:80`, formatted identically in the CLI and GUI
- `--verbose` adds detail; `--json` emits machine-readable output for scripting
- IPv6 literals are bracketed correctly (`[::1]:22`)

**Implementation**: `crates/cli/src/main.rs` (`Commands::List`), `common::format_tunnel_description`, `common::format_host_port`
**Tests**: `daemon_client::tests::test_daemon_base_url` covers IPv6 bracketing

---

## US-1.4 — Inspect one profile ✅

**As a** CLI user
**I want** to see everything about a single profile
**So that** I can check its settings before connecting.

**Acceptance criteria**
- `ssh-tunnel info <name>` shows connection, forwarding and advanced option values
- Whether a credential is stored in the keychain is shown; the credential itself never is

**Implementation**: `crates/cli/src/main.rs` (`Commands::Info`)

---

## US-1.5 — Delete a profile ✅

**As a** CLI user
**I want** to remove a profile I no longer need

**Acceptance criteria**
- `ssh-tunnel delete <name>` removes the TOML file
- Deleting a profile that does not exist reports so clearly rather than failing silently

**Implementation**: `crates/cli/src/main.rs` (`Commands::Delete`), `common::profile_manager::delete_profile_by_name`
**Tests**: `profile_manager::tests::test_profile_save_load_delete_round_trip`

---

## US-1.6 — Manage profiles in the GUI ✅

**As a** desktop user
**I want** to create, edit and delete profiles in a graphical editor
**So that** I never have to touch a config file.

**Acceptance criteria**
- A "New Profile" button on the profiles list, and Edit/Delete on the profile details page
- The dialog is organised into Basic Info, Authentication, Port Forwarding and an Advanced Tuning accordion
- Duplicate names are rejected; editing an existing profile overwrites correctly rather than creating a second one
- The list refreshes after every create, edit and delete, and navigates back to the list after edit or delete
- Escape closes the dialog; the title reads "New Profile" or "Edit Profile" as appropriate
- A file chooser is offered for the SSH key, filtered to likely key files

**Implementation**: `crates/gui-gtk/src/ui/profile_dialog.rs`, `profiles_list.rs`, `profile_details.rs`; `crates/gui-core/src/profiles.rs`

The production GUI extends this source implementation with details,
create/edit/duplicate/confirmed delete, structured field validation, safe credential
operations and local/remote key-path semantics; see [Epic 11](EPIC-11-desktop-gui-v2.md).

**Production GUI implementation**: `crates/gui-core/src/editor.rs`, `runtime.rs`,
`crates/gui-v2/src/profile_editor.rs`, `profile_list.rs`

---

## US-1.7 — Profiles survive an upgrade ✅

**As a** user upgrading from an older version
**I want** my existing profiles to keep working
**So that** upgrading is not a re-setup.

**Acceptance criteria**
- Profiles written before v0.1.7, where `password_storage` was a boolean, still load: `true` becomes `Keychain`, `false` becomes `None`
- Profile metadata is flattened at the top level of the file, and absent optional fields fall back to defaults
- No migration step is required of the user

**Implementation**: `crates/common/src/config.rs` (`PasswordStorage` custom `Deserialize`)
**Tests**: `profile_manager::tests::test_password_storage_backward_compatible_with_boolean`

---

## US-1.8 — Edit a profile from the CLI ❌

**As a** CLI user
**I want** to change a profile without recreating it

**Status**: **Not implemented.** There is no `ssh-tunnel edit` command. The current options
are to edit the TOML file directly, use the GUI, or delete and recreate the profile. Earlier
documentation advertised an `edit` command that has never existed.

**Workaround**: `$EDITOR ~/.config/ssh-tunnel-manager/profiles/<uuid>.toml`
