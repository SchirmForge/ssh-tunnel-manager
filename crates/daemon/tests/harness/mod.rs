// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Test harness: runs a real `ssh-tunnel-daemon` inside a throwaway sandbox.
//!
//! Every config path in the project resolves through `dirs::config_dir()` and the
//! socket and PID file through `dirs::runtime_dir()`, which on Linux honour
//! `XDG_CONFIG_HOME` and `XDG_RUNTIME_DIR`. Those are set on the spawned child's
//! environment rather than this process's, so the developer's real
//! `~/.config/ssh-tunnel-manager` and any running daemon are never touched, and
//! tests can still run in parallel.

#![allow(dead_code)] // Each integration test binary uses a different subset.

pub mod live;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use ssh_tunnel_common::{
    add_auth_header, create_daemon_client, ConnectionMode, DaemonClientConfig,
};
use tempfile::TempDir;

/// How long to wait for a freshly spawned daemon to become reachable.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);

pub struct DaemonHarness {
    sandbox: TempDir,
    child: Option<Child>,
    client_config: DaemonClientConfig,
    log_path: PathBuf,
}

impl DaemonHarness {
    /// Start a daemon in the default Unix-socket mode.
    pub async fn start_unix() -> Self {
        Self::start(None, ConnectionMode::UnixSocket, None).await
    }

    /// Start a daemon on `127.0.0.1` in `tcp-http` mode, on a free port.
    pub async fn start_http() -> Self {
        let port = free_port();
        let config = format!(
            "listener_mode = \"tcp-http\"\n\
             bind_host = \"127.0.0.1\"\n\
             bind_port = {port}\n\
             require_auth = true\n"
        );
        Self::start(Some(&config), ConnectionMode::Http, Some(port)).await
    }

    /// Spawn a daemon that is expected to refuse to start, and return its output.
    ///
    /// Used to assert configuration validation, so it does not wait for readiness.
    pub fn start_expecting_failure(daemon_toml: &str) -> String {
        let sandbox = new_sandbox();
        write_daemon_config(sandbox.path(), daemon_toml);

        let output = daemon_command(sandbox.path())
            .output()
            .expect("Should run daemon");

        assert!(
            !output.status.success(),
            "Daemon was expected to refuse this config but started successfully"
        );

        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }

    async fn start(daemon_toml: Option<&str>, mode: ConnectionMode, port: Option<u16>) -> Self {
        let sandbox = new_sandbox();
        if let Some(toml) = daemon_toml {
            write_daemon_config(sandbox.path(), toml);
        }

        let log_path = sandbox.path().join("daemon.log");
        let log = std::fs::File::create(&log_path).expect("Should create daemon log");

        let child = daemon_command(sandbox.path())
            .stdout(Stdio::from(
                log.try_clone().expect("Should clone log handle"),
            ))
            .stderr(Stdio::from(log))
            .spawn()
            .expect("Should spawn daemon");

        let mut harness = Self {
            client_config: DaemonClientConfig {
                connection_mode: mode,
                daemon_host: "127.0.0.1".to_string(),
                // Must be set before wait_until_ready polls, or it probes the default port.
                daemon_port: port.unwrap_or(DaemonClientConfig::default().daemon_port),
                daemon_url: socket_path(sandbox.path()).display().to_string(),
                ..Default::default()
            },
            sandbox,
            child: Some(child),
            log_path,
        };

        harness.wait_until_ready().await;
        harness.client_config.auth_token = harness.token();
        harness
    }

    /// The daemon's generated auth token.
    pub fn token(&self) -> String {
        std::fs::read_to_string(self.config_dir().join("daemon.token"))
            .unwrap_or_else(|e| panic!("Should read daemon.token: {e}\n{}", self.log()))
            .trim()
            .to_string()
    }

    pub fn config_dir(&self) -> PathBuf {
        self.sandbox
            .path()
            .join("config")
            .join("ssh-tunnel-manager")
    }

    pub fn runtime_dir(&self) -> PathBuf {
        self.sandbox.path().join("run").join("ssh-tunnel-manager")
    }

    pub fn socket_path(&self) -> PathBuf {
        socket_path(self.sandbox.path())
    }

    /// Whatever the daemon wrote to stdout/stderr so far. Included in failure messages.
    pub fn log(&self) -> String {
        std::fs::read_to_string(&self.log_path).unwrap_or_default()
    }

    /// A client config carrying the correct token.
    pub fn client_config(&self) -> DaemonClientConfig {
        self.client_config.clone()
    }

    /// A client config with the given token instead of the real one.
    pub fn client_config_with_token(&self, token: &str) -> DaemonClientConfig {
        DaemonClientConfig {
            auth_token: token.to_string(),
            ..self.client_config.clone()
        }
    }

    /// GET `path` using the daemon's real token.
    pub async fn get(&self, path: &str) -> reqwest::Response {
        self.get_with_config(path, &self.client_config).await
    }

    /// GET `path` with an arbitrary client config, for auth tests.
    pub async fn get_with_config(
        &self,
        path: &str,
        config: &DaemonClientConfig,
    ) -> reqwest::Response {
        let url = format!(
            "{}{path}",
            config.daemon_base_url().expect("Should build base url")
        );
        let client = create_daemon_client(config).expect("Should create client");
        let request = add_auth_header(client.get(&url), config).expect("Should add auth header");
        request
            .send()
            .await
            .unwrap_or_else(|e| panic!("Request to {url} failed: {e}\n{}", self.log()))
    }

    /// A reqwest client wired for this daemon's connection mode.
    pub fn http_client(&self) -> reqwest::Client {
        create_daemon_client(&self.client_config).expect("Should create client")
    }

    /// Write a profile into the sandbox's profile directory, where the daemon
    /// will find it in `ProfileSourceMode::Local`.
    pub fn install_profile(&self, profile: &ssh_tunnel_common::Profile) {
        let dir = self.config_dir().join("profiles");
        std::fs::create_dir_all(&dir).expect("Should create profiles dir");
        let toml = toml::to_string_pretty(profile).expect("Should serialize profile");
        std::fs::write(dir.join(format!("{}.toml", profile.metadata.id)), toml)
            .expect("Should write profile");
    }

    /// Seed `known_hosts` so a connection is not interrupted by host key
    /// verification. Tests that want to exercise verification skip this.
    pub fn trust_host_key(&self, entry: &str) {
        std::fs::write(self.config_dir().join("known_hosts"), entry)
            .expect("Should write known_hosts");
    }

    async fn wait_until_ready(&self) {
        let deadline = Instant::now() + STARTUP_TIMEOUT;

        while Instant::now() < deadline {
            // The daemon writes its token before it starts listening, and the
            // socket/port only appears once it is serving.
            let listening = match self.client_config.connection_mode {
                ConnectionMode::UnixSocket => self.socket_path().exists(),
                _ => true,
            };

            if listening && self.config_dir().join("daemon.token").exists() {
                let config = DaemonClientConfig {
                    auth_token: self.token(),
                    ..self.client_config.clone()
                };
                if let Ok(client) = create_daemon_client(&config) {
                    let url = format!(
                        "{}/api/health",
                        config.daemon_base_url().expect("Should build base url")
                    );
                    if let Ok(request) = add_auth_header(client.get(&url), &config) {
                        if let Ok(response) = request.send().await {
                            if response.status().is_success() {
                                return;
                            }
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        panic!(
            "Daemon did not become ready within {STARTUP_TIMEOUT:?}\n--- daemon log ---\n{}",
            self.log()
        );
    }
}

impl Drop for DaemonHarness {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn new_sandbox() -> TempDir {
    let sandbox = tempfile::Builder::new()
        .prefix("ssh-tunnel-it.")
        .tempdir()
        .expect("Should create sandbox");

    let config_dir = sandbox.path().join("config").join("ssh-tunnel-manager");
    let runtime_dir = sandbox.path().join("run");
    std::fs::create_dir_all(&config_dir).expect("Should create config dir");
    std::fs::create_dir_all(&runtime_dir).expect("Should create runtime dir");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&runtime_dir, std::fs::Permissions::from_mode(0o700))
            .expect("Should set runtime dir permissions");
    }

    sandbox
}

fn write_daemon_config(sandbox: &Path, toml: &str) {
    let path = sandbox
        .join("config")
        .join("ssh-tunnel-manager")
        .join("daemon.toml");
    std::fs::write(path, toml).expect("Should write daemon.toml");
}

fn daemon_command(sandbox: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ssh-tunnel-daemon"));
    command
        .env("XDG_CONFIG_HOME", sandbox.join("config"))
        .env("XDG_RUNTIME_DIR", sandbox.join("run"))
        .env("SSH_TUNNEL_SKIP_KEYRING", "1")
        .env("RUST_LOG", "debug")
        // Do not inherit a HOME that would let anything fall back to the real config.
        .env("HOME", sandbox);
    command
}

fn socket_path(sandbox: &Path) -> PathBuf {
    sandbox
        .join("run")
        .join("ssh-tunnel-manager")
        .join("ssh-tunnel-manager.sock")
}

/// Ask the OS for a free TCP port by binding and immediately releasing one.
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Should bind ephemeral port");
    listener
        .local_addr()
        .expect("Should read local addr")
        .port()
}

/// Format a `known_hosts` host pattern the way OpenSSH — and
/// `daemon::known_hosts::format_host_pattern` — both do: a bare hostname on the
/// default port, `[host]:port` otherwise.
///
/// Getting this wrong is silent and expensive: an entry written as `[host]:22`
/// matches nothing on port 22, so a test that means to poison `known_hosts` ends
/// up asserting against an *unknown* host instead of a *changed* one, and passes
/// for the wrong reason.
pub fn known_hosts_pattern(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    }
}
