// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Security module

use anyhow::{Context, Result};
use keyring::Entry;
use uuid::Uuid;

/// Retrieve password or passphrase from system keychain
pub fn get_stored_password(profile_id: &Uuid) -> Result<String> {
    let entry = Entry::new("ssh-tunnel-manager", &profile_id.to_string())
        .context("Failed to access keychain entry")?;
    entry
        .get_password()
        .context("Failed to retrieve password from keychain")
}
