// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Daemon information and structured health-state presentation.

use adw::prelude::*;
use ssh_tunnel_gui_core::{AppSnapshot, ConnectionMode, DaemonInfo};

use crate::components::{wip_action_button, wip_notice, wrap_actions, DaemonStatusBadge};
use crate::DaemonBadgeState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DaemonPageState {
    Checking,
    Offline,
    Online,
}

impl DaemonPageState {
    fn from_flags(has_refreshed: bool, daemon_connected: bool) -> Self {
        if !has_refreshed {
            Self::Checking
        } else if daemon_connected {
            Self::Online
        } else {
            Self::Offline
        }
    }
}

struct DaemonInfoLabels {
    version: gtk::Label,
    uptime: gtk::Label,
    active_tunnels: gtk::Label,
    process_id: gtk::Label,
    client_mode: gtk::Label,
    listener_mode: gtk::Label,
    socket_path: gtk::Label,
    bind_host: gtk::Label,
    bind_port: gtk::Label,
    authentication: gtk::Label,
    group_access: gtk::Label,
    user: gtk::Label,
    started_at: gtk::Label,
    config_file: gtk::Label,
    known_hosts: gtk::Label,
    ssh_key_dir: gtk::Label,
    credential_store: gtk::Label,
    last_heartbeat: gtk::Label,
}

pub struct DaemonView {
    root: gtk::Box,
    status: DaemonStatusBadge,
    summary: gtk::Label,
    checking: gtk::Box,
    checking_spinner: gtk::Spinner,
    offline: gtk::Box,
    online: gtk::Box,
    info_content: gtk::Box,
    info_unavailable: gtk::Box,
    labels: DaemonInfoLabels,
    refresh_buttons: Vec<gtk::Button>,
}

impl DaemonView {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 14);
        root.add_css_class("stm-daemon-content");

        let status = DaemonStatusBadge::new();
        status.widget().set_halign(gtk::Align::Start);
        root.append(status.widget());

        let summary = gtk::Label::new(None);
        summary.set_xalign(0.0);
        summary.set_wrap(true);
        summary.add_css_class("stm-muted");
        root.append(&summary);

        let (checking, checking_spinner) = checking_state();
        let (offline, offline_retry) = offline_state();
        let (online, info_content, info_unavailable, labels, online_refresh_buttons) =
            online_state();
        root.append(&checking);
        root.append(&offline);
        root.append(&online);

        let mut refresh_buttons = vec![offline_retry];
        refresh_buttons.extend(online_refresh_buttons);

        Self {
            root,
            status,
            summary,
            checking,
            checking_spinner,
            offline,
            online,
            info_content,
            info_unavailable,
            labels,
            refresh_buttons,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn render(&self, snapshot: &AppSnapshot) {
        let badge = DaemonBadgeState::from_snapshot(snapshot);
        self.status.update(badge);

        let page_state =
            DaemonPageState::from_flags(snapshot.has_refreshed, snapshot.daemon_connected);
        self.checking
            .set_visible(page_state == DaemonPageState::Checking);
        self.offline
            .set_visible(page_state == DaemonPageState::Offline);
        self.online
            .set_visible(page_state == DaemonPageState::Online);
        self.checking_spinner
            .set_spinning(page_state == DaemonPageState::Checking);

        self.summary.set_label(match page_state {
            DaemonPageState::Checking => "Checking daemon health and loading its information…",
            DaemonPageState::Offline => {
                "The daemon is not reachable. Profiles remain readable while tunnel actions are disabled."
            }
            DaemonPageState::Online => {
                "The daemon is reachable. Values below are the latest structured health and information snapshot."
            }
        });

        let refresh_busy = !snapshot.has_refreshed || snapshot.refresh_in_flight;
        for button in &self.refresh_buttons {
            button.set_sensitive(!refresh_busy);
        }

        let Some(info) = snapshot.daemon_info.as_ref() else {
            self.info_content.set_visible(false);
            self.info_unavailable
                .set_visible(page_state == DaemonPageState::Online);
            return;
        };

        self.info_content
            .set_visible(page_state == DaemonPageState::Online);
        self.info_unavailable.set_visible(false);
        self.update_info(snapshot, info);
    }

    fn update_info(&self, snapshot: &AppSnapshot, info: &DaemonInfo) {
        self.labels.version.set_label(&info.version);
        self.labels
            .uptime
            .set_label(&format_uptime(info.uptime_seconds));
        self.labels
            .active_tunnels
            .set_label(&info.active_tunnels_count.to_string());
        self.labels.process_id.set_label(&info.pid.to_string());
        self.labels
            .client_mode
            .set_label(connection_mode_label(&snapshot.daemon_connection_mode));

        // These strings are diagnostics only. Presentation and actions are
        // selected exclusively from typed connection/health fields above.
        self.labels.listener_mode.set_label(&info.listener_mode);
        self.labels
            .socket_path
            .set_label(optional_text(info.socket_path.as_deref()));
        self.labels
            .bind_host
            .set_label(optional_text(info.bind_host.as_deref()));
        self.labels.bind_port.set_label(
            &info
                .bind_port
                .map(|port| port.to_string())
                .unwrap_or_else(|| "Not reported".to_string()),
        );
        self.labels.authentication.set_label(if info.require_auth {
            "Required"
        } else {
            "Not required"
        });
        self.labels.group_access.set_label(if info.group_access {
            "Enabled"
        } else {
            "Disabled"
        });
        self.labels.user.set_label(&info.user);
        self.labels.started_at.set_label(&info.started_at);
        self.labels.config_file.set_label(&info.config_file_path);
        self.labels.known_hosts.set_label(&info.known_hosts_path);
        self.labels.ssh_key_dir.set_label(&info.ssh_key_dir);
        self.labels
            .credential_store
            .set_label(optional_text(info.credential_store.as_deref()));
        self.labels.last_heartbeat.set_label(
            &snapshot
                .last_heartbeat
                .map(|timestamp| timestamp.to_rfc3339())
                .unwrap_or_else(|| "Not reported".to_string()),
        );
    }
}

fn checking_state() -> (gtk::Box, gtk::Spinner) {
    let state = message_card();
    let spinner = gtk::Spinner::new();
    spinner.set_spinning(true);
    let title = gtk::Label::new(Some("Checking daemon…"));
    title.add_css_class("title-4");
    let description = gtk::Label::new(Some("Waiting for the first structured health response."));
    description.set_wrap(true);
    description.add_css_class("stm-muted");
    state.append(&spinner);
    state.append(&title);
    state.append(&description);
    (state, spinner)
}

fn offline_state() -> (gtk::Box, gtk::Button) {
    let state = message_card();
    let icon = gtk::Image::from_icon_name("network-offline-symbolic");
    icon.set_pixel_size(34);
    icon.add_css_class("warning");
    let title = gtk::Label::new(Some("Daemon not reachable"));
    title.add_css_class("title-4");
    let description = gtk::Label::new(Some(
        "Retry the health check, or start the daemon outside the app. In-app start is not implemented yet.",
    ));
    description.set_wrap(true);
    description.set_justify(gtk::Justification::Center);
    description.add_css_class("stm-muted");

    let actions = wrap_actions(gtk::Align::Center);
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

    state.append(&icon);
    state.append(&title);
    state.append(&description);
    state.append(&actions);
    (state, retry)
}

fn online_state() -> (
    gtk::Box,
    gtk::Box,
    gtk::Box,
    DaemonInfoLabels,
    Vec<gtk::Button>,
) {
    let online = gtk::Box::new(gtk::Orientation::Vertical, 14);

    let info_content = gtk::Box::new(gtk::Orientation::Vertical, 14);
    let overview = gtk::Box::new(gtk::Orientation::Vertical, 12);
    overview.add_css_class("stm-shell-card");

    let overview_heading = gtk::Box::new(gtk::Orientation::Horizontal, 9);
    let running = gtk::Label::new(Some("Running"));
    running.set_xalign(0.0);
    running.set_hexpand(true);
    running.add_css_class("stm-card-title");
    let version = value_label();
    version.add_css_class("stm-muted");
    overview_heading.append(&running);
    overview_heading.append(&version);
    overview.append(&overview_heading);

    let metrics = adw::WrapBox::builder()
        .orientation(gtk::Orientation::Horizontal)
        .child_spacing(1)
        .line_spacing(1)
        .line_homogeneous(true)
        .hexpand(true)
        .build();
    metrics.add_css_class("stm-daemon-metrics");
    let uptime = metric(&metrics, "Uptime");
    let active_tunnels = metric(&metrics, "Active tunnels");
    let process_id = metric(&metrics, "Process ID");
    overview.append(&metrics);

    let details = gtk::Box::new(gtk::Orientation::Vertical, 0);
    details.add_css_class("stm-shell-card");
    details.add_css_class("stm-daemon-details");
    let client_mode = detail_row(
        &details,
        "Client connection",
        "Structured local configuration",
    );
    let listener_mode = detail_row(&details, "Daemon listener", "Reported by the daemon");
    let socket_path = detail_row(&details, "Socket path", "Reported endpoint");
    let bind_host = detail_row(&details, "Bind host", "Reported endpoint");
    let bind_port = detail_row(&details, "Bind port", "Reported endpoint");
    let authentication = detail_row(&details, "Authentication", "Daemon access policy");
    let group_access = detail_row(&details, "Group access", "Daemon access policy");
    let user = detail_row(&details, "User", "Daemon process owner");
    let started_at = detail_row(&details, "Started at", "Daemon-reported timestamp");
    let config_file = detail_row(&details, "Config file", "Daemon configuration");
    let known_hosts = detail_row(&details, "Known hosts", "Daemon host-key database");
    let ssh_key_dir = detail_row(&details, "SSH key directory", "On the daemon host");
    let credential_store = detail_row(
        &details,
        "Credential store",
        "Diagnostic only; never used to select an action",
    );
    let last_heartbeat = detail_row(&details, "Last heartbeat", "Structured event timestamp");

    let controls = wrap_actions(gtk::Align::End);
    let refresh = gtk::Button::builder()
        .label("Refresh")
        .action_name("win.refresh")
        .tooltip_text("Refresh daemon health and information")
        .build();
    controls.append(&refresh);
    controls.append(&wip_action_button(
        "Restart daemon",
        "win.restart-daemon",
        "No safe daemon restart operation exists",
    ));
    controls.append(&wip_action_button(
        "Stop daemon",
        "win.shutdown-daemon",
        "Daemon shutdown is deferred by the current GUI scope",
    ));

    info_content.append(&overview);
    info_content.append(&details);

    let info_unavailable = message_card();
    let unavailable_title = gtk::Label::new(Some("Daemon information unavailable"));
    unavailable_title.add_css_class("title-4");
    let unavailable_description = gtk::Label::new(Some(
        "The health endpoint is reachable, but this refresh did not return the daemon information payload.",
    ));
    unavailable_description.set_wrap(true);
    unavailable_description.set_justify(gtk::Justification::Center);
    unavailable_description.add_css_class("stm-muted");
    info_unavailable.append(&unavailable_title);
    info_unavailable.append(&unavailable_description);
    info_unavailable.set_visible(false);

    online.append(&info_content);
    online.append(&info_unavailable);
    online.append(&controls);
    online.append(&wip_notice(
        "Daemon lifecycle",
        "Start, restart, and shutdown are visible for planning but remain WIP in the current GUI scope.",
    ));

    (
        online,
        info_content,
        info_unavailable,
        DaemonInfoLabels {
            version,
            uptime,
            active_tunnels,
            process_id,
            client_mode,
            listener_mode,
            socket_path,
            bind_host,
            bind_port,
            authentication,
            group_access,
            user,
            started_at,
            config_file,
            known_hosts,
            ssh_key_dir,
            credential_store,
            last_heartbeat,
        },
        vec![refresh],
    )
}

fn message_card() -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 9);
    card.add_css_class("stm-shell-card");
    card.add_css_class("stm-daemon-message");
    card.set_halign(gtk::Align::Fill);
    card
}

fn metric(container: &adw::WrapBox, title: &str) -> gtk::Label {
    let cell = gtk::Box::new(gtk::Orientation::Vertical, 4);
    cell.set_hexpand(true);
    cell.set_width_request(108);
    cell.add_css_class("stm-daemon-metric");
    let title = gtk::Label::new(Some(title));
    title.set_xalign(0.0);
    title.add_css_class("stm-field-label");
    let value = value_label();
    value.add_css_class("title-4");
    cell.append(&title);
    cell.append(&value);
    container.append(&cell);
    value
}

fn detail_row(container: &gtk::Box, title: &str, description: &str) -> gtk::Label {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("stm-daemon-detail-row");

    let heading = gtk::Box::new(gtk::Orientation::Vertical, 2);
    heading.set_hexpand(true);
    let title = gtk::Label::new(Some(title));
    title.set_xalign(0.0);
    let description = gtk::Label::new(Some(description));
    description.set_xalign(0.0);
    description.set_wrap(true);
    description.add_css_class("stm-muted");
    heading.append(&title);
    heading.append(&description);

    let value = value_label();
    value.set_max_width_chars(48);
    row.append(&heading);
    row.append(&value);
    container.append(&row);
    value
}

fn value_label() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_xalign(1.0);
    label.set_wrap(true);
    label.set_selectable(true);
    label
}

fn optional_text(value: Option<&str>) -> &str {
    value
        .filter(|text| !text.is_empty())
        .unwrap_or("Not reported")
}

fn connection_mode_label(mode: &ConnectionMode) -> &'static str {
    match mode {
        ConnectionMode::UnixSocket => "Unix socket",
        ConnectionMode::Http => "HTTP",
        ConnectionMode::Https => "HTTPS",
    }
}

fn format_uptime(total_seconds: u64) -> String {
    let days = total_seconds / 86_400;
    let hours = (total_seconds % 86_400) / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;

    if days > 0 {
        format!("{days}d {hours:02}h {minutes:02}m")
    } else if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_state_uses_only_structured_health_flags() {
        assert_eq!(
            DaemonPageState::from_flags(false, true),
            DaemonPageState::Checking
        );
        assert_eq!(
            DaemonPageState::from_flags(true, false),
            DaemonPageState::Offline
        );
        assert_eq!(
            DaemonPageState::from_flags(true, true),
            DaemonPageState::Online
        );
    }

    #[test]
    fn uptime_is_formatted_from_the_numeric_daemon_field() {
        assert_eq!(format_uptime(0), "0s");
        assert_eq!(format_uptime(65), "1m 05s");
        assert_eq!(format_uptime(14_523), "4h 02m");
        assert_eq!(format_uptime(533_100), "6d 04h 05m");
    }

    #[test]
    fn connection_mode_is_selected_by_the_enum_variant() {
        assert_eq!(
            connection_mode_label(&ConnectionMode::UnixSocket),
            "Unix socket"
        );
        assert_eq!(connection_mode_label(&ConnectionMode::Http), "HTTP");
        assert_eq!(connection_mode_label(&ConnectionMode::Https), "HTTPS");
    }

    #[test]
    fn descriptive_text_has_no_action_semantics() {
        assert_eq!(
            optional_text(Some("please restart and enter password")),
            "please restart and enter password"
        );
        assert_eq!(optional_text(None), "Not reported");
    }
}
