// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Tier-2 integration tests: real SSH connections against a real server.
//!
//! These cover the authentication flows, which are the most fragile part of the
//! codebase and until now had only ever been tested by hand. They drive
//! `start_tunnel_with_events` -- the same shared SSE-first flow the CLI and GUI
//! use -- so a regression in the client path is caught too.
//!
//! Every test is `#[ignore]`d, so `cargo test` never touches the network. Run
//! them with `make test-live` or `cargo test -- --ignored`.
//!
//! Configuration comes from `.local/testing/ssh-target.env` (gitignored). If it
//! is absent or an account is not configured, the affected test skips with a
//! message rather than failing. See docs/testing/ssh-target.env.template.

mod harness;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use harness::live::{self, LiveTarget};
use harness::DaemonHarness;
use ssh_tunnel_common::{
    start_tunnel_with_events, AuthRequest, AuthRequestType, AuthType, ConnectionConfig,
    ForwardingConfig, ForwardingType, PasswordStorage, Profile, TunnelEvent, TunnelEventHandler,
};

/// How long a live connection attempt may take before the test gives up.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);

// --- Scripted authentication ------------------------------------------------

/// Answers auth prompts from a script, and records what it was asked.
///
/// Each entry is consumed in order; the last one repeats. That lets a test
/// supply a wrong answer first and a correct one after, to prove the daemon
/// re-prompts rather than failing permanently.
struct ScriptedAuth {
    passphrases: Vec<String>,
    passwords: Vec<String>,
    codes: Vec<String>,
    seen: Arc<Mutex<Vec<AuthRequestType>>>,
    connected: Arc<Mutex<bool>>,
}

impl ScriptedAuth {
    fn new() -> Self {
        Self {
            passphrases: Vec::new(),
            passwords: Vec::new(),
            codes: Vec::new(),
            seen: Arc::new(Mutex::new(Vec::new())),
            connected: Arc::new(Mutex::new(false)),
        }
    }

    fn with_passwords(mut self, values: &[&str]) -> Self {
        self.passwords = values.iter().map(|s| s.to_string()).collect();
        self
    }

    fn with_passphrases(mut self, values: &[&str]) -> Self {
        self.passphrases = values.iter().map(|s| s.to_string()).collect();
        self
    }

    fn with_codes(mut self, values: &[String]) -> Self {
        self.codes = values.to_vec();
        self
    }

    fn seen_handle(&self) -> Arc<Mutex<Vec<AuthRequestType>>> {
        self.seen.clone()
    }

    /// Take the next scripted answer, repeating the final one.
    fn next(list: &mut Vec<String>) -> String {
        if list.is_empty() {
            return String::new();
        }
        if list.len() == 1 {
            return list[0].clone();
        }
        list.remove(0)
    }
}

impl TunnelEventHandler for ScriptedAuth {
    fn on_auth_required(&mut self, request: &AuthRequest) -> anyhow::Result<String> {
        self.seen
            .lock()
            .expect("lock should not be poisoned")
            .push(request.auth_type.clone());

        Ok(match request.auth_type {
            AuthRequestType::KeyPassphrase => Self::next(&mut self.passphrases),
            AuthRequestType::Password => Self::next(&mut self.passwords),
            // The daemon deliberately does not interpret keyboard-interactive
            // prompt text: it passes the server's wording through so non-English
            // servers work (the v0.1.10 fix). A PAM stack doing password + TOTP
            // therefore sends "Password: " and "Verification code: " under the
            // same `KeyboardInteractive` type, and only the text distinguishes
            // them. The test can safely read it — unlike the daemon, it controls
            // the server and knows its locale.
            AuthRequestType::TwoFactorCode | AuthRequestType::KeyboardInteractive => {
                let prompt = request.prompt.to_lowercase();
                let wants_password = prompt.contains("password") && !prompt.contains("code");
                if wants_password && !self.passwords.is_empty() {
                    Self::next(&mut self.passwords)
                } else {
                    Self::next(&mut self.codes)
                }
            }
            // Accept the host key on first sight; a dedicated test covers refusal.
            AuthRequestType::HostKeyVerification => "yes".to_string(),
        })
    }

    fn on_connected(&mut self) {
        *self.connected.lock().expect("lock should not be poisoned") = true;
    }

    fn on_event(&mut self, event: &TunnelEvent) {
        eprintln!("event: {event:?}");
    }
}

/// A handler that parks on an auth prompt until the test releases it, then refuses to answer.
///
/// `on_auth_required` is synchronous, so parking it blocks a runtime worker thread. That makes
/// it releasable by necessity rather than tidiness: a plain long `thread::sleep` is still
/// running when the test body finishes, and dropping a runtime waits for its workers, so the
/// suite sat out the full sleep — 300 seconds for what is otherwise a 20-second test.
///
/// Tests using this need `flavor = "multi_thread"`. On the default current-thread runtime the
/// parked handler starves the test's own body, and whether the test works at all comes down to
/// whether the prompt happens to arrive before the body's next await point.
struct ParksUntilReleased(Arc<AtomicBool>);

impl ParksUntilReleased {
    /// Caps the park regardless, so a test that fails before releasing still terminates.
    const MAX_PARK: Duration = Duration::from_secs(60);
}

impl TunnelEventHandler for ParksUntilReleased {
    fn on_auth_required(&mut self, _: &AuthRequest) -> anyhow::Result<String> {
        let deadline = Instant::now() + Self::MAX_PARK;
        while !self.0.load(Ordering::SeqCst) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        anyhow::bail!("this handler never answers")
    }
}

// --- Profile construction ---------------------------------------------------

fn profile_for(target: &LiveTarget, name: &str, user: &str, local_port: u16) -> Profile {
    Profile::new(
        name.to_string(),
        ConnectionConfig {
            host: target.host().to_string(),
            port: target.port(),
            user: user.to_string(),
            auth_type: AuthType::Password,
            key_path: None,
            password_storage: PasswordStorage::None,
        },
        ForwardingConfig {
            forwarding_type: ForwardingType::Local,
            bind_address: "127.0.0.1".to_string(),
            local_port: Some(local_port),
            remote_host: Some(target.forward_host().to_string()),
            remote_port: Some(target.forward_port()),
        },
    )
}

/// A free local port for the forward to bind.
fn free_local_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Should bind ephemeral port");
    listener.local_addr().expect("Should read addr").port()
}

/// Run a tunnel start through the shared SSE-first flow.
async fn start(
    daemon: &DaemonHarness,
    profile: &Profile,
    handler: &mut ScriptedAuth,
) -> anyhow::Result<()> {
    let client = daemon.http_client();
    let config = daemon.client_config();

    tokio::time::timeout(
        CONNECT_TIMEOUT,
        start_tunnel_with_events(&client, &config, profile.metadata.id, profile, handler),
    )
    .await
    .map_err(|_| anyhow::anyhow!("tunnel start timed out after {CONNECT_TIMEOUT:?}"))?
}

/// Read the SSH banner back through the forwarded local port.
///
/// Proves the forward carries bytes, using the target's own sshd so nothing
/// extra has to be deployed there.
async fn read_banner_through_tunnel(local_port: u16) -> anyhow::Result<String> {
    use tokio::io::AsyncReadExt;

    let mut stream = tokio::time::timeout(
        Duration::from_secs(15),
        tokio::net::TcpStream::connect(("127.0.0.1", local_port)),
    )
    .await
    .map_err(|_| anyhow::anyhow!("connecting to the forwarded port timed out"))??;

    let mut buffer = [0u8; 128];
    let read = tokio::time::timeout(Duration::from_secs(15), stream.read(&mut buffer))
        .await
        .map_err(|_| anyhow::anyhow!("reading from the forwarded port timed out"))??;

    Ok(String::from_utf8_lossy(&buffer[..read]).to_string())
}

// --- Host key verification --------------------------------------------------

#[tokio::test]
#[ignore = "needs a live SSH server; run with --ignored"]
async fn first_connection_prompts_for_host_key_and_remembers_it() {
    let (target, values) =
        live_target_or_skip!(&["SSH_TUNNEL_TEST_USER_KEY", "SSH_TUNNEL_TEST_KEY_PATH"]);
    let (user, key_path) = (values[0], values[1]);

    let daemon = DaemonHarness::start_unix().await;
    // Deliberately no trust_host_key(): known_hosts starts empty.

    let port = free_local_port();
    let mut profile = profile_for(target, "hostkey", user, port);
    profile.connection.auth_type = AuthType::Key;
    profile.connection.key_path = Some(key_path.into());
    daemon.install_profile(&profile);

    let mut handler = ScriptedAuth::new();
    let seen = handler.seen_handle();

    start(&daemon, &profile, &mut handler)
        .await
        .unwrap_or_else(|e| panic!("tunnel should start: {e}\n{}", daemon.log()));

    assert!(
        seen.lock()
            .unwrap()
            .contains(&AuthRequestType::HostKeyVerification),
        "an unknown host must raise host key verification, saw {:?}",
        seen.lock().unwrap()
    );

    let known_hosts = daemon.config_dir().join("known_hosts");
    assert!(
        known_hosts.exists(),
        "the accepted host key should be persisted\n{}",
        daemon.log()
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&known_hosts)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "known_hosts must not be world readable");
    }
}

#[tokio::test]
#[ignore = "needs a live SSH server; run with --ignored"]
async fn a_changed_host_key_is_refused() {
    let (target, values) =
        live_target_or_skip!(&["SSH_TUNNEL_TEST_USER_KEY", "SSH_TUNNEL_TEST_KEY_PATH"]);
    let (user, key_path) = (values[0], values[1]);

    let daemon = DaemonHarness::start_unix().await;

    // A syntactically valid entry for this host carrying the wrong key.
    // The pattern must be built the way the daemon builds it, or on the default
    // port the entry matches nothing and this test silently checks nothing.
    let bogus_key = "AAAAC3NzaC1lZDI1NTE5AAAAIP1Ck7Ie5xJm0000000000000000000000000";
    daemon.trust_host_key(&format!(
        "{} ssh-ed25519 {bogus_key}\n",
        harness::known_hosts_pattern(target.host(), target.port())
    ));

    let port = free_local_port();
    let mut profile = profile_for(target, "hostkey-changed", user, port);
    profile.connection.auth_type = AuthType::Key;
    profile.connection.key_path = Some(key_path.into());
    daemon.install_profile(&profile);

    let mut handler = ScriptedAuth::new();
    let result = start(&daemon, &profile, &mut handler).await;

    assert!(
        result.is_err(),
        "a mismatched host key must abort the connection\n{}",
        daemon.log()
    );
}

// --- Key authentication -----------------------------------------------------

#[tokio::test]
#[ignore = "needs a live SSH server; run with --ignored"]
async fn key_authentication_connects_and_forwards_traffic() {
    let (target, values) =
        live_target_or_skip!(&["SSH_TUNNEL_TEST_USER_KEY", "SSH_TUNNEL_TEST_KEY_PATH"]);
    let (user, key_path) = (values[0], values[1]);

    let daemon = DaemonHarness::start_unix().await;

    let port = free_local_port();
    let mut profile = profile_for(target, "key-auth", user, port);
    profile.connection.auth_type = AuthType::Key;
    profile.connection.key_path = Some(key_path.into());
    daemon.install_profile(&profile);

    let mut handler = ScriptedAuth::new();
    start(&daemon, &profile, &mut handler)
        .await
        .unwrap_or_else(|e| panic!("key auth should connect: {e}\n{}", daemon.log()));

    let banner = read_banner_through_tunnel(port)
        .await
        .unwrap_or_else(|e| panic!("forward should carry traffic: {e}\n{}", daemon.log()));

    assert!(
        banner.starts_with("SSH-2.0-"),
        "expected an SSH banner through the tunnel, got {banner:?}"
    );
}

#[tokio::test]
#[ignore = "needs a live SSH server; run with --ignored"]
async fn an_encrypted_key_prompts_for_its_passphrase() {
    let (target, values) = live_target_or_skip!(&[
        "SSH_TUNNEL_TEST_USER_KEY",
        "SSH_TUNNEL_TEST_ENCRYPTED_KEY_PATH",
        "SSH_TUNNEL_TEST_KEY_PASSPHRASE"
    ]);
    let (user, key_path, passphrase) = (values[0], values[1], values[2]);

    let daemon = DaemonHarness::start_unix().await;

    let port = free_local_port();
    let mut profile = profile_for(target, "key-encrypted", user, port);
    profile.connection.auth_type = AuthType::Key;
    profile.connection.key_path = Some(key_path.into());
    daemon.install_profile(&profile);

    let mut handler = ScriptedAuth::new().with_passphrases(&[passphrase]);
    let seen = handler.seen_handle();

    start(&daemon, &profile, &mut handler)
        .await
        .unwrap_or_else(|e| panic!("encrypted key should connect: {e}\n{}", daemon.log()));

    assert!(
        seen.lock()
            .unwrap()
            .contains(&AuthRequestType::KeyPassphrase),
        "an encrypted key must prompt for its passphrase, saw {:?}",
        seen.lock().unwrap()
    );
}

// --- Password authentication ------------------------------------------------

#[tokio::test]
#[ignore = "needs a live SSH server; run with --ignored"]
async fn password_authentication_connects() {
    let (target, values) =
        live_target_or_skip!(&["SSH_TUNNEL_TEST_USER_PASSWORD", "SSH_TUNNEL_TEST_PASSWORD"]);
    let (user, password) = (values[0], values[1]);

    let daemon = DaemonHarness::start_unix().await;

    let port = free_local_port();
    let profile = profile_for(target, "password-auth", user, port);
    daemon.install_profile(&profile);

    let mut handler = ScriptedAuth::new().with_passwords(&[password]);
    start(&daemon, &profile, &mut handler)
        .await
        .unwrap_or_else(|e| panic!("password auth should connect: {e}\n{}", daemon.log()));

    let banner = read_banner_through_tunnel(port)
        .await
        .unwrap_or_else(|e| panic!("forward should carry traffic: {e}\n{}", daemon.log()));
    assert!(banner.starts_with("SSH-2.0-"), "got {banner:?}");
}

#[tokio::test]
#[ignore = "needs a live SSH server; run with --ignored"]
async fn a_wrong_password_is_re_prompted_not_fatal() {
    let (target, values) =
        live_target_or_skip!(&["SSH_TUNNEL_TEST_USER_PASSWORD", "SSH_TUNNEL_TEST_PASSWORD"]);
    let (user, password) = (values[0], values[1]);

    let daemon = DaemonHarness::start_unix().await;

    let port = free_local_port();
    let profile = profile_for(target, "password-retry", user, port);
    daemon.install_profile(&profile);

    let mut handler =
        ScriptedAuth::new().with_passwords(&["definitely-not-the-password", password]);
    let seen = handler.seen_handle();

    start(&daemon, &profile, &mut handler)
        .await
        .unwrap_or_else(|e| panic!("should connect after a retry: {e}\n{}", daemon.log()));

    let password_prompts = seen
        .lock()
        .unwrap()
        .iter()
        .filter(|t| **t == AuthRequestType::Password)
        .count();
    assert!(
        password_prompts >= 2,
        "a wrong password should be re-prompted, saw {password_prompts} password prompt(s)"
    );
}

// --- Two-factor authentication ----------------------------------------------

/// The v0.1.10 fix: a failed keyboard-interactive attempt must re-prompt while
/// the server still offers the method, rather than failing the tunnel outright.
/// Until now nothing guarded this behaviour.
#[tokio::test]
#[ignore = "needs a live SSH server; run with --ignored"]
async fn a_wrong_2fa_code_is_re_prompted_not_fatal() {
    let (target, values) =
        live_target_or_skip!(&["SSH_TUNNEL_TEST_USER_2FA", "SSH_TUNNEL_TEST_TOTP_SECRET"]);
    let (user, totp_secret) = (values[0], values[1]);

    let daemon = DaemonHarness::start_unix().await;

    let port = free_local_port();
    let mut profile = profile_for(target, "2fa-retry", user, port);
    profile.connection.auth_type = AuthType::PasswordWith2FA;
    daemon.install_profile(&profile);

    // Avoid straddling a step boundary, which would make the "correct" code stale.
    if live::totp_seconds_remaining() < 10 {
        tokio::time::sleep(Duration::from_secs(live::totp_seconds_remaining() + 1)).await;
    }

    let codes = vec!["000000".to_string(), live::totp_now(totp_secret)];
    let password = target.get_var("SSH_TUNNEL_TEST_2FA_PASSWORD").unwrap_or("");

    let mut handler = ScriptedAuth::new()
        .with_passwords(&[password])
        .with_codes(&codes);
    let seen = handler.seen_handle();

    start(&daemon, &profile, &mut handler)
        .await
        .unwrap_or_else(|e| panic!("should connect after a wrong code: {e}\n{}", daemon.log()));

    let code_prompts = seen
        .lock()
        .unwrap()
        .iter()
        .filter(|t| {
            matches!(
                t,
                AuthRequestType::TwoFactorCode | AuthRequestType::KeyboardInteractive
            )
        })
        .count();
    assert!(
        code_prompts >= 2,
        "a wrong 2FA code should be re-prompted while the server still offers the \
         method, saw {code_prompts} prompt(s)"
    );
}

#[tokio::test]
#[ignore = "needs a live SSH server; run with --ignored"]
async fn two_factor_authentication_connects_with_a_valid_code() {
    let (target, values) =
        live_target_or_skip!(&["SSH_TUNNEL_TEST_USER_2FA", "SSH_TUNNEL_TEST_TOTP_SECRET"]);
    let (user, totp_secret) = (values[0], values[1]);

    let daemon = DaemonHarness::start_unix().await;

    let port = free_local_port();
    let mut profile = profile_for(target, "2fa", user, port);
    profile.connection.auth_type = AuthType::PasswordWith2FA;
    daemon.install_profile(&profile);

    if live::totp_seconds_remaining() < 10 {
        tokio::time::sleep(Duration::from_secs(live::totp_seconds_remaining() + 1)).await;
    }

    let password = target.get_var("SSH_TUNNEL_TEST_2FA_PASSWORD").unwrap_or("");
    let mut handler = ScriptedAuth::new()
        .with_passwords(&[password])
        .with_codes(&[live::totp_now(totp_secret)]);

    start(&daemon, &profile, &mut handler)
        .await
        .unwrap_or_else(|e| panic!("2FA should connect: {e}\n{}", daemon.log()));

    let banner = read_banner_through_tunnel(port)
        .await
        .unwrap_or_else(|e| panic!("forward should carry traffic: {e}\n{}", daemon.log()));
    assert!(banner.starts_with("SSH-2.0-"), "got {banner:?}");
}

// --- Teardown ---------------------------------------------------------------

#[tokio::test]
#[ignore = "needs a live SSH server; run with --ignored"]
async fn stopping_a_connected_tunnel_returns_promptly() {
    let (target, values) =
        live_target_or_skip!(&["SSH_TUNNEL_TEST_USER_KEY", "SSH_TUNNEL_TEST_KEY_PATH"]);
    let (user, key_path) = (values[0], values[1]);

    let daemon = DaemonHarness::start_unix().await;

    let port = free_local_port();
    let mut profile = profile_for(target, "stop-connected", user, port);
    profile.connection.auth_type = AuthType::Key;
    profile.connection.key_path = Some(key_path.into());
    daemon.install_profile(&profile);

    let mut handler = ScriptedAuth::new();
    start(&daemon, &profile, &mut handler)
        .await
        .unwrap_or_else(|e| panic!("tunnel should start: {e}\n{}", daemon.log()));

    let client = daemon.http_client();
    let config = daemon.client_config();

    let started = std::time::Instant::now();
    tokio::time::timeout(
        Duration::from_secs(10),
        ssh_tunnel_common::stop_tunnel(&client, &config, profile.metadata.id),
    )
    .await
    .unwrap_or_else(|_| panic!("stop should not hang\n{}", daemon.log()))
    .unwrap_or_else(|e| panic!("stop should succeed: {e}\n{}", daemon.log()));

    assert!(
        started.elapsed() < Duration::from_secs(10),
        "stopping a connected tunnel took {:?}",
        started.elapsed()
    );
}

/// Regression guard for the known 60-second hang: cancelling while the daemon
/// is waiting for an auth response must not block on that timeout.
/// Multi-threaded: see [`ParksUntilReleased`].
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs a live SSH server; run with --ignored"]
async fn stopping_during_authentication_returns_promptly() {
    let (target, values) =
        live_target_or_skip!(&["SSH_TUNNEL_TEST_USER_KEY", "SSH_TUNNEL_TEST_KEY_PATH"]);
    let (user, key_path) = (values[0], values[1]);

    let daemon = DaemonHarness::start_unix().await;

    let port = free_local_port();
    let mut profile = profile_for(target, "stop-during-auth", user, port);
    profile.connection.auth_type = AuthType::Key;
    profile.connection.key_path = Some(key_path.into());
    daemon.install_profile(&profile);
    let tunnel_id = profile.metadata.id;

    let client = daemon.http_client();
    let config = daemon.client_config();

    // Start a tunnel whose auth prompt is never answered, leaving the daemon waiting on input.
    let release = Arc::new(AtomicBool::new(false));
    let start_handle = {
        let (client, config, profile) = (client.clone(), config.clone(), profile.clone());
        let release = release.clone();
        tokio::spawn(async move {
            let _ = start_tunnel_with_events(
                &client,
                &config,
                profile.metadata.id,
                &profile,
                &mut ParksUntilReleased(release),
            )
            .await;
        })
    };

    // Give the daemon time to reach the auth prompt.
    tokio::time::sleep(Duration::from_secs(5)).await;

    let started = std::time::Instant::now();
    let _ = tokio::time::timeout(
        Duration::from_secs(20),
        ssh_tunnel_common::stop_tunnel(&client, &config, tunnel_id),
    )
    .await;
    let elapsed = started.elapsed();

    // `abort()` cannot interrupt a blocking synchronous call, so release the park first;
    // otherwise the parked worker outlives the test and the runtime's drop waits for it.
    release.store(true, Ordering::SeqCst);
    start_handle.abort();

    assert!(
        elapsed < Duration::from_secs(15),
        "cancelling during the auth phase took {elapsed:?}; the daemon is waiting out \
         its auth timeout instead of honouring the shutdown signal\n{}",
        daemon.log()
    );
}

/// A client that connects *after* a prompt was raised must still be told about it.
///
/// Events are broadcast only to whoever is listening at the time. Before the daemon replayed
/// outstanding prompts to new subscribers, a prompt raised while no client was attached — or
/// during a reconnect — was lost for good, and the tunnel failed with
/// "Authentication prompt timed out after 60s" having shown the user nothing.
/// Multi-threaded: see [`ParksUntilReleased`].
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs a live SSH server; run with --ignored"]
async fn a_late_subscriber_is_told_about_an_outstanding_prompt() {
    use futures_util::StreamExt;

    let (target, values) =
        live_target_or_skip!(&["SSH_TUNNEL_TEST_USER_KEY", "SSH_TUNNEL_TEST_KEY_PATH"]);
    let (user, key_path) = (values[0], values[1]);

    let daemon = DaemonHarness::start_unix().await;

    let port = free_local_port();
    let mut profile = profile_for(target, "late-subscriber", user, port);
    profile.connection.auth_type = AuthType::Key;
    profile.connection.key_path = Some(key_path.into());
    // No known_hosts is seeded, so the first connection raises a host key prompt.
    daemon.install_profile(&profile);

    let client = daemon.http_client();
    let config = daemon.client_config();

    // Start a tunnel and sit on the prompt, leaving it outstanding for the second subscriber.
    //
    let release = Arc::new(AtomicBool::new(false));
    let start_handle = {
        let (client, config, profile) = (client.clone(), config.clone(), profile.clone());
        let release = release.clone();
        tokio::spawn(async move {
            let _ = start_tunnel_with_events(
                &client,
                &config,
                profile.metadata.id,
                &profile,
                &mut ParksUntilReleased(release),
            )
            .await;
        })
    };

    // Let the daemon reach the prompt.
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Now connect a *second*, entirely fresh subscriber — as a GUI reconnecting would.
    let url = format!(
        "{}/api/events",
        config.daemon_base_url().expect("daemon base url")
    );
    let streaming = ssh_tunnel_common::create_streaming_client(&config).expect("streaming client");
    let response = ssh_tunnel_common::add_auth_header(streaming.get(&url), &config)
        .expect("auth header")
        .send()
        .await
        .expect("second subscriber should connect");

    let mut stream = response.bytes_stream();
    let saw_prompt = tokio::time::timeout(Duration::from_secs(10), async {
        let mut buffer = String::new();
        while let Some(Ok(chunk)) = stream.next().await {
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            if buffer.contains("auth_required") || buffer.contains("AuthRequired") {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);

    // Release the parked handler and let the starting task unwind before asserting, so a
    // failure does not also leave a blocked worker thread behind for the runtime to wait on.
    release.store(true, Ordering::SeqCst);
    let _ = tokio::time::timeout(
        Duration::from_secs(10),
        ssh_tunnel_common::stop_tunnel(&client, &config, profile.metadata.id),
    )
    .await;
    let _ = tokio::time::timeout(Duration::from_secs(15), start_handle).await;

    assert!(
        saw_prompt,
        "a subscriber connecting while a prompt is outstanding must be told about it, \
         otherwise the prompt expires unseen\n{}",
        daemon.log()
    );
}
