// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Keychain management — centralised password and passphrase storage.
//!
//! # Structure
//!
//! The public functions are a thin facade over a [`CredentialStore`]. That seam exists so the
//! logic can be tested without a live keyring: the real implementation talks to the OS store,
//! and tests use an in-memory fake.
//!
//! This matters more than it might look. Until v0.2.0 this module had **no tests at all**,
//! and two defects went unnoticed because of it:
//!
//! * the `keyring` 3 → 4 upgrade silently moved the backing store from the kernel keyutils
//!   keyring to Secret Service, so credentials saved by an earlier version became invisible;
//! * "save to keychain" is a no-op against a remote daemon, because the client writes to its
//!   own keychain while the daemon reads its host's.
//!
//! Both compiled cleanly and passed the whole suite. See `.plan/AUTH-01_credential-storage.md`.
//!
//! # Note on current behaviour
//!
//! The facade deliberately preserves the behaviour that shipped in v0.2.0, including
//! [`has_password`] reporting `false` when the *backend* is broken rather than when the entry
//! is merely absent. The tests below pin that, so the change is visible when AUTH-01 step 1
//! separates the two cases.

use std::sync::{Arc, OnceLock};

use keyring_core::api::CredentialStore as CoreStore;
use keyring_core::{Entry, Error as CoreError};
use uuid::Uuid;

use crate::error::{Error, Result};

/// The service name every credential is filed under.
///
/// Together with the profile UUID this forms the entry key. Changing either orphans every
/// stored credential, so it is asserted in the tests.
const SERVICE: &str = "ssh-tunnel-manager";

// --- The seam ---------------------------------------------------------------

/// A place credentials can be kept.
///
/// `get` distinguishes "no such entry" (`Ok(None)`) from "the store itself failed" (`Err`).
/// The public facade currently collapses the two; keeping them apart here is what allows a
/// test to tell a missing password from a broken keyring, and what AUTH-01 step 1 needs in
/// order to report the difference to the user.
pub trait CredentialStore: Send + Sync {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>>;
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<()>;
    fn delete(&self, service: &str, account: &str) -> Result<()>;
    /// Human-readable name, for logs and `ssh-tunnel info`.
    fn describe(&self) -> &'static str;
}

// --- Store selection ---------------------------------------------------------

/// Which backing store credentials are kept in.
///
/// Linux has two that matter, with opposite trade-offs:
///
/// | | Secret Service | keyutils |
/// |---|---|---|
/// | Needs a D-Bus session | yes | no |
/// | Survives a reboot | yes | **no** |
/// | Suits | desktops | headless daemons |
///
/// `Auto` prefers Secret Service and falls back to keyutils, which is right for both the
/// desktop and the headless server without either having to be configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StoreKind {
    #[default]
    Auto,
    SecretService,
    Keyutils,
    /// Refuse to store anything. Equivalent to `SSH_TUNNEL_SKIP_KEYRING=1`.
    None,
}

impl StoreKind {
    /// Parse a `credential_store` config value or `SSH_TUNNEL_CREDENTIAL_STORE`.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "secret-service" | "secret_service" | "secretservice" => Some(Self::SecretService),
            "keyutils" => Some(Self::Keyutils),
            "none" | "off" | "disabled" => Some(Self::None),
            _ => None,
        }
    }
}

/// Adapts a `keyring-core` store to our [`CredentialStore`] seam.
struct CoreBackedStore {
    name: &'static str,
    store: Arc<CoreStore>,
    /// A store to read through to when this one has no entry.
    ///
    /// Set when the primary is Secret Service and a keyutils keyring is also reachable, so a
    /// credential saved before v0.2.0 is found and adopted rather than appearing lost. See
    /// [`Migration`].
    legacy: Option<Migration>,
}

/// A store to fall back to on a miss, and what to do with what is found there.
struct Migration {
    name: &'static str,
    store: Arc<CoreStore>,
}

impl CoreBackedStore {
    fn entry_in(store: &Arc<CoreStore>, service: &str, account: &str) -> Result<Entry> {
        store
            .build(service, account, None)
            .map_err(|e| Error::Keychain(format!("Failed to access keychain entry: {e}")))
    }

    fn entry(&self, service: &str, account: &str) -> Result<Entry> {
        Self::entry_in(&self.store, service, account)
    }

    /// Look in the legacy store; on a hit, copy into this store and remove the original.
    ///
    /// Moving rather than copying is deliberate: leaving the old entry behind would mean two
    /// copies of the same secret drifting apart, and the next reader could get the stale one.
    /// Both stores belong to the same user on the same machine, so the secret does not become
    /// more exposed by the move.
    ///
    /// A failure to adopt is not fatal — the credential was still found, and returning it is
    /// better than failing because the rewrite did not work.
    fn read_through(&self, service: &str, account: &str) -> Result<Option<String>> {
        let Some(legacy) = &self.legacy else {
            return Ok(None);
        };
        let entry = Self::entry_in(&legacy.store, service, account)?;
        let secret = match entry.get_password() {
            Ok(secret) => secret,
            Err(CoreError::NoEntry) => return Ok(None),
            // A legacy store that cannot be read is not an error for the caller: there simply
            // is no credential to migrate.
            Err(e) => {
                tracing::debug!("Could not read the {} while migrating: {e}", legacy.name);
                return Ok(None);
            }
        };

        match self.entry(service, account).and_then(|e| {
            e.set_password(&secret)
                .map_err(|e| Error::Keychain(e.to_string()))
        }) {
            Ok(()) => {
                let _ = entry.delete_credential();
                tracing::info!(
                    "Moved a saved credential from the {} into the {}",
                    legacy.name,
                    self.name
                );
            }
            Err(e) => tracing::warn!(
                "Found a credential in the {} but could not move it into the {}: {e}",
                legacy.name,
                self.name
            ),
        }
        Ok(Some(secret))
    }
}

impl CredentialStore for CoreBackedStore {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>> {
        match self.entry(service, account)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            // The distinction the `keyring` facade could not express: an absent entry is not
            // a broken store, and the caller needs to be able to tell them apart.
            Err(CoreError::NoEntry) => self.read_through(service, account),
            Err(e) => Err(Error::Keychain(format!(
                "Failed to retrieve password from the {} : {e}",
                self.name
            ))),
        }
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<()> {
        self.entry(service, account)?
            .set_password(secret)
            .map_err(|e| {
                Error::Keychain(format!(
                    "Failed to store password in the {}: {e}",
                    self.name
                ))
            })
    }

    fn delete(&self, service: &str, account: &str) -> Result<()> {
        match self.entry(service, account)?.delete_credential() {
            Ok(()) | Err(CoreError::NoEntry) => Ok(()),
            Err(e) => Err(Error::Keychain(format!(
                "Failed to remove password from the {}: {e}",
                self.name
            ))),
        }
    }

    fn describe(&self) -> &'static str {
        self.name
    }
}

/// Name reported by [`DisabledStore`]. Compared against by [`is_keychain_available`], so it
/// lives in one place.
const DISABLED: &str = "disabled";

/// A store that refuses everything, used for `StoreKind::None` and for the case where no
/// backing store could be installed at all.
struct DisabledStore;

impl CredentialStore for DisabledStore {
    fn get(&self, _: &str, _: &str) -> Result<Option<String>> {
        Ok(None)
    }
    fn set(&self, _: &str, _: &str, _: &str) -> Result<()> {
        Err(Error::Keychain(
            "Credential storage is disabled by configuration".into(),
        ))
    }
    fn delete(&self, _: &str, _: &str) -> Result<()> {
        Ok(())
    }
    fn describe(&self) -> &'static str {
        DISABLED
    }
}

const SECRET_SERVICE: &str = "Secret Service";
const KEYUTILS: &str = "kernel keyutils keyring";

fn open_secret_service() -> std::result::Result<Arc<CoreStore>, String> {
    zbus_secret_service_keyring_store::Store::new()
        .map(|s| s as Arc<CoreStore>)
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "linux")]
fn open_keyutils() -> std::result::Result<Arc<CoreStore>, String> {
    linux_keyutils_keyring_store::Store::new()
        .map(|s| s as Arc<CoreStore>)
        .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "linux"))]
fn open_keyutils() -> std::result::Result<Arc<CoreStore>, String> {
    Err("the keyutils keyring is Linux-only".into())
}

/// Wrap a store, and set it as `keyring-core`'s default so anything reaching for the global
/// gets the same one we do.
fn adopt(
    name: &'static str,
    store: Arc<CoreStore>,
    legacy: Option<Migration>,
) -> Arc<dyn CredentialStore> {
    keyring_core::set_default_store(store.clone());
    Arc::new(CoreBackedStore {
        name,
        store,
        legacy,
    })
}

/// The keyutils keyring, if it can be opened, as a migration source.
///
/// Only ever a *source*: credentials move keyutils → Secret Service, never the other way.
/// The reverse would take a credential that survives a reboot and put it somewhere that does
/// not, which is a downgrade nobody asked for.
fn keyutils_as_legacy() -> Option<Migration> {
    open_keyutils().ok().map(|store| Migration {
        name: KEYUTILS,
        store,
    })
}

/// Install a backing store and return a facade over it.
///
/// Failures are reported rather than swallowed: a user who asked for a specific store and
/// silently got a different one is exactly the v0.2.0 failure repeated.
fn build_store(kind: StoreKind) -> Arc<dyn CredentialStore> {
    match kind {
        StoreKind::None => Arc::new(DisabledStore),

        StoreKind::SecretService => match open_secret_service() {
            Ok(store) => adopt(SECRET_SERVICE, store, keyutils_as_legacy()),
            Err(e) => {
                tracing::warn!("Secret Service was requested but is unavailable: {e}");
                Arc::new(DisabledStore)
            }
        },

        // Explicitly asking for keyutils means wanting the kernel keyring, so nothing is
        // migrated into it.
        StoreKind::Keyutils => match open_keyutils() {
            Ok(store) => adopt(KEYUTILS, store, None),
            Err(e) => {
                tracing::warn!("The keyutils keyring was requested but is unavailable: {e}");
                Arc::new(DisabledStore)
            }
        },

        // Secret Service first: it persists across reboots, which keyutils does not.
        StoreKind::Auto => match open_secret_service() {
            Ok(store) => adopt(SECRET_SERVICE, store, keyutils_as_legacy()),
            Err(ss_err) => match open_keyutils() {
                Ok(store) => {
                    tracing::info!(
                        "Secret Service unavailable ({ss_err}); using the {KEYUTILS}. \
                         Stored credentials will not survive a reboot."
                    );
                    adopt(KEYUTILS, store, None)
                }
                Err(ku_err) => {
                    tracing::warn!(
                        "No credential store available (Secret Service: {ss_err}; \
                         keyutils: {ku_err}). Credentials will not be saved."
                    );
                    Arc::new(DisabledStore)
                }
            },
        },
    }
}

/// The configured store, resolved once per process.
static DEFAULT_STORE: OnceLock<Arc<dyn CredentialStore>> = OnceLock::new();

/// Choose the credential store for this process.
///
/// Optional: the first use of any keychain function resolves `Auto` if this was never called.
/// Call it early from `main` so the choice is logged before anything depends on it.
///
/// Returns the store actually installed, which may not be the one requested — check the logs
/// and [`describe_store`].
pub fn init_store(kind: StoreKind) -> &'static str {
    let store = DEFAULT_STORE.get_or_init(|| build_store(kind));
    store.describe()
}

fn default_store() -> &'static Arc<dyn CredentialStore> {
    DEFAULT_STORE.get_or_init(|| build_store(configured_kind()))
}

/// The store requested by configuration, in precedence order.
///
/// `SSH_TUNNEL_SKIP_KEYRING` is honoured for backwards compatibility; it predates this
/// setting and means the same as `credential_store = "none"`.
fn configured_kind() -> StoreKind {
    if should_skip_keyring() {
        return StoreKind::None;
    }
    std::env::var("SSH_TUNNEL_CREDENTIAL_STORE")
        .ok()
        .and_then(|v| StoreKind::parse(&v))
        .unwrap_or_default()
}

/// Name of the store credentials are currently kept in, for logs and diagnostics.
///
/// "My saved password vanished" is undiagnosable without this — which is exactly how the
/// v0.2.0 backend change stayed hidden.
pub fn describe_store() -> &'static str {
    default_store().describe()
}

fn account_for(profile_id: &Uuid) -> String {
    profile_id.to_string()
}

// --- Public facade ----------------------------------------------------------

/// Store a password or passphrase in the system keychain.
///
/// Filed under the service `ssh-tunnel-manager` with the profile UUID as the account.
///
/// # Examples
/// ```no_run
/// use uuid::Uuid;
/// use ssh_tunnel_common::keychain::store_password;
///
/// let profile_id = Uuid::new_v4();
/// store_password(&profile_id, "my-secret-password")?;
/// # Ok::<(), ssh_tunnel_common::Error>(())
/// ```
pub fn store_password(profile_id: &Uuid, password: &str) -> Result<()> {
    store_password_in(default_store().as_ref(), profile_id, password)
}

/// Retrieve a password or passphrase from the system keychain.
///
/// # Examples
/// ```no_run
/// use uuid::Uuid;
/// use ssh_tunnel_common::keychain::get_password;
///
/// let profile_id = Uuid::new_v4();
/// let password = get_password(&profile_id)?;
/// # Ok::<(), ssh_tunnel_common::Error>(())
/// ```
pub fn get_password(profile_id: &Uuid) -> Result<String> {
    get_password_in(default_store().as_ref(), profile_id)
}

/// Remove a password or passphrase from the system keychain.
///
/// Returns `Ok(())` even when nothing was stored — removal is idempotent.
///
/// # Examples
/// ```no_run
/// use uuid::Uuid;
/// use ssh_tunnel_common::keychain::remove_password;
///
/// let profile_id = Uuid::new_v4();
/// remove_password(&profile_id)?;
/// # Ok::<(), ssh_tunnel_common::Error>(())
/// ```
pub fn remove_password(profile_id: &Uuid) -> Result<()> {
    remove_password_in(default_store().as_ref(), profile_id)
}

/// Whether a password is stored for the given profile.
///
/// `Ok(false)` means nothing is stored. A keychain that cannot be reached is an `Err`, not a
/// `false` — before v0.3.0 the two were indistinguishable, which is why a change of backing
/// store looked to callers like an empty keyring.
///
/// # Examples
/// ```no_run
/// use uuid::Uuid;
/// use ssh_tunnel_common::keychain::has_password;
///
/// let profile_id = Uuid::new_v4();
/// if has_password(&profile_id)? {
///     println!("Password is stored in keychain");
/// }
/// # Ok::<(), ssh_tunnel_common::Error>(())
/// ```
pub fn has_password(profile_id: &Uuid) -> Result<bool> {
    has_password_in(default_store().as_ref(), profile_id)
}

// --- Store-parameterised implementations ------------------------------------
//
// The public functions above pass the default store; tests pass a fake. Injecting per call
// rather than swapping a global keeps tests free of shared mutable state, so they can run in
// parallel like the rest of the suite.

fn store_password_in(store: &dyn CredentialStore, profile_id: &Uuid, password: &str) -> Result<()> {
    store.set(SERVICE, &account_for(profile_id), password)
}

fn get_password_in(store: &dyn CredentialStore, profile_id: &Uuid) -> Result<String> {
    match store.get(SERVICE, &account_for(profile_id))? {
        Some(secret) => Ok(secret),
        None => Err(Error::Keychain(format!(
            "No password stored for profile {profile_id}"
        ))),
    }
}

fn remove_password_in(store: &dyn CredentialStore, profile_id: &Uuid) -> Result<()> {
    store.delete(SERVICE, &account_for(profile_id))
}

fn has_password_in(store: &dyn CredentialStore, profile_id: &Uuid) -> Result<bool> {
    // `Ok(false)` means "nothing stored"; a store that cannot be reached is an error.
    // Collapsing the two is what made the v0.2.0 backend change look like an empty keyring.
    Ok(store.get(SERVICE, &account_for(profile_id))?.is_some())
}

/// Whether a `SSH_TUNNEL_SKIP_KEYRING` value means "skip".
///
/// Split out from the environment lookup so it can be tested: the variable is process-wide,
/// and mutating it would race with every other test in the binary.
fn skip_requested(value: Option<&str>) -> bool {
    matches!(
        value,
        Some("1") | Some("true") | Some("True") | Some("TRUE")
    )
}

/// Whether keyring operations should be skipped entirely.
///
/// True when `SSH_TUNNEL_SKIP_KEYRING` is `1`, `true`, `True` or `TRUE`.
fn should_skip_keyring() -> bool {
    skip_requested(std::env::var("SSH_TUNNEL_SKIP_KEYRING").ok().as_deref())
}

/// Whether the keychain is available and functional.
///
/// Creating an entry forces `keyring` to initialise its credential store, which on Linux
/// means connecting to Secret Service over D-Bus — so this does exercise the stack rather
/// than merely constructing a value.
///
/// It does **not** prove the collection is unlocked, so a `true` here is necessary but not
/// sufficient for a later read to succeed.
///
/// Can be disabled with `SSH_TUNNEL_SKIP_KEYRING=1`.
///
/// # Examples
/// ```no_run
/// use ssh_tunnel_common::keychain::is_keychain_available;
///
/// if is_keychain_available() {
///     println!("Keychain is available");
/// } else {
///     println!("Keychain is not available - running in headless environment?");
/// }
/// ```
pub fn is_keychain_available() -> bool {
    !matches!(default_store().describe(), DISABLED)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    /// In-memory store for tests.
    ///
    /// `fail_everything` simulates a broken or locked keyring, which is the case that
    /// distinguishes "no password stored" from "cannot reach the store" — and the one no
    /// test could reach before, because the real store was called directly.
    #[derive(Default)]
    struct MemoryStore {
        entries: Mutex<HashMap<(String, String), String>>,
        fail_everything: bool,
    }

    impl MemoryStore {
        fn new() -> Self {
            Self::default()
        }

        fn broken() -> Self {
            Self {
                entries: Mutex::new(HashMap::new()),
                fail_everything: true,
            }
        }

        fn keys(&self) -> Vec<(String, String)> {
            let mut k: Vec<_> = self.entries.lock().unwrap().keys().cloned().collect();
            k.sort();
            k
        }
    }

    impl CredentialStore for MemoryStore {
        fn get(&self, service: &str, account: &str) -> Result<Option<String>> {
            if self.fail_everything {
                return Err(Error::Keychain("simulated store failure".into()));
            }
            Ok(self
                .entries
                .lock()
                .unwrap()
                .get(&(service.to_string(), account.to_string()))
                .cloned())
        }

        fn set(&self, service: &str, account: &str, secret: &str) -> Result<()> {
            if self.fail_everything {
                return Err(Error::Keychain("simulated store failure".into()));
            }
            self.entries.lock().unwrap().insert(
                (service.to_string(), account.to_string()),
                secret.to_string(),
            );
            Ok(())
        }

        fn delete(&self, service: &str, account: &str) -> Result<()> {
            if self.fail_everything {
                return Err(Error::Keychain("simulated store failure".into()));
            }
            self.entries
                .lock()
                .unwrap()
                .remove(&(service.to_string(), account.to_string()));
            Ok(())
        }

        fn describe(&self) -> &'static str {
            "in-memory (test)"
        }
    }

    #[test]
    fn a_stored_password_can_be_read_back() {
        let store = MemoryStore::new();
        let id = Uuid::new_v4();
        store_password_in(&store, &id, "hunter2").unwrap();
        assert_eq!(get_password_in(&store, &id).unwrap(), "hunter2");
    }

    #[test]
    fn reading_an_absent_password_is_an_error() {
        let store = MemoryStore::new();
        assert!(get_password_in(&store, &Uuid::new_v4()).is_err());
    }

    #[test]
    fn storing_twice_replaces_the_secret() {
        let store = MemoryStore::new();
        let id = Uuid::new_v4();
        store_password_in(&store, &id, "first").unwrap();
        store_password_in(&store, &id, "second").unwrap();
        assert_eq!(get_password_in(&store, &id).unwrap(), "second");
    }

    #[test]
    fn profiles_do_not_share_credentials() {
        let store = MemoryStore::new();
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        store_password_in(&store, &a, "a-secret").unwrap();
        store_password_in(&store, &b, "b-secret").unwrap();
        assert_eq!(get_password_in(&store, &a).unwrap(), "a-secret");
        assert_eq!(get_password_in(&store, &b).unwrap(), "b-secret");
    }

    #[test]
    fn removal_is_idempotent_and_clears_the_entry() {
        let store = MemoryStore::new();
        let id = Uuid::new_v4();
        store_password_in(&store, &id, "hunter2").unwrap();

        remove_password_in(&store, &id).unwrap();
        assert!(get_password_in(&store, &id).is_err());

        // Removing again must still succeed.
        remove_password_in(&store, &id).unwrap();
    }

    #[test]
    fn removing_a_password_that_was_never_stored_succeeds() {
        let store = MemoryStore::new();
        remove_password_in(&store, &Uuid::new_v4()).unwrap();
    }

    #[test]
    fn has_password_is_true_only_once_something_is_stored() {
        let store = MemoryStore::new();
        let id = Uuid::new_v4();
        assert!(!has_password_in(&store, &id).unwrap());
        store_password_in(&store, &id, "hunter2").unwrap();
        assert!(has_password_in(&store, &id).unwrap());
        remove_password_in(&store, &id).unwrap();
        assert!(!has_password_in(&store, &id).unwrap());
    }

    /// The wart this replaces: until v0.3.0 a broken or locked keyring was indistinguishable
    /// from "no password stored", so a change of backing store looked like an empty keyring
    /// rather than an unreachable one. That is exactly how the v0.2.0 regression presented.
    #[test]
    fn has_password_errors_when_the_store_is_unreachable() {
        let store = MemoryStore::broken();
        assert!(
            has_password_in(&store, &Uuid::new_v4()).is_err(),
            "an unreachable store must not be reported as 'no password stored'"
        );
    }

    #[test]
    fn has_password_is_false_for_a_reachable_store_with_no_entry() {
        let store = MemoryStore::new();
        assert!(!has_password_in(&store, &Uuid::new_v4()).unwrap());
    }

    /// A broken store must not be mistaken for an empty one on the read path.
    #[test]
    fn a_broken_store_is_an_error_when_reading_directly() {
        let store = MemoryStore::broken();
        assert!(get_password_in(&store, &Uuid::new_v4()).is_err());
    }

    /// Guards the entry key. Changing the service name or the account format orphans every
    /// stored credential — silently, since a miss is indistinguishable from "never saved".
    #[test]
    fn credentials_are_filed_under_the_service_name_and_profile_uuid() {
        let store = MemoryStore::new();
        let id = Uuid::new_v4();
        store_password_in(&store, &id, "hunter2").unwrap();
        assert_eq!(
            store.keys(),
            vec![("ssh-tunnel-manager".to_string(), id.to_string())]
        );
    }

    #[test]
    fn skip_keyring_accepts_only_the_documented_spellings() {
        for accepted in ["1", "true", "True", "TRUE"] {
            assert!(skip_requested(Some(accepted)), "{accepted} should skip");
        }
        // Note "TrUe" and "yes": the check is not case-insensitive and not truthy-parsing.
        // Documenting that here is the point — a user setting `SSH_TUNNEL_SKIP_KEYRING=yes`
        // gets the keyring anyway, with no warning.
        for rejected in ["0", "false", "yes", "on", "", "TrUe", " 1"] {
            assert!(
                !skip_requested(Some(rejected)),
                "{rejected} should not skip"
            );
        }
    }

    #[test]
    fn skip_keyring_is_off_when_the_variable_is_unset() {
        assert!(!skip_requested(None));
    }
}
