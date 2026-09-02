// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Modal authentication presentation selected only from structured core data.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use adw::prelude::*;
use ssh_tunnel_gui_core::{AuthAnswer, AuthInputMode, AuthPromptKind, AuthPromptSnapshot};
use uuid::Uuid;

type EventHandler = Rc<dyn Fn(AuthDialogEvent)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthDialogEvent {
    Answer {
        request_id: Uuid,
        answer: AuthAnswer,
    },
    Cancel {
        request_id: Uuid,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthControlKind {
    VisibleText,
    HiddenText,
    HostKeyDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AuthDialogSpec {
    title: &'static str,
    input_label: Option<&'static str>,
    primary_label: &'static str,
    control: AuthControlKind,
    warning: bool,
}

impl AuthDialogSpec {
    fn from_prompt(prompt: &AuthPromptSnapshot) -> Result<Self, AuthDialogProtocolError> {
        let (title, input_label, primary_label) = match prompt.kind {
            AuthPromptKind::KeyPassphrase => {
                ("SSH key passphrase", Some("Passphrase"), "Authenticate")
            }
            AuthPromptKind::Password => ("SSH password", Some("Password"), "Authenticate"),
            AuthPromptKind::TwoFactorCode => {
                ("Verification code", Some("Verification code"), "Submit")
            }
            AuthPromptKind::KeyboardInteractive => {
                ("SSH authentication", Some("Response"), "Submit")
            }
            AuthPromptKind::HostKeyVerification => ("Unknown host key", None, "Accept and record"),
        };

        let control = match (prompt.kind, prompt.input_mode) {
            (AuthPromptKind::HostKeyVerification, AuthInputMode::HostKeyDecision) => {
                AuthControlKind::HostKeyDecision
            }
            (AuthPromptKind::HostKeyVerification, _) => {
                return Err(AuthDialogProtocolError::HostKeyDecisionExpected)
            }
            (_, AuthInputMode::HostKeyDecision) => {
                return Err(AuthDialogProtocolError::TextInputExpected)
            }
            (_, AuthInputMode::HiddenText) => AuthControlKind::HiddenText,
            (_, AuthInputMode::VisibleText) => AuthControlKind::VisibleText,
        };

        Ok(Self {
            title,
            input_label,
            primary_label,
            control,
            warning: matches!(prompt.kind, AuthPromptKind::HostKeyVerification),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthDialogProtocolError {
    HostKeyDecisionExpected,
    TextInputExpected,
}

impl fmt::Display for AuthDialogProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HostKeyDecisionExpected => {
                formatter.write_str("host-key request did not specify a host-key decision")
            }
            Self::TextInputExpected => {
                formatter.write_str("text authentication request specified a host-key decision")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthDialogSync {
    Idle,
    Open,
    Update,
    Replace,
    Close,
}

pub(crate) fn sync_action(current: Option<Uuid>, next: Option<Uuid>) -> AuthDialogSync {
    match (current, next) {
        (None, None) => AuthDialogSync::Idle,
        (None, Some(_)) => AuthDialogSync::Open,
        (Some(_), None) => AuthDialogSync::Close,
        (Some(current), Some(next)) if current == next => AuthDialogSync::Update,
        (Some(_), Some(_)) => AuthDialogSync::Replace,
    }
}

#[derive(Clone)]
enum TextInput {
    Visible(gtk::Entry),
    Hidden(gtk::PasswordEntry),
}

impl TextInput {
    fn widget(&self) -> gtk::Widget {
        match self {
            Self::Visible(entry) => entry.clone().upcast(),
            Self::Hidden(entry) => entry.clone().upcast(),
        }
    }

    fn text(&self) -> String {
        match self {
            Self::Visible(entry) => entry.text().to_string(),
            Self::Hidden(entry) => entry.text().to_string(),
        }
    }

    fn clear(&self) {
        match self {
            Self::Visible(entry) => entry.set_text(""),
            Self::Hidden(entry) => entry.set_text(""),
        }
    }

    fn set_sensitive(&self, sensitive: bool) {
        self.widget().set_sensitive(sensitive);
    }

    fn grab_focus(&self) {
        self.widget().grab_focus();
    }

    fn connect_activate(&self, callback: Rc<dyn Fn()>) {
        match self {
            Self::Visible(entry) => entry.connect_activate(move |_| callback()),
            Self::Hidden(entry) => entry.connect_activate(move |_| callback()),
        };
    }
}

pub struct AuthDialogHandle {
    request_id: Uuid,
    window: adw::Window,
    primary: gtk::Button,
    secondary: gtk::Button,
    input: Option<TextInput>,
    spinner: gtk::Spinner,
    progress_label: gtk::Label,
    error_label: gtk::Label,
    queue_label: gtk::Label,
    dismissing: Rc<Cell<bool>>,
    locally_submitting: Rc<Cell<bool>>,
}

impl AuthDialogHandle {
    pub fn present(
        parent: &adw::ApplicationWindow,
        prompt: &AuthPromptSnapshot,
        queued_requests: usize,
        busy: bool,
        error: Option<&str>,
        handler: EventHandler,
    ) -> Result<Self, AuthDialogProtocolError> {
        let spec = AuthDialogSpec::from_prompt(prompt)?;
        let profile_name = prompt
            .profile_name
            .clone()
            .unwrap_or_else(|| prompt.tunnel_id.to_string());
        let heading = format!("{} — {profile_name}", spec.title);
        let window = adw::Window::builder()
            .transient_for(parent)
            .modal(true)
            .title(&heading)
            .default_width(if spec.warning { 500 } else { 430 })
            .resizable(true)
            .build();

        let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
        content.add_css_class("stm-auth-content");

        let heading_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        if spec.warning {
            let warning = gtk::Image::from_icon_name("dialog-warning-symbolic");
            warning.add_css_class("warning");
            warning.set_pixel_size(20);
            heading_row.append(&warning);
        }
        let title = gtk::Label::new(Some(&heading));
        title.add_css_class("title-3");
        title.set_xalign(0.0);
        title.set_wrap(true);
        title.set_hexpand(true);
        heading_row.append(&title);
        content.append(&heading_row);

        let prompt_copy = gtk::Label::new(Some(&prompt.prompt));
        prompt_copy.add_css_class("stm-auth-prompt");
        prompt_copy.set_xalign(0.0);
        prompt_copy.set_wrap(true);
        prompt_copy.set_selectable(true);
        content.append(&prompt_copy);

        let input = match spec.control {
            AuthControlKind::VisibleText => {
                let entry = gtk::Entry::builder()
                    .activates_default(true)
                    .hexpand(true)
                    .build();
                Some(TextInput::Visible(entry))
            }
            AuthControlKind::HiddenText => {
                let entry = gtk::PasswordEntry::builder()
                    .activates_default(true)
                    .show_peek_icon(true)
                    .hexpand(true)
                    .build();
                Some(TextInput::Hidden(entry))
            }
            AuthControlKind::HostKeyDecision => None,
        };

        if let (Some(input), Some(input_label)) = (&input, spec.input_label) {
            input
                .widget()
                .update_property(&[gtk::accessible::Property::Label(input_label)]);
            content.append(&input.widget());
        }

        let explanation = gtk::Label::new(Some(if spec.warning {
            "The daemon supplied this host-key notice as unparsed copy. Accept only after verifying it independently."
        } else {
            "Cancelling stops this tunnel. Credentials saved from the profile editor are not changed here."
        }));
        explanation.add_css_class("dim-label");
        explanation.set_xalign(0.0);
        explanation.set_wrap(true);
        content.append(&explanation);

        let queue_label = gtk::Label::new(None);
        queue_label.set_accessible_role(gtk::AccessibleRole::Status);
        queue_label.add_css_class("dim-label");
        queue_label.set_xalign(0.0);
        queue_label.set_wrap(true);
        content.append(&queue_label);

        let progress = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let spinner = gtk::Spinner::new();
        progress.append(&spinner);
        let progress_label = gtk::Label::new(Some("Waiting for daemon confirmation…"));
        progress_label.set_accessible_role(gtk::AccessibleRole::Status);
        progress_label.set_xalign(0.0);
        progress.append(&progress_label);
        content.append(&progress);

        let error_label = gtk::Label::new(None);
        error_label.set_accessible_role(gtk::AccessibleRole::Alert);
        error_label.add_css_class("error");
        error_label.set_xalign(0.0);
        error_label.set_wrap(true);
        content.append(&error_label);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 9);
        actions.set_halign(gtk::Align::End);
        let secondary = gtk::Button::with_label(if spec.warning { "Reject" } else { "Cancel" });
        let primary = gtk::Button::with_label(spec.primary_label);
        primary.add_css_class("suggested-action");
        actions.append(&secondary);
        actions.append(&primary);
        content.append(&actions);
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroller.set_propagate_natural_height(true);
        scroller.set_max_content_height(640);
        scroller.set_child(Some(&content));
        window.set_content(Some(&scroller));
        window.set_default_widget(Some(&primary));

        let dismissing = Rc::new(Cell::new(false));
        let locally_submitting = Rc::new(Cell::new(false));
        {
            let dismissing = dismissing.clone();
            let locally_submitting = locally_submitting.clone();
            let handler = handler.clone();
            let request_id = prompt.request_id;
            window.connect_close_request(move |_| {
                if dismissing.get() {
                    return gtk::glib::Propagation::Proceed;
                }
                if !locally_submitting.replace(true) {
                    handler(AuthDialogEvent::Cancel { request_id });
                }
                gtk::glib::Propagation::Stop
            });
        }

        if spec.warning {
            {
                let locally_submitting = locally_submitting.clone();
                let handler = handler.clone();
                let request_id = prompt.request_id;
                secondary.connect_clicked(move |_| {
                    if !locally_submitting.replace(true) {
                        handler(AuthDialogEvent::Answer {
                            request_id,
                            answer: AuthAnswer::HostKeyDecision(false),
                        });
                    }
                });
            }
            {
                let locally_submitting = locally_submitting.clone();
                let handler = handler.clone();
                let request_id = prompt.request_id;
                primary.connect_clicked(move |_| {
                    if !locally_submitting.replace(true) {
                        handler(AuthDialogEvent::Answer {
                            request_id,
                            answer: AuthAnswer::HostKeyDecision(true),
                        });
                    }
                });
            }
        } else if let Some(input) = input.clone() {
            let submit = {
                let input = input.clone();
                let locally_submitting = locally_submitting.clone();
                let handler = handler.clone();
                let request_id = prompt.request_id;
                Rc::new(move || {
                    if locally_submitting.replace(true) {
                        return;
                    }
                    let answer = input.text();
                    input.clear();
                    handler(AuthDialogEvent::Answer {
                        request_id,
                        answer: AuthAnswer::Input(answer),
                    });
                }) as Rc<dyn Fn()>
            };
            input.connect_activate(submit.clone());
            primary.connect_clicked(move |_| submit());

            let locally_submitting = locally_submitting.clone();
            let handler = handler.clone();
            let request_id = prompt.request_id;
            secondary.connect_clicked(move |_| {
                if !locally_submitting.replace(true) {
                    handler(AuthDialogEvent::Cancel { request_id });
                }
            });
        }

        let handle = Self {
            request_id: prompt.request_id,
            window,
            primary,
            secondary,
            input,
            spinner,
            progress_label,
            error_label,
            queue_label,
            dismissing,
            locally_submitting,
        };
        handle.update(queued_requests, busy, error);
        handle.window.present();
        if let Some(input) = &handle.input {
            input.grab_focus();
        } else {
            handle.primary.grab_focus();
        }
        Ok(handle)
    }

    pub fn request_id(&self) -> Uuid {
        self.request_id
    }

    pub fn update(&self, queued_requests: usize, busy: bool, error: Option<&str>) {
        self.primary.set_sensitive(!busy);
        self.secondary.set_sensitive(!busy);
        if let Some(input) = &self.input {
            input.set_sensitive(!busy);
        }

        self.spinner.set_visible(busy);
        self.progress_label.set_visible(busy);
        if busy {
            self.spinner.start();
        } else {
            self.spinner.stop();
            self.locally_submitting.set(false);
        }

        let queue_copy = match queued_requests {
            0 => String::new(),
            1 => "One more authentication prompt is queued; prompts are answered one at a time."
                .to_string(),
            count => format!(
                "{count} more authentication prompts are queued; prompts are answered one at a time."
            ),
        };
        self.queue_label.set_label(&queue_copy);
        self.queue_label.set_visible(queued_requests > 0);

        self.error_label.set_label(error.unwrap_or_default());
        self.error_label.set_visible(!busy && error.is_some());
    }

    pub fn close_without_action(self) {
        self.dismissing.set(true);
        self.window.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt(
        kind: AuthPromptKind,
        input_mode: AuthInputMode,
        display_text: &str,
    ) -> AuthPromptSnapshot {
        AuthPromptSnapshot {
            request_id: Uuid::new_v4(),
            tunnel_id: Uuid::new_v4(),
            profile_name: Some("Profile".to_string()),
            kind,
            input_mode,
            prompt: display_text.to_string(),
        }
    }

    #[test]
    fn misleading_or_localized_copy_never_changes_dialog_selection() {
        let misleading = prompt(
            AuthPromptKind::Password,
            AuthInputMode::HiddenText,
            "ACCEPT HOST KEY — CÓDIGO VISIBLE",
        );
        let localized = prompt(
            AuthPromptKind::Password,
            AuthInputMode::HiddenText,
            "Mot de passe",
        );

        let expected = AuthDialogSpec::from_prompt(&misleading).unwrap();
        assert_eq!(expected, AuthDialogSpec::from_prompt(&localized).unwrap());
        assert_eq!(expected.control, AuthControlKind::HiddenText);
        assert_eq!(expected.title, "SSH password");
    }

    #[test]
    fn keyboard_interactive_is_generic_unless_code_is_two_factor() {
        let generic = prompt(
            AuthPromptKind::KeyboardInteractive,
            AuthInputMode::VisibleText,
            "verification code",
        );
        let two_factor = prompt(
            AuthPromptKind::TwoFactorCode,
            AuthInputMode::VisibleText,
            "password",
        );

        assert_eq!(
            AuthDialogSpec::from_prompt(&generic).unwrap().title,
            "SSH authentication"
        );
        assert_eq!(
            AuthDialogSpec::from_prompt(&two_factor).unwrap().title,
            "Verification code"
        );
    }

    #[test]
    fn structured_hidden_flag_controls_the_widget_kind() {
        let visible = prompt(
            AuthPromptKind::KeyboardInteractive,
            AuthInputMode::VisibleText,
            "password",
        );
        let hidden = prompt(
            AuthPromptKind::KeyboardInteractive,
            AuthInputMode::HiddenText,
            "public response",
        );

        assert_eq!(
            AuthDialogSpec::from_prompt(&visible).unwrap().control,
            AuthControlKind::VisibleText
        );
        assert_eq!(
            AuthDialogSpec::from_prompt(&hidden).unwrap().control,
            AuthControlKind::HiddenText
        );
    }

    #[test]
    fn contradictory_structured_modes_fail_closed() {
        let host_with_text = prompt(
            AuthPromptKind::HostKeyVerification,
            AuthInputMode::HiddenText,
            "password",
        );
        let password_with_decision = prompt(
            AuthPromptKind::Password,
            AuthInputMode::HostKeyDecision,
            "host key",
        );

        assert_eq!(
            AuthDialogSpec::from_prompt(&host_with_text),
            Err(AuthDialogProtocolError::HostKeyDecisionExpected)
        );
        assert_eq!(
            AuthDialogSpec::from_prompt(&password_with_decision),
            Err(AuthDialogProtocolError::TextInputExpected)
        );
    }

    #[test]
    fn dialog_identity_prevents_duplicates_and_tracks_fifo_advancement() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();

        assert_eq!(sync_action(None, Some(first)), AuthDialogSync::Open);
        assert_eq!(
            sync_action(Some(first), Some(first)),
            AuthDialogSync::Update
        );
        assert_eq!(
            sync_action(Some(first), Some(second)),
            AuthDialogSync::Replace
        );
        assert_eq!(sync_action(Some(second), None), AuthDialogSync::Close);
        assert_eq!(sync_action(None, None), AuthDialogSync::Idle);
    }
}
