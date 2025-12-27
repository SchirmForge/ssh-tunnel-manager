// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Profile create/edit dialog for Qt
//!
//! Demonstrates maximum code reuse from gui-core:
//! - validate_profile() - SHARED
//! - save_profile() - SHARED
//! - profile_name_exists() - SHARED
//! Only the Qt widget rendering is Qt-specific

use std::rc::Rc;
use crate::AppState;
use ssh_tunnel_common::config::Profile;

// TODO: Add Qt imports
// use qt_widgets::{QDialog, QFormLayout, QLineEdit, QSpinBox, QComboBox, QPushButton};

/// Create profile dialog (new or edit)
///
/// Code reuse demonstration:
/// - Validation: gui-core::validate_profile() - SHARED
/// - Save: gui-core::save_profile() - SHARED
/// - UI rendering: Qt widgets - Qt-SPECIFIC
pub fn create(_state: Rc<AppState>, _profile: Option<Profile>) {
    // TODO: Implement Qt profile dialog
    //
    // let dialog = QDialog::new();
    // dialog.set_window_title(if profile.is_some() { "Edit Profile" } else { "New Profile" });
    //
    // let layout = QFormLayout::new();
    //
    // // Profile Name
    // let name_edit = QLineEdit::new();
    // if let Some(ref p) = profile {
    //     name_edit.set_text(&QString::from(&p.metadata.name));
    // }
    // layout.add_row("Profile Name", name_edit);
    //
    // // SSH Host
    // let host_edit = QLineEdit::new();
    // layout.add_row("SSH Host", host_edit);
    //
    // // SSH Port
    // let port_spin = QSpinBox::new();
    // port_spin.set_range(1, 65535);
    // port_spin.set_value(22);
    // layout.add_row("SSH Port", port_spin);
    //
    // // ... more fields ...
    //
    // // Save button
    // let save_button = QPushButton::new("Save");
    // save_button.clicked.connect(|| {
    //     let profile = build_profile_from_form();
    //
    //     // Use gui-core validation (SHARED CODE!)
    //     if let Err(e) = ssh_tunnel_gui_core::validate_profile(&profile) {
    //         show_error_dialog(&format!("Validation error: {}", e));
    //         return;
    //     }
    //
    //     // Check for duplicate names (SHARED CODE!)
    //     if ssh_tunnel_gui_core::profile_name_exists(&profile.metadata.name, Some(profile.metadata.id)) {
    //         show_error_dialog("A profile with this name already exists");
    //         return;
    //     }
    //
    //     // Save profile (SHARED CODE!)
    //     match ssh_tunnel_gui_core::save_profile(&profile, true) {
    //         Ok(_) => {
    //             println!("✓ Profile saved successfully");
    //             dialog.accept();
    //         }
    //         Err(e) => show_error_dialog(&format!("Failed to save: {}", e)),
    //     }
    // });
    //
    // layout.add_widget(save_button);
    // dialog.set_layout(layout);
    // dialog
}

// fn build_profile_from_form() -> Profile {
//     // Collect values from Qt widgets and build Profile struct
//     // Profile struct is from ssh-tunnel-common (SHARED)
// }
//
// fn show_error_dialog(message: &str) {
//     // QMessageBox::critical(...)
// }
