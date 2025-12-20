// SSH Tunnel Manager - PID File Management
// Ensures only one daemon instance runs at a time

use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};
use tracing::{debug, info, warn};

/// PID file guard - automatically removes PID file on drop
#[derive(Debug)]
pub struct PidFileGuard {
    path: PathBuf,
}

impl PidFileGuard {
    /// Create a new PID file guard
    ///
    /// This will check if a daemon is already running and fail if so.
    /// On success, it creates a PID file that is automatically removed when dropped.
    pub fn create() -> Result<Self> {
        let path = Self::pid_file_path()?;

        // Check if PID file exists
        if path.exists() {
            // Try to read the existing PID
            match fs::read_to_string(&path) {
                Ok(pid_str) => {
                    if let Ok(pid) = pid_str.trim().parse::<u32>() {
                        // Check if the process is actually running
                        if Self::is_process_running(pid) {
                            anyhow::bail!(
                                "Daemon is already running with PID {}. \
                                 Stop the existing daemon first or remove {} if it's stale.",
                                pid,
                                path.display()
                            );
                        } else {
                            warn!(
                                "Found stale PID file for process {} (not running), removing it",
                                pid
                            );
                            // Remove stale PID file
                            fs::remove_file(&path)
                                .context("Failed to remove stale PID file")?;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read PID file {}: {}", path.display(), e);
                    // If we can't read it, try to remove it
                    let _ = fs::remove_file(&path);
                }
            }
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create runtime directory")?;
        }

        // Write our PID to the file
        let pid = std::process::id();
        fs::write(&path, pid.to_string())
            .context("Failed to write PID file")?;

        info!("Created PID file at {} with PID {}", path.display(), pid);

        Ok(Self { path })
    }

    /// Get the path to the PID file
    fn pid_file_path() -> Result<PathBuf> {
        let runtime_dir = dirs::runtime_dir()
            .or_else(|| dirs::cache_dir())
            .ok_or_else(|| anyhow::anyhow!("Could not determine runtime directory"))?;

        Ok(runtime_dir.join("ssh-tunnel-manager").join("daemon.pid"))
    }

    /// Check if a process with the given PID is running
    ///
    /// This uses platform-specific methods to check process existence
    #[cfg(unix)]
    fn is_process_running(pid: u32) -> bool {
        // On Unix, we can use kill(pid, 0) to check if a process exists
        // This doesn't actually send a signal, just checks permissions
        unsafe {
            let result = libc::kill(pid as i32, 0);
            if result == 0 {
                // Process exists
                return true;
            }

            // Check errno to distinguish between different errors
            let errno = *libc::__errno_location();
            match errno {
                libc::ESRCH => false,  // No such process
                libc::EPERM => true,   // Process exists but we don't have permission
                _ => false,
            }
        }
    }

    #[cfg(not(unix))]
    fn is_process_running(_pid: u32) -> bool {
        // On non-Unix systems, conservatively assume process might be running
        // This means manual cleanup might be needed on Windows
        warn!("Process existence check not implemented for this platform");
        true
    }
}

impl Drop for PidFileGuard {
    fn drop(&mut self) {
        // Remove PID file when the guard is dropped
        match fs::remove_file(&self.path) {
            Ok(_) => {
                debug!("Removed PID file: {}", self.path.display());
            }
            Err(e) => {
                warn!("Failed to remove PID file {}: {}", self.path.display(), e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid_file_prevents_multiple_instances() {
        // First instance should succeed
        let _guard1 = PidFileGuard::create().expect("First instance should succeed");

        // Second instance should fail
        let result = PidFileGuard::create();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already running"));

        // After first guard is dropped, second instance should succeed
        drop(_guard1);
        let _guard2 = PidFileGuard::create().expect("Should succeed after first is dropped");
    }

    #[test]
    fn test_current_process_is_running() {
        let current_pid = std::process::id();
        assert!(PidFileGuard::is_process_running(current_pid));
    }

    #[test]
    fn test_nonexistent_process_not_running() {
        // PID 1 is usually init/systemd, but we'll use a very high unlikely PID
        // On most systems, PIDs don't go this high
        assert!(!PidFileGuard::is_process_running(999999));
    }
}
