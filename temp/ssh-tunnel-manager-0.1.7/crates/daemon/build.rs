// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Build script to capture build timestamp and other metadata

use std::process::Command;

fn main() {
    // Get the current timestamp
    let output = Command::new("date")
        .args(&["+%Y-%m-%d %H:%M:%S %Z"])
        .output()
        .expect("Failed to execute date command");

    let build_date = String::from_utf8(output.stdout)
        .expect("Invalid UTF-8 from date command")
        .trim()
        .to_string();

    // Set build date as environment variable for the build
    println!("cargo:rustc-env=BUILD_DATE={}", build_date);

    // Get git commit hash if available
    if let Ok(output) = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            let git_hash = String::from_utf8(output.stdout)
                .unwrap_or_default()
                .trim()
                .to_string();
            println!("cargo:rustc-env=GIT_HASH={}", git_hash);
        }
    }

    // Rerun if build script changes
    println!("cargo:rerun-if-changed=build.rs");
}
