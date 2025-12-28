// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Build script for cxx-qt integration

fn main() {
    // Temporarily disabled until cxx-qt bridge macro issues are resolved
    // Will re-enable once we figure out the correct syntax/configuration

    // For now, just ensure Qt is available
    println!("cargo:rerun-if-env-changed=QT_VERSION_MAJOR");
}
