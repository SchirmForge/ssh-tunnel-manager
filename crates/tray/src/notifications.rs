// Desktop notifications for tunnel events

use notify_rust::{Notification, Timeout};
use uuid::Uuid;

/// Show notification when a tunnel disconnects
pub fn show_disconnect_notification(profile_name: &str, reason: &str, profile_id: Uuid) {
    let mut notification = Notification::new();
    notification
        .summary(&format!("Tunnel Disconnected: {}", profile_name))
        .body(&format!("Reason: {}\n\nClick to reconnect", reason))
        .icon("network-offline")
        .timeout(Timeout::Milliseconds(10000))
        .action("reconnect", "Reconnect")
        .action("default", "default");

    // Store profile_id in hint for reconnect action
    notification.hint(notify_rust::Hint::Custom(
        "x-tunnel-id".to_string(),
        profile_id.to_string(),
    ));

    if let Err(e) = notification.show() {
        eprintln!("Failed to show notification: {}", e);
    }
}

/// Show notification for tunnel errors
pub fn show_error_notification(profile_name: &str, error: &str) {
    if let Err(e) = Notification::new()
        .summary(&format!("Tunnel Error: {}", profile_name))
        .body(error)
        .icon("dialog-error")
        .timeout(Timeout::Milliseconds(10000))
        .show()
    {
        eprintln!("Failed to show notification: {}", e);
    }
}

/// Show notification when successfully connected
pub fn show_connected_notification(profile_name: &str) {
    if let Err(e) = Notification::new()
        .summary(&format!("Tunnel Connected: {}", profile_name))
        .body("Connection established successfully")
        .icon("network-transmit-receive")
        .timeout(Timeout::Milliseconds(3000))
        .show()
    {
        eprintln!("Failed to show notification: {}", e);
    }
}
