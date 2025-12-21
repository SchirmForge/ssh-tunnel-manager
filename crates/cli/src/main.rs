// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// SSH Tunnel Manager - CLI Client
// Command-line interface for managing SSH tunnels

mod config;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table};
use dialoguer::{Confirm, Input, Password};
use reqwest::Client;
use std::fs;
use std::path::PathBuf;
//TODO: implement ctrl-c to cancel the connection (would require this include)
// use tokio::signal;
use futures::StreamExt;

use ssh_tunnel_common::{
    delete_profile_by_name, load_profile_by_name, profile_exists_by_name, save_profile,
    start_tunnel_with_events, stop_tunnel as stop_tunnel_shared, AuthRequest,
    AuthType, ConnectionConfig, DaemonTunnelEvent, ForwardingConfig, ForwardingType, Profile,
    TunnelEventHandler, TunnelOptions, Uuid,
};

#[derive(Parser)]
#[command(name = "ssh-tunnel")]
#[command(about = "SSH Tunnel Manager CLI", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new tunnel profile
    Add {
        /// Profile name
        name: String,

        /// Remote SSH host
        #[arg(short = 'H', long)]
        remote_host: Option<String>,

        /// Remote SSH port
        #[arg(short = 'P', long, default_value = "22")]
        remote_port: Option<u16>,

        /// SSH username
        #[arg(short = 'u', long)]
        user: Option<String>,

        /// Path to SSH private key
        #[arg(short = 'k', long)]
        key_path: Option<PathBuf>,

        /// Local bind address (default: 127.0.0.1)
        #[arg(short = 'b', long, default_value = "127.0.0.1")]
        bind_address: Option<String>,

        /// Local port to bind
        #[arg(short = 'l', long, default_value = "4443")]
        local_port: Option<u16>,

        /// Remote host to forward to (default: localhost on remote)
        #[arg(short = 'r', long, default_value = "localhost")]
        forward_host: Option<String>,

        /// Remote port to forward to
        #[arg(short = 'p', long, default_value = "443")]
        forward_port: Option<u16>,

        /// Skip interactive prompts (use provided args only)
        #[arg(short = 'y', long)]
        non_interactive: bool,

        /// Enable SSH compression
        #[arg(long)]
        compression: Option<bool>,

        /// Keepalive interval in seconds (0 to disable)
        #[arg(long)]
        keepalive_interval: Option<u64>,

        /// Enable auto-reconnect on failure
        #[arg(long)]
        auto_reconnect: Option<bool>,

        /// Maximum reconnect attempts (0 for unlimited)
        #[arg(long)]
        reconnect_attempts: Option<u32>,

        /// Delay between reconnect attempts in seconds
        #[arg(long)]
        reconnect_delay: Option<u64>,

        /// Enable TCP keepalive on forwarded connections
        #[arg(long)]
        tcp_keepalive: Option<bool>,

        /// Maximum SSH packet size in bytes
        #[arg(long)]
        max_packet_size: Option<u32>,

        /// SSH window size in bytes
        #[arg(long)]
        window_size: Option<u32>,
    },

    /// List all tunnel profiles
    List {
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,

        /// Output as JSON for scripting
        #[arg(short, long)]
        json: bool,
    },

    /// Delete a profile
    Delete {
        /// Profile name
        name: String,
    },

    /// Show detailed information about a profile
    Info {
        /// Profile name
        name: String,
    },

    /// Start a tunnel
    Start {
        /// Profile name
        name: String,
    },

    /// Stop a tunnel
    Stop {
        /// Profile name
        name: String,
    },

    /// Restart a tunnel
    Restart {
        /// Profile name
        name: String,
    },

    /// Show tunnel status
    Status {
        /// Profile name (optional, shows all if not specified)
        name: Option<String>,
    },

    /// Daemon management
    Daemon {
        #[command(subcommand)]
        action: DaemonCommands,
    },

    Watch {
        /// Optional profile name to filter by
        name: Option<String>,
    },
}

#[derive(Subcommand)]
enum DaemonCommands {
    /// Start the daemon
    Start,
    /// Stop the daemon
    Stop,
    /// Check daemon status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Add {
            name,
            remote_host,
            remote_port,
            user,
            key_path,
            bind_address,
            local_port,
            forward_host,
            forward_port,
            non_interactive,
            compression,
            keepalive_interval,
            auto_reconnect,
            reconnect_attempts,
            reconnect_delay,
            tcp_keepalive,
            max_packet_size,
            window_size,
        } => {
            add_profile(
                name,
                remote_host,
                remote_port,
                user,
                key_path,
                bind_address,
                local_port,
                forward_host,
                forward_port,
                non_interactive,
                compression,
                keepalive_interval,
                auto_reconnect,
                reconnect_attempts,
                reconnect_delay,
                tcp_keepalive,
                max_packet_size,
                window_size,
            )
            .await?;
        }
        Commands::List { verbose, json } => {
            list_profiles(verbose, json).await?;
        }
        Commands::Delete { name } => {
            delete_profile(name).await?;
        }
        Commands::Info { name } => {
            show_profile_info(name).await?;
        }
        Commands::Start { name } => {
            start_tunnel(name).await?;
        }
        Commands::Stop { name } => {
            stop_tunnel(name).await?;
        }
        Commands::Restart { name } => {
            println!("Restarting tunnel: {}", name);
            // TODO: Implement restart command
        }
        Commands::Status { name } => {
            match name {
                Some(n) => println!("Status for: {}", n),
                None => println!("Status for all tunnels"),
            }
            // TODO: Implement status command
        }
        Commands::Daemon { action } => {
            match action {
                DaemonCommands::Start => {
                    println!("Starting daemon...");
                    // TODO: Implement daemon start
                }
                DaemonCommands::Stop => {
                    println!("Stopping daemon...");
                    // TODO: Implement daemon stop
                }
                DaemonCommands::Status => {
                    println!("Checking daemon status...");
                    // TODO: Implement daemon status
                }
            }
        }
        Commands::Watch { name } => {
            watch_events(name).await?;
        }
    }

    Ok(())
}

/// CLI event handler for interactive authentication and status display
struct CliEventHandler {
    profile: Profile,
}

impl TunnelEventHandler for CliEventHandler {
    fn on_auth_required(&mut self, request: &AuthRequest) -> Result<String> {
        prompt_for_auth(request)
    }

    fn on_connected(&mut self) {
        announce_connected(&self.profile);
    }

    fn on_event(&mut self, event: &DaemonTunnelEvent) {
        match event {
            DaemonTunnelEvent::Starting { .. } => {
                println!("{}", "Start request accepted, connecting...".dimmed());
            }
            _ => {}
        }
    }
}

async fn start_tunnel(name: String) -> Result<()> {
    let profile = load_profile_by_name(&name)?;
    let tunnel_id = profile.metadata.id;

    println!(
        "{}",
        format!(
            "Starting tunnel '{}' ({})",
            profile.metadata.name, tunnel_id
        )
        .green()
        .bold()
    );

    let client = create_daemon_client()?;
    let cli_config = config::CliConfig::load()?;

    let mut handler = CliEventHandler { profile };

    // Use the shared SSE-first helper
    start_tunnel_with_events(&client, &cli_config.daemon_config, tunnel_id, &mut handler).await
}

/// Prompt user for authentication input
fn prompt_for_auth(auth_request: &AuthRequest) -> Result<String> {
    // Display the prompt exactly as received from the SSH server
    // The 'hidden' field (from SSH protocol's 'echo' field) determines input visibility

    if auth_request.hidden {
        // Hidden input (password, passphrase, 2FA code, etc.)
        let response = Password::new()
            .with_prompt(&auth_request.prompt)
            .interact()
            .context("Failed to read password input")?;
        Ok(response)
    } else {
        // Visible input
        let response: String = Input::new()
            .with_prompt(&auth_request.prompt)
            .interact_text()
            .context("Failed to read input")?;
        Ok(response)
    }
}

fn announce_connected(profile: &Profile) {
    println!();
    println!(
        "{}",
        format!(
            "✓ Tunnel connected! Forwarding {}:{} → {}:{}",
            profile.forwarding.bind_address,
            profile.forwarding.local_port.unwrap_or(0),
            profile.forwarding.remote_host.as_deref().unwrap_or("?"),
            profile.forwarding.remote_port.unwrap_or(0)
        )
        .green()
        .bold()
    );
    println!();
    println!(
        "{}",
        "Tunnel is running. Press Ctrl+C to stop (or use 'ssh-tunnel stop')".dimmed()
    );
}

async fn stop_tunnel(name: String) -> Result<()> {
    let profile = load_profile_by_name(&name)?;
    let tunnel_id = profile.metadata.id;

    println!(
        "{}",
        format!(
            "Stopping tunnel '{}' ({})",
            profile.metadata.name, tunnel_id
        )
        .yellow()
    );

    let client = create_daemon_client()?;
    let cli_config = config::CliConfig::load()?;

    // Use the shared stop helper
    stop_tunnel_shared(&client, &cli_config.daemon_config, tunnel_id).await?;

    println!("{}", "✓ Tunnel stopped".green().bold());
    Ok(())
}

async fn add_profile(
    name: String,
    remote_host: Option<String>,
    remote_port: Option<u16>,
    user: Option<String>,
    key_path: Option<PathBuf>,
    bind_address: Option<String>,
    local_port: Option<u16>,
    forward_host: Option<String>,
    forward_port: Option<u16>,
    non_interactive: bool,
    compression: Option<bool>,
    keepalive_interval: Option<u64>,
    auto_reconnect: Option<bool>,
    reconnect_attempts: Option<u32>,
    reconnect_delay: Option<u64>,
    tcp_keepalive: Option<bool>,
    max_packet_size: Option<u32>,
    window_size: Option<u32>,
) -> Result<()> {
    println!("{}", "Creating new SSH tunnel profile".bold().green());
    println!();

    // Check if profile with this name already exists
    if profile_exists_by_name(&name) {
        anyhow::bail!(
            "A profile with the name '{}' already exists. Please choose a different name or delete the existing profile first.",
            name.yellow()
        );
    }

    // Generate profile ID early so we can use it for keychain storage
    let profile_id = Uuid::new_v4();

    // Gather all required information
    let remote_host = if let Some(host) = remote_host {
        host
    } else if non_interactive {
        anyhow::bail!("Remote host is required in non-interactive mode");
    } else {
        Input::new()
            .with_prompt("Remote SSH host (hostname or IP)")
            .interact_text()?
    };

    let remote_port = if let Some(port) = remote_port {
        port
    } else if non_interactive {
        22
    } else {
        Input::<u16>::new()
            .with_prompt("Remote SSH port")
            .default(22)
            .interact_text()?
    };

    let user = if let Some(u) = user {
        u
    } else if non_interactive {
        anyhow::bail!("Username is required in non-interactive mode");
    } else {
        Input::new().with_prompt("SSH username").interact_text()?
    };

    // Authentication type selection
    let (auth_type, key_path, password_stored) = if let Some(path) = key_path {
        // Key path provided via CLI argument - use key auth
        validate_ssh_key(&path)?;

        // Check if the key is encrypted and needs a passphrase
        if is_key_encrypted(&path)? {
            println!("{}", "SSH key is encrypted and requires a passphrase.".yellow());

            let passphrase = Password::new()
                .with_prompt("SSH key passphrase")
                .interact()?;

            // Validate the passphrase
            if let Err(e) = validate_key_passphrase(&path, &passphrase) {
                anyhow::bail!("Invalid passphrase: {}", e);
            }

            // Always ask if they want to store it (even in non-interactive mode)
            // The passphrase prompt itself already broke non-interactivity
            let store_passphrase = Confirm::new()
                .with_prompt("Store passphrase in system keychain?")
                .default(!non_interactive)  // Default to yes in interactive mode, no in non-interactive
                .interact()?;

            if store_passphrase {
                store_password_in_keychain(&profile_id, &passphrase)?;
            }

            (AuthType::Key, Some(path), store_passphrase)
        } else {
            // Unencrypted key - no passphrase needed
            (AuthType::Key, Some(path), false)
        }
    } else if non_interactive {
        anyhow::bail!("SSH key path is required in non-interactive mode");
    } else {
        // Interactive mode - ask user for authentication method
        let key_path_input: String = Input::new()
            .with_prompt("Path to SSH private key (or press Enter for password authentication)")
            .allow_empty(true)
            .interact_text()?;

        if key_path_input.trim().is_empty() {
            // Password authentication
            let password = Password::new()
                .with_prompt("SSH password")
                .interact()?;

            let store_password = Confirm::new()
                .with_prompt("Store password in system keychain?")
                .default(false)
                .interact()?;

            if store_password {
                println!("{}", "⚠️  Note: Password cannot be validated until first connection.".yellow());
                println!("{}", "    If the password is incorrect, you'll be prompted again when starting the tunnel.".dimmed());
                store_password_in_keychain(&profile_id, &password)?;
            }

            (AuthType::Password, None, store_password)
        } else {
            // Key authentication
            let key_path = PathBuf::from(shellexpand::tilde(&key_path_input).to_string());
            validate_ssh_key(&key_path)?;

            // Ask about passphrase storage
            let store_passphrase = Confirm::new()
                .with_prompt("Does this key have a passphrase you want to store in keychain?")
                .default(false)
                .interact()?;

            if store_passphrase {
                let passphrase = Password::new()
                    .with_prompt("SSH key passphrase")
                    .interact()?;

                // Validate the passphrase by attempting to load the key
                if let Err(e) = validate_key_passphrase(&key_path, &passphrase) {
                    println!("{}", format!("⚠️  Failed to load SSH key with provided passphrase: {}", e).yellow());
                    println!("{}", "The passphrase will not be stored.".yellow());
                    (AuthType::Key, Some(key_path), false)
                } else {
                    store_password_in_keychain(&profile_id, &passphrase)?;
                    (AuthType::Key, Some(key_path), store_passphrase)
                }
            } else {
                (AuthType::Key, Some(key_path), store_passphrase)
            }
        }
    };

    let bind_address = if let Some(addr) = bind_address {
        addr
    } else if non_interactive {
        "127.0.0.1".to_string()
    } else {
        Input::new()
            .with_prompt("Local bind address")
            .default("localhost".to_string())
            .interact_text()?
    };

    let local_port = if let Some(port) = local_port {
        validate_local_port(port, non_interactive)?;
        port
    } else if non_interactive {
        anyhow::bail!("Local port is required in non-interactive mode");
    } else {
        let port: u16 = Input::new()
            .with_prompt("Local port to bind")
            .interact_text()?;
        validate_local_port(port, non_interactive)?;
        port
    };

    let forward_host = if let Some(host) = forward_host {
        host
    } else if non_interactive {
        "localhost".to_string()
    } else {
        Input::new()
            .with_prompt("Remote host to forward to")
            .default("localhost".to_string())
            .interact_text()?
    };

    let forward_port = if let Some(port) = forward_port {
        port
    } else if non_interactive {
        anyhow::bail!("Remote port is required in non-interactive mode");
    } else {
        Input::new()
            .with_prompt("Remote port to forward to")
            .interact_text()?
    };

    // Show bind address info if not loopback
    if !ssh_tunnel_common::is_loopback_address(&bind_address) {
        println!();
        println!(
            "{}",
            format!(
                "⚠️  Binding to {} (accessible from local network/VMs)",
                bind_address
            )
            .yellow()
        );
    }

    // Advanced Options Section
    // Check if any CLI option flags were provided
    let any_options_provided = compression.is_some()
        || keepalive_interval.is_some()
        || auto_reconnect.is_some()
        || reconnect_attempts.is_some()
        || reconnect_delay.is_some()
        || tcp_keepalive.is_some()
        || max_packet_size.is_some()
        || window_size.is_some();

    let options = if any_options_provided {
        // CLI flags provided: use them (falling back to defaults for unprovided options)
        TunnelOptions {
            compression: compression.unwrap_or(false),
            keepalive_interval: keepalive_interval.unwrap_or(60),
            auto_reconnect: auto_reconnect.unwrap_or(true),
            reconnect_attempts: reconnect_attempts.unwrap_or(3),
            reconnect_delay: reconnect_delay.unwrap_or(5),
            tcp_keepalive: tcp_keepalive.unwrap_or(false),
            max_packet_size: max_packet_size.unwrap_or(65536),
            window_size: window_size.unwrap_or(2097152),
        }
    } else if non_interactive {
        // Non-interactive mode with no CLI options: use defaults
        TunnelOptions::default()
    } else {
        // Interactive mode with no CLI options: ask user if they want to configure
        println!();
        println!("{}", "=== Advanced Options ===".bold());
        let configure_advanced = Confirm::new()
            .with_prompt("Configure advanced tunnel options?")
            .default(false)
            .interact()?;

        if configure_advanced {
            TunnelOptions {
                compression: Confirm::new()
                    .with_prompt("Enable SSH compression?")
                    .default(false)
                    .interact()?,
                keepalive_interval: Input::new()
                    .with_prompt("Keepalive interval (seconds, 0 to disable)")
                    .default(60)
                    .interact_text()?,
                auto_reconnect: Confirm::new()
                    .with_prompt("Enable auto-reconnect on failure?")
                    .default(true)
                    .interact()?,
                reconnect_attempts: Input::new()
                    .with_prompt("Maximum reconnect attempts (0 for unlimited)")
                    .default(3)
                    .interact_text()?,
                reconnect_delay: Input::new()
                    .with_prompt("Delay between reconnect attempts (seconds)")
                    .default(5)
                    .interact_text()?,
                tcp_keepalive: Confirm::new()
                    .with_prompt("Enable TCP keepalive on forwarded connections?")
                    .default(false)
                    .interact()?,
                max_packet_size: Input::new()
                    .with_prompt("Maximum packet size (bytes)")
                    .default(65535)
                    .interact_text()?,
                window_size: Input::new()
                    .with_prompt("Window size (bytes)")
                    .default(2097152)
                    .interact_text()?,
            }
        } else {
            TunnelOptions::default()
        }
    };

    // Create the profile
    let connection = ConnectionConfig {
        host: remote_host.clone(),
        port: remote_port,
        user: user.clone(),
        auth_type,
        key_path,
        password_stored,
    };

    let forwarding = ForwardingConfig {
        forwarding_type: ForwardingType::Local,
        local_port: Some(local_port),
        remote_host: Some(forward_host.clone()),
        remote_port: Some(forward_port),
        bind_address: bind_address.clone(),
    };

    // Create profile with custom ID and options
    let mut profile = Profile::new_with_options(name.clone(), connection, forwarding, options);
    // Override the generated ID with our pre-generated one (used for keychain)
    profile.metadata.id = profile_id;

    // Validate the profile
    profile.validate().context("Profile validation failed")?;

    // Save the profile
    let profile_path = save_profile(&profile, false)?;

    // Success message
    println!();
    println!("{}", "✓ Profile created successfully!".green().bold());
    println!("  Saved to: {}", profile_path.display().to_string().dimmed());
    println!();
    println!("{}", "Profile Summary:".bold());
    println!("  Name: {}", name.cyan());
    println!("  Remote: {}@{}:{}", user, remote_host, remote_port);
    if let Some(ref kp) = profile.connection.key_path {
        println!("  Key: {}", kp.display());
    } else {
        println!("  Auth: Password");
    }
    println!(
        "  Tunnel: {}:{} → {}:{}",
        bind_address, local_port, forward_host, forward_port
    );
    println!();
    println!(
        "Start the tunnel with: {}",
        format!("ssh-tunnel start {}", name).yellow()
    );

    Ok(())
}

fn store_password_in_keychain(profile_id: &Uuid, password: &str) -> Result<()> {
    use keyring::Entry;

    let entry = Entry::new("ssh-tunnel-manager", &profile_id.to_string())
        .context("Failed to create keychain entry")?;
    entry
        .set_password(password)
        .context("Failed to store password in keychain")?;
    println!("{}", "  ✓ Password stored in system keychain".green());
    Ok(())
}

fn validate_ssh_key(key_path: &PathBuf) -> Result<()> {
    // Check if file exists
    if !key_path.exists() {
        anyhow::bail!(
            "SSH key not found: {}\n  \
             Create a key with: ssh-keygen -t ed25519",
            key_path.display()
        );
    }

    // Check if it's a file
    if !key_path.is_file() {
        anyhow::bail!("SSH key path is not a file: {}", key_path.display());
    }

    // Check permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(key_path)?;
        let permissions = metadata.permissions();
        let mode = permissions.mode();

        // SSH keys should be 0600 or 0400
        if mode & 0o077 != 0 {
            println!(
                "{}",
                format!(
                    "⚠️  SSH key has insecure permissions: {:o}\n   \
                     Fix with: chmod 600 {}",
                    mode & 0o777,
                    key_path.display()
                )
                .yellow()
            );

            if !Confirm::new()
                .with_prompt("Continue anyway?")
                .default(false)
                .interact()?
            {
                anyhow::bail!("Aborted due to insecure key permissions");
            }
        }
    }

    Ok(())
}

fn is_key_encrypted(key_path: &PathBuf) -> Result<bool> {
    use russh_keys::decode_secret_key;

    // Read the key file
    let key_data = fs::read_to_string(key_path)
        .context("Failed to read SSH key file")?;

    // Try to decode without a passphrase
    match decode_secret_key(&key_data, None) {
        Ok(_) => Ok(false),  // Key loaded successfully without passphrase - not encrypted
        Err(_) => Ok(true),  // Failed to load - likely encrypted (or corrupted, but we'll find out)
    }
}

fn validate_key_passphrase(key_path: &PathBuf, passphrase: &str) -> Result<()> {
    use russh_keys::decode_secret_key;

    // Read the key file
    let key_data = fs::read_to_string(key_path)
        .context("Failed to read SSH key file")?;

    // Attempt to decode with the passphrase
    decode_secret_key(&key_data, Some(passphrase))
        .map_err(|e| anyhow::anyhow!("Invalid passphrase: {}", e))?;

    Ok(())
}

fn validate_local_port(port: u16, non_interactive: bool) -> Result<()> {
    if port <= 1024 {
        let warning = format!(
            "⚠️  Port {} requires root/admin privileges (privileged port)",
            port
        );
        println!("{}", warning.yellow());

        if !non_interactive {
            if !Confirm::new()
                .with_prompt("Continue with this port?")
                .default(false)
                .interact()?
            {
                anyhow::bail!("Aborted due to privileged port selection");
            }
        }
    }
    Ok(())
}

async fn list_profiles(verbose: bool, json: bool) -> Result<()> {
    // Load all profiles from common module
    let mut profiles = ssh_tunnel_common::load_all_profiles()?;

    if profiles.is_empty() {
        println!("{}", "No profiles found.".yellow());
        println!("Create one with: {}", "ssh-tunnel add <name>".cyan());
        return Ok(());
    }

    // Sort profiles by name
    profiles.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));

    if json {
        // JSON output for scripting
        let json_output = serde_json::to_string_pretty(&profiles)?;
        println!("{}", json_output);
    } else if verbose {
        // Verbose output
        print_profiles_verbose(&profiles);
    } else {
        // Table output (default)
        print_profiles_table(&profiles);
    }

    Ok(())
}

/// Create an HTTP client configured to connect to the daemon
fn create_daemon_client() -> Result<Client> {
    let cli_config = config::CliConfig::load()?;
    ssh_tunnel_common::create_daemon_client(&cli_config.daemon_config)
}

/// Get the daemon base URL for API requests
fn daemon_base_url() -> Result<String> {
    let cli_config = config::CliConfig::load()?;
    cli_config.daemon_config.daemon_base_url()
}

/// Add authentication header to request if configured
fn add_auth_header(request: reqwest::RequestBuilder) -> Result<reqwest::RequestBuilder> {
    let cli_config = config::CliConfig::load()?;
    ssh_tunnel_common::add_auth_header(request, &cli_config.daemon_config)
}

fn print_profiles_table(profiles: &[Profile]) {
    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);

    // Header
    table.set_header(vec![
        Cell::new("Name")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new("Remote")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new("Tunnel")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new("Tags")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
    ]);

    // Rows
    for profile in profiles {
        let remote = format!(
            "{}@{}:{}",
            profile.connection.user, profile.connection.host, profile.connection.port
        );

        let tunnel = if let (Some(local_port), Some(remote_host), Some(remote_port)) = (
            profile.forwarding.local_port,
            &profile.forwarding.remote_host,
            profile.forwarding.remote_port,
        ) {
            format!(
                "{}:{} → {}:{}",
                profile.forwarding.bind_address, local_port, remote_host, remote_port
            )
        } else {
            "N/A".to_string()
        };

        let tags = if profile.metadata.tags.is_empty() {
            "-".to_string()
        } else {
            profile.metadata.tags.join(", ")
        };

        table.add_row(vec![
            Cell::new(&profile.metadata.name).fg(Color::Green),
            Cell::new(remote),
            Cell::new(tunnel),
            Cell::new(tags).fg(Color::DarkGrey),
        ]);
    }

    println!();
    println!("{}", table);
    println!();
    println!("{} profile(s) found", profiles.len().to_string().cyan());
    println!();
}

fn print_profiles_verbose(profiles: &[Profile]) {
    println!();
    for (i, profile) in profiles.iter().enumerate() {
        if i > 0 {
            println!("{}", "─".repeat(80).dimmed());
        }

        println!(
            "{}",
            format!("Profile: {}", profile.metadata.name).bold().green()
        );
        println!("  ID: {}", profile.metadata.id.to_string().dimmed());

        if let Some(desc) = &profile.metadata.description {
            println!("  Description: {}", desc);
        }

        println!();
        println!("{}", "  Connection:".bold());
        println!(
            "    Remote: {}@{}:{}",
            profile.connection.user, profile.connection.host, profile.connection.port
        );
        println!("    Auth: {:?}", profile.connection.auth_type);
        if let Some(key_path) = &profile.connection.key_path {
            println!("    Key: {}", key_path.display());
        }

        println!();
        println!("{}", "  Forwarding:".bold());
        println!("    Type: {:?}", profile.forwarding.forwarding_type);
        if let Some(local_port) = profile.forwarding.local_port {
            println!(
                "    Local: {}:{}",
                profile.forwarding.bind_address, local_port
            );
        }
        if let (Some(remote_host), Some(remote_port)) = (
            &profile.forwarding.remote_host,
            profile.forwarding.remote_port,
        ) {
            println!("    Remote: {}:{}", remote_host, remote_port);
        }

        if profile.options.auto_reconnect {
            println!();
            println!("{}", "  Options:".bold());
            println!(
                "    Auto-reconnect: enabled (max {} attempts)",
                profile.options.reconnect_attempts
            );
            println!("    Keepalive: {}s", profile.options.keepalive_interval);
        }

        if !profile.metadata.tags.is_empty() {
            println!();
            println!("  Tags: {}", profile.metadata.tags.join(", ").cyan());
        }

        println!();
        println!(
            "  Created: {}",
            profile
                .metadata
                .created_at
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
                .dimmed()
        );
        println!(
            "  Modified: {}",
            profile
                .metadata
                .modified_at
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
                .dimmed()
        );
        println!();
    }
}

async fn watch_events(name: Option<String>) -> Result<()> {
    let client = create_daemon_client()?;
    let base_url = daemon_base_url()?;

    // If a name is given, resolve it to a profile ID so we can filter.
    let filter_id = if let Some(name) = name {
        let profile = load_profile_by_name(&name)?;
        Some(profile.metadata.id)
    } else {
        None
    };

    let url = format!("{}/api/events", base_url);
    let resp = add_auth_header(client.get(&url))?
        .send()
        .await
        .context("Failed to connect to events stream")?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Daemon returned non-success status for events: {}",
            resp.status()
        );
    }

    println!(
        "{}",
        "Connected to event stream. Press Ctrl+C to stop.".dimmed()
    );

    let mut stream = resp.bytes_stream();
    use std::str;

    // Very simple SSE parser: read lines and handle "data: " ones.
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error reading from event stream")?;
        let text = str::from_utf8(&chunk).unwrap_or("");

        buffer.push_str(text);

        // Process complete lines
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim_end().to_string();
            buffer.drain(..=pos);

            // Ignore comments / empty lines
            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if let Some(rest) = line.strip_prefix("data:") {
                let json_str = rest.trim();
                if json_str.is_empty() {
                    continue;
                }

                match serde_json::from_str::<DaemonTunnelEvent>(json_str) {
                    Ok(ev) => {
                        // Optional filter by tunnel id
                        if let Some(fid) = filter_id {
                            let id = match &ev {
                                DaemonTunnelEvent::Starting { id }
                                | DaemonTunnelEvent::Connected { id }
                                | DaemonTunnelEvent::Disconnected { id, .. }
                                | DaemonTunnelEvent::Error { id, .. }
                                | DaemonTunnelEvent::AuthRequired { id, .. } => Some(id),
                                DaemonTunnelEvent::Heartbeat { .. } => None,
                            };
                            if let Some(id) = id {
                                if *id != fid {
                                    continue;
                                }
                            }
                        }

                        match ev {
                            DaemonTunnelEvent::Starting { id } => {
                                println!("{}", format!("Starting tunnel {id}").cyan());
                            }
                            DaemonTunnelEvent::Connected { id } => {
                                println!("{}", format!("Tunnel {id} connected").green());
                            }
                            DaemonTunnelEvent::Disconnected { id, reason } => {
                                println!(
                                    "{}",
                                    format!("Tunnel {id} disconnected: {reason}").yellow()
                                );
                            }
                            DaemonTunnelEvent::Error { id, error } => {
                                eprintln!("{}", format!("Tunnel {id} error: {error}").red());
                            }
                            DaemonTunnelEvent::AuthRequired { id, request } => {
                                println!(
                                    "{}",
                                    format!("Auth required for tunnel {id}: {}", request.prompt)
                                        .magenta()
                                );
                            }
                            DaemonTunnelEvent::Heartbeat { .. } => {
                                // Ignore heartbeats in watch mode
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to parse event JSON: {e} (line: {json_str})");
                    }
                }
            }
        }
    }

    Ok(())
}

/// Delete a profile by name
async fn delete_profile(name: String) -> Result<()> {
    // Check if profile exists
    if !profile_exists_by_name(&name) {
        anyhow::bail!("Profile '{}' not found", name.yellow());
    }

    // Confirm deletion
    let confirm = Confirm::new()
        .with_prompt(format!(
            "Are you sure you want to delete profile '{}'?",
            name.yellow()
        ))
        .default(false)
        .interact()?;

    if !confirm {
        println!("{}", "Deletion cancelled".dimmed());
        return Ok(());
    }

    // Delete the profile
    let profile_path = delete_profile_by_name(&name)?;

    println!();
    println!(
        "{}",
        format!("Profile '{}' deleted successfully", name).green()
    );
    println!("  Removed: {}", profile_path.display().to_string().dimmed());
    println!();

    Ok(())
}

/// Show detailed information about a profile
async fn show_profile_info(name: String) -> Result<()> {
    // Load the profile
    let profile = load_profile_by_name(&name)?;

    println!();
    println!("{}", format!("Profile: {}", profile.metadata.name).bold().green());
    println!("  ID: {}", profile.metadata.id.to_string().dimmed());

    if let Some(desc) = &profile.metadata.description {
        println!("  Description: {}", desc);
    }

    if !profile.metadata.tags.is_empty() {
        println!("  Tags: {}", profile.metadata.tags.join(", ").cyan());
    }

    println!();
    println!("{}", "  Connection:".bold());
    println!("    Host: {}", profile.connection.host);
    println!("    Port: {}", profile.connection.port);
    println!("    User: {}", profile.connection.user);
    println!("    Auth: {:?}", profile.connection.auth_type);

    if let Some(key_path) = &profile.connection.key_path {
        println!("    Key:  {}", key_path.display());
    }

    println!();
    println!("{}", "  Port Forwarding:".bold());
    println!("    Type:        {:?}", profile.forwarding.forwarding_type);
    println!("    Bind:        {}", profile.forwarding.bind_address);

    if let Some(local_port) = profile.forwarding.local_port {
        println!("    Local Port:  {}", local_port);
    }

    if let Some(remote_host) = &profile.forwarding.remote_host {
        println!("    Remote Host: {}", remote_host);
    }

    if let Some(remote_port) = profile.forwarding.remote_port {
        println!("    Remote Port: {}", remote_port);
    }

    println!();
    println!("{}", "  Options:".bold());
    println!("    Compression:       {}", profile.options.compression);
    println!("    Keepalive:         {} seconds", profile.options.keepalive_interval);
    println!("    Auto-reconnect:    {}", profile.options.auto_reconnect);

    if profile.options.auto_reconnect {
        println!("    Reconnect Attempts: {}", profile.options.reconnect_attempts);
        println!("    Reconnect Delay:    {} seconds", profile.options.reconnect_delay);
    }

    println!("    TCP Keepalive:     {}", profile.options.tcp_keepalive);
    println!("    Max Packet Size:   {} bytes", profile.options.max_packet_size);
    println!("    Window Size:       {} bytes", profile.options.window_size);

    println!();

    Ok(())
}
