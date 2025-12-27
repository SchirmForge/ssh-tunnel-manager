// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

use gtk4::{gdk, CssProvider};

/// Load application CSS so custom classes are available.
pub fn load() {
    let provider = CssProvider::new();
    provider.load_from_string(include_str!("style.css"));

    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_USER,
        );
    }
}
