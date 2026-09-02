// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Phase 3 profile-list presentation and interaction routing.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::gdk;
use ssh_tunnel_gui_core::{
    ActionAvailability, AppCommand, AppSnapshot, ProfileSnapshot, SortMode, StatusColor,
    TunnelStatus, UiPreferences,
};
use uuid::Uuid;

use crate::components::{wip_action_button, wip_badge, wrap_actions};

type EventHandler = Rc<dyn Fn(ProfileListEvent)>;
type HandlerSlot = Rc<RefCell<Option<EventHandler>>>;

pub enum ProfileListEvent {
    Command(AppCommand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrimaryAction {
    Connect,
    Cancel,
    Disconnect,
    Retry,
}

impl PrimaryAction {
    fn label(self) -> &'static str {
        match self {
            Self::Connect => "Connect",
            Self::Cancel => "Cancel",
            Self::Disconnect => "Disconnect",
            Self::Retry => "Retry",
        }
    }

    fn command(self, profile_id: Uuid) -> AppCommand {
        match self {
            Self::Connect => AppCommand::ConnectProfile(profile_id),
            Self::Cancel => AppCommand::CancelConnection(profile_id),
            Self::Disconnect => AppCommand::DisconnectProfile(profile_id),
            Self::Retry => AppCommand::RetryProfile(profile_id),
        }
    }
}

#[derive(Clone)]
struct ProfileSection {
    root: gtk::Box,
    count: gtk::Label,
    list: gtk::ListBox,
}

impl ProfileSection {
    fn new(title: &str, pinned: bool) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 7);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        header.add_css_class("stm-section-header");
        if pinned {
            let icon = gtk::Image::from_icon_name("starred-symbolic");
            icon.set_pixel_size(12);
            header.append(&icon);
        }

        let title = gtk::Label::new(Some(title));
        title.set_xalign(0.0);
        title.set_hexpand(true);
        let count = gtk::Label::new(Some("0"));
        count.add_css_class("dim-label");
        header.append(&title);
        header.append(&count);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.add_css_class("boxed-list");
        list.add_css_class("stm-profile-list");

        root.append(&header);
        root.append(&list);
        root.set_visible(false);

        Self { root, count, list }
    }

    fn clear(&self) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
    }
}

struct SearchRow {
    row: gtk::ListBoxRow,
    haystack: String,
    pinned: bool,
}

#[derive(Clone)]
struct FilterState {
    rows: Rc<RefCell<Vec<SearchRow>>>,
    pinned: ProfileSection,
    all: ProfileSection,
    empty: gtk::Box,
    no_results: gtk::Box,
}

impl FilterState {
    fn apply(&self, query: &str) {
        let query = normalize_search(query);
        let mut pinned_count = 0;
        let mut all_count = 0;

        for search_row in self.rows.borrow().iter() {
            let matches = query.is_empty() || search_row.haystack.contains(&query);
            search_row.row.set_visible(matches);
            if matches && search_row.pinned {
                pinned_count += 1;
            } else if matches {
                all_count += 1;
            }
        }

        self.pinned.count.set_label(&pinned_count.to_string());
        self.all.count.set_label(&all_count.to_string());
        self.pinned.root.set_visible(pinned_count > 0);
        self.all.root.set_visible(all_count > 0);

        let has_rows = !self.rows.borrow().is_empty();
        let has_matches = pinned_count + all_count > 0;
        self.no_results.set_visible(has_rows && !has_matches);
        if has_rows {
            self.empty.set_visible(false);
        }
    }
}

pub struct ProfileListView {
    root: gtk::Box,
    controls: gtk::Box,
    connected_only: gtk::CheckButton,
    sort_mode: gtk::DropDown,
    offline_banner: gtk::Box,
    offline_retry: gtk::Button,
    empty: gtk::Box,
    empty_title: gtk::Label,
    empty_description: gtk::Label,
    empty_actions: adw::WrapBox,
    filter: FilterState,
    search_entry: gtk::SearchEntry,
    handler: HandlerSlot,
    rendering: Rc<Cell<bool>>,
    selection_blocked: Rc<Cell<bool>>,
}

impl ProfileListView {
    pub fn new() -> Self {
        let handler = HandlerSlot::default();
        let rendering = Rc::new(Cell::new(false));
        let selection_blocked = Rc::new(Cell::new(false));

        let root = gtk::Box::new(gtk::Orientation::Vertical, 14);
        root.add_css_class("stm-profiles-content");

        let controls = gtk::Box::new(gtk::Orientation::Vertical, 9);
        let primary_controls = gtk::Box::new(gtk::Orientation::Horizontal, 9);
        let search_entry = gtk::SearchEntry::new();
        search_entry.set_placeholder_text(Some("Search profiles"));
        search_entry.set_hexpand(true);
        search_entry.set_accessible_role(gtk::AccessibleRole::SearchBox);
        search_entry.update_property(&[
            gtk::accessible::Property::Label("Search profiles"),
            gtk::accessible::Property::KeyShortcuts("Control+F"),
        ]);
        primary_controls.append(&search_entry);
        let new_profile = gtk::Button::builder()
            .label("New profile")
            .action_name("win.new-profile")
            .build();
        new_profile.add_css_class("suggested-action");
        primary_controls.append(&new_profile);

        let secondary_controls = gtk::Box::new(gtk::Orientation::Horizontal, 9);
        secondary_controls.set_halign(gtk::Align::End);
        let connected_only = gtk::CheckButton::with_label("Connected only");
        connected_only.set_tooltip_text(Some("Persist this filter in ui.toml"));
        let sort_mode = gtk::DropDown::from_strings(&["Manual order", "Name"]);
        sort_mode.set_tooltip_text(Some("Choose the persisted profile ordering"));
        sort_mode.update_property(&[gtk::accessible::Property::Label("Profile sort order")]);
        secondary_controls.append(&connected_only);
        secondary_controls.append(&sort_mode);
        controls.append(&primary_controls);
        controls.append(&secondary_controls);

        let (offline_banner, offline_retry) = offline_notice();
        let (empty, empty_title, empty_description, empty_actions) = empty_state();
        let no_results = message_state(
            "system-search-symbolic",
            "No matching profiles",
            "Try another name, host, user, or forwarding address.",
        );
        no_results.set_visible(false);

        let pinned = ProfileSection::new("Pinned", true);
        let all = ProfileSection::new("All profiles", false);
        let filter = FilterState {
            rows: Rc::new(RefCell::new(Vec::new())),
            pinned,
            all,
            empty: empty.clone(),
            no_results: no_results.clone(),
        };

        root.append(&controls);
        root.append(&offline_banner);
        root.append(&empty);
        root.append(&no_results);
        root.append(&filter.pinned.root);
        root.append(&filter.all.root);

        {
            let filter = filter.clone();
            search_entry.connect_search_changed(move |entry| filter.apply(entry.text().as_str()));
        }
        {
            let handler = handler.clone();
            let rendering = rendering.clone();
            connected_only.connect_toggled(move |button| {
                if !rendering.get() {
                    emit_command(&handler, AppCommand::SetConnectedOnly(button.is_active()));
                }
            });
        }
        {
            let handler = handler.clone();
            let rendering = rendering.clone();
            sort_mode.connect_selected_notify(move |dropdown| {
                if rendering.get() {
                    return;
                }
                let mode = match dropdown.selected() {
                    0 => SortMode::Manual,
                    _ => SortMode::Name,
                };
                emit_command(&handler, AppCommand::SetSortMode(mode));
            });
        }

        connect_selection(
            &filter.pinned.list,
            &filter.all.list,
            &handler,
            &selection_blocked,
        );
        connect_selection(
            &filter.all.list,
            &filter.pinned.list,
            &handler,
            &selection_blocked,
        );

        Self {
            root,
            controls,
            connected_only,
            sort_mode,
            offline_banner,
            offline_retry,
            empty,
            empty_title,
            empty_description,
            empty_actions,
            filter,
            search_entry,
            handler,
            rendering,
            selection_blocked,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn set_handler(&self, handler: EventHandler) {
        self.handler.replace(Some(handler));
    }

    pub fn focus_search(&self) {
        self.search_entry.grab_focus();
    }

    pub fn render(&self, snapshot: &AppSnapshot) {
        self.rendering.set(true);
        self.selection_blocked.set(true);
        self.connected_only
            .set_active(snapshot.preferences.connected_only);
        self.sort_mode
            .set_selected(match snapshot.preferences.sort_mode {
                SortMode::Manual => 0,
                SortMode::Name => 1,
            });
        self.offline_banner
            .set_visible(snapshot.has_refreshed && !snapshot.daemon_connected);
        self.offline_retry
            .set_sensitive(!snapshot.refresh_in_flight);

        self.filter.pinned.clear();
        self.filter.all.clear();
        self.filter.rows.borrow_mut().clear();

        for profile in &snapshot.profiles {
            let row = profile_row(profile, &snapshot.preferences, &self.handler);
            row.set_widget_name(&profile.view.id.to_string());
            let list = if profile.pinned {
                &self.filter.pinned.list
            } else {
                &self.filter.all.list
            };
            list.append(&row);
            if snapshot.selected_profile == Some(profile.view.id) {
                list.select_row(Some(&row));
            }

            self.filter.rows.borrow_mut().push(SearchRow {
                row,
                haystack: profile_search_text(profile),
                pinned: profile.pinned,
            });
        }

        let complete_profile_count = snapshot
            .preferences
            .profile_order
            .len()
            .max(snapshot.profiles.len());
        let visible_profile_count = snapshot.profiles.len();
        self.controls.set_visible(complete_profile_count > 0);
        if visible_profile_count == 0 {
            if snapshot.preferences.connected_only && complete_profile_count > 0 {
                self.empty_title.set_label("No connected profiles");
                self.empty_description.set_label(
                    "Profiles are still available. Turn off Connected only to see them.",
                );
                self.empty_actions.set_visible(false);
            } else {
                self.empty_title.set_label("No profiles yet");
                self.empty_description.set_label(
                    "Create a profile for one SSH server and the ports you want forwarded.",
                );
                self.empty_actions.set_visible(true);
            }
            self.empty.set_visible(true);
        } else {
            self.empty.set_visible(false);
        }

        self.selection_blocked.set(false);
        self.rendering.set(false);
        self.filter.apply(self.search_entry.text().as_str());
    }
}

fn connect_selection(
    list: &gtk::ListBox,
    other: &gtk::ListBox,
    handler: &HandlerSlot,
    blocked: &Rc<Cell<bool>>,
) {
    let other = other.clone();
    let handler = handler.clone();
    let blocked = blocked.clone();
    list.connect_row_selected(move |_, selected| {
        if blocked.get() {
            return;
        }
        let Some(row) = selected else {
            return;
        };
        let Ok(profile_id) = Uuid::parse_str(row.widget_name().as_str()) else {
            return;
        };

        blocked.set(true);
        other.unselect_all();
        blocked.set(false);
        emit_command(&handler, AppCommand::SelectProfile(Some(profile_id)));
    });
}

fn profile_row(
    profile: &ProfileSnapshot,
    preferences: &UiPreferences,
    handler: &HandlerSlot,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("stm-profile-row");
    row.set_selectable(true);
    row.set_activatable(true);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.add_css_class("stm-profile-row-content");

    let drag_handle = gtk::Image::from_icon_name("list-drag-handle-symbolic");
    drag_handle.add_css_class("dim-label");
    drag_handle.set_tooltip_text(Some(if preferences.sort_mode == SortMode::Manual {
        "Drag within this section to reorder"
    } else {
        "Choose Manual order to drag profiles"
    }));
    drag_handle.set_sensitive(preferences.sort_mode == SortMode::Manual);
    content.append(&drag_handle);

    if preferences.sort_mode == SortMode::Manual {
        install_drag_source(&drag_handle, profile.view.id);
        install_drop_target(&row, profile, preferences, handler);
    }

    let identity = gtk::Box::new(gtk::Orientation::Vertical, 4);
    identity.set_hexpand(true);

    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = gtk::Label::new(Some(&profile.view.name));
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name.set_tooltip_text(Some(&profile.view.name));
    name.add_css_class("heading");
    heading.append(&name);
    heading.append(&status_badge(profile));

    let summary = gtk::Label::new(Some(&format!(
        "{}  ·  {}",
        profile.view.connection_summary, profile.view.forwarding_description
    )));
    summary.set_xalign(0.0);
    summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
    summary.set_tooltip_text(Some(&format!(
        "{} · {}",
        profile.view.connection_summary, profile.view.forwarding_description
    )));
    summary.add_css_class("stm-profile-summary");
    summary.add_css_class("dim-label");

    identity.append(&heading);
    identity.append(&summary);
    if let TunnelStatus::Failed(reason) = &profile.view.status {
        if !reason.trim().is_empty() {
            let error = gtk::Label::new(Some(reason));
            error.set_xalign(0.0);
            error.set_ellipsize(gtk::pango::EllipsizeMode::End);
            error.set_tooltip_text(Some(reason));
            error.add_css_class("error");
            error.add_css_class("stm-profile-error");
            identity.append(&error);
        }
    }
    content.append(&identity);

    let action_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    action_box.add_css_class("linked");
    let primary = primary_action(&profile.actions);
    let primary_button = gtk::Button::with_label(
        primary
            .map(PrimaryAction::label)
            .unwrap_or_else(|| inactive_action_label(&profile.view.status)),
    );
    primary_button.update_property(&[gtk::accessible::Property::Label(&format!(
        "{} {}",
        primary
            .map(PrimaryAction::label)
            .unwrap_or_else(|| inactive_action_label(&profile.view.status)),
        profile.view.name
    ))]);
    primary_button.set_sensitive(primary.is_some());
    if matches!(primary, Some(PrimaryAction::Connect | PrimaryAction::Retry)) {
        primary_button.add_css_class("suggested-action");
    }
    if let Some(action) = primary {
        let handler = handler.clone();
        let profile_id = profile.view.id;
        primary_button.connect_clicked(move |_| {
            emit_command(&handler, action.command(profile_id));
        });
    }

    let more = gtk::MenuButton::builder()
        .icon_name("pan-down-symbolic")
        .tooltip_text("Profile details and actions")
        .build();
    more.update_property(&[gtk::accessible::Property::Label(&format!(
        "Details and actions for {}",
        profile.view.name
    ))]);
    more.set_popover(Some(&profile_popover(profile, preferences, handler)));
    action_box.append(&primary_button);
    action_box.append(&more);
    content.append(&action_box);

    row.set_child(Some(&content));
    row.update_property(&[gtk::accessible::Property::Label(&format!(
        "{}, {}, {}, {}",
        profile.view.name,
        profile.view.status_text,
        profile.view.connection_summary,
        profile.view.forwarding_description
    ))]);
    row
}

fn profile_popover(
    profile: &ProfileSnapshot,
    preferences: &UiPreferences,
    handler: &HandlerSlot,
) -> gtk::Popover {
    let popover = gtk::Popover::new();
    popover.add_css_class("stm-profile-popover");

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_size_request(360, -1);

    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::new(Some(&profile.view.name));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.add_css_class("heading");
    heading.append(&title);
    heading.append(&status_badge(profile));
    content.append(&heading);

    let connection = gtk::Label::new(Some(&profile.view.connection_summary));
    connection.set_xalign(0.0);
    connection.set_selectable(true);
    connection.add_css_class("dim-label");
    content.append(&connection);

    let forwarding = gtk::Label::new(Some(&profile.view.forwarding_description));
    forwarding.set_xalign(0.0);
    forwarding.set_wrap(true);
    forwarding.set_selectable(true);
    content.append(&forwarding);

    let details = gtk::Grid::builder()
        .column_spacing(14)
        .row_spacing(6)
        .build();
    append_detail(
        &details,
        0,
        "Forwarding",
        profile.details.forwarding_type_display,
    );
    append_detail(
        &details,
        1,
        "Authentication",
        profile.details.auth_type_display,
    );
    append_detail(
        &details,
        2,
        "Credential",
        profile.details.password_storage_display,
    );
    if let Some(key_path) = &profile.details.key_path {
        append_detail(&details, 3, "SSH key", key_path);
    }
    content.append(&details);
    if profile.details.unsupported_forwarding {
        let unsupported = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let copy = gtk::Label::new(Some(
            "This forwarding mode is preserved, but the daemon currently rejects it.",
        ));
        copy.set_xalign(0.0);
        copy.set_wrap(true);
        copy.set_hexpand(true);
        unsupported.append(&copy);
        unsupported.append(&wip_badge("Forwarding runtime is unavailable"));
        content.append(&unsupported);
    }

    let auto_reconnect_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    auto_reconnect_row.add_css_class("stm-popover-setting");
    let auto_label = gtk::Label::new(Some("Auto-reconnect"));
    auto_label.set_xalign(0.0);
    auto_label.set_hexpand(true);
    auto_reconnect_row.append(&auto_label);
    auto_reconnect_row.append(&wip_badge(
        "Stored on the profile; daemon enforcement is not complete",
    ));
    let auto_reconnect = gtk::Switch::new();
    auto_reconnect.set_active(profile.auto_reconnect);
    auto_reconnect.set_sensitive(profile.actions.toggle_auto_reconnect);
    auto_reconnect.update_property(&[gtk::accessible::Property::Label("Auto-reconnect")]);
    {
        let handler = handler.clone();
        let profile_id = profile.view.id;
        auto_reconnect.connect_active_notify(move |switch| {
            emit_command(
                &handler,
                AppCommand::SetAutoReconnect {
                    profile_id,
                    enabled: switch.is_active(),
                },
            );
        });
    }
    auto_reconnect_row.append(&auto_reconnect);
    content.append(&auto_reconnect_row);

    let tuning = gtk::Expander::new(Some("Fine tuning"));
    let tuning_grid = gtk::Grid::builder()
        .column_spacing(14)
        .row_spacing(6)
        .margin_top(8)
        .build();
    append_detail(
        &tuning_grid,
        0,
        "Compression",
        if profile.details.compression {
            "On · WIP"
        } else {
            "Off · WIP"
        },
    );
    append_detail(
        &tuning_grid,
        1,
        "Keepalive",
        &format!("{} s · WIP", profile.details.keepalive_interval),
    );
    append_detail(
        &tuning_grid,
        2,
        "Reconnect",
        &format!(
            "{} attempts / {} s · WIP",
            profile.details.reconnect_attempts, profile.details.reconnect_delay
        ),
    );
    append_detail(
        &tuning_grid,
        3,
        "TCP keepalive",
        if profile.details.tcp_keepalive {
            "On · WIP"
        } else {
            "Off · WIP"
        },
    );
    append_detail(
        &tuning_grid,
        4,
        "Packet / window",
        &format!(
            "{} / {} bytes",
            profile.details.max_packet_size, profile.details.window_size
        ),
    );
    tuning.set_child(Some(&tuning_grid));
    content.append(&tuning);

    let order_actions = wrap_actions(gtk::Align::Start);
    let move_up_target = adjacent_reorder_index(preferences, profile.view.id, ReorderDirection::Up);
    let move_up = gtk::Button::with_label("Move up");
    move_up.set_sensitive(move_up_target.is_some());
    move_up.set_tooltip_text(Some(if preferences.sort_mode == SortMode::Manual {
        "Move earlier within this pin section"
    } else {
        "Choose Manual order to reorder profiles"
    }));
    if let Some(new_index) = move_up_target {
        let handler = handler.clone();
        let profile_id = profile.view.id;
        move_up.connect_clicked(move |_| {
            emit_command(
                &handler,
                AppCommand::MoveProfile {
                    profile_id,
                    new_index,
                },
            );
        });
    }

    let move_down_target =
        adjacent_reorder_index(preferences, profile.view.id, ReorderDirection::Down);
    let move_down = gtk::Button::with_label("Move down");
    move_down.set_sensitive(move_down_target.is_some());
    move_down.set_tooltip_text(Some(if preferences.sort_mode == SortMode::Manual {
        "Move later within this pin section"
    } else {
        "Choose Manual order to reorder profiles"
    }));
    if let Some(new_index) = move_down_target {
        let handler = handler.clone();
        let profile_id = profile.view.id;
        move_down.connect_clicked(move |_| {
            emit_command(
                &handler,
                AppCommand::MoveProfile {
                    profile_id,
                    new_index,
                },
            );
        });
    }

    let pin = gtk::Button::with_label(if profile.pinned { "Unpin" } else { "Pin" });
    pin.set_sensitive(profile.actions.pin);
    {
        let handler = handler.clone();
        let profile_id = profile.view.id;
        let pinned = profile.pinned;
        pin.connect_clicked(move |_| {
            emit_command(
                &handler,
                AppCommand::SetProfilePinned {
                    profile_id,
                    pinned: !pinned,
                },
            );
        });
    }

    order_actions.append(&move_up);
    order_actions.append(&move_down);
    order_actions.append(&pin);
    content.append(&order_actions);

    let profile_actions = wrap_actions(gtk::Align::Start);
    let edit = gtk::Button::with_label("Edit");
    edit.set_sensitive(profile.actions.edit);
    {
        let handler = handler.clone();
        let profile_id = profile.view.id;
        edit.connect_clicked(move |_| emit_command(&handler, AppCommand::EditProfile(profile_id)));
    }

    let duplicate = gtk::Button::with_label("Duplicate");
    duplicate.set_sensitive(profile.actions.duplicate);
    {
        let handler = handler.clone();
        let profile_id = profile.view.id;
        duplicate.connect_clicked(move |_| {
            emit_command(&handler, AppCommand::DuplicateProfile(profile_id));
        });
    }

    let delete = gtk::Button::with_label("Delete");
    delete.add_css_class("destructive-action");
    delete.set_sensitive(profile.actions.delete);
    {
        let handler = handler.clone();
        let profile_id = profile.view.id;
        delete.connect_clicked(move |_| {
            emit_command(&handler, AppCommand::DeleteProfile(profile_id));
        });
    }

    profile_actions.append(&edit);
    profile_actions.append(&duplicate);
    profile_actions.append(&delete);
    content.append(&profile_actions);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroller.set_propagate_natural_height(true);
    scroller.set_max_content_height(640);
    scroller.set_child(Some(&content));
    popover.set_child(Some(&scroller));
    popover
}

fn status_badge(profile: &ProfileSnapshot) -> gtk::Label {
    let badge = gtk::Label::new(Some(&profile.view.status_text));
    badge.add_css_class("stm-profile-status");
    badge.add_css_class(match profile.view.status_color {
        StatusColor::Green => "stm-profile-status-connected",
        StatusColor::Orange => "stm-profile-status-transition",
        StatusColor::Red => "stm-profile-status-failed",
        StatusColor::Gray => "stm-profile-status-idle",
    });
    badge
}

fn install_drag_source(handle: &gtk::Image, profile_id: Uuid) {
    let drag_source = gtk::DragSource::new();
    drag_source.set_actions(gdk::DragAction::MOVE);
    drag_source.connect_prepare(move |_, _, _| {
        Some(gdk::ContentProvider::for_value(
            &profile_id.to_string().to_value(),
        ))
    });
    handle.add_controller(drag_source);
}

fn install_drop_target(
    row: &gtk::ListBoxRow,
    profile: &ProfileSnapshot,
    preferences: &UiPreferences,
    handler: &HandlerSlot,
) {
    let target = gtk::DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
    let target_id = profile.view.id;
    let target_pinned = profile.pinned;
    let preferences = preferences.clone();
    let handler = handler.clone();
    let target_row = row.clone();
    target.connect_drop(move |_, value, _, y| {
        let Ok(source) = value.get::<String>() else {
            return false;
        };
        let Ok(source_id) = Uuid::parse_str(&source) else {
            return false;
        };
        let after = y >= f64::from(target_row.height()) / 2.0;
        let Some(new_index) =
            reorder_index(&preferences, source_id, target_id, target_pinned, after)
        else {
            return false;
        };

        emit_command(
            &handler,
            AppCommand::MoveProfile {
                profile_id: source_id,
                new_index,
            },
        );
        true
    });
    row.add_controller(target);
}

fn primary_action(actions: &ActionAvailability) -> Option<PrimaryAction> {
    let candidates = [
        (actions.connect, PrimaryAction::Connect),
        (actions.cancel_connection, PrimaryAction::Cancel),
        (actions.disconnect, PrimaryAction::Disconnect),
        (actions.retry, PrimaryAction::Retry),
    ];
    let mut enabled = candidates
        .into_iter()
        .filter_map(|(enabled, action)| enabled.then_some(action));
    let action = enabled.next()?;
    enabled.next().is_none().then_some(action)
}

fn inactive_action_label(status: &TunnelStatus) -> &'static str {
    match status {
        TunnelStatus::Connecting | TunnelStatus::WaitingForAuth | TunnelStatus::Reconnecting => {
            "Cancel"
        }
        TunnelStatus::Connected | TunnelStatus::Disconnecting => "Disconnect",
        TunnelStatus::Failed(_) => "Retry",
        TunnelStatus::NotConnected | TunnelStatus::Disconnected => "Connect",
    }
}

fn profile_search_text(profile: &ProfileSnapshot) -> String {
    normalize_search(&format!(
        "{} {} {} {} {}",
        profile.view.name,
        profile.view.user,
        profile.view.host,
        profile.view.connection_summary,
        profile.view.forwarding_description
    ))
}

fn normalize_search(value: &str) -> String {
    value.trim().to_lowercase()
}

fn reorder_index(
    preferences: &UiPreferences,
    source_id: Uuid,
    target_id: Uuid,
    target_pinned: bool,
    after: bool,
) -> Option<usize> {
    if preferences.sort_mode != SortMode::Manual
        || source_id == target_id
        || preferences.is_pinned(source_id) != target_pinned
    {
        return None;
    }

    let source_index = preferences
        .profile_order
        .iter()
        .position(|id| *id == source_id)?;
    let target_index = preferences
        .profile_order
        .iter()
        .position(|id| *id == target_id)?;

    Some(if after {
        if source_index < target_index {
            target_index
        } else {
            target_index + 1
        }
    } else if source_index < target_index {
        target_index.saturating_sub(1)
    } else {
        target_index
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReorderDirection {
    Up,
    Down,
}

fn adjacent_reorder_index(
    preferences: &UiPreferences,
    profile_id: Uuid,
    direction: ReorderDirection,
) -> Option<usize> {
    if preferences.sort_mode != SortMode::Manual {
        return None;
    }

    let pinned = preferences.is_pinned(profile_id);
    let section_positions: Vec<usize> = preferences
        .profile_order
        .iter()
        .enumerate()
        .filter_map(|(index, id)| (preferences.is_pinned(*id) == pinned).then_some(index))
        .collect();
    let current = section_positions
        .iter()
        .position(|index| preferences.profile_order[*index] == profile_id)?;
    let target = match direction {
        ReorderDirection::Up => current.checked_sub(1)?,
        ReorderDirection::Down => current
            .checked_add(1)
            .filter(|next| *next < section_positions.len())?,
    };
    Some(section_positions[target])
}

fn emit_command(handler: &HandlerSlot, command: AppCommand) {
    emit_event(handler, ProfileListEvent::Command(command));
}

fn emit_event(handler: &HandlerSlot, event: ProfileListEvent) {
    let callback = handler.borrow().clone();
    if let Some(callback) = callback {
        callback(event);
    }
}

fn append_detail(grid: &gtk::Grid, row: i32, label: &str, value: &str) {
    let label = gtk::Label::new(Some(label));
    label.set_xalign(0.0);
    label.add_css_class("dim-label");
    let value = gtk::Label::new(Some(value));
    value.set_xalign(0.0);
    value.set_selectable(true);
    value.set_wrap(true);
    grid.attach(&label, 0, row, 1, 1);
    grid.attach(&value, 1, row, 1, 1);
}

fn offline_notice() -> (gtk::Box, gtk::Button) {
    let banner = gtk::Box::new(gtk::Orientation::Vertical, 9);
    banner.add_css_class("warning");
    banner.add_css_class("stm-offline-banner");
    let message = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
    let label = gtk::Label::new(Some(
        "Daemon not reachable. Profiles remain readable, but tunnel actions are disabled.",
    ));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_wrap(true);
    message.append(&icon);
    message.append(&label);
    banner.append(&message);
    let actions = wrap_actions(gtk::Align::End);
    let retry = gtk::Button::builder()
        .label("Retry")
        .action_name("win.refresh")
        .tooltip_text("Run a daemon health check now")
        .build();
    retry.add_css_class("suggested-action");
    actions.append(&retry);
    actions.append(&wip_action_button(
        "Start daemon",
        "win.start-daemon",
        "No safe daemon start operation exists",
    ));
    banner.append(&actions);
    (banner, retry)
}

fn empty_state() -> (gtk::Box, gtk::Label, gtk::Label, adw::WrapBox) {
    let state = gtk::Box::new(gtk::Orientation::Vertical, 8);
    state.add_css_class("stm-empty-state");
    let icon = gtk::Image::from_icon_name("network-server-symbolic");
    icon.set_pixel_size(42);
    icon.add_css_class("dim-label");
    let title = gtk::Label::new(Some("No profiles yet"));
    title.add_css_class("title-3");
    let description = gtk::Label::new(Some(
        "Create a profile for one SSH server and the ports you want forwarded.",
    ));
    description.set_wrap(true);
    description.set_justify(gtk::Justification::Center);
    description.add_css_class("dim-label");
    state.append(&icon);
    state.append(&title);
    state.append(&description);

    let actions = wrap_actions(gtk::Align::Center);
    let create = gtk::Button::builder()
        .label("New profile")
        .action_name("win.new-profile")
        .build();
    create.add_css_class("suggested-action");
    actions.append(&create);
    actions.append(&wip_action_button(
        "Import from ~/.ssh/config",
        "win.import-ssh",
        "SSH configuration import is not implemented",
    ));
    state.append(&actions);

    (state, title, description, actions)
}

fn message_state(icon_name: &str, title: &str, description: &str) -> gtk::Box {
    let state = gtk::Box::new(gtk::Orientation::Vertical, 7);
    state.add_css_class("stm-empty-state");
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(32);
    icon.add_css_class("dim-label");
    let title = gtk::Label::new(Some(title));
    title.add_css_class("title-4");
    let description = gtk::Label::new(Some(description));
    description.add_css_class("dim-label");
    description.set_wrap(true);
    state.append(&icon);
    state.append(&title);
    state.append(&description);
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    fn availability() -> ActionAvailability {
        ActionAvailability {
            open_details: true,
            connect: false,
            cancel_connection: false,
            disconnect: false,
            retry: false,
            edit: true,
            duplicate: true,
            delete: true,
            pin: true,
            toggle_auto_reconnect: true,
        }
    }

    #[test]
    fn primary_action_is_selected_only_from_structured_availability() {
        let mut actions = availability();
        actions.retry = true;
        assert_eq!(primary_action(&actions), Some(PrimaryAction::Retry));

        actions.connect = true;
        assert_eq!(primary_action(&actions), None, "contradictions fail closed");
    }

    #[test]
    fn failed_display_text_cannot_enable_a_different_action() {
        let mut actions = availability();
        actions.retry = true;
        let status = TunnelStatus::Failed(
            "Connected. Please disconnect now and enter a password".to_string(),
        );

        assert_eq!(primary_action(&actions), Some(PrimaryAction::Retry));
        assert_eq!(inactive_action_label(&status), "Retry");
    }

    #[test]
    fn manual_reordering_stays_within_a_pin_section() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let third = Uuid::new_v4();
        let preferences = UiPreferences {
            profile_order: vec![first, second, third],
            pinned_profiles: vec![first],
            ..UiPreferences::default()
        };

        assert_eq!(
            reorder_index(&preferences, third, second, false, false),
            Some(1)
        );
        assert_eq!(
            reorder_index(&preferences, second, third, false, true),
            Some(2)
        );
        assert_eq!(
            reorder_index(&preferences, first, second, false, false),
            None
        );
    }

    #[test]
    fn name_sort_disables_manual_reordering() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let preferences = UiPreferences {
            profile_order: vec![first, second],
            sort_mode: SortMode::Name,
            ..UiPreferences::default()
        };

        assert_eq!(
            reorder_index(&preferences, first, second, false, false),
            None
        );
        assert_eq!(
            adjacent_reorder_index(&preferences, first, ReorderDirection::Down),
            None
        );
    }

    #[test]
    fn keyboard_reordering_stays_within_the_profiles_pin_section() {
        let pinned_first = Uuid::new_v4();
        let unpinned_first = Uuid::new_v4();
        let pinned_second = Uuid::new_v4();
        let unpinned_second = Uuid::new_v4();
        let preferences = UiPreferences {
            profile_order: vec![pinned_first, unpinned_first, pinned_second, unpinned_second],
            pinned_profiles: vec![pinned_first, pinned_second],
            ..UiPreferences::default()
        };

        assert_eq!(
            adjacent_reorder_index(&preferences, pinned_first, ReorderDirection::Down),
            Some(2)
        );
        assert_eq!(
            adjacent_reorder_index(&preferences, pinned_second, ReorderDirection::Up),
            Some(0)
        );
        assert_eq!(
            adjacent_reorder_index(&preferences, pinned_first, ReorderDirection::Up),
            None
        );
        assert_eq!(
            adjacent_reorder_index(&preferences, unpinned_second, ReorderDirection::Down),
            None
        );
    }
}
