// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Sidebar widget - Profile list

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Label, ListBox, Orientation, ScrolledWindow};
use libadwaita as adw;
use std::rc::Rc;

use crate::models::profile_model::ProfileModel;
use crate::utils::profiles;
use super::window::AppState;
use super::details;
use super::profile_dialog;

/// Create the sidebar widget (profile list)
pub fn create(state: Rc<AppState>, window: &adw::ApplicationWindow) -> GtkBox {
    let sidebar = GtkBox::new(Orientation::Vertical, 0);

    // Create scrolled window for profile list
    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    // Create list box for profiles
    let list_box = ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::Single);
    list_box.add_css_class("navigation-sidebar");

    // Placeholder: Empty state
    let empty_state = create_empty_state();
    list_box.set_placeholder(Some(&empty_state));

    // Store list box in state for refresh functionality
    state.profile_list.replace(Some(list_box.clone()));

    // Load profiles and populate list
    load_profiles_into_list(&list_box, state.clone());

    // Prevent auto-selection of first profile on startup
    list_box.unselect_all();

    // Handle profile selection
    {
        let state = state.clone();
        list_box.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                // Get the profile from the row's data
                unsafe {
                    if let Some(profile) = row.data::<ProfileModel>("profile") {
                        let profile = profile.as_ref();
                        state.selected_profile.replace(Some(profile.clone()));

                        // Update details panel
                        if let (Some(details_widget), Some(window)) = (
                            state.details_widget.borrow().as_ref(),
                            state.window.borrow().as_ref(),
                        ) {
                            details::update_with_profile(
                                details_widget,
                                profile,
                                state.clone(),
                                window,
                            );
                        }
                    }
                }
            }
        });
    }

    scrolled.set_child(Some(&list_box));

    // Add buttons at bottom
    let button_box = GtkBox::new(Orientation::Vertical, 6);
    button_box.set_margin_start(12);
    button_box.set_margin_end(12);
    button_box.set_margin_top(6);
    button_box.set_margin_bottom(12);

    // Button row for New Profile and Refresh
    let button_row = GtkBox::new(Orientation::Horizontal, 6);

    let new_button = gtk4::Button::builder()
        .label("New Profile")
        .hexpand(true)
        .build();
    new_button.add_css_class("suggested-action");

    let refresh_button = gtk4::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Reload profile list")
        .build();

    // Wire up new profile button
    {
        let window = window.clone();
        let state = state.clone();
        new_button.connect_clicked(move |_| {
            profile_dialog::show_new_profile_dialog(&window, state.clone());
        });
    }

    // Wire up refresh button
    {
        let state = state.clone();
        refresh_button.connect_clicked(move |_| {
            reload_profile_list(state.clone());
        });
    }

    button_row.append(&new_button);
    button_row.append(&refresh_button);
    button_box.append(&button_row);

    sidebar.append(&scrolled);
    sidebar.append(&button_box);

    sidebar
}

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

/// Create empty state widget
fn create_empty_state() -> adw::StatusPage {
    adw::StatusPage::builder()
        .icon_name("list-add-symbolic")
        .title("No Profiles")
        .description("Create a new profile to get started")
        .vexpand(true)
        .build()
}
