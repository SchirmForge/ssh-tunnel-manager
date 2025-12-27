// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Main window for Qt GUI
//!
//! This mirrors the GTK window.rs but uses Qt widgets.
//! The AppState pattern is the same, with AppCore for business logic.

use std::rc::Rc;
use crate::AppState;

// TODO: Add Qt imports
// use qt_widgets::{QMainWindow, QWidget, QVBoxLayout, QMenuBar};

/// Create the main application window
///
/// Architecture:
/// - Uses AppCore from gui-core for state management
/// - Qt-specific widgets for rendering
/// - Event handlers update AppCore and then UI
pub fn create(_state: Rc<AppState>) {
    // TODO: Implement Qt main window
    //
    // let window = QMainWindow::new();
    // window.set_window_title("SSH Tunnel Manager");
    // window.set_minimum_size(800, 600);
    //
    // // Create central widget with navigation
    // let central = QWidget::new();
    // let layout = QVBoxLayout::new();
    //
    // // Add menu bar
    // let menu_bar = create_menu_bar();
    // window.set_menu_bar(menu_bar);
    //
    // // Add profiles list (uses ProfileViewModel from gui-core)
    // let profiles_widget = super::profiles_list::create(state.clone());
    // layout.add_widget(profiles_widget);
    //
    // central.set_layout(layout);
    // window.set_central_widget(central);
    //
    // // Set up event listener for daemon events
    // setup_event_listener(state, window);
    //
    // window
}

// fn create_menu_bar() -> QMenuBar {
//     // File menu with New Profile, Settings, Quit
//     // Help menu with Help, About
// }

// fn setup_event_listener(state: Rc<AppState>, window: QMainWindow) {
//     // Similar to GTK: spawn async task to listen to daemon SSE
//     // When events arrive, use event_handler module to process them
// }
