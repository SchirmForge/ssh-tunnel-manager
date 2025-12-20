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
    Profiles,
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

    // Profiles item
    let profiles_row = create_nav_row("Profiles", "folder-documents-symbolic");
    list_box.append(&profiles_row);

    // Daemon item
    let daemon_row = create_nav_row("Daemon", "preferences-system-symbolic");
    list_box.append(&daemon_row);

    // Select Profiles by default
    list_box.select_row(Some(&profiles_row));

    // Handle selection changes
    {
        let state = state.clone();
        list_box.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                let index = row.index();
                let page = match index {
                    0 => NavigationPage::Profiles,
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
    if let Some(nav_view) = state.nav_view.borrow().as_ref() {
        // Find and navigate to the appropriate page
        let page_name = match page {
            NavigationPage::Profiles => "Profiles",
            NavigationPage::Daemon => "Daemon",
        };

        // The NavigationView should have pages with these titles
        // Pop all pages first to go back to root
        while nav_view.visible_page().is_some() {
            if let Some(visible) = nav_view.visible_page() {
                if visible.title().as_str() == page_name {
                    break; // Already on the correct page
                }
                if !nav_view.pop() {
                    break; // Can't pop anymore
                }
            }
        }

        // Now find and push the correct page
        // Since we added both pages in window.rs, we need to navigate to the right one
        let stack = nav_view.navigation_stack();
        for i in 0..stack.n_items() {
            if let Some(item) = stack.item(i) {
                if let Ok(page_widget) = item.downcast::<adw::NavigationPage>() {
                    if page_widget.title().as_str() == page_name {
                        nav_view.replace(&[page_widget]);
                        break;
                    }
                }
            }
        }
    }
}
