// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Profiles list widget using Qt
//!
//! Key difference from GTK implementation:
//! - Uses QListWidget or QTableView instead of GTK ListBox
//! - Same ProfileViewModel from gui-core for data
//! - Same business logic for loading/displaying profiles

use std::rc::Rc;
use crate::AppState;
use ssh_tunnel_gui_core::{ProfileViewModel, StatusColor};
use ssh_tunnel_common::TunnelStatus;

// TODO: Add Qt imports
// use qt_widgets::{QListWidget, QListWidgetItem, QPushButton, QVBoxLayout};
// use qt_core::{QVariant, QString};

/// Create the profiles list widget
///
/// This demonstrates code reuse:
/// - Uses gui-core's load_profiles() - SHARED
/// - Uses ProfileViewModel for display data - SHARED
/// - Qt-specific: QListWidget rendering
pub fn create(_state: Rc<AppState>) {
    // TODO: Implement Qt profiles list
    //
    // let widget = QWidget::new();
    // let layout = QVBoxLayout::new();
    //
    // // Add "New Profile" button at top
    // let new_button = QPushButton::new("New Profile");
    // new_button.clicked.connect(|| {
    //     let dialog = super::profile_dialog::create(state.clone(), None);
    //     dialog.exec();
    // });
    // layout.add_widget(new_button);
    //
    // // Create list widget
    // let list = QListWidget::new();
    // populate_profiles(list, state.clone());
    // layout.add_widget(list);
    //
    // widget.set_layout(layout);
    // widget
}

/// Populate profiles list using ProfileViewModel from gui-core
///
/// IMPORTANT: This shows the code reuse - same logic as GTK!
fn populate_profiles(_state: Rc<AppState>) {
    // Load profiles using gui-core (SHARED CODE)
    // let profiles = ssh_tunnel_gui_core::load_profiles().unwrap_or_default();
    //
    // for profile in profiles {
    //     // Get status from AppCore (SHARED)
    //     let status = {
    //         let core = state.core.borrow();
    //         core.tunnel_statuses.get(&profile.metadata.id)
    //             .cloned()
    //             .unwrap_or(TunnelStatus::NotConnected)
    //     };
    //
    //     // Create ProfileViewModel (SHARED)
    //     let view_model = ProfileViewModel::from_profile(&profile, status);
    //
    //     // Create Qt list item (Qt-SPECIFIC)
    //     let item = QListWidgetItem::new(&QString::from(&view_model.name));
    //
    //     // Set icon based on status color (Qt-SPECIFIC rendering)
    //     let icon = create_status_icon(&view_model.status_color);
    //     item.set_icon(icon);
    //
    //     // Set subtitle/description
    //     item.set_tooltip(&QString::from(&view_model.connection_summary));
    //
    //     list.add_item(item);
    // }
}

/// Create status icon based on StatusColor from ProfileViewModel
///
/// Qt-specific rendering of shared StatusColor enum
fn create_status_icon(_color: &StatusColor) {
    // match color {
    //     StatusColor::Green => QIcon::from_theme("emblem-success"),
    //     StatusColor::Orange => QIcon::from_theme("emblem-warning"),
    //     StatusColor::Red => QIcon::from_theme("emblem-error"),
    //     StatusColor::Gray => QIcon::from_theme("emblem-inactive"),
    // }
}

/// Update profile status in the list
///
/// Called by event_handler when status changes
pub fn update_profile_status(_profile_id: uuid::Uuid, _status: TunnelStatus) {
    // Find the list item by profile_id
    // Update its icon based on new status
    // This is Qt-specific, but uses shared TunnelStatus type
}
