# Authentication Dialog Testing Procedure

This document provides a comprehensive test procedure for the SSH authentication dialog functionality in the GUI.

## Context

This testing procedure validates the fix for authentication dialog issues including:
- Multiple dialogs appearing when canceling
- Unresponsive/frozen dialog buttons and text input
- 60-second timeout on cancellation
- SSH retry behavior causing duplicate dialogs

## Implementation Details

The fix uses GTK's traditional nested `MainLoop` pattern:
- Synchronous `TunnelEventHandler::on_auth_required()` trait method
- `glib::MainLoop::new()` creates a nested event loop for modal dialog behavior
- `main_loop.run()` blocks but continues processing GTK events (keeps UI responsive)
- Dialog callback stores response in `Arc<Mutex>` and quits the nested loop
- Cancellation flag prevents showing subsequent auth dialogs from SSH retry attempts

## Test Scenarios

### Test 1: Single Auth with Correct Password

**Setup**: SSH tunnel profile with password-protected key or host

**Steps**:
1. Click "Start" button in GUI
2. Wait for authentication dialog to appear
3. Enter the correct password in the dialog
4. Press Enter or click "Submit"

**Expected Behavior**:
- ✅ Dialog appears once
- ✅ Dialog is responsive (text input works, buttons respond immediately)
- ✅ After submitting password, tunnel connects successfully
- ✅ Status changes to "Connected"
- ✅ **NO second dialog appears**
- ✅ No GUI freezing or delays

**Expected Logs**:
```
[AUTH] on_auth_required called for tunnel <uuid>
[AUTH] Prompt: <prompt text>
[AUTH] Hidden: true
[AUTH] Checking cancellation flag: false
[AUTH] Creating response storage and nested main loop
[AUTH] Showing auth dialog on main thread
[AUTH] Waiting for dialog response (main_loop.run())...
[AUTH] Dialog callback invoked
[AUTH] Result type: Some(text)
[AUTH] Response length: X chars
[AUTH] Response stored in Arc<Mutex>
[AUTH] User provided input, not setting cancelled flag
[AUTH] Quitting nested main loop
[AUTH] Main loop exited, extracting response
[AUTH] Response extracted from storage
[AUTH] Returning Ok with X chars of text
✓ Tunnel connected! Forwarding ...
```

---

### Test 2: Single Auth with Cancel ⚠️ CRITICAL TEST

**Setup**: SSH tunnel profile with password-protected key or host

**Steps**:
1. Click "Start" button in GUI
2. Wait for authentication dialog to appear
3. Click "Cancel" button (or press Escape)

**Expected Behavior**:
- ✅ Dialog appears once
- ✅ Dialog is responsive
- ✅ After clicking Cancel, dialog closes immediately
- ✅ Status updates to "Disconnected" or "Error"
- ✅ **NO second dialog appears** (this was the major bug)
- ✅ **NO 60-second timeout** (clean shutdown)
- ✅ GUI shows tunnel as stopped

**Expected Logs**:
```
[AUTH] on_auth_required called for tunnel <uuid>
[AUTH] Prompt: <prompt text>
[AUTH] Hidden: true
[AUTH] Checking cancellation flag: false
[AUTH] Creating response storage and nested main loop
[AUTH] Showing auth dialog on main thread
[AUTH] Waiting for dialog response (main_loop.run())...
[AUTH] Dialog callback invoked
[AUTH] Result type: None (cancelled)
[AUTH] Response stored in Arc<Mutex>
[AUTH] Setting cancelled flag to true
[AUTH] Quitting nested main loop
[AUTH] Main loop exited, extracting response
[AUTH] Response extracted from storage
[AUTH] Returning error: Authentication was cancelled
✗ Failed to start tunnel: Authentication was cancelled
Stopping tunnel due to cancelled authentication...
```

**Critical Check**: Watch for subsequent auth request. If SSH retries with different method, should see:
```
[AUTH] on_auth_required called for tunnel <uuid>
[AUTH] Prompt: <different prompt>
[AUTH] Hidden: <possibly different>
[AUTH] Checking cancellation flag: true
[AUTH] Auth already cancelled, rejecting subsequent auth request
```

---

### Test 3: Multiple Rapid Start Attempts

**Setup**: SSH tunnel profile (any auth type)

**Steps**:
1. Click "Start" button
2. Wait for auth dialog to appear
3. Enter password and submit
4. As soon as tunnel connects, click "Stop"
5. Immediately click "Start" again (rapid retry)
6. Repeat 3-4 times in quick succession

**Expected Behavior**:
- ✅ Each start shows a fresh auth dialog
- ✅ All dialogs are responsive
- ✅ No freezing or hanging
- ✅ Each dialog works independently
- ✅ No duplicate dialogs
- ✅ Cancellation from previous attempt doesn't affect new attempt

**Expected Logs**:
Each attempt should show fresh handler instance with `cancelled: false` at start.

---

### Test 4: Cancel Then Immediate Retry

**Setup**: SSH tunnel profile with auth

**Steps**:
1. Click "Start" button
2. Auth dialog appears
3. Click "Cancel"
4. Immediately click "Start" again

**Expected Behavior**:
- ✅ First dialog appears and cancels cleanly
- ✅ Second start creates **new handler instance** with fresh state
- ✅ Second dialog appears and is fully functional
- ✅ Second attempt is independent of first

**Expected Logs**:
Second start should show:
```
[AUTH] on_auth_required called for tunnel <uuid>
[AUTH] Checking cancellation flag: false  <-- Fresh handler, not cancelled
```

---

### Test 5: SSH Retry Scenario (Multiple Auth Methods) ⚠️ CRITICAL TEST

**Setup**: SSH server configured with multiple auth methods (e.g., publickey first, then password)

**Steps**:
1. Click "Start" button
2. First auth dialog appears (e.g., for key passphrase)
3. Click "Cancel"

**Expected Behavior**:
- ✅ First dialog appears
- ✅ User clicks Cancel
- ✅ First dialog closes
- ✅ **Second auth request is rejected automatically** (SSH tried next method)
- ✅ **NO second dialog appears to user**
- ✅ Tunnel stops cleanly

**Expected Logs**:
```
[AUTH] on_auth_required called for tunnel <uuid>
[AUTH] Prompt: Enter passphrase for key...
[AUTH] Checking cancellation flag: false
... [dialog shown and cancelled] ...
[AUTH] Setting cancelled flag to true
[AUTH] Returning error: Authentication was cancelled

[Within same connection attempt, SSH retries:]
[AUTH] on_auth_required called for tunnel <uuid>
[AUTH] Prompt: Password:
[AUTH] Checking cancellation flag: true
[AUTH] Auth already cancelled, rejecting subsequent auth request
```

---

### Test 6: Dialog Responsiveness Test ⚠️ CRITICAL TEST

**Setup**: Any SSH tunnel with auth

**Steps**:
1. Click "Start"
2. Dialog appears
3. Test typing in the text field
4. Test hovering over buttons (visual feedback)
5. Test clicking Submit button
6. Test clicking Cancel button
7. Test pressing Enter key
8. Test pressing Escape key

**Expected Behavior**:
- ✅ Text input field accepts input immediately
- ✅ Characters appear as typed (no lag)
- ✅ Buttons show hover effects
- ✅ Buttons respond to clicks immediately (no delay)
- ✅ Enter key submits
- ✅ Escape key cancels
- ✅ **NO freezing or unresponsiveness**

---

### Test 7: Wrong Password Then Retry

**Setup**: SSH tunnel with password auth

**Steps**:
1. Click "Start"
2. Enter wrong password
3. Click "Submit"
4. SSH rejects, may request auth again
5. Enter correct password this time

**Expected Behavior**:
- ✅ First auth attempt with wrong password fails
- ✅ SSH may request auth again (depends on server config)
- ✅ Each dialog is responsive
- ✅ Eventually either connects or fails cleanly

**Note**: This tests the handler across multiple auth attempts within same connection.

---

### Test 8: Concurrent Tunnel Starts

**Setup**: Multiple SSH tunnel profiles

**Steps**:
1. Start tunnel 1 (dialog appears)
2. While dialog 1 is open, start tunnel 2 from different profile
3. Both dialogs should be independent

**Expected Behavior**:
- ✅ Each tunnel has its own handler instance
- ✅ Each dialog operates independently
- ✅ Cancelling one doesn't affect the other
- ✅ No cross-contamination of state

---

## Success Metrics Summary

After all tests:
- **Zero crashes** or panics
- **Zero frozen dialogs** (all buttons and input responsive)
- **Zero duplicate dialogs** when cancelling
- **Zero 60-second timeouts** on cancel
- **Clean shutdown** on every cancel
- **Successful connection** when correct password provided
- **Logs show clear flow** at every step

---

## Debugging Guide

If issues occur, check logs for:

1. **Main loop not quitting**: Missing "Main loop exited" log after callback
   - Indicates callback never fired or `main_loop.quit()` wasn't called
   - Check if dialog was properly shown

2. **Callback not firing**: Missing "Dialog callback invoked" log
   - Dialog might not be wired up correctly
   - Check `show_auth_dialog_with_callback` implementation

3. **Wrong response storage**: Logs show stored but extracted as None
   - Race condition in response storage
   - Check Arc<Mutex> cloning and scoping

4. **Cancellation flag stuck**: Flag is true on fresh handler instance
   - Handler not being recreated properly for new tunnel attempts
   - Check handler instantiation in profile_details.rs and details.rs

5. **Multiple auth requests**: Multiple "on_auth_required called" for same attempt
   - This is normal if SSH server tries multiple auth methods
   - Should see second request rejected if first was cancelled

---

## Log Interpretation

### Successful auth flow:
```
[AUTH] on_auth_required called
[AUTH] Checking cancellation flag: false
[AUTH] Showing auth dialog
[AUTH] Waiting for dialog response
[AUTH] Dialog callback invoked
[AUTH] Result type: Some(text)
[AUTH] Returning Ok with X chars
```

### Cancelled auth flow:
```
[AUTH] on_auth_required called
[AUTH] Checking cancellation flag: false
[AUTH] Showing auth dialog
[AUTH] Waiting for dialog response
[AUTH] Dialog callback invoked
[AUTH] Result type: None (cancelled)
[AUTH] Setting cancelled flag to true
[AUTH] Returning error: Authentication was cancelled
```

### SSH retry (subsequent request after cancel):
```
[AUTH] on_auth_required called
[AUTH] Checking cancellation flag: true
[AUTH] Auth already cancelled, rejecting subsequent auth request
```

---

## Known Limitations

1. **Nested MainLoop Pattern**: This is a traditional GTK pattern that blocks but processes events. It's well-tested in GTK applications but may feel unusual in async Rust code.

2. **SSH Retry Behavior**: Some SSH servers may send multiple auth requests with different methods. The cancellation flag handles this by rejecting subsequent requests after the first cancellation.

3. **Per-Handler Cancellation State**: Each tunnel start creates a new handler with fresh state. This is intentional to allow retry attempts to work independently.

---

## Related Files

Implementation files:
- [crates/common/src/daemon_client.rs](crates/common/src/daemon_client.rs) - Trait definition
- [crates/gui/src/ui/tunnel_handler.rs](crates/gui/src/ui/tunnel_handler.rs) - GTK implementation
- [crates/cli/src/main.rs](crates/cli/src/main.rs) - CLI implementation

Testing locations:
- [crates/gui/src/ui/profile_details.rs](crates/gui/src/ui/profile_details.rs) - Main tunnel start location
- [crates/gui/src/ui/details.rs](crates/gui/src/ui/details.rs) - Alternative tunnel start location
