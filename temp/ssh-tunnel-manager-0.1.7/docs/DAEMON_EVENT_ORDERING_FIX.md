# Daemon Event Ordering Fix - Port Binding Errors

## Problem

When two tunnels tried to bind to the same port, the daemon would:
1. Send `Connected` event to CLI/GUI
2. **Then** attempt to bind the port
3. Fail to bind (port already in use)
4. Send `Error` event

This resulted in the CLI showing "✓ Tunnel connected!" even though the tunnel actually failed.

### Example from Logs

```
2025-12-16T09:19:22.127269Z  INFO ssh_tunnel_daemon: Tunnel event: Connected { id: c2ba51db... }
2025-12-16T09:19:22.127315Z ERROR ssh_tunnel_daemon::tunnel: Tunnel test2 failed: Failed to bind to 127.0.0.1:4443: Address already in use (os error 98)
2025-12-16T09:19:22.127348Z  INFO ssh_tunnel_daemon: Tunnel event: Error { id: c2ba51db..., error: "Failed to bind..." }
```

## Root Cause

In [tunnel.rs:682](crates/daemon/src/tunnel.rs#L682), the `Connected` event was broadcast in `monitor_tunnel()` immediately after SSH authentication succeeded, **before** attempting to bind the local port.

The port binding happened later in `run_local_forward_task()` (lines 1142-1156), which meant any binding errors (like "address already in use") occurred after the client already received the `Connected` event.

## Solution

Moved the `Connected` event broadcast to **after** successful port binding:

### Changes to `monitor_tunnel()` (lines 663-697)

**Before:**
```rust
async fn monitor_tunnel(...) -> Result<()> {
    let id = profile.metadata.id;

    // Update status to connected
    {
        let mut tunnels = tunnels.write().await;
        if let Some(tunnel) = tunnels.get_mut(&id) {
            tunnel.status = TunnelStatus::Connected;
            tunnel.pending_auth = None;
        }
    }
    if let Err(e) = event_tx.send(TunnelEvent::Connected { id }) {
        debug!("Failed to broadcast Connected event for {}: {}", id, e);
    }

    // Run port forwarding based on type (blocks until forwarding ends)
    let forward_result = tokio::select! {
        ...
        result = async {
            match profile.forwarding.forwarding_type {
                ForwardingType::Local => {
                    run_local_forward_task(&session, &profile).await
                }
                ...
            }
        } => result
    };
```

**After:**
```rust
async fn monitor_tunnel(...) -> Result<()> {
    let id = profile.metadata.id;

    // Run port forwarding based on type (blocks until forwarding ends)
    // Note: The Connected event is sent AFTER successful port binding inside the forwarding task
    let forward_result = tokio::select! {
        ...
        result = async {
            match profile.forwarding.forwarding_type {
                ForwardingType::Local => {
                    run_local_forward_task(&session, &profile, tunnels.clone(), event_tx.clone()).await
                }
                ...
            }
        } => result
    };
```

### Changes to `run_local_forward_task()` (lines 1101-1162)

**Added parameters:**
```rust
async fn run_local_forward_task(
    session: &Handle<ClientHandler>,
    profile: &Profile,
    tunnels: Arc<RwLock<HashMap<Uuid, ActiveTunnel>>>,  // NEW
    event_tx: broadcast::Sender<TunnelEvent>,            // NEW
) -> Result<()> {
    let id = profile.metadata.id;
    ...
```

**Added event broadcast after successful port binding:**
```rust
    // Bind local port
    let listener = match TcpListener::bind(bind_addr).await {
        Ok(l) => l,
        Err(e) => {
            // Detect permission errors specifically for privileged ports
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                return Err(anyhow::anyhow!("Permission denied..."));
            }
            return Err(anyhow::anyhow!("Failed to bind to {}: {}", bind_addr, e));
        }
    };

    info!("Listening on {}", bind_addr);

    // Port binding successful! Update status and broadcast Connected event
    {
        let mut tunnels = tunnels.write().await;
        if let Some(tunnel) = tunnels.get_mut(&id) {
            tunnel.status = TunnelStatus::Connected;
            tunnel.pending_auth = None;
        }
    }
    if let Err(e) = event_tx.send(TunnelEvent::Connected { id }) {
        debug!("Failed to broadcast Connected event for {}: {}", id, e);
    }
```

## Event Flow Timeline

### Before Fix

1. SSH authentication succeeds
2. ✅ **`Connected` event sent** ← CLI shows "Tunnel connected!"
3. Attempt to bind port
4. ❌ Port binding fails (address already in use)
5. `Error` event sent ← Too late, CLI already showed success

### After Fix

1. SSH authentication succeeds
2. Attempt to bind port
3. **Two possible outcomes:**
   - **Success:** Port binding succeeds → ✅ `Connected` event sent → CLI shows "Tunnel connected!"
   - **Failure:** Port binding fails → ❌ `Error` event sent → CLI shows error immediately

## Benefits

1. **Accurate status reporting** - CLI/GUI only shows "Connected" when tunnel is truly operational
2. **Immediate error feedback** - Port binding errors are shown to user right away
3. **No false positives** - Eliminates the "looks connected but actually failed" scenario
4. **SSE-first approach** - Uses real-time events correctly, no client-side workarounds needed

## Testing

To test the fix:

1. Start a tunnel on port 4443:
   ```bash
   ssh-tunnel start profile1  # Uses port 4443
   ```

2. Try to start another tunnel on the same port:
   ```bash
   ssh-tunnel start profile2  # Also uses port 4443
   ```

**Expected behavior:**
- First tunnel: Shows "✓ Tunnel connected!"
- Second tunnel: Shows error "Failed to bind to 127.0.0.1:4443: Address already in use" immediately, never shows "connected"

## Future Considerations

When implementing Remote and Dynamic (SOCKS) forwarding:
- Follow the same pattern: broadcast `Connected` only after the forwarding mechanism is fully initialized
- For Remote forwarding: After SSH server successfully sets up the remote listener
- For Dynamic (SOCKS) forwarding: After local SOCKS proxy successfully binds
