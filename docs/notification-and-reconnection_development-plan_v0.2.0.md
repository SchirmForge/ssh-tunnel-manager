# v0.2.0: Desktop Notifications with Automatic Reconnection

> **Corrections applied 2026-08-30, after the stabilisation pass.** This plan was written
> against an earlier tree and several of its premises no longer hold. Read these first:
>
> 1. **`crates/tray` no longer exists.** It had been outside the workspace build since
>    v0.1.6 and duplicated the SSE client that `crates/common/src/sse.rs` now owns, so it
>    was deleted. The "Notification Infrastructure ✓ Already implemented" claim in
>    *Current State Analysis* is therefore **false** - that code is gone.
>    **Phase 3 (tray notification actions) and §4.2 (tray event handler) do not apply.**
>    Phase 2's `crates/gui-gtk/src/ui/notifications.rs` becomes the primary
>    implementation, not a duplicate of an existing one.
> 2. **§1.3 is already done - skip it.** It asks to extract `connect_and_forward()` from
>    `run_tunnel()`. That split already exists: `run_tunnel()` is already just
>    `establish_connection()` plus `monitor_tunnel()`. The retry loop wraps those two
>    directly.
> 3. **§1.1's `can_auto_reconnect_without_auth()` is wrong in both directions.** It
>    rejects unencrypted keys (`PasswordStorage::None`), which are the most common
>    eligible case, and rejects `Password` + keychain, which the daemon *does* support -
>    it retrieves stored passwords in `authenticate_with_password()`. Use the eligibility
>    table in [PROJECT_STATUS.md](PROJECT_STATUS.md) instead.
> 4. **Auto-reconnect must be a global setting with a per-profile override**, and the
>    current `auto_reconnect` default of `true` is wrong - it flags 2FA profiles that can
>    never reconnect unattended. See PROJECT_STATUS.md for the agreed design.
> 5. **§1.4, §4.1 and §5 are accurate.** `TunnelStatus::Reconnecting` and
>    `TunnelDomainEvent::Reconnecting` do already exist unused in
>    `crates/common/src/types.rs`, while the wire types (`daemon::api::OutgoingEvent` and
>    `common::sse::TunnelEvent`) genuinely lack the variant.
> 6. **There is now a test suite to extend.** Add reconnection coverage to
>    `crates/daemon/tests/`, and note that the live tier
>    (`crates/daemon/tests/live_ssh.rs`) can drop a connection against a real server. See
>    [DEVELOPMENT.md](DEVELOPMENT.md#testing).

## Overview

Implement desktop notifications for tunnel disconnect events with options to manually reconnect or automatically reconnect (when auto_reconnect=true and authentication doesn't require user input). This builds on existing notification infrastructure in the tray app and wires up the auto-reconnect configuration that already exists but isn't implemented.

## User Requirements

1. **Desktop notifications** when tunnels disconnect, showing:
   - Profile name
   - Disconnect reason
   - Action buttons to reconnect

2. **Automatic reconnection** when:
   - Profile has `auto_reconnect = true`
   - Authentication is possible without user input (SSH key-only or keychain-stored credentials)
   - Respects `reconnect_attempts` and `reconnect_delay` settings

3. **Manual reconnection** via notification action button when auto-reconnect fails or isn't available

## Current State Analysis

### What Already Exists ✓

1. **Notification Infrastructure** ([crates/tray/src/notifications.rs](crates/tray/src/notifications.rs)):
   - `show_disconnect_notification(profile_name, reason, profile_id)` - Already implemented
   - `show_error_notification(profile_name, error)` - Already implemented
   - `show_connected_notification(profile_name)` - Already implemented
   - Uses `notify-rust` crate (v4.11)
   - Disconnect notifications include "Reconnect" action button

2. **Auto-Reconnect Configuration** ([crates/common/src/config.rs:157-184](crates/common/src/config.rs#L157-L184)):
   - `TunnelOptions.auto_reconnect: bool` (default: true)
   - `TunnelOptions.reconnect_attempts: u32` (default: 3, 0 = unlimited)
   - `TunnelOptions.reconnect_delay: u64` (default: 5 seconds)
   - Full UI support in CLI and GUI

3. **Event System** ([crates/common/src/types.rs:36-95](crates/common/src/types.rs#L36-L95)):
   - `TunnelStatus::Reconnecting` - Status exists but unused
   - `TunnelDomainEvent::Reconnecting` - Event type with attempt counter
   - `TunnelStatus::Failed(String)` - Captures failure reasons

4. **Keyring Integration** ([crates/common/src/keychain.rs](crates/common/src/keychain.rs)):
   - Fully implemented for storing/retrieving passphrases
   - Used in daemon for encrypted SSH keys

5. **SSE Event Handling**:
   - Tray: [crates/tray/src/daemon_monitor.rs](crates/tray/src/daemon_monitor.rs) - Already calls notifications
   - GUI: [crates/gui-gtk/src/ui/event_handler.rs](crates/gui-gtk/src/ui/event_handler.rs) - Uses in-app toasts only

### What Needs Implementation ✗

1. **Daemon Auto-Reconnect Loop** ([crates/daemon/src/tunnel.rs](crates/daemon/src/tunnel.rs)):
   - Currently connects once, fails permanently on disconnect
   - No retry logic exists
   - Needs reconnection loop with exponential backoff

2. **Auth-Free Detection Logic**:
   - Determine if profile can reconnect without user input
   - Check: key-only auth + (no passphrase OR keychain storage)

3. **GUI Desktop Notifications** ([crates/gui-gtk/src/ui/event_handler.rs](crates/gui-gtk/src/ui/event_handler.rs)):
   - Currently only shows in-app toasts
   - Should also trigger desktop notifications

4. **Notification Action Handling**:
   - Tray notifications have "Reconnect" button but it's not wired
   - Need to handle notification action callbacks

5. **Notification Preferences** (Future enhancement):
   - No configuration for enabling/disabling notifications
   - No quiet hours or notification filtering

## Implementation Plan

### Phase 1: Daemon Auto-Reconnect Logic

**File:** [crates/daemon/src/tunnel.rs](crates/daemon/src/tunnel.rs)

#### 1.1 Add Auto-Reconnect Detection Function

**Location:** After `run_tunnel()` function (~line 300)

```rust
/// Determine if a profile can auto-reconnect without user input
fn can_auto_reconnect_without_auth(profile: &Profile) -> bool {
    // Must have auto-reconnect enabled
    if !profile.options.auto_reconnect {
        return false;
    }

    // Must use key authentication
    if profile.connection.auth_type != AuthType::Key {
        return false;
    }

    // Key must either have no passphrase or use keychain storage
    profile.connection.password_storage == PasswordStorage::Keychain
    // Note: We can't detect unencrypted keys without trying to load them,
    // but that's fine - if loading fails, we'll just mark it as needing auth
}
```

#### 1.2 Implement Reconnection Loop in `run_tunnel()`

**Location:** Replace the single connection attempt in `run_tunnel()` with a retry loop

**Current structure** (~line 270-400):
```rust
pub async fn run_tunnel(
    profile: Profile,
    tunnel_id: Uuid,
    event_tx: broadcast::Sender<TunnelEvent>,
    auth_ctx: Arc<AuthContext>,
    mut shutdown_rx: mpsc::Receiver<()>,
) {
    // Connect once
    // If fails, exit
}
```

**New structure:**
```rust
pub async fn run_tunnel(
    profile: Profile,
    tunnel_id: Uuid,
    event_tx: broadcast::Sender<TunnelEvent>,
    auth_ctx: Arc<AuthContext>,
    mut shutdown_rx: mpsc::Receiver<()>,
) {
    let max_attempts = if profile.options.reconnect_attempts == 0 {
        u32::MAX // unlimited
    } else {
        profile.options.reconnect_attempts
    };

    let mut attempt = 0;
    let can_auto_reconnect = can_auto_reconnect_without_auth(&profile);

    loop {
        attempt += 1;

        // Emit reconnecting event if this is a retry
        if attempt > 1 {
            let _ = event_tx.send(TunnelEvent::Reconnecting {
                id: tunnel_id,
                attempt
            });
            info!("Reconnecting tunnel {} (attempt {}/{})",
                  profile.metadata.name, attempt, max_attempts);
        }

        // Attempt connection
        match connect_and_forward(&profile, tunnel_id, &event_tx, &auth_ctx, &mut shutdown_rx).await {
            Ok(()) => {
                // Clean disconnect (user stop or graceful shutdown)
                info!("Tunnel {} disconnected cleanly", profile.metadata.name);
                break;
            }
            Err(e) => {
                error!("Tunnel {} failed: {}", profile.metadata.name, e);

                // Check if we should retry
                if !can_auto_reconnect {
                    info!("Auto-reconnect not available (needs user auth)");
                    break;
                }

                if attempt >= max_attempts {
                    info!("Max reconnection attempts reached ({}/{})", attempt, max_attempts);
                    break;
                }

                // Wait before retry with exponential backoff
                let delay = profile.options.reconnect_delay * (2_u64.pow((attempt - 1).min(5)));
                info!("Waiting {}s before reconnection attempt...", delay);

                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(delay)) => {}
                    _ = shutdown_rx.recv() => {
                        info!("Shutdown requested during reconnection delay");
                        break;
                    }
                }
            }
        }
    }

    // Final cleanup
    let _ = event_tx.send(TunnelEvent::Disconnected {
        id: tunnel_id,
        reason: "Connection closed".to_string(),
    });
}
```

#### 1.3 Extract Connection Logic to `connect_and_forward()`

**Location:** New function after `run_tunnel()`

Extract the current connection logic from `run_tunnel()` into a separate function that:
- Returns `Ok(())` on clean disconnect
- Returns `Err(anyhow::Error)` on connection failure
- Handles all SSH connection, auth, and forwarding
- Monitors forwarding until disconnect or failure

```rust
async fn connect_and_forward(
    profile: &Profile,
    tunnel_id: Uuid,
    event_tx: &broadcast::Sender<TunnelEvent>,
    auth_ctx: &Arc<AuthContext>,
    shutdown_rx: &mut mpsc::Receiver<()>,
) -> Result<()> {
    // All current connection logic here
    // Return Ok(()) on clean disconnect
    // Return Err() on failure
}
```

#### 1.4 Add `TunnelEvent::Reconnecting`

**Location:** [crates/daemon/src/tunnel.rs:33-39](crates/daemon/src/tunnel.rs#L33-L39)

```rust
#[derive(Debug, Clone)]
pub enum TunnelEvent {
    Starting { id: Uuid },
    Connected { id: Uuid },
    Reconnecting { id: Uuid, attempt: u32 },  // ADD THIS
    Disconnected { id: Uuid, reason: String },
    Error { id: Uuid, error: String },
    AuthRequired { id: Uuid, request: AuthRequest },
}
```

#### 1.5 Update Event Broadcasting in API

**File:** [crates/daemon/src/api.rs](crates/daemon/src/api.rs)

**Location:** SSE event conversion (~line 400-450)

Add handling for `TunnelEvent::Reconnecting` to convert to SSE `OutgoingEvent`:

```rust
TunnelEvent::Reconnecting { id, attempt } => OutgoingEvent::Reconnecting {
    id,
    attempt
},
```

**Also update `OutgoingEvent` enum** (~line 50):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutgoingEvent {
    Starting { id: Uuid },
    Connected { id: Uuid },
    Reconnecting { id: Uuid, attempt: u32 },  // ADD THIS
    Disconnected { id: Uuid, reason: String },
    Error { id: Uuid, error: String },
    AuthRequired { id: Uuid, request: AuthRequest },
    Heartbeat { timestamp: DateTime<Utc> },
}
```

### Phase 2: GUI Desktop Notifications

**File:** [crates/gui-gtk/src/ui/event_handler.rs](crates/gui-gtk/src/ui/event_handler.rs)

#### 2.1 Add notify-rust Dependency

**File:** [crates/gui-gtk/Cargo.toml](crates/gui-gtk/Cargo.toml)

```toml
# Add to dependencies section (after line 71):
notify-rust = { workspace = true }
```

#### 2.2 Create Notifications Module

**New File:** `crates/gui-gtk/src/ui/notifications.rs`

```rust
use notify_rust::{Notification, Timeout};
use uuid::Uuid;

/// Show desktop notification when tunnel disconnects
pub fn show_disconnect_notification(profile_name: &str, reason: &str, profile_id: Uuid) {
    if let Err(e) = Notification::new()
        .summary(&format!("Tunnel '{}' Disconnected", profile_name))
        .body(reason)
        .icon("network-offline")
        .timeout(Timeout::Milliseconds(10000))
        .action("reconnect", "Reconnect")
        .hint(notify_rust::Hint::Category("network.disconnected".into()))
        .hint(notify_rust::Hint::Custom("profile-id".into(), profile_id.to_string().into()))
        .show()
    {
        eprintln!("Failed to show disconnect notification: {}", e);
    }
}

/// Show desktop notification when tunnel encounters an error
pub fn show_error_notification(profile_name: &str, error: &str) {
    if let Err(e) = Notification::new()
        .summary(&format!("Tunnel '{}' Error", profile_name))
        .body(error)
        .icon("dialog-error")
        .timeout(Timeout::Milliseconds(5000))
        .show()
    {
        eprintln!("Failed to show error notification: {}", e);
    }
}

/// Show desktop notification when tunnel successfully connects
pub fn show_connected_notification(profile_name: &str) {
    if let Err(e) = Notification::new()
        .summary(&format!("Tunnel '{}' Connected", profile_name))
        .body("SSH tunnel connection established")
        .icon("network-transmit-receive")
        .timeout(Timeout::Milliseconds(3000))
        .show()
    {
        eprintln!("Failed to show connected notification: {}", e);
    }
}
```

#### 2.3 Wire Notifications to Event Handler

**File:** [crates/gui-gtk/src/ui/event_handler.rs](crates/gui-gtk/src/ui/event_handler.rs)

**Add module import** (after line 10):
```rust
mod notifications;
```

**Update `handle_status_changed()`** (~line 64-77 for Disconnected case):
```rust
TunnelStatus::Disconnected => {
    status_banner.set_title("Disconnected");
    status_banner.add_css_class("info");
    start_button.set_sensitive(true);
    stop_button.set_sensitive(false);

    // ADD: Desktop notification
    if let Some(profile) = app_core.profiles().iter().find(|p| p.metadata.id == profile_id) {
        notifications::show_disconnect_notification(
            &profile.metadata.name,
            "Connection closed",
            profile_id
        );
    }
}
```

**Update `handle_error()`** (~line 164-179):
```rust
pub fn handle_error(/* ... */) {
    // Existing toast code...

    // ADD: Desktop notification
    if let Some(profile) = app_core.profiles().iter().find(|p| p.metadata.id == profile_id) {
        notifications::show_error_notification(&profile.metadata.name, error);
    }
}
```

**Add notification for Connected events** in `window.rs` event loop (~line 475):
```rust
TunnelEvent::Connected { id } => {
    // Existing code...

    // ADD: Desktop notification
    if let Some(profile) = app_core.profiles().iter().find(|p| p.metadata.id == id) {
        crate::ui::notifications::show_connected_notification(&profile.metadata.name);
    }
}
```

#### 2.4 Update mod.rs to Export Notifications

**File:** [crates/gui-gtk/src/ui/mod.rs](crates/gui-gtk/src/ui/mod.rs)

Add:
```rust
pub mod notifications;
```

### Phase 3: Notification Action Handling (Tray)

**File:** [crates/tray/src/tray.rs](crates/tray/src/tray.rs)

#### 3.1 Handle Notification Actions

The tray app already shows disconnect notifications with a "Reconnect" button, but the action isn't handled. We need to:

**Add notification action listener** (after tray initialization ~line 200):

```rust
// Subscribe to notification actions (D-Bus signal)
// Note: notify-rust doesn't directly support action callbacks
// We need to handle this via D-Bus or polling

// Option 1: Use D-Bus directly to listen for action signals
// Option 2: Store pending reconnect requests and poll

// For simplicity, we'll implement a signal handler
use std::sync::Mutex;
static PENDING_RECONNECTS: Mutex<Vec<Uuid>> = Mutex::new(Vec::new());

// When notification is shown, store the profile_id
// When user clicks "Reconnect", the profile_id will be available
```

**Note:** `notify-rust` doesn't provide direct action callbacks. We have two options:

1. **Use D-Bus directly** to listen for `ActionInvoked` signals from the notification daemon
2. **Simplified approach:** Add a "Reconnect" menu item in the tray for recently disconnected tunnels

**Recommended: Simplified Approach**

Update tray menu to include reconnect options for recently failed tunnels:

```rust
// In build_menu() or update_menu()
if let Some(failed) = recently_disconnected_tunnels.get(&profile_id) {
    menu.add_item(&format!("↻ Reconnect '{}'", failed.name), move || {
        // Call start_tunnel API
        start_tunnel_from_tray(profile_id);
    });
}
```

### Phase 4: SSE Event Updates

#### 4.1 Update Common SSE Types

**File:** [crates/common/src/sse.rs](crates/common/src/sse.rs)

**Add `Reconnecting` variant** to `TunnelEvent` enum (~line 20):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TunnelEvent {
    Starting { id: Uuid },
    Connected { id: Uuid },
    Reconnecting { id: Uuid, attempt: u32 },  // ADD THIS
    Disconnected { id: Uuid, reason: String },
    Error { id: Uuid, error: String },
    AuthRequired { id: Uuid, request: AuthRequest },
    Heartbeat { timestamp: DateTime<Utc> },
}
```

#### 4.2 Update Tray Event Handler

**File:** [crates/tray/src/daemon_monitor.rs](crates/tray/src/daemon_monitor.rs)

**Add Reconnecting case** in event handler (~line 210):

```rust
TunnelEvent::Reconnecting { id, attempt } => {
    info!("Tunnel {} reconnecting (attempt {})", id, attempt);

    if let Some(profile) = state.find_profile_by_tunnel_id(id) {
        // Optional: Show notification for reconnection attempts
        if attempt == 1 {
            notifications::show_info_notification(
                &format!("Reconnecting '{}'...", profile.name),
                "Attempting to restore tunnel connection"
            );
        }
    }
}
```

#### 4.3 Update GUI Event Handler

**File:** [crates/gui-gtk/src/ui/window.rs](crates/gui-gtk/src/ui/window.rs)

**Add Reconnecting case** in SSE event loop (~line 485):

```rust
TunnelEvent::Reconnecting { id, attempt } => {
    info!("Tunnel {} reconnecting (attempt {})", id, attempt);

    // Update status to show reconnection
    if let Some(profiles_list) = weak_profiles_list.upgrade() {
        profiles_list.update_tunnel_status(id, TunnelStatus::Reconnecting);
    }

    // Update profile details if viewing this profile
    if app_core.selected_profile_id() == Some(id) {
        event_handler::handle_status_changed(
            &app_core,
            id,
            TunnelStatus::Reconnecting,
            // ... other params
        );
    }
}
```

### Phase 5: UI Updates for Reconnecting Status

#### 5.1 Update Status Banner Styling

**File:** [crates/gui-gtk/src/ui/event_handler.rs](crates/gui-gtk/src/ui/event_handler.rs)

**Add Reconnecting case** in `handle_status_changed()` (~line 50):

```rust
TunnelStatus::Reconnecting => {
    status_banner.set_title("Reconnecting...");
    status_banner.add_css_class("warning");
    start_button.set_sensitive(false);
    stop_button.set_sensitive(true);  // Allow canceling reconnect
}
```

#### 5.2 Update Profile List Status Display

**File:** [crates/gui-gtk/src/ui/profiles_list.rs](crates/gui-gtk/src/ui/profiles_list.rs)

**Add Reconnecting status** in status label update (~line 250):

```rust
TunnelStatus::Reconnecting => {
    status_label.set_text("Reconnecting...");
    status_label.add_css_class("status-reconnecting");
}
```

#### 5.3 Add CSS Styling for Reconnecting

**File:** [crates/gui-gtk/src/ui/style.rs](crates/gui-gtk/src/ui/style.rs)

```css
.status-reconnecting {
    color: @warning_color;
    font-weight: bold;
}
```

## Testing Plan

### Unit Tests

1. **Auto-Reconnect Detection** (`tunnel.rs`):
   - Test `can_auto_reconnect_without_auth()` with various profile configs
   - Key auth + no passphrase → true
   - Key auth + keychain → true
   - Password auth → false
   - Key auth + no keychain → false

2. **Reconnection Logic** (`tunnel.rs`):
   - Test retry loop with max attempts
   - Test exponential backoff delays
   - Test clean disconnect exits loop
   - Test shutdown signal during retry

### Integration Tests

1. **Daemon Auto-Reconnect**:
   - Start tunnel with auto_reconnect=true
   - Kill SSH connection
   - Verify daemon attempts reconnection
   - Verify reconnect count respects max_attempts

2. **Desktop Notifications**:
   - Trigger disconnect event
   - Verify desktop notification appears
   - Verify notification shows correct profile name and reason

3. **GUI Event Handling**:
   - Disconnect tunnel
   - Verify GUI shows reconnecting status
   - Verify desktop notification appears
   - Verify reconnection attempts display

### Manual Testing Checklist

- [ ] Daemon reconnects automatically when network drops
- [ ] Daemon respects reconnect_attempts limit
- [ ] Daemon respects reconnect_delay timing
- [ ] Desktop notifications show on disconnect
- [ ] Desktop notifications show on error
- [ ] Desktop notifications show on successful connect
- [ ] GUI shows "Reconnecting..." status
- [ ] CLI watch shows reconnecting events
- [ ] Tray icon updates during reconnection
- [ ] Auth-required profiles don't auto-reconnect
- [ ] Key-only profiles auto-reconnect successfully

## Files to Modify

### Core Implementation

1. **[crates/daemon/src/tunnel.rs](crates/daemon/src/tunnel.rs)**
   - Add `can_auto_reconnect_without_auth()`
   - Implement reconnection loop in `run_tunnel()`
   - Extract `connect_and_forward()` function
   - Add `TunnelEvent::Reconnecting` variant

2. **[crates/daemon/src/api.rs](crates/daemon/src/api.rs)**
   - Add `OutgoingEvent::Reconnecting` handling
   - Update SSE event conversion

3. **[crates/common/src/sse.rs](crates/common/src/sse.rs)**
   - Add `TunnelEvent::Reconnecting` to SSE types

### GUI Notifications

4. **[crates/gui-gtk/src/ui/notifications.rs](crates/gui-gtk/src/ui/notifications.rs)** (NEW)
   - Create notification module
   - Implement desktop notification functions

5. **[crates/gui-gtk/src/ui/event_handler.rs](crates/gui-gtk/src/ui/event_handler.rs)**
   - Wire notifications to disconnect events
   - Wire notifications to error events
   - Add Reconnecting status handling

6. **[crates/gui-gtk/src/ui/window.rs](crates/gui-gtk/src/ui/window.rs)**
   - Handle `TunnelEvent::Reconnecting` in SSE loop
   - Add connected notification

7. **[crates/gui-gtk/src/ui/mod.rs](crates/gui-gtk/src/ui/mod.rs)**
   - Export notifications module

8. **[crates/gui-gtk/Cargo.toml](crates/gui-gtk/Cargo.toml)**
   - Add notify-rust dependency

### Tray Updates

9. **[crates/tray/src/daemon_monitor.rs](crates/tray/src/daemon_monitor.rs)**
   - Handle `TunnelEvent::Reconnecting` events

10. **[crates/tray/src/tray.rs](crates/tray/src/tray.rs)** (Optional)
    - Add reconnect menu items for failed tunnels

### UI Polish

11. **[crates/gui-gtk/src/ui/profiles_list.rs](crates/gui-gtk/src/ui/profiles_list.rs)**
    - Add Reconnecting status display

12. **[crates/gui-gtk/src/ui/style.rs](crates/gui-gtk/src/ui/style.rs)**
    - Add CSS for reconnecting status

## Implementation Order

1. **Phase 1** (Core functionality):
   - Daemon auto-reconnect loop
   - SSE event updates
   - ~4-6 hours

2. **Phase 2** (GUI notifications):
   - Desktop notification integration
   - Event handler wiring
   - ~2-3 hours

3. **Phase 3** (UI polish):
   - Reconnecting status display
   - CSS styling
   - ~1-2 hours

4. **Phase 4** (Testing):
   - Unit tests
   - Integration tests
   - Manual testing
   - ~3-4 hours

**Total Estimate: 10-15 hours**

## Future Enhancements (Not in v0.2.0)

1. **Notification Preferences**:
   - Enable/disable notifications per event type
   - Quiet hours configuration
   - Sound settings

2. **Advanced Reconnection**:
   - Network change detection
   - VPN-aware reconnection
   - Connection quality monitoring

3. **Notification Action Handling**:
   - D-Bus action callbacks for "Reconnect" button
   - Quick reconnect from notification

4. **Reconnection Strategy**:
   - Jittered exponential backoff
   - Circuit breaker pattern
   - Health check before reconnect

## Success Criteria

- [ ] Tunnels with auto_reconnect=true automatically reconnect after network interruption
- [ ] Desktop notifications appear for disconnect, error, and connect events
- [ ] Reconnection attempts are visible in GUI with "Reconnecting..." status
- [ ] Reconnection respects configured limits (attempts, delay)
- [ ] Tunnels requiring user auth do NOT attempt auto-reconnect
- [ ] All existing functionality continues to work (no regressions)
- [ ] Documentation updated with auto-reconnect behavior
