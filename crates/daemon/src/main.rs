// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// SSH Tunnel Manager - Daemon
// Core service for managing SSH tunnels

mod api;
mod auth;
mod config;
mod known_hosts;
mod monitor;
mod permissions;
mod pidfile;
mod security;
mod tls;
mod tunnel;

use std::sync::Arc;

use anyhow::{Context, Result};
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use hyper_util::rt::TokioIo;
use tokio::net::UnixListener;
use tower::Service;
use tracing::{debug, error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use api::{create_router, AppState};
use config::{DaemonConfig, ListenerMode};
use tunnel::TunnelManager;

#[tokio::main]
async fn main() -> Result<()> {
    // Set restrictive umask before creating any files
    permissions::set_restrictive_umask();

    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ssh_tunnel_daemon=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("SSH Tunnel Manager Daemon starting...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    info!(
        "Build: {} ({})",
        option_env!("BUILD_DATE").unwrap_or("unknown"),
        option_env!("GIT_HASH").unwrap_or("unknown")
    );

    // Create PID file to prevent multiple instances
    let _pid_guard = pidfile::PidFileGuard::create()
        .context("Failed to create PID file - another daemon may already be running")?;

    // Load daemon configuration
    let daemon_config = DaemonConfig::load()?;
    info!("Listener mode: {:?}", daemon_config.listener_mode);
    info!("Authentication required: {}", daemon_config.require_auth);

    // Load or generate authentication token if required
    let (auth_token, token_was_generated) = if daemon_config.require_auth {
        let (token, was_new) = auth::load_or_generate_token(&daemon_config.auth_token_path)?;
        (Some(token), was_new)
    } else {
        info!("Authentication disabled - API endpoints are publicly accessible");
        (None, false)
    };

    // For HTTPS mode, ensure certificate is valid and get fingerprint
    // This must happen BEFORE writing the CLI config snippet
    // We use create_tls_config() which handles both generation and expiry checking,
    // ensuring we get the fingerprint of the actual cert that will be used
    let tls_fingerprint = if daemon_config.listener_mode == ListenerMode::TcpHttps {
        // Create TLS config - this handles generation, expiry checking, and auto-regeneration
        let _ = tls::create_tls_config(&daemon_config.tls_cert_path, &daemon_config.tls_key_path)?;
        // Now we can safely get the fingerprint of the current, valid certificate
        Some(tls::get_cert_fingerprint(&daemon_config.tls_cert_path)?)
    } else {
        None
    };

    // Write CLI config snippet if token was newly generated OR in HTTPS mode
    // (HTTPS always writes snippet because cert may have been regenerated)
    if token_was_generated || daemon_config.listener_mode == ListenerMode::TcpHttps {
        config::write_cli_config_snippet(
            &daemon_config.listener_mode,
            &daemon_config.bind_host,
            daemon_config.bind_port,
            auth_token.as_deref(),
            tls_fingerprint.as_deref(),
        )?;
    }

    // Create the tunnel manager
    let tunnel_manager = TunnelManager::new(daemon_config.known_hosts_path.clone());

    // Subscribe to tunnel events for logging
    let mut event_rx = tunnel_manager.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            info!("Tunnel event: {:?}", event);
        }
    });

    // Create shutdown broadcast channel for graceful SSE stream termination
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);

    // Create shared state
    let state = Arc::new(AppState {
        tunnel_manager,
        shutdown_tx: shutdown_tx.clone(),
    });
    let shutdown_manager = state.tunnel_manager.clone();

    // Create API router with optional authentication
    let app = if let Some(token) = auth_token {
        let auth_state = auth::AuthState::new(token);
        create_router(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth::auth_middleware,
            ))
    } else {
        create_router(state)
    };

    // Start listener based on configured mode
    match daemon_config.listener_mode {
        ListenerMode::UnixSocket => {
            serve_unix_socket(app, &daemon_config, shutdown_manager.clone(), shutdown_tx).await?;
        }
        ListenerMode::TcpHttp => {
            let bind_address = ssh_tunnel_common::format_host_port(&daemon_config.bind_host, daemon_config.bind_port);
            serve_tcp_http(app, &bind_address, shutdown_manager.clone(), shutdown_tx)
                .await?;
        }
        ListenerMode::TcpHttps => {
            let bind_address = ssh_tunnel_common::format_host_port(&daemon_config.bind_host, daemon_config.bind_port);
            serve_tcp_https(
                app,
                &bind_address,
                &daemon_config.tls_cert_path,
                &daemon_config.tls_key_path,
                shutdown_manager.clone(),
                shutdown_tx,
            )
            .await?;
        }
    }

    info!("Daemon shut down");
    Ok(())
}

/// Serve on Unix domain socket (local-only, no TLS)
async fn serve_unix_socket(
    app: axum::Router,
    daemon_config: &DaemonConfig,
    tunnel_manager: TunnelManager,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
) -> Result<()> {
    // Get socket path
    let socket_path = config::socket_path()?;

    // Remove existing socket file if it exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path).context("Failed to remove existing socket file")?;
    }

    // Create parent directory with appropriate permissions
    if let Some(parent) = socket_path.parent() {
        permissions::ensure_directory_with_permissions(parent, daemon_config.group_access)?;
    }

    // Bind to Unix socket
    let listener = UnixListener::bind(&socket_path).context(format!(
        "Failed to bind to socket: {}",
        socket_path.display()
    ))?;

    // Set socket permissions immediately after binding
    permissions::set_socket_permissions(&socket_path, daemon_config.group_access)?;

    info!("Daemon listening on Unix socket: {}", socket_path.display());
    info!("Daemon started successfully");

    // Set up shutdown signal
    let (shutdown_signal_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    let shutdown_broadcast = shutdown_tx.clone();
    tokio::spawn(async move {
        wait_for_shutdown(tunnel_manager).await;
        // Signal SSE streams to close
        let _ = shutdown_broadcast.send(());
        // Signal server to stop accepting connections
        let _ = shutdown_signal_tx.send(()).await;
    });

    // Accept connections
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Shutting down server...");
                break;
            }

            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, _addr)) => {
                        let app = app.clone();

                        tokio::spawn(async move {
                            let stream = TokioIo::new(stream);

                            let hyper_service = hyper::service::service_fn(move |request: hyper::Request<hyper::body::Incoming>| {
                                let mut app = app.clone();
                                async move {
                                    app.call(request).await
                                }
                            });

                            if let Err(err) = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                                .serve_connection_with_upgrades(stream, hyper_service)
                                .await
                            {
                                // Client disconnects (e.g., Ctrl+C on watch command) are normal
                                let err_msg = err.to_string();
                                if err_msg.contains("connection closed") || err_msg.contains("Broken pipe") {
                                    debug!("Client disconnected: {}", err);
                                } else {
                                    error!("Error serving connection: {}", err);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                    }
                }
            }
        }
    }

    // Cleanup socket
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    Ok(())
}

/// Serve on TCP with HTTP (localhost-only, no TLS)
async fn serve_tcp_http(
    app: axum::Router,
    bind_address: &str,
    tunnel_manager: TunnelManager,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
) -> Result<()> {
    info!("Daemon listening on TCP (HTTP): {}", bind_address);
    info!("⚠️  WARNING: HTTP mode has no encryption - use only on localhost!");
    info!("Daemon started successfully");

    let listener = tokio::net::TcpListener::bind(bind_address)
        .await
        .context(format!("Failed to bind to {}", bind_address))?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(tunnel_manager, shutdown_tx))
        .await
        .context("TCP HTTP server error")?;

    Ok(())
}

/// Serve on TCP with HTTPS/TLS (network-ready, secure)
async fn serve_tcp_https(
    app: axum::Router,
    bind_address: &str,
    cert_path: &std::path::Path,
    key_path: &std::path::Path,
    tunnel_manager: TunnelManager,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
) -> Result<()> {
    // Create or load TLS configuration
    let rustls_config = tls::create_tls_config(cert_path, key_path)?;
    let tls_config = RustlsConfig::from_config(rustls_config);

    info!("Daemon listening on TCP (HTTPS): {}", bind_address);
    info!("TLS enabled - secure for network access");
    info!("Daemon started successfully");

    // Bind to TCP address using std::net::TcpListener (required by axum_server)
    let std_listener = std::net::TcpListener::bind(bind_address)
        .context(format!("Failed to bind to {}", bind_address))?;

    // Set to non-blocking mode for tokio
    std_listener
        .set_nonblocking(true)
        .context("Failed to set listener to non-blocking")?;

    let handle = Handle::new();
    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        shutdown_signal(tunnel_manager, shutdown_tx).await;
        // Connections should close immediately when SSE streams end
        shutdown_handle.graceful_shutdown(None);
    });

    axum_server::from_tcp_rustls(std_listener, tls_config)
        .handle(handle)
        .serve(app.into_make_service())
        .await
        .context("TCP HTTPS server error")?;

    Ok(())
}

/// Graceful shutdown signal handler
async fn shutdown_signal(
    tunnel_manager: TunnelManager,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
) {
    wait_for_shutdown(tunnel_manager).await;
    // Signal all SSE streams to close
    let _ = shutdown_tx.send(());
}

/// Wait for Ctrl+C or SIGTERM, then stop all tunnels and notify receivers (used by Unix socket server)
async fn wait_for_shutdown(tunnel_manager: TunnelManager) {
    #[cfg(unix)]
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("Failed to install SIGTERM handler");

    #[cfg(unix)]
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down");
        }
        _ = sigterm.recv() => {
            info!("Received SIGTERM, shutting down");
        }
    };

    #[cfg(not(unix))]
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down");
        }
    };

    tunnel_manager.stop_all().await;
    info!("All tunnels stopped");
}
