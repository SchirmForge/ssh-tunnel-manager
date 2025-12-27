// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Profile operations and validation

use ssh_tunnel_common::Profile;
use anyhow::Result;
use uuid::Uuid;

/// Load all profiles from disk
pub fn load_profiles() -> Result<Vec<Profile>> {
    ssh_tunnel_common::load_all_profiles()
}

/// Save a profile (create or update)
pub fn save_profile(profile: &Profile, overwrite: bool) -> Result<()> {
    ssh_tunnel_common::save_profile(profile, overwrite)?;
    Ok(())
}

/// Delete a profile by ID
pub fn delete_profile(profile_id: Uuid) -> Result<()> {
    ssh_tunnel_common::delete_profile_by_id(&profile_id)?;
    Ok(())
}

/// Validate profile data before saving
pub fn validate_profile(profile: &Profile) -> Result<()> {
    // Basic validation
    if profile.metadata.name.trim().is_empty() {
        anyhow::bail!("Profile name cannot be empty");
    }

    if profile.connection.host.trim().is_empty() {
        anyhow::bail!("Host cannot be empty");
    }

    if profile.connection.user.trim().is_empty() {
        anyhow::bail!("User cannot be empty");
    }

    if profile.connection.port == 0 {
        anyhow::bail!("Port must be greater than 0");
    }

    // SSH key validation
    use ssh_tunnel_common::AuthType;
    if matches!(profile.connection.auth_type, AuthType::Key) {
        if profile.connection.key_path.is_none() {
            anyhow::bail!("SSH key path is required when using key authentication");
        }
    }

    // Forwarding validation
    use ssh_tunnel_common::ForwardingType;
    match &profile.forwarding.forwarding_type {
        ForwardingType::Local => {
            if profile.forwarding.local_port.is_none() || profile.forwarding.local_port == Some(0) {
                anyhow::bail!("Local port must be greater than 0");
            }
            if profile.forwarding.remote_port.is_none() || profile.forwarding.remote_port == Some(0) {
                anyhow::bail!("Remote port must be greater than 0");
            }
        }
        ForwardingType::Remote => {
            if profile.forwarding.local_port.is_none() || profile.forwarding.local_port == Some(0) {
                anyhow::bail!("Local port must be greater than 0");
            }
            if profile.forwarding.remote_port.is_none() || profile.forwarding.remote_port == Some(0) {
                anyhow::bail!("Remote port must be greater than 0");
            }
        }
        ForwardingType::Dynamic => {
            if profile.forwarding.local_port.is_none() || profile.forwarding.local_port == Some(0) {
                anyhow::bail!("SOCKS port must be greater than 0");
            }
        }
    }

    Ok(())
}

/// Check if profile name already exists (excluding given ID)
pub fn profile_name_exists(name: &str, exclude_id: Option<Uuid>) -> bool {
    match load_profiles() {
        Ok(profiles) => {
            profiles.iter().any(|p| {
                p.metadata.name.eq_ignore_ascii_case(name)
                    && exclude_id.map_or(true, |id| p.metadata.id != id)
            })
        }
        Err(_) => false,
    }
}
