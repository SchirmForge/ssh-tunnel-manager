# GUI SSE Integration

## Overview

The GUI now uses the **shared SSE-first tunnel control flow** from the common module, providing consistent behavior with the CLI and reliable event handling for tunnel operations.

## Implementation

### GTK Event Handler

Created `GtkTunnelEventHandler` in [crates/gui/src/ui/tunnel_handler.rs](../crates/gui/src/ui/tunnel_handler.rs) that implements the `TunnelEventHandler` trait from `ssh-tunnel-common`.

**Key Features:**
- Shows GTK authentication dialogs when auth is required
- Supports both hidden (password) and visible (text) input based on SSH server requirements
- Updates UI with connection status and error messages
- Handles user cancellation gracefully

**Implementation Details:**
```rust
pub struct GtkTunnelEventHandler {
    profile: Profile,
    window: adw::ApplicationWindow,
    auth_response_tx: Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>>,
    status_callback: Option<Rc<dyn Fn(&str)>>,
}

impl TunnelEventHandler for GtkTunnelEventHandler {
    fn on_auth_required(&mut self, request: &AuthRequest) -> Result<String> {
        // Shows GTK dialog and blocks until user responds
        self.show_auth_dialog_sync(&request.prompt, request.hidden)
    }

    fn on_connected(&mut self) {
        // Updates UI with connection success message
    }

    fn on_event(&mut self, event: &DaemonTunnelEvent) {
        // Updates UI with real-time status (Starting, Error, Disconnected, etc.)
    }
}
```

### Updated Tunnel Control Functions

#### Start Tunnel ([crates/gui/src/ui/details.rs:405-446](../crates/gui/src/ui/details.rs#L405-L446))

**Before:**
```rust
async fn start_tunnel_async(profile: &ProfileModel, state: &Rc<AppState>) -> anyhow::Result<()> {
    let daemon_client = state.daemon_client.borrow()...;
    daemon_client.start_tunnel(profile_id).await?;
    Ok(())
}
```

**After:**
```rust
async fn start_tunnel_async(profile: &ProfileModel, state: &Rc<AppState>) -> anyhow::Result<()> {
    use ssh_tunnel_common::{create_daemon_client, start_tunnel_with_events};
    use crate::ui::tunnel_handler::GtkTunnelEventHandler;

    let client = create_daemon_client(&daemon_config)?;
    let mut handler = GtkTunnelEventHandler::new(inner_profile.clone(), window, None);

    // Uses shared SSE-first helper
    start_tunnel_with_events(&client, &daemon_config, tunnel_id, &mut handler).await?;
    Ok(())
}
```

#### Stop Tunnel ([crates/gui/src/ui/details.rs:449-474](../crates/gui/src/ui/details.rs#L449-L474))

**Before:**
```rust
async fn stop_tunnel_async(profile: &ProfileModel, state: &Rc<AppState>) -> anyhow::Result<()> {
    let daemon_client = state.daemon_client.borrow()...;
    daemon_client.stop_tunnel(profile_id).await?;
    Ok(())
}
```

**After:**
```rust
async fn stop_tunnel_async(profile: &ProfileModel, state: &Rc<AppState>) -> anyhow::Result<()> {
    use ssh_tunnel_common::{create_daemon_client, stop_tunnel};

    let client = create_daemon_client(&daemon_config)?;
    stop_tunnel(&client, &daemon_config, tunnel_id).await?;
    Ok(())
}
```

## Event Flow

### Authentication Flow

1. User clicks "Start Tunnel"
2. GUI calls `start_tunnel_with_events()` with `GtkTunnelEventHandler`
3. SSE connection established **before** tunnel start request
4. Daemon starts tunnel and sends events via SSE
5. If auth required:
   - Handler receives `AuthRequired` event
   - `on_auth_required()` shows GTK dialog
   - User enters password/code
   - Response sent back to daemon
   - Tunnel continues connecting
6. On success:
   - Handler receives `Connected` event
   - `on_connected()` updates UI with success message

### Error Flow

1. User clicks "Start Tunnel"
2. SSE connection established
3. Daemon attempts to connect
4. If error occurs (connection refused, port in use, etc.):
   - Handler receives `Error` event immediately
   - `on_event()` updates UI with error message
   - Function returns with error
   - GUI shows error dialog

## Benefits Over Old Approach

### Before (Direct API calls)
- ❌ No real-time status updates during connection
- ❌ Auth prompts required polling or separate SSE subscription
- ❌ Race conditions between start request and SSE subscription
- ❌ Duplicate event handling code between CLI and GUI
- ❌ Could miss critical error events

### After (Shared SSE flow)
- ✅ Real-time status updates from daemon
- ✅ Immediate error reporting (port conflicts, connection failures)
- ✅ Integrated auth handling via GTK dialogs
- ✅ No race conditions - SSE ready before start request
- ✅ Consistent behavior with CLI
- ✅ Single source of truth for tunnel control logic

## Authentication Dialog

The auth dialog supports both password and visible text input:

```rust
fn show_auth_dialog_internal(
    window: &adw::ApplicationWindow,
    prompt: &str,
    hidden: bool,  // True for passwords, false for visible text
    auth_tx: Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>>,
) {
    // Create appropriate widget
    let entry: gtk4::Widget = if hidden {
        gtk4::PasswordEntry::builder()...  // Hidden input for passwords
    } else {
        gtk4::Entry::builder()...          // Visible input for text
    };

    // Show dialog, wait for response, send via channel
}
```

**User Experience:**
- Prompt text comes directly from SSH server (e.g., "Password:", "Enter 2FA code:")
- Password fields use `PasswordEntry` with peek icon
- Text fields use regular `Entry` for visible input
- Enter key submits the dialog
- Cancel button aborts the connection

## Future Enhancements

### Status Labels
Currently, the handler accepts an optional `status_callback` parameter:
```rust
let mut handler = GtkTunnelEventHandler::new(
    inner_profile.clone(),
    window,
    None, // TODO: Add status callback when UI supports it
);
```

**Future:** Add a status label to the UI and pass a callback:
```rust
let status_label = gtk4::Label::new(Some("Ready"));
let status_callback = {
    let label = status_label.clone();
    Rc::new(move |msg: &str| {
        label.set_text(msg);
    })
};

let mut handler = GtkTunnelEventHandler::new(
    inner_profile.clone(),
    window,
    Some(status_callback),
);
```

This would show real-time status like:
- "Connecting to SSH server..."
- "Authentication required..."
- "✓ Tunnel connected! Forwarding 127.0.0.1:8080 → remote:80"

### Progress Indicators

Add a spinner or progress bar that shows during connection:
- Start spinner when "Starting..." is shown
- Stop spinner on success or error
- Visual feedback for long-running operations

### Retry Logic

Add UI for retrying failed connections:
- Show "Retry" button on connection errors
- Offer to edit profile if auth fails repeatedly
- Remember last error for troubleshooting

## Testing

To test the SSE integration:

1. **Build the GUI:**
   ```bash
   cargo build --package ssh-tunnel-gui
   ```

2. **Run the daemon:**
   ```bash
   ssh-tunnel-daemon
   ```

3. **Run the GUI:**
   ```bash
   ssh-tunnel-gui
   ```

4. **Test scenarios:**
   - Start tunnel with stored credentials → Should connect without prompts
   - Start tunnel without stored credentials → Should show auth dialog
   - Start tunnel with wrong credentials → Should show error dialog
   - Start tunnel when port is in use → Should show error immediately
   - Start tunnel to unreachable server → Should show connection error

## Related Documentation

- [Shared SSE Flow](SHARED_SSE_FLOW.md) - Details on the common SSE helpers
- [SSE Race Condition Fix](SSE_RACE_CONDITION_FIX.md) - Race condition prevention
- [Daemon Event Ordering Fix](DAEMON_EVENT_ORDERING_FIX.md) - Event ordering guarantees

## Summary

The GUI now uses the same battle-tested SSE-first flow as the CLI, providing:
- **Reliability:** No missed events, proper error reporting
- **Consistency:** Same behavior across CLI and GUI
- **Maintainability:** Single implementation to maintain
- **User Experience:** Real-time feedback and integrated auth dialogs

The integration maintains GTK best practices while leveraging the shared async infrastructure from the common module.
