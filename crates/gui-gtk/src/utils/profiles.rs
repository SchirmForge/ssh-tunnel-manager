// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Profile loading utilities

use anyhow::Result;
use ssh_tunnel_common::config::Profile;
use std::path::PathBuf;

/// Load all profiles from the profiles directory
pub fn load_all_profiles() -> Result<Vec<Profile>> {
    let profiles_dir = get_profiles_dir()?;

    // Ensure directory exists
    if !profiles_dir.exists() {
        return Ok(Vec::new());
    }

    // Use the common crate's profile manager
    ssh_tunnel_common::profile_manager::load_all_profiles()
}

/// Get the profiles directory path
pub fn get_profiles_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;

    let profiles_dir = config_dir
        .join("ssh-tunnel-manager")
        .join("profiles");

    Ok(profiles_dir)
}

/// Check if profiles directory exists and create if needed
pub fn ensure_profiles_dir() -> Result<PathBuf> {
    let profiles_dir = get_profiles_dir()?;

    if !profiles_dir.exists() {
        std::fs::create_dir_all(&profiles_dir)?;
    }

    Ok(profiles_dir)
}
