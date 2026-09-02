// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use gtk::{gio, glib};
use ssh_tunnel_gui_core::{
    AppCommand, AppController, AppSnapshot, ClientSetupDiscovery, ClientSetupRepository,
    ControllerEffect, ControllerEvent, DaemonClientConfig, PresentationRequest,
};
use tokio::sync::mpsc;

use crate::action_router::{ActionRoute, ShellAction, ShellPage};
use crate::auth_dialog::{sync_action, AuthDialogEvent, AuthDialogHandle, AuthDialogSync};
use crate::bridge::{RuntimeBridge, RuntimeMessage};
use crate::components::DaemonStatusBadge;
use crate::daemon_view::DaemonView;
use crate::profile_editor::{present as present_profile_editor, present_delete_confirmation};
use crate::profile_list::{ProfileListEvent, ProfileListView};
use crate::setup_wizard;
use crate::shell_state::ShellViewState;

thread_local! {
    static ACTIVE_SESSION: RefCell<Option<Rc<UiSession>>> = const { RefCell::new(None) };
}

pub fn activate(application: &adw::Application) {
    if let Some(window) = application.active_window() {
        window.present();
        return;
    }

    let repository = match ClientSetupRepository::for_default_paths() {
        Ok(repository) => repository,
        Err(error) => {
            setup_wizard::present_unavailable(
                application,
                &format!("Unable to locate the client configuration directory: {error}"),
            );
            return;
        }
    };

    match repository.discover() {
        ClientSetupDiscovery::Ready(config) => start_main(application, config),
        discovery => {
            let setup_application = application.clone();
            let completion_application = application.clone();
            setup_wizard::present(
                &setup_application,
                repository,
                discovery,
                Rc::new(move |config| start_main(&completion_application, config)),
            );
        }
    }
}

fn start_main(application: &adw::Application, config: DaemonClientConfig) {
    let shell = Shell::new(application);
    let (bridge, receiver, startup_error) = match RuntimeBridge::spawn_with_config(config) {
        Ok((bridge, receiver)) => (Some(bridge), Some(receiver), None),
        Err(error) => (
            None,
            None,
            Some(format!("Unable to start the GUI runtime: {error}")),
        ),
    };

    let mut controller = AppController::new();
    if let Some(message) = startup_error.as_ref() {
        controller.apply_event(ControllerEvent::DaemonConnectionChanged(false));
        controller.apply_event(ControllerEvent::RefreshFinished);
        controller.apply_event(ControllerEvent::RuntimeError {
            profile_id: None,
            message: message.clone(),
        });
    }

    let session = Rc::new(UiSession {
        controller: RefCell::new(controller),
        bridge,
        shell,
        auth_dialog: RefCell::new(None),
        rejected_auth_request: RefCell::new(None),
    });

    if let Some(message) = startup_error {
        session.shell.show_toast(&message);
    }

    session.install_actions();
    session.install_profile_list_handler();
    if let Some(receiver) = receiver {
        attach_runtime_receiver(receiver, Rc::downgrade(&session));
    }

    ACTIVE_SESSION.with(|active| active.replace(Some(session.clone())));
    session.shell.window.connect_close_request(|_| {
        ACTIVE_SESSION.with(|active| active.replace(None));
        glib::Propagation::Proceed
    });

    session.render();
    session.shell.window.present();
}

struct UiSession {
    controller: RefCell<AppController>,
    bridge: Option<RuntimeBridge>,
    shell: Shell,
    auth_dialog: RefCell<Option<AuthDialogHandle>>,
    rejected_auth_request: RefCell<Option<uuid::Uuid>>,
}

impl UiSession {
    fn install_actions(self: &Rc<Self>) {
        self.add_action("refresh", ShellAction::Refresh);
        self.add_action("new-profile", ShellAction::CreateProfile);
        self.add_action("profiles", ShellAction::OpenProfiles);
        self.add_action("daemon", ShellAction::OpenDaemon);
        self.add_action("import-ssh", ShellAction::ImportSshConfig);
        self.add_action("shutdown-daemon", ShellAction::ShutdownDaemon);
        self.add_action("start-daemon", ShellAction::StartDaemon);
        self.add_action("restart-daemon", ShellAction::RestartDaemon);
        self.install_focus_search_action();
    }

    fn install_focus_search_action(self: &Rc<Self>) {
        let action = gio::SimpleAction::new("focus-search", None);
        let weak_session = Rc::downgrade(self);
        action.connect_activate(move |_, _| {
            if let Some(session) = weak_session.upgrade() {
                session.shell.navigate(ShellPage::Profiles);
                session.shell.profile_list.focus_search();
            }
        });
        self.shell.window.add_action(&action);
    }

    fn add_action(self: &Rc<Self>, name: &str, shell_action: ShellAction) {
        let action = gio::SimpleAction::new(name, None);
        let weak_session = Rc::downgrade(self);
        action.connect_activate(move |_, _| {
            if let Some(session) = weak_session.upgrade() {
                session.activate_shell_action(shell_action);
            }
        });
        self.shell.window.add_action(&action);
    }

    fn install_profile_list_handler(self: &Rc<Self>) {
        let weak_session = Rc::downgrade(self);
        self.shell.profile_list.set_handler(Rc::new(move |event| {
            let Some(session) = weak_session.upgrade() else {
                return;
            };
            match event {
                ProfileListEvent::Command(command) => session.dispatch(command),
            }
        }));
    }

    fn activate_shell_action(self: &Rc<Self>, action: ShellAction) {
        let features = self.controller.borrow().snapshot().features;
        match action.route(&features) {
            ActionRoute::Dispatch(command) => self.dispatch(command),
            ActionRoute::Navigate(page) => self.shell.navigate(page),
            ActionRoute::Wip { reason } => {
                self.shell.show_toast(&format!("WIP — {reason}"));
            }
            ActionRoute::Unavailable { reason } => self.shell.show_toast(&reason),
        }
    }

    fn dispatch(self: &Rc<Self>, command: AppCommand) {
        let effects = match self.controller.borrow_mut().dispatch(command) {
            Ok(effects) => effects,
            Err(error) => {
                self.shell.show_toast(&error.to_string());
                self.render();
                return;
            }
        };
        self.render();

        let Some(bridge) = &self.bridge else {
            if !effects.is_empty() {
                self.shell.show_toast("The GUI runtime is unavailable");
            }
            for effect in effects {
                self.apply_effect_delivery_failure(effect, "The GUI runtime is unavailable");
            }
            return;
        };

        for effect in effects {
            if let Err(error) = bridge.execute(effect) {
                self.apply_effect_delivery_failure(error.0, "The GUI runtime stopped unexpectedly");
                return;
            }
        }
    }

    fn apply_effect_delivery_failure(self: &Rc<Self>, effect: ControllerEffect, message: &str) {
        if matches!(effect, ControllerEffect::Refresh) {
            self.controller
                .borrow_mut()
                .apply_event(ControllerEvent::RefreshFinished);
        }
        let event = match effect {
            ControllerEffect::SubmitAuthentication(submission) => {
                ControllerEvent::AuthenticationSubmissionFailed {
                    request_id: submission.request_id,
                    message: message.to_string(),
                }
            }
            ControllerEffect::CancelAuthentication { request_id, .. } => {
                ControllerEvent::AuthenticationSubmissionFailed {
                    request_id,
                    message: message.to_string(),
                }
            }
            _ => ControllerEvent::RuntimeError {
                profile_id: None,
                message: message.to_string(),
            },
        };
        self.apply_event(event);
    }

    fn handle_runtime_message(self: &Rc<Self>, message: RuntimeMessage) {
        match message {
            RuntimeMessage::EffectFinished(result) => {
                for event in result.events {
                    self.controller.borrow_mut().apply_event(event);
                }
                if let Some(request) = result.presentation {
                    self.handle_presentation_request(request);
                }
                self.render();
            }
            RuntimeMessage::Event(event) => self.apply_event(event),
            RuntimeMessage::InitializationFailed(message) => {
                {
                    let mut controller = self.controller.borrow_mut();
                    controller.apply_event(ControllerEvent::DaemonConnectionChanged(false));
                    controller.apply_event(ControllerEvent::RefreshFinished);
                }
                self.apply_event(ControllerEvent::RuntimeError {
                    profile_id: None,
                    message,
                });
            }
            RuntimeMessage::ListenerFailed(message) => {
                self.apply_event(ControllerEvent::RuntimeError {
                    profile_id: None,
                    message,
                });
            }
        }
    }

    fn handle_presentation_request(self: &Rc<Self>, request: PresentationRequest) {
        let weak_session = Rc::downgrade(self);
        let handler = Rc::new(move |command| {
            if let Some(session) = weak_session.upgrade() {
                session.dispatch(command);
            }
        });
        match request {
            PresentationRequest::ProfileEditor(editor) => {
                present_profile_editor(&self.shell.window, *editor, handler)
            }
            PresentationRequest::ConfirmDelete(delete) => {
                present_delete_confirmation(&self.shell.window, delete, handler)
            }
        }
    }

    fn apply_event(self: &Rc<Self>, event: ControllerEvent) {
        self.controller.borrow_mut().apply_event(event);
        self.render();
    }

    fn render(self: &Rc<Self>) {
        let snapshot = self.controller.borrow().snapshot();
        self.shell.render(&snapshot);
        self.sync_auth_dialog(&snapshot);
    }

    fn sync_auth_dialog(self: &Rc<Self>, snapshot: &AppSnapshot) {
        let current_request = self
            .auth_dialog
            .borrow()
            .as_ref()
            .map(AuthDialogHandle::request_id);
        let next_request = snapshot
            .active_auth
            .as_ref()
            .map(|prompt| prompt.request_id);

        if self
            .rejected_auth_request
            .borrow()
            .is_some_and(|request_id| Some(request_id) != next_request)
        {
            self.rejected_auth_request.replace(None);
        }

        match sync_action(current_request, next_request) {
            AuthDialogSync::Idle => {}
            AuthDialogSync::Update => {
                if let Some(dialog) = self.auth_dialog.borrow().as_ref() {
                    dialog.update(
                        snapshot.queued_auth_requests,
                        snapshot.auth_submission_in_flight,
                        snapshot.active_auth_error.as_deref(),
                    );
                }
            }
            AuthDialogSync::Close => self.close_auth_dialog(),
            AuthDialogSync::Replace => {
                self.close_auth_dialog();
                self.open_auth_dialog(snapshot);
            }
            AuthDialogSync::Open => self.open_auth_dialog(snapshot),
        }
    }

    fn open_auth_dialog(self: &Rc<Self>, snapshot: &AppSnapshot) {
        let Some(prompt) = snapshot.active_auth.as_ref() else {
            return;
        };
        if self.rejected_auth_request.borrow().as_ref() == Some(&prompt.request_id) {
            return;
        }

        let weak_session = Rc::downgrade(self);
        let handler = Rc::new(move |event| {
            let Some(session) = weak_session.upgrade() else {
                return;
            };
            let command = match event {
                AuthDialogEvent::Answer { request_id, answer } => {
                    AppCommand::AnswerAuthentication { request_id, answer }
                }
                AuthDialogEvent::Cancel { request_id } => {
                    AppCommand::CancelAuthentication { request_id }
                }
            };
            session.dispatch(command);
        });

        match AuthDialogHandle::present(
            &self.shell.window,
            prompt,
            snapshot.queued_auth_requests,
            snapshot.auth_submission_in_flight,
            snapshot.active_auth_error.as_deref(),
            handler,
        ) {
            Ok(dialog) => {
                self.auth_dialog.replace(Some(dialog));
            }
            Err(error) => {
                self.rejected_auth_request.replace(Some(prompt.request_id));
                self.shell.show_toast(&format!(
                    "Unsupported authentication request; cancelling safely: {error}"
                ));
                self.dispatch(AppCommand::CancelAuthentication {
                    request_id: prompt.request_id,
                });
            }
        }
    }

    fn close_auth_dialog(&self) {
        if let Some(dialog) = self.auth_dialog.borrow_mut().take() {
            dialog.close_without_action();
        }
    }
}

fn attach_runtime_receiver(
    mut receiver: mpsc::UnboundedReceiver<RuntimeMessage>,
    session: Weak<UiSession>,
) {
    glib::timeout_add_local(Duration::from_millis(16), move || {
        let Some(session) = session.upgrade() else {
            return glib::ControlFlow::Break;
        };

        loop {
            match receiver.try_recv() {
                Ok(message) => session.handle_runtime_message(message),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    return glib::ControlFlow::Break;
                }
            }
        }

        glib::ControlFlow::Continue
    });
}

struct Shell {
    window: adw::ApplicationWindow,
    navigation: adw::NavigationView,
    toast_overlay: adw::ToastOverlay,
    main_status: DaemonStatusBadge,
    daemon_status_button: gtk::Button,
    profile_list: ProfileListView,
    daemon_view: DaemonView,
    refresh_buttons: Vec<gtk::Button>,
    error_card: gtk::Box,
    error_label: gtk::Label,
}

impl Shell {
    fn new(application: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(application)
            .title("SSH Tunnel Manager")
            .default_width(960)
            .default_height(680)
            .width_request(420)
            .height_request(360)
            .build();

        let navigation = adw::NavigationView::new();
        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&navigation));
        window.set_content(Some(&toast_overlay));

        let main_status = DaemonStatusBadge::new();
        let profile_list = ProfileListView::new();
        let (profiles_page, error_card, error_label, profiles_refresh, daemon_status_button) =
            build_profiles_page(&main_status, &profile_list);
        let daemon_view = DaemonView::new();
        let (daemon_page, daemon_refresh) = build_daemon_page(&daemon_view);

        navigation.add(&profiles_page);
        navigation.add(&daemon_page);

        Self {
            window,
            navigation,
            toast_overlay,
            main_status,
            daemon_status_button,
            profile_list,
            daemon_view,
            refresh_buttons: vec![profiles_refresh, daemon_refresh],
            error_card,
            error_label,
        }
    }

    fn render(&self, snapshot: &AppSnapshot) {
        let state = ShellViewState::from_snapshot(snapshot);
        self.main_status.update(state.daemon);
        self.daemon_status_button
            .update_property(&[gtk::accessible::Property::Label(&format!(
                "Open daemon status: {}",
                state.daemon.label()
            ))]);
        self.daemon_view.render(snapshot);
        for button in &self.refresh_buttons {
            button.set_sensitive(snapshot.has_refreshed && !snapshot.refresh_in_flight);
        }
        self.profile_list.render(snapshot);

        self.error_card.set_visible(state.has_error);
        self.error_label
            .set_label(state.error_message.as_deref().unwrap_or_default());
    }

    fn navigate(&self, page: ShellPage) {
        let tag = match page {
            ShellPage::Profiles => "profiles",
            ShellPage::Daemon => "daemon",
        };
        if self.navigation.visible_page_tag().as_deref() == Some(tag) {
            return;
        }

        match page {
            ShellPage::Profiles => {
                self.navigation.pop_to_tag("profiles");
            }
            ShellPage::Daemon => self.navigation.push_by_tag("daemon"),
        }
    }

    fn show_toast(&self, message: &str) {
        self.toast_overlay.add_toast(adw::Toast::new(message));
    }
}

fn build_profiles_page(
    status: &DaemonStatusBadge,
    profile_list: &ProfileListView,
) -> (
    adw::NavigationPage,
    gtk::Box,
    gtk::Label,
    gtk::Button,
    gtk::Button,
) {
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(
        &adw::WindowTitle::builder()
            .title("SSH Tunnel Manager")
            .build(),
    ));

    let status_button = gtk::Button::builder()
        .action_name("win.daemon")
        .tooltip_text("Open daemon status")
        .build();
    status_button.add_css_class("flat");
    status_button.update_property(&[gtk::accessible::Property::Label("Open daemon status")]);
    status_button.set_child(Some(status.widget()));
    header.pack_start(&status_button);

    let refresh_button = gtk::Button::builder()
        .action_name("win.refresh")
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh profiles and daemon status")
        .build();
    refresh_button.update_property(&[
        gtk::accessible::Property::Label("Refresh profiles and daemon status"),
        gtk::accessible::Property::KeyShortcuts("F5 Control+R"),
    ]);
    header.pack_end(&refresh_button);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let (error_card, error_label) = error_notice();
    content.append(&error_card);
    content.append(profile_list.widget());

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroller.set_child(Some(&content));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scroller));

    (
        adw::NavigationPage::builder()
            .title("Profiles")
            .tag("profiles")
            .child(&toolbar)
            .build(),
        error_card,
        error_label,
        refresh_button,
        status_button,
    )
}

fn build_daemon_page(daemon_view: &DaemonView) -> (adw::NavigationPage, gtk::Button) {
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::builder().title("Daemon").build()));

    let refresh_button = gtk::Button::builder()
        .action_name("win.refresh")
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh daemon status")
        .build();
    refresh_button.update_property(&[
        gtk::accessible::Property::Label("Refresh daemon status"),
        gtk::accessible::Property::KeyShortcuts("F5 Control+R"),
    ]);
    header.pack_end(&refresh_button);

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroller.set_child(Some(daemon_view.widget()));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scroller));

    (
        adw::NavigationPage::builder()
            .title("Daemon")
            .tag("daemon")
            .child(&toolbar)
            .build(),
        refresh_button,
    )
}

fn error_notice() -> (gtk::Box, gtk::Label) {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    card.add_css_class("error");
    card.add_css_class("stm-shell-card");
    card.set_visible(false);

    let icon = gtk::Image::from_icon_name("dialog-error-symbolic");
    let label = gtk::Label::new(None);
    label.set_accessible_role(gtk::AccessibleRole::Alert);
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    card.append(&icon);
    card.append(&label);

    (card, label)
}
