// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Help dialog showing usage documentation

use gtk4::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use super::markdown::markdown_to_pango;

/// Show the help dialog
pub fn show_help_dialog(window: &adw::ApplicationWindow) {
    let help_content = include_str!("../../../gui-core/assets/help.md");
    let pango_markup = markdown_to_pango(help_content);

    let dialog = adw::Window::new();
    dialog.set_title(Some("SSH Tunnel Manager - Help"));
    dialog.set_default_size(700, 600);
    dialog.set_modal(true);
    dialog.set_transient_for(Some(window));

    // Create main content box
    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // Header bar
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk4::Label::new(Some("Help"))));
    content_box.append(&header);

    // Scrolled window for content
    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);

    // Text view for rendered markdown content
    let text_view = gtk4::TextView::new();
    text_view.set_editable(false);
    text_view.set_cursor_visible(false);
    text_view.set_wrap_mode(gtk4::WrapMode::Word);
    text_view.set_margin_start(20);
    text_view.set_margin_end(20);
    text_view.set_margin_top(20);
    text_view.set_margin_bottom(20);

    let buffer = text_view.buffer();

    // Insert the Pango markup
    let mut start_iter = buffer.start_iter();
    buffer.insert_markup(&mut start_iter, &pango_markup);

    scrolled.set_child(Some(&text_view));
    content_box.append(&scrolled);

    dialog.set_content(Some(&content_box));
    dialog.present();
}
