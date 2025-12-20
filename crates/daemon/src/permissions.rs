// Permissions and security hardening for daemon files and directories

use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tracing::{debug, info};

/// Set restrictive umask to prevent file permission leaks
/// Should be called early in main() before creating any files
pub fn set_restrictive_umask() {
    #[cfg(unix)]
    {
        // Set umask to 0077 (rwx------) - only owner can access
        // This ensures any files created inherit restrictive permissions
        unsafe {
            libc::umask(0o077);
        }
        debug!("Set restrictive umask: 0077");
    }
}

/// Set file permissions to 0600 (owner read/write only)
pub fn set_file_permissions_private(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let perms = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, perms)
            .context(format!("Failed to set permissions on {}", path.display()))?;
        debug!("Set file permissions to 0600: {}", path.display());
    }
    Ok(())
}

/// Set directory permissions based on group_access setting
/// - If group_access=false: 0700 (owner only)
/// - If group_access=true: 0770 (owner and group)
pub fn set_directory_permissions(path: &Path, group_access: bool) -> Result<()> {
    #[cfg(unix)]
    {
        let mode = if group_access { 0o770 } else { 0o700 };
        let perms = fs::Permissions::from_mode(mode);
        fs::set_permissions(path, perms)
            .context(format!("Failed to set permissions on {}", path.display()))?;
        info!(
            "Set directory permissions to {:o}: {}",
            mode,
            path.display()
        );
    }
    Ok(())
}

/// Set Unix socket permissions based on group_access setting
/// - If group_access=false: 0600 (owner only)
/// - If group_access=true: 0660 (owner and group)
pub fn set_socket_permissions(path: &Path, group_access: bool) -> Result<()> {
    #[cfg(unix)]
    {
        let mode = if group_access { 0o660 } else { 0o600 };
        let perms = fs::Permissions::from_mode(mode);
        fs::set_permissions(path, perms)
            .context(format!("Failed to set permissions on {}", path.display()))?;
        info!(
            "Set socket permissions to {:o}: {}",
            mode,
            path.display()
        );
    }
    Ok(())
}

/// Ensure a directory exists and set appropriate permissions
/// Creates parent directories as needed with correct permissions
pub fn ensure_directory_with_permissions(path: &Path, group_access: bool) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)
            .context(format!("Failed to create directory {}", path.display()))?;
        debug!("Created directory: {}", path.display());
    }

    set_directory_permissions(path, group_access)?;
    Ok(())
}
