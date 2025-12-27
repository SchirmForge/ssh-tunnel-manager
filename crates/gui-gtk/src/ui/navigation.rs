// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Navigation sidebar (left panel with Profiles and Daemon sections)

use gtk4::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;

use super::window::AppState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NavigationPage {
    Client,
    Daemon,
}

/// Create the navigation sidebar with Profiles and Daemon options
pub fn create(state: Rc<AppState>, status_icon: gtk4::Image) -> gtk4::Box {
    let nav_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // Create header bar for the sidebar (with daemon status and menu)
    let header = adw::HeaderBar::new();
    header.set_show_start_title_buttons(false); // No window controls
    header.set_show_end_title_buttons(false);
    header.add_css_class("flat"); // Match sidebar background
    header.add_css_class("nav-header");
    header.set_title_widget(Some(&gtk4::Label::new(Some("SSH Tunnel Manager"))));

    // Add daemon connection status indicator to header
    header.pack_start(&status_icon);

    // Add menu button to header bar
    let menu_button = gtk4::MenuButton::new();
    menu_button.set_icon_name("open-menu-symbolic");
    menu_button.set_tooltip_text(Some("Main Menu"));

    // Create menu with Help and About items
    let menu = gio::Menu::new();
    menu.append(Some("Help"), Some("app.help"));
    menu.append(Some("About"), Some("app.about"));

    let popover = gtk4::PopoverMenu::from_model(Some(&menu));
    menu_button.set_popover(Some(&popover));

    header.pack_end(&menu_button);

    nav_box.append(&header);

    // Create list box for navigation items
    let list_box = gtk4::ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::Single);
    list_box.add_css_class("navigation-sidebar");
    list_box.set_vexpand(true);

    // Client item
    let client_row = create_nav_row("Client", "folder-documents-symbolic");
    list_box.append(&client_row);

    // Daemon item
    let daemon_row = create_nav_row("Daemon", "preferences-system-symbolic");
    list_box.append(&daemon_row);

    // Select Client by default
    list_box.select_row(Some(&client_row));

    // Handle selection changes
    {
        let state = state.clone();
        list_box.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                let index = row.index();
                let page = match index {
                    0 => NavigationPage::Client,
                    1 => NavigationPage::Daemon,
                    _ => return,
                };

                // Update the content area based on selection
                on_navigation_changed(&state, page);
            }
        });
    }

    nav_box.append(&list_box);
    nav_box
}

/// Create a navigation row with icon and label
fn create_nav_row(title: &str, icon_name: &str) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);

    let icon = gtk4::Image::from_icon_name(icon_name);
    icon.set_icon_size(gtk4::IconSize::Normal);
    row.add_prefix(&icon);

    row
}

/// Handle navigation page changes
fn on_navigation_changed(state: &AppState, page: NavigationPage) {
    eprintln!("Navigation changed to: {:?}", page);

    state.current_nav_page.replace(page);

    // Get the navigation view from state
    let nav_view = match state.nav_view.borrow().as_ref() {
        Some(view) => view.clone(),
        None => {
            eprintln!("Warning: nav_view not initialized");
            return;
        }
    };

    // Get the target page reference directly (no title-based lookup!)
    let target_page = match page {
        NavigationPage::Client => state.client_page.borrow().clone(),
        NavigationPage::Daemon => state.daemon_page.borrow().clone(),
    };

    let target_page = match target_page {
        Some(p) => p,
        None => {
            eprintln!("Warning: target page not initialized");
            return;
        }
    };

    // Check if we're already on the correct page
    if let Some(visible) = nav_view.visible_page() {
        if visible == target_page {
            eprintln!("Already on the target page");
            return;
        }
    }

    // Pop back to root (Client page)
    while nav_view.pop() {
        eprintln!("Popped a page");
    }

    // Push the target page if it's not the Client page
    match page {
        NavigationPage::Client => {
            // Already at root after popping
            eprintln!("Navigated to Client page (root)");
        }
        NavigationPage::Daemon => {
            // Push the daemon page
            nav_view.push(&target_page);
            eprintln!("Pushed Daemon page");
        }
    }
}
