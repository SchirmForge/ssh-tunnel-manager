// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Keychain tests against the **real** OS credential store.
//!
//! The unit tests in `src/keychain.rs` use an in-memory fake, which proves the logic but says
//! nothing about the backend. These do the opposite: they exercise whatever `keyring` actually
//! resolves to, and are the only thing that would catch a repeat of the v0.2.0 regression,
//! where a dependency upgrade silently moved the store from the kernel keyutils keyring to
//! Secret Service and made every previously saved credential invisible.
//!
//! They need a session D-Bus and a Secret Service provider with an **unlocked** collection,
//! so they are `#[ignore]`d and never run under a plain `cargo test`. Run them with:
//!
//! ```text
//! make test-keychain-live
//! ```
//!
//! which wraps the suite in `dbus-run-session` with a throwaway `gnome-keyring-daemon`. CI
//! runs the same target.
//!
//! Every test writes under a random UUID and deletes it afterwards, so nothing is left in the
//! developer's real keyring even when run against a live desktop session.

use ssh_tunnel_common::keychain::{
    describe_store, get_password, has_password, is_keychain_available, remove_password,
    store_password,
};
use uuid::Uuid;

/// Deletes its profile's entry on drop, including when a test panics part-way.
struct Cleanup(Uuid);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = remove_password(&self.0);
    }
}

fn require_keychain() {
    assert!(
        is_keychain_available(),
        "no usable credential store: these tests need a session D-Bus and an unlocked \
         Secret Service collection. Run them with `make test-keychain-live`."
    );
}

#[test]
#[ignore = "needs a real credential store; run with `make test-keychain-live`"]
fn a_password_survives_a_round_trip_through_the_real_store() {
    require_keychain();
    let id = Uuid::new_v4();
    let _cleanup = Cleanup(id);

    store_password(&id, "hunter2").expect("store should succeed");
    assert_eq!(get_password(&id).expect("read should succeed"), "hunter2");
}

/// The regression guard.
///
/// If a dependency upgrade changes which backend `keyring` writes to, a write followed by a
/// read still passes — both go to the new store. What breaks is *persistence across
/// processes*, which is what the daemon actually depends on: the GUI writes, the daemon reads
/// later from a separate process.
///
/// Spawning a second process is the cheapest way to prove the credential really landed
/// somewhere shared rather than in per-process state.
#[test]
#[ignore = "needs a real credential store; run with `make test-keychain-live`"]
fn a_password_is_visible_to_a_separate_process() {
    require_keychain();
    let id = Uuid::new_v4();
    let _cleanup = Cleanup(id);

    store_password(&id, "cross-process-secret").expect("store should succeed");

    // Re-exec this test binary, asking it to read the entry back.
    let exe = std::env::current_exe().expect("test binary path");
    let output = std::process::Command::new(exe)
        .args(["--exact", "helper_reads_back", "--ignored", "--nocapture"])
        .env("SSH_TUNNEL_TEST_READBACK_ID", id.to_string())
        .output()
        .expect("should be able to re-exec the test binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("READBACK=cross-process-secret"),
        "a credential written by one process must be readable by another.\n\
         stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Helper for the test above. Not a test in its own right — it only does anything when
/// `SSH_TUNNEL_TEST_READBACK_ID` is set, which the parent does when re-execing.
#[test]
#[ignore = "helper for a_password_is_visible_to_a_separate_process"]
fn helper_reads_back() {
    let Ok(id) = std::env::var("SSH_TUNNEL_TEST_READBACK_ID") else {
        return;
    };
    let id: Uuid = id.parse().expect("valid uuid");
    match get_password(&id) {
        Ok(secret) => println!("READBACK={secret}"),
        Err(e) => println!("READBACK-FAILED={e}"),
    }
}

#[test]
#[ignore = "needs a real credential store; run with `make test-keychain-live`"]
fn removal_actually_removes_and_is_idempotent() {
    require_keychain();
    let id = Uuid::new_v4();
    let _cleanup = Cleanup(id);

    store_password(&id, "hunter2").expect("store should succeed");
    assert!(has_password(&id).expect("has_password should not error"));

    remove_password(&id).expect("first removal should succeed");
    assert!(!has_password(&id).expect("has_password should not error"));

    remove_password(&id).expect("removal must be idempotent");
}

#[test]
#[ignore = "needs a real credential store; run with `make test-keychain-live`"]
fn storing_twice_replaces_rather_than_duplicates() {
    require_keychain();
    let id = Uuid::new_v4();
    let _cleanup = Cleanup(id);

    store_password(&id, "first").expect("store should succeed");
    store_password(&id, "second").expect("overwrite should succeed");
    assert_eq!(get_password(&id).expect("read should succeed"), "second");
}

#[test]
#[ignore = "needs a real credential store; run with `make test-keychain-live`"]
fn an_unsaved_profile_has_no_password() {
    require_keychain();
    // Never stored, so nothing to clean up.
    assert!(!has_password(&Uuid::new_v4()).expect("has_password should not error"));
}

/// Reports which backing store was selected, and asserts one was.
///
/// Run under `make test-keychain-live` this should say Secret Service; run with no D-Bus
/// session it should say the kernel keyutils keyring. Printing it is the point — silent
/// store selection is what made the v0.2.0 regression invisible.
#[test]
#[ignore = "needs a real credential store; run with `make test-keychain-live`"]
fn reports_which_store_it_selected() {
    let store = describe_store();
    println!("SELECTED-STORE={store}");
    assert_ne!(store, "disabled", "no credential store could be installed");
}

/// The v0.2.0 migration, end to end against both real stores.
///
/// Writes a credential into the kernel keyutils keyring — where anything saved before v0.2.0
/// still sits — then reads it back through the normal API with Secret Service as the primary
/// store. The credential must be found, and afterwards must live in Secret Service with the
/// keyutils copy gone, so the two cannot drift apart.
///
/// Needs both stores reachable, so it only runs under `make test-keychain-live`.
#[test]
#[ignore = "needs both credential stores; run with `make test-keychain-live`"]
fn a_credential_left_in_keyutils_is_adopted_by_secret_service() {
    use keyring_core::api::CredentialStore as CoreStore;
    use std::sync::Arc;

    let Ok(keyutils) = linux_keyutils_keyring_store::Store::new() else {
        eprintln!("SKIP: no keyutils keyring on this system");
        return;
    };
    let keyutils = keyutils as Arc<CoreStore>;

    let id = Uuid::new_v4();
    let _cleanup = Cleanup(id);
    let account = id.to_string();

    // Seed the old store, as an install predating v0.2.0 would have.
    let legacy = keyutils
        .build("ssh-tunnel-manager", &account, None)
        .expect("build a keyutils entry");
    legacy
        .set_password("pre-upgrade-secret")
        .expect("seed the keyutils keyring");

    // Reading through the normal API must find it.
    assert_eq!(
        get_password(&id).expect("the credential should be found by reading through"),
        "pre-upgrade-secret",
        "a credential saved before v0.2.0 must not appear lost"
    );

    // ...and it must now live in the primary store.
    assert_eq!(
        describe_store(),
        "Secret Service",
        "this test only means something when Secret Service is the primary store"
    );
    assert_eq!(
        get_password(&id).expect("second read should come from the primary store"),
        "pre-upgrade-secret"
    );

    // The old copy must be gone, so the two cannot drift apart.
    let legacy_after = keyutils
        .build("ssh-tunnel-manager", &account, None)
        .expect("build a keyutils entry");
    assert!(
        legacy_after.get_password().is_err(),
        "the keyutils copy must be removed once adopted, not left to go stale"
    );
}
