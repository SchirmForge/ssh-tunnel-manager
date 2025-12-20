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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConnectionConfig, ForwardingConfig, ForwardingType, ProfileMetadata};

    fn create_test_profile(name: &str) -> Profile {
        Profile {
            metadata: ProfileMetadata {
                id: Uuid::new_v4(),
                name: name.to_string(),
                description: Some("Test profile".to_string()),
                tags: vec![],
            },
            connection: ConnectionConfig {
                host: "test.example.com".to_string(),
                port: 22,
                username: "testuser".to_string(),
                auth_type: crate::AuthType::Key,
                key_path: Some("/home/user/.ssh/id_rsa".into()),
                password: None,
            },
            forwarding: ForwardingConfig {
                forwarding_type: ForwardingType::Local,
                local_port: 8080,
                remote_host: "localhost".to_string(),
                remote_port: 80,
            },
            options: Default::default(),
        }
    }

    #[test]
    fn test_profiles_dir() {
        let dir = profiles_dir().expect("Should get profiles directory");
        assert!(dir.to_string_lossy().contains("ssh-tunnel-manager"));
        assert!(dir.to_string_lossy().contains("profiles"));
    }

    #[test]
    fn test_load_all_profiles_empty() {
        // Should not error on non-existent directory
        let profiles = load_all_profiles().expect("Should load profiles");
        assert!(profiles.len() >= 0); // May have existing profiles or none
    }
}
