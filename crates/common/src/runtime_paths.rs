// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Where the daemon keeps its Unix socket and PID file, and where clients look for them.
//!
//! This is one module because there used to be three, and they disagreed. The socket path
//! collapsed `$XDG_RUNTIME_DIR` that was already named `ssh-tunnel-manager` instead of nesting
//! a second level; the PID file did not, and additionally fell back to the cache directory.
//! Under the project's own systemd unit, which sets `XDG_RUNTIME_DIR=/run/ssh-tunnel-manager`,
//! that put the socket at `/run/ssh-tunnel-manager/ssh-tunnel-manager.sock` but the PID file at
//! `/run/ssh-tunnel-manager/ssh-tunnel-manager/daemon.pid` — a nested directory outside the
//! `RuntimeDirectory=` that systemd creates and cleans up.
//!
//! A daemon and a client that disagree about the socket path do not fail loudly; the client
//! simply reports that no daemon is running.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::Result;

/// Subdirectory (and `RuntimeDirectory=`) name used under a runtime directory.
pub const RUNTIME_SUBDIR: &str = "ssh-tunnel-manager";

/// The daemon's Unix socket file name.
pub const SOCKET_FILE_NAME: &str = "ssh-tunnel-manager.sock";

/// The daemon's PID file name.
pub const PID_FILE_NAME: &str = "daemon.pid";

/// Runtime directory for a daemon with no login session, e.g. a systemd service account.
///
/// The shipped unit creates this via `RuntimeDirectory=ssh-tunnel-manager` and points
/// `XDG_RUNTIME_DIR` at it.
pub const SYSTEM_RUNTIME_DIR: &str = "/run/ssh-tunnel-manager";

/// Normalise a runtime directory to the directory that holds our runtime files.
///
/// `$XDG_RUNTIME_DIR` may already *be* our directory — the systemd unit points it straight at
/// `/run/ssh-tunnel-manager` — in which case appending [`RUNTIME_SUBDIR`] again would nest a
/// pointless second level.
pub fn runtime_subdir(runtime_dir: &Path) -> PathBuf {
    if runtime_dir.file_name() == Some(OsStr::new(RUNTIME_SUBDIR)) {
        runtime_dir.to_path_buf()
    } else {
        runtime_dir.join(RUNTIME_SUBDIR)
    }
}

/// The directory this process should keep its runtime files in, as a daemon.
///
/// Falls back to [`SYSTEM_RUNTIME_DIR`] when there is no `$XDG_RUNTIME_DIR`. A dedicated
/// service account has no login session, so `pam_systemd` never creates `/run/user/<uid>` for
/// it; without this fallback such a daemon cannot start at all in Unix-socket mode.
pub fn daemon_runtime_dir() -> Result<PathBuf> {
    Ok(match dirs::runtime_dir() {
        Some(dir) => runtime_subdir(&dir),
        None => PathBuf::from(SYSTEM_RUNTIME_DIR),
    })
}

/// The socket this process should bind, as a daemon.
pub fn daemon_socket_path() -> Result<PathBuf> {
    Ok(daemon_runtime_dir()?.join(SOCKET_FILE_NAME))
}

/// The PID file this process should write, as a daemon.
///
/// Deliberately derived from the same directory as the socket: the two must not drift.
pub fn daemon_pid_path() -> Result<PathBuf> {
    Ok(daemon_runtime_dir()?.join(PID_FILE_NAME))
}

/// Sockets a client should look for, in priority order.
///
/// The user's own runtime directory first (the common case: a per-user daemon), then the
/// pre-subdirectory layout, then the system location used by a service account.
pub fn client_socket_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(runtime_dir) = dirs::runtime_dir() {
        candidates.push(runtime_subdir(&runtime_dir).join(SOCKET_FILE_NAME));
        // Backward compatibility: the layout before runtime files got their own subdirectory.
        candidates.push(runtime_dir.join(SOCKET_FILE_NAME));
    }

    candidates.push(PathBuf::from(SYSTEM_RUNTIME_DIR).join(SOCKET_FILE_NAME));
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_runtime_dir_gets_our_subdirectory() {
        assert_eq!(
            runtime_subdir(Path::new("/run/user/1000")),
            Path::new("/run/user/1000/ssh-tunnel-manager")
        );
    }

    /// The systemd unit sets `XDG_RUNTIME_DIR=/run/ssh-tunnel-manager` directly, so appending
    /// the subdirectory again would nest a second level outside `RuntimeDirectory=`.
    #[test]
    fn a_runtime_dir_that_is_already_ours_is_not_nested_again() {
        assert_eq!(
            runtime_subdir(Path::new("/run/ssh-tunnel-manager")),
            Path::new("/run/ssh-tunnel-manager")
        );
    }

    /// The property that was actually broken: the socket and the PID file must live together,
    /// whatever `$XDG_RUNTIME_DIR` happens to be.
    #[test]
    fn the_socket_and_the_pid_file_always_share_a_directory() {
        for runtime_dir in [
            Path::new("/run/user/1000"),
            Path::new("/run/ssh-tunnel-manager"),
            Path::new("/some/other/place"),
        ] {
            let dir = runtime_subdir(runtime_dir);
            let socket = dir.join(SOCKET_FILE_NAME);
            let pid = dir.join(PID_FILE_NAME);
            assert_eq!(
                socket.parent(),
                pid.parent(),
                "socket and PID file diverged for {}",
                runtime_dir.display()
            );
        }
    }

    /// A service account has no login session and so no `$XDG_RUNTIME_DIR`. Erroring there is
    /// what stopped a non-systemd system daemon from starting at all.
    #[test]
    fn a_client_always_considers_the_system_location() {
        let candidates = client_socket_candidates();
        assert!(
            candidates.contains(&PathBuf::from(
                "/run/ssh-tunnel-manager/ssh-tunnel-manager.sock"
            )),
            "a client must be able to find a daemon running as a service account: {candidates:?}"
        );
    }
}
