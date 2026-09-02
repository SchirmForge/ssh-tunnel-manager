// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

use adw::prelude::*;

use crate::DaemonBadgeState;

#[derive(Clone)]
pub struct DaemonStatusBadge {
    root: gtk::Box,
    label: gtk::Label,
}

impl DaemonStatusBadge {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        root.add_css_class("stm-status-pill");
        root.add_css_class("stm-status-offline");

        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("stm-status-dot");

        let label = gtk::Label::new(None);
        label.set_xalign(0.0);

        root.append(&dot);
        root.append(&label);

        let badge = Self { root, label };
        badge.update(DaemonBadgeState::Offline);
        badge
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn update(&self, state: DaemonBadgeState) {
        self.root.remove_css_class("stm-status-online");
        self.root.remove_css_class("stm-status-offline");
        self.root.remove_css_class("stm-status-checking");
        self.root.add_css_class(match state {
            DaemonBadgeState::Online { .. } => "stm-status-online",
            DaemonBadgeState::Checking => "stm-status-checking",
            DaemonBadgeState::Offline => "stm-status-offline",
        });

        let label = state.label();
        self.label.set_label(&label);
        self.root.set_tooltip_text(Some(&label));
        self.root
            .update_property(&[gtk::accessible::Property::Label(&label)]);
    }
}

pub fn wip_badge(reason: &str) -> gtk::Label {
    let badge = gtk::Label::new(Some("WIP"));
    badge.add_css_class("stm-wip-badge");
    badge.set_tooltip_text(Some(reason));
    badge.update_property(&[gtk::accessible::Property::Description(reason)]);
    badge
}

pub fn wip_action_button(label: &str, action_name: &str, reason: &str) -> gtk::Button {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 7);
    content.append(&gtk::Label::new(Some(label)));
    content.append(&wip_badge(reason));

    let button = gtk::Button::builder()
        .action_name(action_name)
        .tooltip_text(reason)
        .build();
    button.set_child(Some(&content));
    button.update_property(&[
        gtk::accessible::Property::Label(label),
        gtk::accessible::Property::Description(reason),
    ]);
    button
}

pub fn wrap_actions(halign: gtk::Align) -> adw::WrapBox {
    adw::WrapBox::builder()
        .orientation(gtk::Orientation::Horizontal)
        .child_spacing(8)
        .line_spacing(8)
        .halign(halign)
        .build()
}

pub fn wip_notice(title: &str, description: &str) -> gtk::Box {
    let notice = gtk::Box::new(gtk::Orientation::Vertical, 8);
    notice.add_css_class("stm-shell-card");
    notice.set_hexpand(true);

    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::new(Some(title));
    title.add_css_class("stm-card-title");
    title.set_xalign(0.0);
    heading.append(&title);
    heading.append(&wip_badge(description));

    let description_label = gtk::Label::new(Some(description));
    description_label.add_css_class("stm-muted");
    description_label.set_wrap(true);
    description_label.set_xalign(0.0);

    notice.append(&heading);
    notice.append(&description_label);
    notice
}
