// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Profiles list page (shows all profiles in app-style list)

use gtk4::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;

use super::window::AppState;
use super::auth_dialog;
use crate::models::profile_model::ProfileModel;
use ssh_tunnel_common::config::Profile;
use ssh_tunnel_common::types::TunnelStatus;
use uuid::Uuid;

/// Create the profiles list view (like apps list in GNOME Settings)
pub fn create(state: Rc<AppState>) -> adw::NavigationPage {
    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // Create header bar for the profiles list
    let header = adw::HeaderBar::new();
    header.set_show_back_button(false);
    header.add_css_class("flat"); // Match content background

    // Add "New Profile" button to header
    let new_button = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Create New Profile")
        .build();

    // Wire up button to show new profile dialog
    {
        let state_clone = state.clone();
        new_button.connect_clicked(move |_| {
            if let Some(window) = state_clone.window.borrow().as_ref() {
                super::profile_dialog::show_new_profile_dialog(window, state_clone.clone());
            }
        });
    }

    header.pack_end(&new_button);

    content_box.append(&header);

    // Create scrolled window for profile list
    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);

    // Create container box with margins for proper spacing
    let container_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    container_box.set_margin_top(24);
    container_box.set_margin_bottom(24);
    container_box.set_margin_start(24);
    container_box.set_margin_end(24);

    // Create list box for profiles
    let list_box = gtk4::ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::None);
    list_box.add_css_class("boxed-list");

    // Store list box reference in state for updates
    state.profile_list.replace(Some(list_box.clone()));

    // Populate profiles
    populate_profiles(&list_box, state.clone());

    container_box.append(&list_box);
    scrolled.set_child(Some(&container_box));

    // Create clamp for centered content
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(800);
    clamp.set_child(Some(&scrolled));

    content_box.append(&clamp);

    // Create navigation page
    let page = adw::NavigationPage::builder()
        .title("Profiles")
        .child(&content_box)
        .build();

    page
}

/// Populate the list box with profile rows (public for refresh after save)
pub fn populate_profiles(list_box: &gtk4::ListBox, state: Rc<AppState>) {
    // Clear existing rows
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    // Load profiles from config directory
    let profiles = match load_profiles() {
        Ok(profiles) => profiles,
        Err(e) => {
            eprintln!("Failed to load profiles: {}", e);
            Vec::new()
        }
    };

    if profiles.is_empty() {
        // Show empty state
        let empty_row = create_empty_state();
        list_box.append(&empty_row);
        return;
    }

    // Create a row for each profile
    for profile in profiles {
        let profile_id = profile.metadata.id;
        let profile_model = ProfileModel::new(profile);
        let row = create_profile_row(&profile_model, state.clone());
        list_box.append(&row);

        // Query initial status from daemon asynchronously
        let state_clone = state.clone();
        let list_box_clone = list_box.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Some(client) = state_clone.daemon_client.borrow().as_ref() {
                match client.get_tunnel_status(profile_id).await {
                    Ok(Some(status_response)) => {
                        eprintln!("Initial status for profile {}: {:?}", profile_id, status_response.status);
                        update_profile_status(&list_box_clone, profile_id, status_response.status);
                        if let Some(request) = status_response.pending_auth {
                            if let Some(window) = state_clone.window.borrow().as_ref() {
                                auth_dialog::handle_auth_request(window, request, state_clone.clone());
                            }
                        }
                    }
                    Ok(None) => {
                        eprintln!("No status found for profile {}", profile_id);
                    }
                    Err(e) => {
                        eprintln!("Failed to get initial status for profile {}: {}", profile_id, e);
                    }
                }
            }
        });
    }
}

/// Create a profile row (app-style)
fn create_profile_row(profile: &ProfileModel, state: Rc<AppState>) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(&profile.name());

    // Set subtitle with connection info
    let subtitle = format!("{}@{}", profile.user(), profile.host());
    row.set_subtitle(&subtitle);

    // Add status dot
    let status = profile.status();
    eprintln!("Creating profile row for {} with status: {:?}", profile.name(), status);
    let status_dot = create_status_dot(&status);
    row.add_prefix(&status_dot);

    // Add icon
    let icon = gtk4::Image::from_icon_name("network-server-symbolic");
    icon.set_icon_size(gtk4::IconSize::Large);
    row.add_prefix(&icon);

    // Add chevron to indicate it's clickable
    let chevron = gtk4::Image::from_icon_name("go-next-symbolic");
    row.add_suffix(&chevron);

    // Make row activatable
    row.set_activatable(true);

    // Store profile ID and status dot widget for status updates
    if let Some(prof) = profile.profile() {
        unsafe {
            row.set_data("profile_id", prof.metadata.id.to_string());
            row.set_data("status_dot", status_dot);
        }
    }

    // Handle click to show profile details
    {
        let profile = profile.clone();
        let state = state.clone();
        row.connect_activated(move |_| {
            eprintln!("Profile selected: {}", profile.name());
            state.selected_profile.replace(Some(profile.clone()));

            // Navigate to profile details page
            if let Some(nav_view) = state.nav_view.borrow().as_ref() {
                let details_page = super::profile_details::create(state.clone(), &profile);
                nav_view.push(&details_page);
            }
        });
    }

    row
}

/// Create status dot icon based on tunnel status
fn create_status_dot(status: &TunnelStatus) -> gtk4::Label {
    let (symbol, css_class) = match status {
        TunnelStatus::Connected => ("●", "status-connected"),
        TunnelStatus::Connecting | TunnelStatus::WaitingForAuth |
        TunnelStatus::Reconnecting | TunnelStatus::Disconnecting =>
            ("●", "status-warning"),
        TunnelStatus::Failed(_) =>
            ("●", "status-error"),
        TunnelStatus::Disconnected | TunnelStatus::NotConnected =>
            ("●", "status-inactive"),
    };

    let dot = gtk4::Label::new(Some(symbol));
    dot.add_css_class(css_class);
    dot.add_css_class("status-dot");
    // Add some margin for spacing
    dot.set_margin_end(8);
    dot
}

/// Create empty state placeholder
fn create_empty_state() -> gtk4::Box {
    let empty_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    empty_box.set_valign(gtk4::Align::Center);
    empty_box.set_vexpand(true);
    empty_box.set_margin_top(48);
    empty_box.set_margin_bottom(48);

    let icon = gtk4::Image::from_icon_name("folder-documents-symbolic");
    icon.set_pixel_size(128);
    icon.add_css_class("dim-label");
    empty_box.append(&icon);

    let label = gtk4::Label::new(Some("No Profiles"));
    label.add_css_class("title-1");
    empty_box.append(&label);

    let sublabel = gtk4::Label::new(Some("Create a profile to get started"));
    sublabel.add_css_class("dim-label");
    empty_box.append(&sublabel);

    empty_box
}

/// Load profiles from the config directory
fn load_profiles() -> anyhow::Result<Vec<Profile>> {
    use std::fs;
    use std::path::PathBuf;

    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
    let profiles_dir: PathBuf = config_dir.join("ssh-tunnel-manager").join("profiles");

    if !profiles_dir.exists() {
        return Ok(Vec::new());
    }

    let mut profiles = Vec::new();

    for entry in fs::read_dir(profiles_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            let contents = fs::read_to_string(&path)?;
            match toml::from_str::<Profile>(&contents) {
                Ok(profile) => profiles.push(profile),
                Err(e) => eprintln!("Failed to parse profile {:?}: {}", path, e),
            }
        }
    }

    // Sort by name
    profiles.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));

    Ok(profiles)
}

/// Update a profile's status in the list and refresh its display
pub fn update_profile_status(list_box: &gtk4::ListBox, profile_id: Uuid, status: TunnelStatus) {
    eprintln!("Updating profile {} status to: {:?}", profile_id, status);
    let mut index = 0;
    while let Some(row) = list_box.row_at_index(index) {
        eprintln!("  Checking row at index {}", index);
        // ActionRow IS a ListBoxRow, so the row itself is the ActionRow
        if let Some(action_row) = row.downcast_ref::<adw::ActionRow>() {
            eprintln!("    Found ActionRow");
            // Get profile ID from the action row's data
            if let Some(stored_id) = unsafe { action_row.data::<String>("profile_id") } {
                let stored_id_str: &String = unsafe { stored_id.as_ref() };
                eprintln!("    Stored ID: {}, Looking for: {}", stored_id_str, profile_id);
                let profile_id_str = profile_id.to_string();
                if stored_id_str == &profile_id_str {
                    eprintln!("    ✓ Found matching profile row for {}, updating status dot", profile_id);

                    // Get the stored status dot widget and update it
                    if let Some(status_dot) = unsafe { action_row.data::<gtk4::Label>("status_dot") } {
                        let status_dot_ref: &gtk4::Label = unsafe { status_dot.as_ref() };
                        eprintln!("    Found stored status dot widget, updating...");

                        // Determine new symbol and CSS class
                        let (new_symbol, new_css_class) = match &status {
                            TunnelStatus::Connected => ("●", "status-connected"),
                            TunnelStatus::Connecting | TunnelStatus::WaitingForAuth |
                            TunnelStatus::Reconnecting | TunnelStatus::Disconnecting =>
                                ("●", "status-warning"),
                            TunnelStatus::Failed(_) =>
                                ("●", "status-error"),
                            TunnelStatus::Disconnected | TunnelStatus::NotConnected =>
                                ("●", "status-inactive"),
                        };

                        // Remove old CSS classes
                        status_dot_ref.remove_css_class("status-connected");
                        status_dot_ref.remove_css_class("status-warning");
                        status_dot_ref.remove_css_class("status-error");
                        status_dot_ref.remove_css_class("status-inactive");

                        // Update label text and add new CSS class
                        status_dot_ref.set_text(new_symbol);
                        status_dot_ref.add_css_class(new_css_class);

                        eprintln!("    ✓ Updated status dot to: {} ({})", new_symbol, new_css_class);
                    } else {
                        eprintln!("    ✗ No status dot widget stored!");
                    }

                    break;
                }
            }
        }
        index += 1;
    }
}
