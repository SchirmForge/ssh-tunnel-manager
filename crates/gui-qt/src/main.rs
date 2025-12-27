// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Qt6 desktop application for SSH Tunnel Manager
//!
//! This GUI uses the same business logic from gui-core as the GTK implementation,
//! demonstrating ~60-70% code reuse across different GUI frameworks.
//!
//! Built with qmetaobject-rs for Qt6/QML integration.

use qmetaobject::prelude::*;
use ssh_tunnel_gui_core::AppCore;
use std::cell::RefCell;

mod daemon;
mod models;
mod ui;

use models::ProfilesListModel;

/// Application backend for Qt GUI
///
/// This struct bridges QML (UI) with Rust business logic from gui-core.
/// The AppCore contains all framework-agnostic business logic (~60-70% code reuse).
#[derive(Default, QObject)]
struct AppBackend {
    base: qt_base_class!(trait QObject),

    // Business logic from gui-core (SHARED across GTK/Qt)
    core: RefCell<AppCore>,
}

impl AppBackend {
    fn new() -> Self {
        Self {
            base: Default::default(),
            core: RefCell::new(AppCore::new()),
        }
    }
}

fn main() {
    // Create QML engine
    let mut engine = QmlEngine::new();

    // Create backend instance and get pinned reference
    let backend = QObjectBox::new(AppBackend::new());
    engine.set_object_property("appBackend".into(), backend.pinned());

    // Create profiles list model and get pinned reference
    let profiles_model = QObjectBox::new(ProfilesListModel::new());
    engine.set_object_property("profilesModel".into(), profiles_model.pinned());

    // Load QML file
    let qml_path = concat!(env!("CARGO_MANIFEST_DIR"), "/qml/main.qml");
    engine.load_file(qml_path.into());

    // Run Qt event loop
    engine.exec();
}
