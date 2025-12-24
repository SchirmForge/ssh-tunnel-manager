// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Daemon settings page

use gtk4::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;

use super::window::AppState;

/// Create the daemon settings view
pub fn create(_state: Rc<AppState>) -> adw::NavigationPage {
    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // Create header bar
    let header = adw::HeaderBar::new();
    header.set_show_back_button(false);
    header.add_css_class("flat"); // Match content background
    content_box.append(&header);

    // Create scrolled window
    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);

    // Create preferences group
    let prefs_page = adw::PreferencesPage::new();

    // Connection settings group
    let connection_group = adw::PreferencesGroup::new();
    connection_group.set_title("Connection");
    connection_group.set_description(Some("Daemon connection settings"));

    // Daemon URL row
    let url_row = adw::EntryRow::new();
    url_row.set_title("Daemon URL");
    url_row.set_text("https://127.0.0.1:3443");
    connection_group.add(&url_row);

    // Auth token row
    let token_row = adw::PasswordEntryRow::new();
    token_row.set_title("Authentication Token");
    connection_group.add(&token_row);

    prefs_page.add(&connection_group);

    // Service settings group
    let service_group = adw::PreferencesGroup::new();
    service_group.set_title("Service");
    service_group.set_description(Some("Daemon service management"));

    // Auto-start row
    let autostart_row = adw::ActionRow::new();
    autostart_row.set_title("Start daemon automatically");
    autostart_row.set_subtitle("Launch daemon when logging in");

    let autostart_switch = gtk4::Switch::new();
    autostart_switch.set_valign(gtk4::Align::Center);
    autostart_row.add_suffix(&autostart_switch);
    autostart_row.set_activatable_widget(Some(&autostart_switch));

    service_group.add(&autostart_row);

    prefs_page.add(&service_group);

    scrolled.set_child(Some(&prefs_page));

    // Create clamp for centered content
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(800);
    clamp.set_child(Some(&scrolled));

    content_box.append(&clamp);

    // Create navigation page
    let page = adw::NavigationPage::builder()
        .title("Daemon")
        .child(&content_box)
        .build();

    page
}
