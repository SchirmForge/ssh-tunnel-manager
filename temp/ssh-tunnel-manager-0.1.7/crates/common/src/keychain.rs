// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Keychain management module - centralized password/passphrase storage

use keyring::Entry;
use uuid::Uuid;

use crate::error::{Error, Result};

/// Store a password or passphrase in the system keychain
///
/// Uses the service name "ssh-tunnel-manager" and the profile UUID as the username.
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
    let entry = Entry::new("ssh-tunnel-manager", &profile_id.to_string())
        .map_err(|e| Error::Keychain(format!("Failed to create keychain entry: {}", e)))?;

    entry
        .set_password(password)
        .map_err(|e| Error::Keychain(format!("Failed to store password in keychain: {}", e)))?;

    Ok(())
}

/// Retrieve a password or passphrase from the system keychain
///
/// Uses the service name "ssh-tunnel-manager" and the profile UUID as the username.
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
    let entry = Entry::new("ssh-tunnel-manager", &profile_id.to_string())
        .map_err(|e| Error::Keychain(format!("Failed to access keychain entry: {}", e)))?;

    entry
        .get_password()
        .map_err(|e| Error::Keychain(format!("Failed to retrieve password from keychain: {}", e)))
}

/// Remove a password or passphrase from the system keychain
///
/// Returns `Ok(())` even if the password doesn't exist (idempotent operation).
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
    let entry = Entry::new("ssh-tunnel-manager", &profile_id.to_string())
        .map_err(|e| Error::Keychain(format!("Failed to create keychain entry: {}", e)))?;

    // Ignore error if password doesn't exist - this is idempotent
    let _ = entry.delete_credential();

    Ok(())
}

/// Check if a password exists in the keychain for the given profile
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
    match get_password(profile_id) {
        Ok(_) => Ok(true),
        Err(Error::Keychain(_)) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Check if keyring operations should be completely skipped
///
/// Returns `true` if the SSH_TUNNEL_SKIP_KEYRING environment variable is set to 1, true, or TRUE.
fn should_skip_keyring() -> bool {
    if let Ok(val) = std::env::var("SSH_TUNNEL_SKIP_KEYRING") {
        matches!(val.as_str(), "1" | "true" | "True" | "TRUE")
    } else {
        false
    }
}

/// Check if keychain/keyring is available and functional
///
/// Performs a lightweight test by attempting to create a temporary entry.
/// Does NOT actually store any data - just tests accessibility.
///
/// Returns `true` if keyring operations will succeed, `false` otherwise.
///
/// Can be explicitly disabled by setting the `SSH_TUNNEL_SKIP_KEYRING` environment
/// variable to `1`, `true`, or `TRUE`.
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
    // Check for explicit override
    if should_skip_keyring() {
        return false;
    }

    // Try to create a test entry without storing data
    // This tests the entire stack: DBus session, Secret Service daemon, permissions
    match Entry::new("ssh-tunnel-manager", "__availability_test__") {
        Ok(_) => {
            // Entry creation succeeded - keyring is available
            // Note: We don't need to clean up, Entry::new doesn't persist anything
            true
        }
        Err(_) => {
            // Entry creation failed - keyring unavailable
            // Common causes:
            // - No DBus session bus
            // - No Secret Service daemon (gnome-keyring, kwallet, etc.)
            // - Permission denied
            // - Platform keyring locked or inaccessible
            false
        }
    }
}
