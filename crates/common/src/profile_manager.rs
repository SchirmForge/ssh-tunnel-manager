// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// SSH Tunnel Manager - Profile Manager Module
// Shared profile I/O operations for CLI, GUI, and Daemon

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::Profile;

/// Get the profiles directory path
pub fn profiles_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
    Ok(config_dir.join("ssh-tunnel-manager").join("profiles"))
}

/// Load all profiles from the config directory
pub fn load_all_profiles() -> Result<Vec<Profile>> {
    let profile_dir = profiles_dir()?;

    if !profile_dir.exists() {
        debug!(
            "Profiles directory does not exist: {}",
            profile_dir.display()
        );
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&profile_dir).context("Failed to read profiles directory")?;

    let mut profiles = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Skip non-TOML files
        if path.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }

        match load_profile(&path) {
            Ok(profile) => {
                debug!(
                    "Loaded profile: {} ({})",
                    profile.metadata.name, profile.metadata.id
                );
                profiles.push(profile);
            }
            Err(e) => {
                warn!("Failed to load profile {}: {}", path.display(), e);
            }
        }
    }

    Ok(profiles)
}

/// Load a single profile by its UUID
pub fn load_profile_by_id(id: &Uuid) -> Result<Profile> {
    let profile_dir = profiles_dir()?;
    let profile_path = profile_dir.join(format!("{}.toml", id));

    if !profile_path.exists() {
        anyhow::bail!("Profile not found: {}", id);
    }

    load_profile(&profile_path)
}

/// Load a single profile by its name
pub fn load_profile_by_name(name: &str) -> Result<Profile> {
    let profile_dir = profiles_dir()?;

    if !profile_dir.exists() {
        anyhow::bail!(
            "Profiles directory does not exist: {}",
            profile_dir.display()
        );
    }

    let entries = fs::read_dir(&profile_dir).context("Failed to read profiles directory")?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Skip non-TOML files
        if path.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }

        match load_profile(&path) {
            Ok(profile) => {
                if profile.metadata.name == name {
                    return Ok(profile);
                }
            }
            Err(e) => {
                warn!("Failed to load profile {}: {}", path.display(), e);
            }
        }
    }

    anyhow::bail!("Profile '{}' not found", name);
}

/// Load a single profile from a path
pub fn load_profile(path: &std::path::Path) -> Result<Profile> {
    let contents =
        fs::read_to_string(path).context(format!("Failed to read {}", path.display()))?;

    let profile: Profile =
        toml::from_str(&contents).context(format!("Failed to parse {}", path.display()))?;

    Ok(profile)
}

/// Save a profile to disk
///
/// # Arguments
/// * `profile` - The profile to save
/// * `overwrite` - If true, overwrite existing profile. If false, error if exists.
///
/// # Returns
/// The path where the profile was saved
pub fn save_profile(profile: &Profile, overwrite: bool) -> Result<PathBuf> {
    let profile_dir = profiles_dir()?;

    // Create directory if it doesn't exist
    fs::create_dir_all(&profile_dir).context("Failed to create profile directory")?;

    // Get profile path
    let profile_path = profile.config_path()?;

    // Check if profile already exists
    if !overwrite && profile_path.exists() {
        anyhow::bail!(
            "Profile '{}' already exists at: {}",
            profile.metadata.name,
            profile_path.display()
        );
    }

    // Serialize profile to TOML
    let toml_content = toml::to_string_pretty(&profile).context("Failed to serialize profile")?;

    // Write to file
    fs::write(&profile_path, toml_content).context(format!(
        "Failed to write profile to {}",
        profile_path.display()
    ))?;

    debug!("Saved profile '{}' to {}", profile.metadata.name, profile_path.display());

    Ok(profile_path)
}

/// Delete a profile from disk by UUID
pub fn delete_profile_by_id(id: &Uuid) -> Result<PathBuf> {
    let profile_dir = profiles_dir()?;
    let profile_path = profile_dir.join(format!("{}.toml", id));

    if !profile_path.exists() {
        anyhow::bail!("Profile not found: {}", id);
    }

    fs::remove_file(&profile_path).context(format!(
        "Failed to delete profile from {}",
        profile_path.display()
    ))?;

    debug!("Deleted profile at {}", profile_path.display());

    Ok(profile_path)
}

/// Delete a profile from disk by name
pub fn delete_profile_by_name(name: &str) -> Result<PathBuf> {
    let profile = load_profile_by_name(name)?;
    delete_profile_by_id(&profile.metadata.id)
}

/// Check if a profile with the given name exists
pub fn profile_exists_by_name(name: &str) -> bool {
    load_profile_by_name(name).is_ok()
}

/// Check if a profile with the given ID exists
pub fn profile_exists_by_id(id: &Uuid) -> bool {
    load_profile_by_id(id).is_ok()
}

/// Prepare a profile for remote daemon usage (Hybrid mode)
///
/// Converts SSH key path to filename only for sending via API to remote daemon.
/// The daemon will look for the key in its own ~/.ssh/ directory.
///
/// # Arguments
/// * `profile` - The profile to prepare
///
/// # Returns
/// A cloned profile with SSH key path converted to filename only
///
/// # Errors
/// Returns error if the profile has a key path but extracting filename fails
///
/// # Example
/// ```
/// use ssh_tunnel_common::{Profile, prepare_profile_for_remote};
/// // Profile with key_path: Some("/home/user/.ssh/id_ed25519")
/// // Returns profile with key_path: Some("id_ed25519")
/// ```
pub fn prepare_profile_for_remote(profile: &Profile) -> Result<Profile> {
    let mut remote_profile = profile.clone();

    // Convert SSH key path to filename only
    if let Some(key_path) = &profile.connection.key_path {
        let filename = key_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid SSH key path: {}", key_path.display()))?
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("SSH key filename contains invalid UTF-8"))?;

        remote_profile.connection.key_path = Some(PathBuf::from(filename));
    }

    Ok(remote_profile)
}

/// Generate SSH key setup instructions for remote daemon
///
/// Creates a user-friendly message with scp commands for copying SSH keys
/// to the daemon host.
///
/// # Arguments
/// * `key_path` - Path to the SSH private key on local machine
/// * `daemon_host` - Optional hostname/IP of the daemon (if None, uses "DAEMON_HOST" placeholder)
///
/// # Returns
/// Formatted string with scp and chmod commands
///
/// # Example
/// ```
/// use std::path::Path;
/// use ssh_tunnel_common::get_remote_key_setup_message;
///
/// let msg = get_remote_key_setup_message(
///     Path::new("/home/user/.ssh/id_ed25519"),
///     Some("example.com"),
///     Some("/var/lib/daemon_user/.ssh")
/// );
/// // Returns message with: scp /home/user/.ssh/id_ed25519 example.com:/var/lib/daemon_user/.ssh/id_ed25519
/// ```
pub fn get_remote_key_setup_message(
    key_path: &std::path::Path,
    daemon_host: Option<&str>,
    daemon_ssh_dir: Option<&str>,
) -> String {
    let host = daemon_host.unwrap_or("DAEMON_HOST");
    let key_display = key_path.display();

    let filename = key_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("SSH_KEY");

    // Use daemon's actual SSH directory if provided, otherwise fall back to ~/.ssh
    let ssh_dir = daemon_ssh_dir.unwrap_or("~/.ssh");
    let full_remote_path = format!("{}/{}", ssh_dir, filename);

    format!(
        "SSH key must be available on the daemon host.\n\n\
        Copy the SSH key '{}' to the daemon's .ssh directory:\n   \
           {} → {}:{}\n\n\
        Then ensure correct permissions (if needed):\n   \
           chmod 600 {}\n\n\
        The daemon will look for the key at: {}",
        filename, key_display, host, ssh_dir, full_remote_path, full_remote_path
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProfileMetadata;
    use crate::{ConnectionConfig, ForwardingConfig, ForwardingType};
    use std::path::Path;
    use std::sync::Mutex;

    fn create_test_profile(name: &str) -> Profile {
        use chrono::Utc;

        Profile {
            metadata: ProfileMetadata {
                id: Uuid::new_v4(),
                name: name.to_string(),
                description: Some("Test profile".to_string()),
                created_at: Utc::now(),
                modified_at: Utc::now(),
                tags: vec![],
            },
            connection: ConnectionConfig {
                host: "test.example.com".to_string(),
                port: 22,
                user: "testuser".to_string(),
                auth_type: crate::AuthType::Key,
                key_path: Some("/home/user/.ssh/id_rsa".into()),
                password_storage: crate::PasswordStorage::None,
            },
            forwarding: ForwardingConfig {
                forwarding_type: ForwardingType::Local,
                bind_address: "127.0.0.1".to_string(),
                local_port: Some(8080),
                remote_host: Some("localhost".to_string()),
                remote_port: Some(80),
            },
            options: Default::default(),
        }
    }

    /// Serialises the tests that redirect `XDG_CONFIG_HOME`.
    ///
    /// `profiles_dir()` resolves through `dirs::config_dir()`, which reads that variable.
    /// Environment mutation is process-global, so tests touching it must not run
    /// concurrently — and must never be allowed to hit the developer's real
    /// `~/.config/ssh-tunnel-manager`.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Runs `f` with `XDG_CONFIG_HOME` pointed at a fresh temporary directory,
    /// restoring the previous value afterwards.
    fn with_sandboxed_config<R>(f: impl FnOnce(&Path) -> R) -> R {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let tmp = tempfile::tempdir().expect("Should create temp dir");
        let previous = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", tmp.path());

        let result = f(tmp.path());

        match previous {
            Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }

        result
    }

    #[test]
    fn test_profiles_dir_is_under_config_home() {
        with_sandboxed_config(|root| {
            let dir = profiles_dir().expect("Should get profiles directory");
            assert_eq!(dir, root.join("ssh-tunnel-manager").join("profiles"));
        });
    }

    #[test]
    fn test_load_all_profiles_empty_when_dir_missing() {
        with_sandboxed_config(|_| {
            let profiles = load_all_profiles().expect("Should load profiles");
            assert!(
                profiles.is_empty(),
                "A fresh config home must yield no profiles, got {}",
                profiles.len()
            );
        });
    }

    #[test]
    fn test_profile_save_load_delete_round_trip() {
        with_sandboxed_config(|_| {
            let profile = create_test_profile("round-trip");
            let id = profile.metadata.id;

            let path = save_profile(&profile, false).expect("Should save profile");
            assert!(path.exists(), "Saved profile file should exist");

            // Refuses to clobber unless explicitly asked to.
            assert!(
                save_profile(&profile, false).is_err(),
                "Saving over an existing profile without overwrite must fail"
            );
            save_profile(&profile, true).expect("Overwrite should succeed");

            let by_id = load_profile_by_id(&id).expect("Should load by id");
            assert_eq!(by_id.metadata.name, "round-trip");
            assert_eq!(by_id.connection.host, "test.example.com");
            assert_eq!(by_id.forwarding.local_port, Some(8080));

            let by_name = load_profile_by_name("round-trip").expect("Should load by name");
            assert_eq!(by_name.metadata.id, id);

            assert_eq!(load_all_profiles().expect("Should load all").len(), 1);
            assert!(profile_exists_by_id(&id));
            assert!(profile_exists_by_name("round-trip"));

            delete_profile_by_id(&id).expect("Should delete profile");
            assert!(!profile_exists_by_id(&id));
            assert!(load_all_profiles().expect("Should load all").is_empty());
        });
    }

    /// Profiles written before v0.1.7 stored `password_stored` as a boolean.
    /// `PasswordStorage` replaced it with an enum and must still read the old form.
    #[test]
    fn test_password_storage_backward_compatible_with_boolean() {
        let legacy_true = r#"
            id = "550e8400-e29b-41d4-a716-446655440000"
            name = "legacy"
            created_at = "2024-01-15T10:30:00Z"
            modified_at = "2024-01-15T10:30:00Z"

            [connection]
            host = "example.com"
            port = 22
            user = "user"
            auth_type = "key"
            key_path = "/home/user/.ssh/id_ed25519"
            password_storage = true

            [forwarding]
            type = "local"
            local_port = 5432
            remote_host = "db.internal"
            remote_port = 5432
        "#;

        let profile: Profile = toml::from_str(legacy_true).expect("Legacy boolean should parse");
        assert_eq!(
            profile.connection.password_storage,
            crate::PasswordStorage::Keychain,
            "password_storage = true must migrate to Keychain"
        );

        let legacy_false = legacy_true.replace("password_storage = true", "password_storage = false");
        let profile: Profile = toml::from_str(&legacy_false).expect("Legacy boolean should parse");
        assert_eq!(
            profile.connection.password_storage,
            crate::PasswordStorage::None,
            "password_storage = false must migrate to None"
        );

        // And the current string form still round-trips.
        for (text, expected) in [
            ("\"keychain\"", crate::PasswordStorage::Keychain),
            ("\"none\"", crate::PasswordStorage::None),
            ("\"file\"", crate::PasswordStorage::File),
        ] {
            let source = legacy_true.replace("password_storage = true", &format!("password_storage = {text}"));
            let profile: Profile = toml::from_str(&source).expect("String form should parse");
            assert_eq!(profile.connection.password_storage, expected);
        }
    }

    #[test]
    fn test_prepare_profile_for_remote_reduces_key_to_filename() {
        let profile = create_test_profile("remote");
        let remote = prepare_profile_for_remote(&profile).expect("Should prepare profile");
        assert_eq!(
            remote.connection.key_path,
            Some(PathBuf::from("id_rsa")),
            "Hybrid mode must send the key filename only, never the local path"
        );
    }
}
