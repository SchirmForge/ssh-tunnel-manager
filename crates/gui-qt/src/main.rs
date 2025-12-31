// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Qt6 desktop application for SSH Tunnel Manager
//!
//! Minimal runnable QML shell; business wiring comes later.

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl, QString};
use std::path::PathBuf;

mod daemon;
mod models;
mod ui;

fn main() {
    // Initialize Qt application
    let mut app = QGuiApplication::new();

    // Load QML from the crate's qml/ directory
    let qml_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("qml")
        .join("main.qml");

    let qml_url = QUrl::from_local_file(&QString::from(
        qml_path
            .to_str()
            .expect("Failed to convert QML path to string"),
    ));

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine_ref) = engine.as_mut() {
        engine_ref.load(&qml_url);
    }

    // Run event loop
    if let Some(mut app_ref) = app.as_mut() {
        std::process::exit(app_ref.exec());
    }
}
