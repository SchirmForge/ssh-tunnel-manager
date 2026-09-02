// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Non-blocking first-launch client connection setup.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use ssh_tunnel_gui_core::{
    ClientSetupDiscovery, ClientSetupDraft, ClientSetupField, ClientSetupIssue,
    ClientSetupIssueKind, ClientSetupRepository, ClientSetupValidationCode,
    ClientSetupValidationErrors, ConnectionMode, DaemonClientConfig,
};

thread_local! {
    static ACTIVE_SETUP: RefCell<Option<Rc<SetupWizard>>> = const { RefCell::new(None) };
}

type CompletionHandler = Rc<dyn Fn(DaemonClientConfig)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetupPage {
    Overview,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SetupFlowState {
    page: SetupPage,
    runtime_started: bool,
}

impl Default for SetupFlowState {
    fn default() -> Self {
        Self {
            page: SetupPage::Overview,
            runtime_started: false,
        }
    }
}

impl SetupFlowState {
    fn show_manual(&mut self) {
        if !self.runtime_started {
            self.page = SetupPage::Manual;
        }
    }

    fn cancel_manual(&mut self) {
        if !self.runtime_started {
            self.page = SetupPage::Overview;
        }
    }

    fn begin_runtime(&mut self) -> bool {
        if self.runtime_started {
            false
        } else {
            self.runtime_started = true;
            true
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SetupFieldVisibility {
    socket: bool,
    network: bool,
    fingerprint: bool,
}

impl SetupFieldVisibility {
    fn for_mode(mode: &ConnectionMode) -> Self {
        Self {
            socket: matches!(mode, ConnectionMode::UnixSocket),
            network: matches!(mode, ConnectionMode::Http | ConnectionMode::Https),
            fingerprint: matches!(mode, ConnectionMode::Https),
        }
    }
}

pub fn present(
    application: &adw::Application,
    repository: ClientSetupRepository,
    discovery: ClientSetupDiscovery,
    on_complete: CompletionHandler,
) {
    let wizard = SetupWizard::new(application, repository, on_complete);
    wizard.apply_discovery(discovery);
    wizard.install_handlers();

    ACTIVE_SETUP.with(|active| active.replace(Some(wizard.clone())));
    wizard.window.connect_close_request(|_| {
        ACTIVE_SETUP.with(|active| active.replace(None));
        gtk::glib::Propagation::Proceed
    });
    wizard.window.present();
}

pub fn present_unavailable(application: &adw::Application, message: &str) {
    let window = adw::ApplicationWindow::builder()
        .application(application)
        .title("SSH Tunnel Manager setup")
        .default_width(560)
        .default_height(360)
        .build();

    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.set_margin_top(48);
    content.set_margin_bottom(48);
    content.set_margin_start(36);
    content.set_margin_end(36);
    content.set_valign(gtk::Align::Center);

    let icon = gtk::Image::from_icon_name("dialog-error-symbolic");
    icon.set_pixel_size(48);
    let title = gtk::Label::new(Some("Client setup is unavailable"));
    title.add_css_class("title-1");
    let detail = gtk::Label::new(Some(message));
    detail.set_wrap(true);
    detail.set_justify(gtk::Justification::Center);
    detail.set_accessible_role(gtk::AccessibleRole::Alert);
    let quit = gtk::Button::with_label("Quit");
    quit.add_css_class("suggested-action");
    let application = application.clone();
    quit.connect_clicked(move |_| application.quit());

    content.append(&icon);
    content.append(&title);
    content.append(&detail);
    content.append(&quit);
    window.set_content(Some(&content));
    window.present();
}

struct SetupWizard {
    application: adw::Application,
    repository: ClientSetupRepository,
    on_complete: CompletionHandler,
    flow: RefCell<SetupFlowState>,
    snippet_available: Cell<bool>,
    skip_ssh_setup_warning: Cell<bool>,
    window: adw::ApplicationWindow,
    stack: gtk::Stack,
    overview_description: gtk::Label,
    overview_error: gtk::Label,
    import_button: gtk::Button,
    mode_row: adw::ComboRow,
    socket_group: adw::PreferencesGroup,
    socket_row: adw::EntryRow,
    network_group: adw::PreferencesGroup,
    host_row: adw::EntryRow,
    port_row: adw::EntryRow,
    fingerprint_row: adw::EntryRow,
    token_row: adw::PasswordEntryRow,
    validation_label: gtk::Label,
    save_button: gtk::Button,
    cancel_button: gtk::Button,
    retry_button: gtk::Button,
    manual_button: gtk::Button,
    quit_button: gtk::Button,
}

impl SetupWizard {
    fn new(
        application: &adw::Application,
        repository: ClientSetupRepository,
        on_complete: CompletionHandler,
    ) -> Rc<Self> {
        let window = adw::ApplicationWindow::builder()
            .application(application)
            .title("SSH Tunnel Manager setup")
            .default_width(640)
            .default_height(680)
            .width_request(380)
            .height_request(460)
            .build();

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::SlideLeftRight)
            .build();

        let overview = gtk::Box::new(gtk::Orientation::Vertical, 16);
        overview.set_margin_top(48);
        overview.set_margin_bottom(48);
        overview.set_margin_start(36);
        overview.set_margin_end(36);
        overview.set_valign(gtk::Align::Center);

        let icon = gtk::Image::from_icon_name("network-server-symbolic");
        icon.set_pixel_size(64);
        let title = gtk::Label::new(Some("Connect to the daemon"));
        title.add_css_class("title-1");
        let overview_description = gtk::Label::new(None);
        overview_description.set_wrap(true);
        overview_description.set_justify(gtk::Justification::Center);
        overview_description.set_max_width_chars(58);

        let overview_error = gtk::Label::new(None);
        overview_error.add_css_class("error");
        overview_error.set_accessible_role(gtk::AccessibleRole::Alert);
        overview_error.set_wrap(true);
        overview_error.set_justify(gtk::Justification::Center);
        overview_error.set_visible(false);

        let import_button = gtk::Button::with_label("Use detected configuration");
        import_button.add_css_class("suggested-action");
        let manual_button = gtk::Button::with_label("Configure manually");
        let retry_button = gtk::Button::with_label("Check again");
        let quit_button = gtk::Button::with_label("Quit");

        let buttons = gtk::Box::new(gtk::Orientation::Vertical, 8);
        buttons.set_halign(gtk::Align::Center);
        buttons.set_size_request(280, -1);
        buttons.append(&import_button);
        buttons.append(&manual_button);
        buttons.append(&retry_button);
        buttons.append(&quit_button);

        overview.append(&icon);
        overview.append(&title);
        overview.append(&overview_description);
        overview.append(&overview_error);
        overview.append(&buttons);
        stack.add_named(&overview, Some("overview"));

        let manual_toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(
            &adw::WindowTitle::builder()
                .title("Daemon connection")
                .subtitle("Stored in cli.toml")
                .build(),
        ));
        manual_toolbar.add_top_bar(&header);

        let preferences = adw::PreferencesPage::new();

        let validation_group = adw::PreferencesGroup::new();
        let validation_label = gtk::Label::new(None);
        validation_label.add_css_class("error");
        validation_label.set_accessible_role(gtk::AccessibleRole::Alert);
        validation_label.set_wrap(true);
        validation_label.set_xalign(0.0);
        validation_label.set_visible(false);
        validation_group.add(&validation_label);
        preferences.add(&validation_group);

        let mode_group = adw::PreferencesGroup::new();
        mode_group.set_title("Connection mode");
        mode_group.set_description(Some(
            "Use a local socket when the daemon runs for this user. Use HTTPS for a remote daemon; HTTP is intended only for trusted local testing.",
        ));
        let modes = gtk::StringList::new(&[
            "Unix socket (local)",
            "HTTP (local testing)",
            "HTTPS (network)",
        ]);
        let mode_row = adw::ComboRow::builder()
            .title("Connect using")
            .model(&modes)
            .selected(0)
            .build();
        mode_group.add(&mode_row);
        preferences.add(&mode_group);

        let socket_group = adw::PreferencesGroup::new();
        socket_group.set_title("Unix socket");
        socket_group.set_description(Some(
            "Leave the path empty to discover the user or system daemon socket automatically.",
        ));
        let socket_row = adw::EntryRow::new();
        socket_row.set_title("Socket path override (optional)");
        socket_group.add(&socket_row);
        preferences.add(&socket_group);

        let network_group = adw::PreferencesGroup::new();
        network_group.set_title("Network endpoint");
        let host_row = adw::EntryRow::new();
        host_row.set_title("Daemon host");
        host_row.set_text("127.0.0.1");
        let port_row = adw::EntryRow::new();
        port_row.set_title("Daemon port");
        port_row.set_text("3443");
        let fingerprint_row = adw::EntryRow::new();
        fingerprint_row.set_title("TLS certificate fingerprint");
        network_group.add(&host_row);
        network_group.add(&port_row);
        network_group.add(&fingerprint_row);
        preferences.add(&network_group);

        let authentication_group = adw::PreferencesGroup::new();
        authentication_group.set_title("Authentication");
        authentication_group.set_description(Some(
            "Enter the API token generated by the daemon. It is saved only in the protected client configuration file.",
        ));
        let token_row = adw::PasswordEntryRow::new();
        token_row.set_title("Authentication token");
        authentication_group.add(&token_row);
        preferences.add(&authentication_group);

        let action_group = adw::PreferencesGroup::new();
        let action_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        action_box.set_halign(gtk::Align::End);
        let cancel_button = gtk::Button::with_label("Cancel");
        let save_button = gtk::Button::with_label("Save and continue");
        save_button.add_css_class("suggested-action");
        action_box.append(&cancel_button);
        action_box.append(&save_button);
        action_group.add(&action_box);
        preferences.add(&action_group);

        let scroller = gtk::ScrolledWindow::new();
        scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroller.set_child(Some(&preferences));
        manual_toolbar.set_content(Some(&scroller));
        stack.add_named(&manual_toolbar, Some("manual"));

        window.set_content(Some(&stack));

        let wizard = Rc::new(Self {
            application: application.clone(),
            repository,
            on_complete,
            flow: RefCell::new(SetupFlowState::default()),
            snippet_available: Cell::new(false),
            skip_ssh_setup_warning: Cell::new(false),
            window,
            stack,
            overview_description,
            overview_error,
            import_button,
            mode_row,
            socket_group,
            socket_row,
            network_group,
            host_row,
            port_row,
            fingerprint_row,
            token_row,
            validation_label,
            save_button,
            cancel_button,
            retry_button,
            manual_button,
            quit_button,
        });
        wizard.update_field_visibility();
        wizard
    }

    fn install_handlers(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.mode_row.connect_selected_notify(move |_| {
            if let Some(wizard) = weak.upgrade() {
                wizard.clear_validation();
                wizard.update_field_visibility();
            }
        });

        let weak = Rc::downgrade(self);
        self.manual_button.connect_clicked(move |_| {
            if let Some(wizard) = weak.upgrade() {
                wizard.show_manual();
            }
        });

        let weak = Rc::downgrade(self);
        self.cancel_button.connect_clicked(move |_| {
            if let Some(wizard) = weak.upgrade() {
                wizard.cancel_manual();
            }
        });

        let weak = Rc::downgrade(self);
        self.retry_button.connect_clicked(move |_| {
            if let Some(wizard) = weak.upgrade() {
                wizard.retry_discovery();
            }
        });

        let weak = Rc::downgrade(self);
        self.import_button.connect_clicked(move |_| {
            if let Some(wizard) = weak.upgrade() {
                wizard.import_snippet();
            }
        });

        let weak = Rc::downgrade(self);
        self.save_button.connect_clicked(move |_| {
            if let Some(wizard) = weak.upgrade() {
                wizard.save_manual_configuration();
            }
        });

        let application = self.application.clone();
        self.quit_button
            .connect_clicked(move |_| application.quit());
    }

    fn apply_discovery(&self, discovery: ClientSetupDiscovery) {
        self.overview_error.set_visible(false);
        match discovery {
            ClientSetupDiscovery::Ready(config) => self.complete(config),
            ClientSetupDiscovery::SnippetAvailable => {
                self.snippet_available.set(true);
                self.import_button.set_visible(true);
                self.overview_description.set_label(
                    "A daemon-generated client configuration was found. You can import it or enter the connection settings manually.",
                );
                self.show_overview();
            }
            ClientSetupDiscovery::SetupRequired(issue) => {
                self.snippet_available.set(false);
                self.import_button.set_visible(false);
                self.apply_setup_issue(&issue);
                if issue.kind == ClientSetupIssueKind::Incomplete {
                    if let Ok(draft) = self.repository.load_existing() {
                        self.populate_draft(&draft);
                    }
                }
                self.show_overview();
            }
        }
    }

    fn apply_setup_issue(&self, issue: &ClientSetupIssue) {
        let description = match issue.kind {
            ClientSetupIssueKind::Missing => {
                "No client connection configuration was found. Configure how this application should connect to an existing daemon."
            }
            ClientSetupIssueKind::Unreadable => {
                "The existing client configuration cannot be read. Review its permissions or save a replacement configuration."
            }
            ClientSetupIssueKind::Malformed => {
                "The existing client configuration is malformed. You can save a replacement after reviewing the settings."
            }
            ClientSetupIssueKind::Incomplete => {
                "The existing client configuration is incomplete. Review the highlighted settings and save a replacement."
            }
        };
        self.overview_description.set_label(description);
        self.overview_error.set_label(&issue.message);
        self.overview_error
            .set_visible(issue.kind != ClientSetupIssueKind::Missing);
    }

    fn retry_discovery(&self) {
        self.apply_discovery(self.repository.discover());
    }

    fn import_snippet(&self) {
        if !self.snippet_available.get() {
            return;
        }

        match self.repository.load_snippet() {
            Ok(draft) => {
                self.populate_draft(&draft);
                match draft.validate() {
                    Ok(config) => self.persist_and_complete(config),
                    Err(errors) => {
                        self.show_manual();
                        self.show_validation(&errors);
                    }
                }
            }
            Err(error) => {
                self.overview_error.set_label(&format!(
                    "The detected configuration could not be imported: {error}"
                ));
                self.overview_error.set_visible(true);
                self.show_manual();
            }
        }
    }

    fn save_manual_configuration(&self) {
        let draft = self.collect_draft();
        match draft.validate() {
            Ok(config) => self.persist_and_complete(config),
            Err(errors) => self.show_validation(&errors),
        }
    }

    fn persist_and_complete(&self, config: DaemonClientConfig) {
        match self.repository.persist_config(&config) {
            Ok(()) => self.complete(config),
            Err(error) => {
                self.validation_label
                    .set_label(&format!("The configuration could not be saved: {error}"));
                self.validation_label.set_visible(true);
            }
        }
    }

    fn complete(&self, config: DaemonClientConfig) {
        if !self.flow.borrow_mut().begin_runtime() {
            return;
        }
        (self.on_complete)(config);
        ACTIVE_SETUP.with(|active| active.replace(None));
        self.window.close();
    }

    fn show_manual(&self) {
        self.flow.borrow_mut().show_manual();
        self.stack.set_visible_child_name("manual");
        self.token_row.grab_focus();
    }

    fn cancel_manual(&self) {
        self.flow.borrow_mut().cancel_manual();
        self.clear_validation();
        self.show_overview();
    }

    fn show_overview(&self) {
        self.stack.set_visible_child_name("overview");
    }

    fn selected_mode(&self) -> ConnectionMode {
        match self.mode_row.selected() {
            1 => ConnectionMode::Http,
            2 => ConnectionMode::Https,
            _ => ConnectionMode::UnixSocket,
        }
    }

    fn populate_draft(&self, draft: &ClientSetupDraft) {
        self.mode_row.set_selected(match draft.connection_mode {
            ConnectionMode::UnixSocket => 0,
            ConnectionMode::Http => 1,
            ConnectionMode::Https => 2,
        });
        self.socket_row.set_text(&draft.daemon_url);
        self.host_row.set_text(&draft.daemon_host);
        self.port_row.set_text(&draft.daemon_port);
        self.fingerprint_row.set_text(&draft.tls_cert_fingerprint);
        self.token_row.set_text(draft.authentication_token());
        self.skip_ssh_setup_warning
            .set(draft.skip_ssh_setup_warning);
        self.update_field_visibility();
    }

    fn collect_draft(&self) -> ClientSetupDraft {
        let mut draft = ClientSetupDraft::default();
        draft.connection_mode = self.selected_mode();
        draft.daemon_url = self.socket_row.text().to_string();
        draft.daemon_host = self.host_row.text().to_string();
        draft.daemon_port = self.port_row.text().to_string();
        draft.tls_cert_fingerprint = self.fingerprint_row.text().to_string();
        draft.set_authentication_token(self.token_row.text().to_string());
        draft.skip_ssh_setup_warning = self.skip_ssh_setup_warning.get();
        draft
    }

    fn update_field_visibility(&self) {
        let visibility = SetupFieldVisibility::for_mode(&self.selected_mode());
        self.socket_group.set_visible(visibility.socket);
        self.network_group.set_visible(visibility.network);
        self.fingerprint_row.set_visible(visibility.fingerprint);
    }

    fn clear_validation(&self) {
        self.validation_label.set_visible(false);
        for row in [
            self.host_row.upcast_ref::<gtk::Widget>(),
            self.port_row.upcast_ref::<gtk::Widget>(),
            self.token_row.upcast_ref::<gtk::Widget>(),
            self.fingerprint_row.upcast_ref::<gtk::Widget>(),
        ] {
            row.remove_css_class("error");
        }
    }

    fn show_validation(&self, errors: &ClientSetupValidationErrors) {
        self.clear_validation();
        let mut messages = Vec::new();
        for error in errors.errors() {
            let (row, message): (&gtk::Widget, &str) = match (error.field, error.code) {
                (ClientSetupField::Host, ClientSetupValidationCode::Required) => (
                    self.host_row.upcast_ref(),
                    "Enter the daemon host or IP address.",
                ),
                (ClientSetupField::Host, ClientSetupValidationCode::InvalidHost) => (
                    self.host_row.upcast_ref(),
                    "Enter a valid hostname, IPv4 address, or IPv6 address.",
                ),
                (ClientSetupField::Host, ClientSetupValidationCode::NonLoopbackHttp) => (
                    self.host_row.upcast_ref(),
                    "Plain HTTP is restricted to localhost. Use HTTPS for a remote daemon.",
                ),
                (ClientSetupField::Port, ClientSetupValidationCode::InvalidPort) => (
                    self.port_row.upcast_ref(),
                    "Enter a port between 1 and 65535.",
                ),
                (ClientSetupField::AuthenticationToken, ClientSetupValidationCode::Required) => (
                    self.token_row.upcast_ref(),
                    "Enter the authentication token generated by the daemon.",
                ),
                (ClientSetupField::TlsFingerprint, ClientSetupValidationCode::Required) => (
                    self.fingerprint_row.upcast_ref(),
                    "Enter the daemon TLS certificate fingerprint.",
                ),
                (
                    ClientSetupField::TlsFingerprint,
                    ClientSetupValidationCode::InvalidFingerprint,
                ) => (
                    self.fingerprint_row.upcast_ref(),
                    "Enter the 32-byte SHA-256 fingerprint as colon-separated hexadecimal octets.",
                ),
                _ => continue,
            };
            row.add_css_class("error");
            messages.push(message);
        }
        self.validation_label.set_label(&messages.join("\n"));
        self.validation_label.set_visible(!messages.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_visibility_is_derived_only_from_connection_mode() {
        assert_eq!(
            SetupFieldVisibility::for_mode(&ConnectionMode::UnixSocket),
            SetupFieldVisibility {
                socket: true,
                network: false,
                fingerprint: false,
            }
        );
        assert_eq!(
            SetupFieldVisibility::for_mode(&ConnectionMode::Http),
            SetupFieldVisibility {
                socket: false,
                network: true,
                fingerprint: false,
            }
        );
        assert_eq!(
            SetupFieldVisibility::for_mode(&ConnectionMode::Https),
            SetupFieldVisibility {
                socket: false,
                network: true,
                fingerprint: true,
            }
        );
    }

    #[test]
    fn cancel_returns_to_setup_without_starting_runtime() {
        let mut flow = SetupFlowState::default();
        flow.show_manual();
        assert_eq!(flow.page, SetupPage::Manual);
        flow.cancel_manual();
        assert_eq!(flow.page, SetupPage::Overview);
        assert!(!flow.runtime_started);
    }

    #[test]
    fn runtime_can_start_only_once() {
        let mut flow = SetupFlowState::default();
        assert!(flow.begin_runtime());
        assert!(!flow.begin_runtime());
    }
}
