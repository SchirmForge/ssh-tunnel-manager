# Shared SSE-First Tunnel Control Flow

## Overview

The SSE (Server-Sent Events) first tunnel control flow has been extracted from the CLI into `crates/common/src/daemon_client.rs` to be shared between CLI and GUI implementations.

## Key Components

### 1. `TunnelEvent` Enum
Represents all possible events from the daemon's SSE stream:
- `Starting { id }` - Tunnel is starting
- `Connected { id }` - Tunnel successfully connected
- `Disconnected { id, reason }` - Tunnel disconnected
- `Error { id, error }` - Tunnel encountered an error
- `AuthRequired { id, request }` - Authentication needed
- `Heartbeat { timestamp }` - Keep-alive event

### 2. `TunnelEventHandler` Trait
Implement this trait to handle tunnel events and authentication:

```rust
pub trait TunnelEventHandler: Send {
    /// Called when authentication is required (REQUIRED)
    fn on_auth_required(&mut self, request: &AuthRequest) -> Result<String>;

    /// Called when tunnel successfully connects (optional)
    fn on_connected(&mut self) {}

    /// Called on any event for logging/status updates (optional)
    fn on_event(&mut self, event: &TunnelEvent) {}
}
```

### 3. `start_tunnel_with_events()`
Main function for starting tunnels with real-time event handling:

```rust
pub async fn start_tunnel_with_events<H: TunnelEventHandler>(
    client: &Client,
    config: &DaemonClientConfig,
    tunnel_id: Uuid,
    handler: &mut H,
) -> Result<()>
```

**Features:**
- Real-time status updates via SSE
- Interactive authentication handling
- Automatic timeout (60 seconds overall, 15 second idle fallback)
- Fallback to REST polling if SSE fails
- Filters events by tunnel ID

### 4. `stop_tunnel()`
Simple function for stopping tunnels:

```rust
pub async fn stop_tunnel(
    client: &Client,
    config: &DaemonClientConfig,
    tunnel_id: Uuid,
) -> Result<()>
```

## Usage Example (CLI)

```rust
use ssh_tunnel_common::{
    start_tunnel_with_events, TunnelEventHandler, DaemonTunnelEvent,
    AuthRequest, Profile,
};

struct CliEventHandler {
    profile: Profile,
}

impl TunnelEventHandler for CliEventHandler {
    fn on_auth_required(&mut self, request: &AuthRequest) -> Result<String> {
        // Prompt user for password/passphrase
        Password::new()
            .with_prompt(&request.prompt)
            .interact()
    }

    fn on_connected(&mut self) {
        println!("✓ Tunnel connected!");
    }

    fn on_event(&mut self, event: &DaemonTunnelEvent) {
        match event {
            DaemonTunnelEvent::Starting { .. } => {
                println!("Connecting...");
            }
            _ => {}
        }
    }
}

async fn start_tunnel(profile: Profile) -> Result<()> {
    let client = create_daemon_client()?;
    let config = DaemonClientConfig::default();
    let tunnel_id = profile.metadata.id;

    let mut handler = CliEventHandler { profile };

    start_tunnel_with_events(&client, &config, tunnel_id, &mut handler).await
}
```

## Usage Example (GUI/GTK)

```rust
use gtk::prelude::*;
use ssh_tunnel_common::{
    start_tunnel_with_events, TunnelEventHandler, DaemonTunnelEvent,
};

struct GuiEventHandler {
    window: ApplicationWindow,
    status_label: Label,
    auth_dialog: AuthDialog,
}

impl TunnelEventHandler for GuiEventHandler {
    fn on_auth_required(&mut self, request: &AuthRequest) -> Result<String> {
        // Show GTK dialog for authentication
        self.auth_dialog.show_and_wait(&request.prompt)
    }

    fn on_connected(&mut self) {
        self.status_label.set_text("✓ Connected");
    }

    fn on_event(&mut self, event: &DaemonTunnelEvent) {
        match event {
            DaemonTunnelEvent::Starting { .. } => {
                self.status_label.set_text("Connecting...");
            }
            DaemonTunnelEvent::Error { error, .. } => {
                self.status_label.set_text(&format!("Error: {}", error));
            }
            _ => {}
        }
    }
}
```

## Benefits

1. **Code Reuse**: Eliminates duplicate SSE handling logic between CLI and GUI
2. **Consistency**: Ensures identical behavior across all clients
3. **Maintainability**: Single source of truth for tunnel control flow
4. **Flexibility**: Event handler trait allows custom UI/UX per client
5. **Robustness**: Built-in timeouts, error handling, and fallback mechanisms

## Socket Path Auto-Detection

The `DaemonClientConfig::socket_path()` method now automatically detects the daemon socket in multiple locations:

1. Explicit path in config (`daemon_url` as absolute path)
2. User runtime directory (`/run/user/<uid>/ssh-tunnel-manager.sock`)
3. System-wide location (`/run/ssh-tunnel-manager/ssh-tunnel-manager.sock`)

This ensures the CLI/GUI can connect whether the daemon is running as:
- The same user (user systemd service)
- A different user (system systemd service)

## Migration Guide

### Before (Old CLI Code)

```rust
// 200+ lines of SSE parsing, event handling, timeout logic, etc.
async fn start_tunnel(name: String) -> Result<()> {
    // Manual SSE subscription
    // Manual event parsing
    // Manual timeout handling
    // Manual auth handling
    // ...
}
```

### After (Shared Flow)

```rust
// ~20 lines - just implement the handler!
struct CliEventHandler { profile: Profile }

impl TunnelEventHandler for CliEventHandler {
    fn on_auth_required(&mut self, request: &AuthRequest) -> Result<String> {
        prompt_for_auth(request)
    }
    fn on_connected(&mut self) {
        announce_connected(&self.profile);
    }
}

async fn start_tunnel(name: String) -> Result<()> {
    let profile = load_profile_by_name(&name)?;
    let client = create_daemon_client()?;
    let config = CliConfig::load()?.daemon_config;
    let mut handler = CliEventHandler { profile };

    start_tunnel_with_events(&client, &config, tunnel_id, &mut handler).await
}
```

## Next Steps

- [ ] Integrate shared flow into GUI (`crates/gui`)
- [ ] Add connection timeout configuration options
- [ ] Consider adding event streaming helpers for watch/monitor commands
