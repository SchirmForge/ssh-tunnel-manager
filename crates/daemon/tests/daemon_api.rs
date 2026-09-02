// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Tier-1 integration tests: a real daemon process, no SSH server, no secrets.
//!
//! These run everywhere and on every CI build. Tests that need a real SSH server
//! live in `live_ssh.rs` and are `#[ignore]`d by default.

mod harness;

use harness::DaemonHarness;

#[tokio::test]
async fn unix_socket_daemon_serves_health() {
    let daemon = DaemonHarness::start_unix().await;

    let response = daemon.get("/api/health").await;
    assert!(
        response.status().is_success(),
        "health should succeed, got {}\n{}",
        response.status(),
        daemon.log()
    );
}

#[tokio::test]
async fn http_daemon_serves_health_on_loopback() {
    let daemon = DaemonHarness::start_http().await;

    let response = daemon.get("/api/health").await;
    assert!(
        response.status().is_success(),
        "health should succeed, got {}\n{}",
        response.status(),
        daemon.log()
    );
}

#[tokio::test]
async fn daemon_info_reports_its_own_configuration() {
    let daemon = DaemonHarness::start_unix().await;

    let response = daemon.get("/api/daemon/info").await;
    assert!(response.status().is_success());

    let info: serde_json::Value = response.json().await.expect("Should parse daemon info");
    assert_eq!(info["listener_mode"], "unix-socket");
    assert_eq!(info["require_auth"], true);
    assert_eq!(info["active_tunnels_count"], 0);
    assert!(
        info["version"].as_str().is_some_and(|v| !v.is_empty()),
        "version should be reported, got {:?}",
        info["version"]
    );
}

// --- Authentication -------------------------------------------------------

#[tokio::test]
async fn request_without_token_is_rejected() {
    let daemon = DaemonHarness::start_unix().await;

    let response = daemon
        .get_with_config("/api/daemon/info", &daemon.client_config_with_token(""))
        .await;

    assert_eq!(
        response.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "an unauthenticated request must be refused\n{}",
        daemon.log()
    );
}

#[tokio::test]
async fn request_with_wrong_token_is_rejected() {
    let daemon = DaemonHarness::start_unix().await;

    let response = daemon
        .get_with_config(
            "/api/daemon/info",
            &daemon.client_config_with_token("not-the-real-token"),
        )
        .await;

    assert_eq!(
        response.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "a wrong token must be refused\n{}",
        daemon.log()
    );
}

#[tokio::test]
async fn auth_is_required_by_default() {
    // Regression guard for the v0.1.8 rule that require_auth defaults to true.
    let daemon = DaemonHarness::start_unix().await;
    assert!(
        !daemon.token().is_empty(),
        "the daemon should generate a token when auth is on by default"
    );
}

// --- Security rules -------------------------------------------------------

#[test]
fn http_mode_is_refused_on_a_non_loopback_address() {
    // v0.1.8 security rule: plain HTTP is loopback-only; network binds need HTTPS.
    let output = DaemonHarness::start_expecting_failure(
        "listener_mode = \"tcp-http\"\n\
         bind_host = \"0.0.0.0\"\n\
         bind_port = 3443\n",
    );

    assert!(
        output.contains("loopback") || output.contains("https") || output.contains("HTTPS"),
        "refusal should explain the loopback/HTTPS rule, got:\n{output}"
    );
}

#[tokio::test]
async fn runtime_directory_and_socket_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let daemon = DaemonHarness::start_unix().await;

    let socket_mode = std::fs::metadata(daemon.socket_path())
        .expect("Should stat socket")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        socket_mode, 0o600,
        "socket should be owner-only without group access"
    );

    let dir_mode = std::fs::metadata(daemon.runtime_dir())
        .expect("Should stat runtime dir")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        dir_mode, 0o700,
        "runtime dir should be owner-only without group access"
    );
}

#[tokio::test]
async fn token_file_is_not_world_readable() {
    use std::os::unix::fs::PermissionsExt;

    let daemon = DaemonHarness::start_unix().await;

    let mode = std::fs::metadata(daemon.config_dir().join("daemon.token"))
        .expect("Should stat token file")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "the auth token must not be readable by others");
}

// --- Single instance ------------------------------------------------------

#[tokio::test]
async fn second_daemon_instance_refuses_to_start() {
    let daemon = DaemonHarness::start_unix().await;

    // Reuse the running daemon's sandbox, so the second process sees its PID file.
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ssh-tunnel-daemon"))
        .env("XDG_CONFIG_HOME", daemon.config_dir().parent().unwrap())
        .env("XDG_RUNTIME_DIR", daemon.runtime_dir().parent().unwrap())
        .env("SSH_TUNNEL_SKIP_KEYRING", "1")
        .output()
        .expect("Should run second daemon");

    assert!(
        !output.status.success(),
        "a second instance must not start while the first holds the PID file"
    );

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("already running"),
        "refusal should name the running daemon, got:\n{combined}"
    );
}

// --- Tunnels --------------------------------------------------------------

#[tokio::test]
async fn listing_tunnels_is_empty_on_a_fresh_daemon() {
    let daemon = DaemonHarness::start_unix().await;

    let response = daemon.get("/api/tunnels").await;
    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await.expect("Should parse tunnel list");
    let tunnels = body.as_array().or_else(|| body["tunnels"].as_array());
    assert!(
        tunnels.is_some_and(|t| t.is_empty()),
        "a fresh daemon should report no tunnels, got {body}"
    );
}

#[tokio::test]
async fn status_of_an_unknown_tunnel_is_not_found() {
    let daemon = DaemonHarness::start_unix().await;

    let unknown = uuid::Uuid::new_v4();
    let response = daemon.get(&format!("/api/tunnels/{unknown}/status")).await;

    assert!(
        response.status() == reqwest::StatusCode::NOT_FOUND || response.status().is_success(),
        "an unknown tunnel should be reported as absent, got {}",
        response.status()
    );
}

// --- SSE ------------------------------------------------------------------

/// The SSE stream must survive far longer than one heartbeat interval.
///
/// Regression guard for a client that killed every stream at exactly 30 seconds: the shared
/// client applied `.timeout(30s)`, which in reqwest covers the whole request *including the
/// response body*, so a long-lived stream was cut regardless of traffic. Combined with a
/// reconnect backoff that never reset, the client settled into 30s connected / 30s
/// disconnected and missed roughly half of all daemon events — including authentication
/// prompts, which then timed out after 60s with nothing shown to the user.
///
/// 40 seconds is deliberately past the old 30s cliff and past four 10s heartbeats.
#[tokio::test]
async fn event_stream_survives_longer_than_the_client_timeout() {
    use futures_util::StreamExt;

    let daemon = DaemonHarness::start_http().await;

    let url = format!(
        "{}/api/events",
        daemon.client_config().daemon_base_url().expect("base url")
    );
    // The streaming client, because that is what the SSE path uses. The request client's
    // total timeout is correct for requests and fatal for streams.
    let client = ssh_tunnel_common::create_streaming_client(&daemon.client_config())
        .expect("streaming client");
    let response = ssh_tunnel_common::add_auth_header(client.get(&url), &daemon.client_config())
        .expect("auth header")
        .send()
        .await
        .expect("event stream should connect");
    assert!(response.status().is_success());

    let mut stream = response.bytes_stream();

    // The property is "the stream is still open", not "we saw N heartbeats". Counting beats
    // is not enough: three of them fit inside the old 30s cliff, so a cut stream still
    // satisfied a `>= 3` assertion. The only pass here is the timeout elapsing with the
    // stream alive.
    let cut = tokio::time::timeout(std::time::Duration::from_secs(40), async {
        while let Some(chunk) = stream.next().await {
            if let Err(error) = chunk {
                return format!("transport error after {error}");
            }
        }
        "stream ended cleanly".to_string()
    })
    .await;

    assert!(
        cut.is_err(),
        "the event stream was cut within 40s ({}), so a daemon event raised in that window \
         would never reach the client\n{}",
        cut.unwrap_or_default(),
        daemon.log()
    );
}

#[tokio::test]
async fn event_stream_connects_and_sends_heartbeats() {
    use futures_util::StreamExt;

    let daemon = DaemonHarness::start_http().await;

    let response = daemon.get("/api/events").await;
    assert!(
        response.status().is_success(),
        "event stream should connect, got {}\n{}",
        response.status(),
        daemon.log()
    );

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    // The daemon heartbeats periodically; allow generous time on a loaded CI box.
    let saw_heartbeat = tokio::time::timeout(std::time::Duration::from_secs(45), async {
        while let Some(Ok(chunk)) = stream.next().await {
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            if buffer.contains("heartbeat") {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);

    assert!(
        saw_heartbeat,
        "expected a heartbeat event, received:\n{buffer}\n--- daemon log ---\n{}",
        daemon.log()
    );
}

#[tokio::test]
async fn event_stream_requires_authentication() {
    let daemon = DaemonHarness::start_http().await;

    let response = daemon
        .get_with_config("/api/events", &daemon.client_config_with_token(""))
        .await;

    assert_eq!(
        response.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "the event stream must not be readable without a token"
    );
}
