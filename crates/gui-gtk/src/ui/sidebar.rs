// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Sidebar widget - Profile list

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Label, ListBox, Orientation};
use std::rc::Rc;

use crate::models::profile_model::ProfileModel;
use crate::utils::profiles;
use super::window::AppState;
use super::details;

/// Reload the profile list from disk
pub fn reload_profile_list(state: Rc<AppState>) {
    if let Some(list_box) = state.profile_list.borrow().as_ref() {
        // Clear existing rows
        while let Some(child) = list_box.first_child() {
            list_box.remove(&child);
        }

        // Reload profiles
        load_profiles_into_list(list_box, state.clone());

        // Clear selection and details panel
        list_box.unselect_all();
        state.selected_profile.replace(None);
        if let Some(details_widget) = state.details_widget.borrow().as_ref() {
            while let Some(child) = details_widget.first_child() {
                details_widget.remove(&child);
            }
            let placeholder = details::create_placeholder();
            details_widget.append(&placeholder);
        }
    }
}

/// Load profiles from disk and add them to the list
fn load_profiles_into_list(list_box: &ListBox, state: Rc<AppState>) {
    match profiles::load_all_profiles() {
        Ok(profile_list) => {
            for profile in profile_list {
                let profile_model = ProfileModel::new(profile);
                let row = create_profile_row(&profile_model, state.clone());

                // Store profile data in the row for selection handling
                unsafe {
                    row.set_data("profile", profile_model);
                }

                list_box.append(&row);
            }
        }
        Err(e) => {
            eprintln!("Failed to load profiles: {}", e);
            // Directory might not exist yet - this is okay for first run
        }
    }
}

/// Create a list box row for a profile
fn create_profile_row(profile: &ProfileModel, _state: Rc<AppState>) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::new();

    let hbox = GtkBox::new(Orientation::Horizontal, 12);
    hbox.set_margin_start(12);
    hbox.set_margin_end(12);
    hbox.set_margin_top(8);
    hbox.set_margin_bottom(8);

    // Status indicator (placeholder - always stopped for now)
    let status_icon = gtk4::Image::from_icon_name("media-playback-stop-symbolic");
    status_icon.set_icon_size(gtk4::IconSize::Normal);
    status_icon.add_css_class("dim-label");

    // Profile info
    let vbox = GtkBox::new(Orientation::Vertical, 4);
    vbox.set_hexpand(true);

    let name_label = Label::new(Some(&profile.name()));
    name_label.set_halign(gtk4::Align::Start);
    name_label.add_css_class("heading");

    let host_label = Label::new(Some(&ssh_tunnel_common::format_host_port(&profile.host(), profile.port())));
    host_label.set_halign(gtk4::Align::Start);
    host_label.add_css_class("dim-label");
    host_label.add_css_class("caption");

    vbox.append(&name_label);
    vbox.append(&host_label);

    hbox.append(&status_icon);
    hbox.append(&vbox);

    row.set_child(Some(&hbox));
    row
}
