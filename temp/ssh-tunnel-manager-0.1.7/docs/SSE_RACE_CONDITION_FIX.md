# SSE Race Condition Fix - Missing Error Events

## Problem

When starting tunnels that fail quickly (e.g., connection refused), the second and subsequent CLI invocations would not receive error events from the daemon. Instead, they would show:

```
Starting tunnel 'test2' (c2ba51db-a5a4-43bf-ad9f-6be04e8c9665)
Event stream error: error decoding response body
Error: Event stream closed and tunnel status unknown
```

While the daemon logs clearly showed:

```
2025-12-16T11:13:48.076216Z  INFO ssh_tunnel_daemon: Tunnel event: Starting { id: c2ba51db... }
2025-12-16T11:13:48.076648Z ERROR ssh_tunnel_daemon::tunnel: Tunnel c2ba51db... failed: Failed to connect...
2025-12-16T11:13:48.076714Z  INFO ssh_tunnel_daemon: Tunnel event: Error { id: c2ba51db..., error: "Failed to connect..." }
```

The first CLI invocation would correctly show:
```
Error: Tunnel failed: Failed to connect to 192.168.122.217:22: Connection refused (os error 111)
```

But subsequent invocations would miss the `Error` event entirely.

## Root Cause

### Race Condition

The issue was a **race condition** between SSE subscription and event broadcasting:

1. **Old Flow:**
   ```
   Client: Send POST /api/tunnels/{id}/start
   Daemon: Broadcast "Starting" event
   Daemon: Spawn tunnel task (returns immediately)
   Client: Receives 200 OK from start request
   Client: Subscribes to GET /api/events (SSE)
   [Background] Tunnel task fails immediately
   [Background] Daemon broadcasts "Error" event ← MISSED!
   Client: SSE subscription establishes
   Client: Never receives the Error event (already broadcast)
   ```

2. **Why it worked for the first invocation:**
   - Fresh daemon state, no recent events in broadcast buffer
   - Slower timing allowed SSE to establish before error

3. **Why it failed for subsequent invocations:**
   - Broadcast channel has 100-event buffer
   - Multiple rapid start/fail cycles fill the buffer
   - When new client subscribes, it's already "lagged" behind
   - Lagged events get dropped silently by `BroadcastStream`

### Broadcast Channel Lag

From [api.rs:322-327](crates/daemon/src/api.rs#L322-L327):

```rust
Err(lagged) => {
    // We lagged behind in the broadcast channel
    // This happens when events are broadcast faster than this client can consume them
    // Continue processing - the client will catch up with future events
    tracing::debug!("Event stream lagged by {} events, continuing", lagged.0);
    None
}
```

The `RecvError::Lagged` error from the broadcast channel indicates that events were sent faster than the receiver could consume them. The receiver skips the lagged events and continues with newer events. **Critical error events can be lost this way.**

## Solution

### Subscribe to SSE BEFORE Sending Start Request

The fix ensures the SSE subscription is fully established **before** the start request is sent, guaranteeing the client can receive all events from the tunnel lifecycle.

#### Changes to `start_tunnel_with_events()` ([daemon_client.rs:299-419](crates/common/src/daemon_client.rs#L299-L419))

**New Flow:**

1. ✅ Spawn SSE subscription task (connects to `/api/events`)
2. ✅ Wait for SSE connection to be established (with 5-second timeout)
3. ✅ Send POST `/api/tunnels/{id}/start` request
4. ✅ Daemon broadcasts events (client is already listening)
5. ✅ Client receives all events in order

**Implementation:**

```rust
pub async fn start_tunnel_with_events<H: TunnelEventHandler>(
    client: &Client,
    config: &DaemonClientConfig,
    tunnel_id: Uuid,
    handler: &mut H,
) -> Result<()> {
    let base_url = config.daemon_base_url()?;

    // Subscribe to SSE events BEFORE sending start request
    // This ensures we don't miss any events that fire immediately after the tunnel starts
    let client_for_events = client.clone();
    let config_for_events = config.clone();
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (sse_ready_tx, mut sse_ready_rx) = tokio::sync::mpsc::channel::<Result<()>>(1);

    tokio::spawn(async move {
        // ... connect to /api/events ...

        if !resp.status().is_success() {
            let err = anyhow::anyhow!(...);
            let _ = sse_ready_tx.send(Err(anyhow::anyhow!("{}", err))).await;
            let _ = event_tx.send(Err(err));
            return;
        }

        // SSE connection established - signal ready
        let _ = sse_ready_tx.send(Ok(())).await;

        // ... process events ...
    });

    // Wait for SSE connection to be ready (with timeout)
    match tokio::time::timeout(Duration::from_secs(5), sse_ready_rx.recv()).await {
        Ok(Some(Ok(()))) => {
            // SSE connection established, proceed with start request
        }
        Ok(Some(Err(e))) => {
            anyhow::bail!("Failed to establish SSE connection: {}", e);
        }
        Ok(None) | Err(_) => {
            anyhow::bail!("Timed out waiting for SSE connection to establish");
        }
    }

    // Now send start request (SSE is ready to receive events)
    let url = format!("{}/api/tunnels/{}/start", base_url, tunnel_id);
    let resp = add_auth_header(client.post(&url), config)?
        .send()
        .await
        .context("Failed to send start request to daemon. Is the daemon running?")?;

    // ... handle response and wait for events ...
}
```

**Key Components:**

1. **`sse_ready_tx/sse_ready_rx` channel** - Signals when SSE connection is established
2. **Line 338:** Signal sent after successful SSE response (status 200)
3. **Lines 395-406:** Main thread waits for SSE ready signal (5-second timeout)
4. **Lines 408-419:** Start request sent only after SSE is confirmed ready

## Event Flow Timeline

### Before Fix

```
T+0ms:  CLI sends POST /start
T+1ms:  Daemon broadcasts "Starting"
T+2ms:  Daemon spawns tunnel task, returns 200 OK
T+3ms:  CLI receives 200 OK
T+4ms:  CLI begins connecting to GET /events
T+5ms:  Tunnel task fails, daemon broadcasts "Error" ❌ MISSED
T+10ms: CLI SSE connection established (too late)
T+20ms: CLI times out, shows "Event stream closed"
```

### After Fix

```
T+0ms:  CLI begins connecting to GET /events
T+10ms: CLI SSE connection established ✅
T+11ms: CLI sends POST /start
T+12ms: Daemon broadcasts "Starting" ✅ Received
T+13ms: Daemon spawns tunnel task, returns 200 OK
T+14ms: Tunnel task fails, daemon broadcasts "Error" ✅ Received
T+15ms: CLI shows correct error message
```

## Benefits

1. **Reliable event delivery** - No race condition between subscription and event broadcasting
2. **Consistent behavior** - First and subsequent invocations behave identically
3. **Fail-fast** - SSE connection issues are detected before sending start request
4. **Timeout protection** - 5-second timeout prevents hanging if SSE fails to connect
5. **Better error messages** - Users see actual errors instead of "stream closed"

## Testing

To test the fix:

1. **Setup:** Start the daemon and have a profile configured to connect to an unreachable server
   ```bash
   ssh-tunnel-daemon &
   ```

2. **Test rapid failures:**
   ```bash
   # Run multiple times in quick succession
   ssh-tunnel start test1  # Connection refused
   ssh-tunnel start test2  # Connection refused
   ssh-tunnel start test3  # Connection refused
   ```

3. **Expected behavior (all invocations):**
   ```
   Starting tunnel 'test1' (...)
   Error: Tunnel failed: Failed to connect to 192.168.122.217:22: Connection refused (os error 111)
   ```

4. **Test port conflicts:**
   ```bash
   ssh-tunnel start profile1  # Uses port 4443, succeeds
   ssh-tunnel start profile2  # Also uses port 4443, should fail
   ```

   Second should show:
   ```
   Error: Tunnel failed: Failed to bind to 127.0.0.1:4443: Address already in use (os error 98)
   ```

## Related Issues Fixed

This fix complements the [Daemon Event Ordering Fix](DAEMON_EVENT_ORDERING_FIX.md) which ensured the daemon sends `Connected` events **after** successful port binding. Together, these fixes ensure:

1. ✅ Events are sent in the correct order (daemon-side)
2. ✅ Events are reliably received (client-side)
3. ✅ Error messages are accurate and immediate
4. ✅ No false-positive "connected" messages
5. ✅ No missing error events on rapid retries
