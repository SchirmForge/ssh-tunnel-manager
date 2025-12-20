// GTK dialogs for profile selection and confirmation
// For now, we just launch the GUI for complex selections

use uuid::Uuid;

// These functions are placeholders - the tray will launch the GUI
// for complex operations like selecting profiles

/// Show profile selection dialog
/// For the tray, this is handled by launching the GUI
pub async fn show_profile_selection_dialog() -> Option<Uuid> {
    // Launch GUI instead
    None
}

/// Show tunnel selection dialog (for stopping)
/// For the tray, this is handled by launching the GUI
pub async fn show_tunnel_selection_dialog() -> Option<Uuid> {
    // Launch GUI instead
    None
}

/// Show reconnect confirmation dialog
/// For the tray, this is handled by launching the GUI
pub async fn show_reconnect_dialog(_profile_name: &str, _profile_id: Uuid) -> bool {
    // Launch GUI instead
    false
}
