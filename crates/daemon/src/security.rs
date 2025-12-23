// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Security module

use anyhow::Result;
use uuid::Uuid;

/// Retrieve password or passphrase from system keychain
///
/// This is a convenience wrapper around ssh_tunnel_common::keychain::get_password
/// that converts the error type to anyhow::Error.
pub fn get_stored_password(profile_id: &Uuid) -> Result<String> {
    ssh_tunnel_common::get_password(profile_id)
        .map_err(|e| anyhow::anyhow!("Failed to retrieve password from keychain: {}", e))
}
