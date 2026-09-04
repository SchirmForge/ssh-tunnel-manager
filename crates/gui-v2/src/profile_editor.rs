// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Single-sheet create/edit/duplicate profile UI.

use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use ssh_tunnel_gui_core::{
    AppCommand, AuthType, CredentialUpdate, ForwardingType, PasswordStorage, ProfileEditorMode,
    ProfileEditorSession, ProfileReconnectRequest, ProfileSaveRequest, SecretValue,
    StoredCredentialState,
};

use crate::components::wrap_actions;

type CommandHandler = Rc<dyn Fn(AppCommand)>;

pub fn present_delete_confirmation(
    parent: &adw::ApplicationWindow,
    request: ssh_tunnel_gui_core::ProfileDeletionRequest,
    handler: CommandHandler,
) {
    let dialog = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Delete profile?")
        .default_width(430)
        .resizable(false)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content.set_margin_top(24);
    content.set_margin_bottom(20);
    content.set_margin_start(24);
    content.set_margin_end(24);

    let title = gtk::Label::new(Some(&format!("Delete “{}”?", request.profile_name)));
    title.add_css_class("title-3");
    title.set_wrap(true);
    content.append(&title);
    let explanation = gtk::Label::new(Some(
        "The profile file and its client-held credential will be removed. This cannot be undone.",
    ));
    explanation.set_wrap(true);
    explanation.set_justify(gtk::Justification::Center);
    explanation.add_css_class("dim-label");
    content.append(&explanation);

    let actions = wrap_actions(gtk::Align::Center);
    let cancel = gtk::Button::with_label("Cancel");
    {
        let dialog = dialog.clone();
        cancel.connect_clicked(move |_| dialog.close());
    }
    let delete = gtk::Button::with_label("Delete profile");
    delete.add_css_class("destructive-action");
    {
        let dialog = dialog.clone();
        delete.connect_clicked(move |_| {
            handler(AppCommand::ConfirmDeleteProfile(request.profile_id));
            dialog.close();
        });
    }
    actions.append(&cancel);
    actions.append(&delete);
    content.append(&actions);
    dialog.set_content(Some(&content));
    gtk::prelude::GtkWindowExt::set_focus(&dialog, Some(&cancel));
    dialog.present();
}

pub fn present_reconnect_confirmation(
    parent: &adw::ApplicationWindow,
    request: ProfileReconnectRequest,
    handler: CommandHandler,
) {
    let dialog = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Profile updated")
        .default_width(460)
        .resizable(false)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content.set_margin_top(24);
    content.set_margin_bottom(20);
    content.set_margin_start(24);
    content.set_margin_end(24);

    let title = gtk::Label::new(Some(&format!("Reconnect “{}” now?", request.profile_name)));
    title.add_css_class("title-3");
    title.set_wrap(true);
    content.append(&title);
    let explanation = gtk::Label::new(Some(
        "The changes were saved, but the current connection is still using the previous settings. They will take effect the next time this profile connects.",
    ));
    explanation.set_wrap(true);
    explanation.set_justify(gtk::Justification::Center);
    explanation.add_css_class("dim-label");
    content.append(&explanation);

    let actions = wrap_actions(gtk::Align::Center);
    let ok = gtk::Button::with_label("OK");
    {
        let dialog = dialog.clone();
        ok.connect_clicked(move |_| dialog.close());
    }
    let reconnect = gtk::Button::with_label("Reconnect now");
    reconnect.add_css_class("suggested-action");
    {
        let dialog = dialog.clone();
        reconnect.connect_clicked(move |_| {
            handler(AppCommand::ReconnectProfile(request.profile_id));
            dialog.close();
        });
    }
    actions.append(&ok);
    actions.append(&reconnect);
    content.append(&actions);
    dialog.set_content(Some(&content));
    dialog.set_default_widget(Some(&ok));
    gtk::prelude::GtkWindowExt::set_focus(&dialog, Some(&ok));
    dialog.present();
}

pub fn present(
    parent: &adw::ApplicationWindow,
    session: ProfileEditorSession,
    handler: CommandHandler,
) {
    let title = match session.mode {
        ProfileEditorMode::Create => "New profile",
        ProfileEditorMode::Edit => "Edit profile",
        ProfileEditorMode::Duplicate => "Duplicate profile",
    };
    let dialog = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(title)
        .default_width(620)
        .default_height(760)
        .build();

    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::builder().title(title).build()));
    toolbar.add_top_bar(&header);

    let cancel = gtk::Button::with_label("Cancel");
    {
        let dialog = dialog.clone();
        cancel.connect_clicked(move |_| dialog.close());
    }
    header.pack_start(&cancel);

    let save = gtk::Button::with_label("Save profile");
    save.add_css_class("suggested-action");
    header.pack_end(&save);
    dialog.set_default_widget(Some(&save));

    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.add_css_class("stm-editor-content");

    let error = gtk::Label::new(None);
    error.set_accessible_role(gtk::AccessibleRole::Alert);
    error.add_css_class("error");
    error.set_xalign(0.0);
    error.set_wrap(true);
    error.set_visible(false);
    content.append(&error);

    let name = text_entry("Name", &session.draft.name);
    content.append(&field("Profile", &name));

    let host = text_entry("Host", &session.draft.host);
    let ssh_port = text_entry("Port", &session.draft.ssh_port);
    ssh_port.set_width_chars(7);
    let user = text_entry("User", &session.draft.user);
    let server_box = gtk::Box::new(gtk::Orientation::Vertical, 9);
    let server_endpoint = adw::WrapBox::builder()
        .orientation(gtk::Orientation::Horizontal)
        .child_spacing(9)
        .line_spacing(9)
        .build();
    host.set_hexpand(true);
    server_endpoint.append(&host);
    server_endpoint.append(&ssh_port);
    server_box.append(&server_endpoint);
    server_box.append(&user);
    content.append(&field("SSH server", &server_box));

    let forwarding_box = gtk::Box::new(gtk::Orientation::Vertical, 9);
    let forwarding_kind = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let forwarding_label = gtk::Label::new(Some(match session.draft.forwarding_type {
        ForwardingType::Local => "Local forwarding",
        ForwardingType::Remote => "Remote forwarding",
        ForwardingType::Dynamic => "Dynamic / SOCKS forwarding",
    }));
    forwarding_label.set_xalign(0.0);
    forwarding_label.set_hexpand(true);
    forwarding_kind.append(&forwarding_label);
    if !matches!(session.draft.forwarding_type, ForwardingType::Local) {
        forwarding_kind.append(&wip_label(
            "The daemon rejects this forwarding mode. Its existing values are preserved.",
        ));
    }
    forwarding_box.append(&forwarding_kind);

    let bind_address = text_entry("Bind address", &session.draft.bind_address);
    let local_port = text_entry("Local port", &session.draft.local_port);
    let remote_host = text_entry("Remote host", &session.draft.remote_host);
    let remote_port = text_entry("Remote port", &session.draft.remote_port);
    local_port.set_width_chars(8);
    remote_port.set_width_chars(8);
    let local_endpoint = adw::WrapBox::builder()
        .orientation(gtk::Orientation::Horizontal)
        .child_spacing(9)
        .line_spacing(9)
        .build();
    bind_address.set_hexpand(true);
    local_endpoint.append(&bind_address);
    local_endpoint.append(&local_port);
    let remote_endpoint = adw::WrapBox::builder()
        .orientation(gtk::Orientation::Horizontal)
        .child_spacing(9)
        .line_spacing(9)
        .build();
    remote_host.set_hexpand(true);
    remote_endpoint.append(&remote_host);
    remote_endpoint.append(&remote_port);
    let forwarding_fields = gtk::Box::new(gtk::Orientation::Vertical, 9);
    forwarding_fields.append(&local_endpoint);
    forwarding_fields.append(&remote_endpoint);
    forwarding_box.append(&forwarding_fields);

    let exposure_warning = gtk::Label::new(Some(
        "This bind address exposes the forwarded port beyond this machine.",
    ));
    exposure_warning.add_css_class("warning");
    exposure_warning.set_xalign(0.0);
    exposure_warning.set_wrap(true);
    let update_exposure = {
        let warning = exposure_warning.clone();
        move |entry: &gtk::Entry| {
            let value = entry.text();
            warning.set_visible(matches!(value.trim(), "0.0.0.0" | "::"));
        }
    };
    update_exposure(&bind_address);
    bind_address.connect_changed(update_exposure);
    forwarding_box.append(&exposure_warning);
    let forwarding_supported = matches!(session.draft.forwarding_type, ForwardingType::Local);
    forwarding_fields.set_sensitive(forwarding_supported);
    content.append(&field("Forwarding", &forwarding_box));

    let auth_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
    let auth_type = gtk::DropDown::from_strings(&["SSH key", "Password", "Password + 2FA"]);
    auth_type.update_property(&[gtk::accessible::Property::Label("Authentication method")]);
    auth_type.set_selected(match session.draft.auth_type {
        AuthType::Key => 0,
        AuthType::Password => 1,
        AuthType::PasswordWith2FA => 2,
    });
    auth_box.append(&auth_type);

    let key_panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let key_line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let key_path = text_entry(
        if session.daemon_is_local {
            "Private key file"
        } else {
            "Key path on daemon host"
        },
        &session.draft.key_path,
    );
    key_path.set_hexpand(true);
    key_line.append(&key_path);
    if session.daemon_is_local {
        let browse = gtk::Button::with_label("Browse");
        {
            let dialog = dialog.clone();
            let key_path = key_path.clone();
            browse.connect_clicked(move |_| {
                let picker = gtk::FileDialog::builder()
                    .title("Select an SSH private key")
                    .modal(true)
                    .build();
                let key_path = key_path.clone();
                picker.open(Some(&dialog), gio::Cancellable::NONE, move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            key_path.set_text(&path.to_string_lossy());
                        }
                    }
                });
            });
        }
        key_line.append(&browse);
    } else {
        key_panel.append(&wip_label(
            "Key upload is not implemented. Enter the filename or path already available to the daemon.",
        ));
    }
    key_panel.append(&key_line);
    auth_box.append(&key_panel);

    let password_panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let storage_preserved = matches!(
        session.draft.password_storage,
        PasswordStorage::DaemonHost | PasswordStorage::File
    );
    let storage = if storage_preserved {
        gtk::DropDown::from_strings(&[match session.draft.password_storage {
            PasswordStorage::DaemonHost => "Daemon-host storage · preserve only",
            PasswordStorage::File => "Daemon-host file · WIP",
            _ => unreachable!(),
        }])
    } else {
        gtk::DropDown::from_strings(&["Don't save", "Save on this client"])
    };
    storage.set_sensitive(!storage_preserved);
    storage.update_property(&[gtk::accessible::Property::Label("Credential storage")]);
    if !storage_preserved && session.draft.password_storage == PasswordStorage::Client {
        storage.set_selected(1);
    }
    password_panel.append(&storage);
    let unavailable_storage = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let unavailable_copy = gtk::Label::new(Some("Daemon-host storage  ·  File storage"));
    unavailable_copy.set_xalign(0.0);
    unavailable_copy.set_hexpand(true);
    unavailable_copy.set_sensitive(false);
    unavailable_storage.append(&unavailable_copy);
    unavailable_storage.append(&wip_label(
        "No typed daemon credential-write capability exists. These modes are never enabled from daemon text.",
    ));
    password_panel.append(&unavailable_storage);

    let credential_state = gtk::Label::new(Some(match &session.client_credential {
        StoredCredentialState::Stored => {
            "A client credential is stored. Leave the field blank to keep it."
        }
        StoredCredentialState::NotStored => "No client credential is currently stored.",
        StoredCredentialState::Unavailable(_) => {
            "The client credential store is currently unavailable."
        }
    }));
    credential_state.set_xalign(0.0);
    credential_state.set_wrap(true);
    credential_state.add_css_class("dim-label");
    password_panel.append(&credential_state);

    let secret = gtk::PasswordEntry::new();
    secret.set_placeholder_text(Some("Password or key passphrase"));
    secret.set_show_peek_icon(true);
    secret.set_accessible_role(gtk::AccessibleRole::TextBox);
    secret.update_property(&[gtk::accessible::Property::Label(
        "Password or key passphrase",
    )]);
    password_panel.append(&secret);
    auth_box.append(&password_panel);

    let two_factor_panel = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let two_factor_copy = gtk::Label::new(Some(
        "Server second-factor prompts remain interactive. Stored TOTP generation is not implemented.",
    ));
    two_factor_copy.set_xalign(0.0);
    two_factor_copy.set_hexpand(true);
    two_factor_copy.set_wrap(true);
    two_factor_panel.append(&two_factor_copy);
    two_factor_panel.append(&wip_label("Stored TOTP · WIP"));
    auth_box.append(&two_factor_panel);

    let update_auth_visibility = {
        let key_panel = key_panel.clone();
        let password_panel = password_panel.clone();
        let two_factor_panel = two_factor_panel.clone();
        move |dropdown: &gtk::DropDown| {
            let selected = dropdown.selected();
            key_panel.set_visible(selected == 0);
            password_panel.set_visible(true);
            two_factor_panel.set_visible(selected == 2);
        }
    };
    update_auth_visibility(&auth_type);
    auth_type.connect_selected_notify(update_auth_visibility);
    {
        let secret = secret.clone();
        storage.connect_selected_notify(move |dropdown| {
            secret.set_visible(storage_preserved || dropdown.selected() == 1);
        });
    }
    secret.set_visible(storage_preserved || storage.selected() == 1);
    content.append(&field("Authentication", &auth_box));

    let options_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let compression = option_switch(
        &options_box,
        "Compression",
        session.draft.compression,
        Some("Runtime support · WIP"),
    );
    let keepalive = option_entry(
        &options_box,
        "Keepalive interval (seconds; 0 disables)",
        &session.draft.keepalive_interval,
        Some("Configurable interval · WIP"),
    );
    let auto_reconnect = option_switch(
        &options_box,
        "Auto-reconnect",
        session.draft.auto_reconnect,
        Some("Daemon enforcement · WIP"),
    );
    let reconnect_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let reconnect_attempts = option_entry(
        &reconnect_box,
        "Reconnect attempts (0 is unlimited)",
        &session.draft.reconnect_attempts,
        None,
    );
    let reconnect_delay = option_entry(
        &reconnect_box,
        "Reconnect delay (seconds)",
        &session.draft.reconnect_delay,
        None,
    );
    reconnect_box.set_visible(auto_reconnect.is_active());
    {
        let reconnect_box = reconnect_box.clone();
        auto_reconnect.connect_active_notify(move |switch| {
            reconnect_box.set_visible(switch.is_active());
        });
    }
    options_box.append(&reconnect_box);
    let tcp_keepalive = option_switch(
        &options_box,
        "TCP keepalive",
        session.draft.tcp_keepalive,
        Some("Daemon enforcement · WIP"),
    );

    let advanced = gtk::Expander::new(Some("Packet and channel tuning"));
    let advanced_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    advanced_box.set_margin_top(8);
    let max_packet_size = option_entry(
        &advanced_box,
        "Maximum packet size",
        &session.draft.max_packet_size,
        None,
    );
    let window_size = option_entry(
        &advanced_box,
        "Channel window size",
        &session.draft.window_size,
        None,
    );
    advanced.set_child(Some(&advanced_box));
    options_box.append(&advanced);
    content.append(&field("Fine tuning", &options_box));

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroller.set_child(Some(&content));
    toolbar.set_content(Some(&scroller));
    dialog.set_content(Some(&toolbar));
    gtk::prelude::GtkWindowExt::set_focus(&dialog, Some(&name));

    {
        let dialog = dialog.clone();
        let error = error.clone();
        let draft_template = session.draft.clone();
        let editor_mode = session.mode;
        let initial_storage = session.draft.password_storage;
        let initially_stored = matches!(session.client_credential, StoredCredentialState::Stored);
        save.connect_clicked(move |_| {
            let mut draft = draft_template.clone();
            draft.name = name.text().to_string();
            draft.host = host.text().to_string();
            draft.ssh_port = ssh_port.text().to_string();
            draft.user = user.text().to_string();
            draft.bind_address = bind_address.text().to_string();
            draft.local_port = local_port.text().to_string();
            draft.remote_host = remote_host.text().to_string();
            draft.remote_port = remote_port.text().to_string();
            draft.auth_type = match auth_type.selected() {
                0 => AuthType::Key,
                1 => AuthType::Password,
                _ => AuthType::PasswordWith2FA,
            };
            draft.key_path = key_path.text().to_string();
            draft.password_storage = if storage_preserved {
                initial_storage
            } else if storage.selected() == 1 {
                PasswordStorage::Client
            } else {
                PasswordStorage::None
            };
            draft.compression = compression.is_active();
            draft.keepalive_interval = keepalive.text().to_string();
            draft.auto_reconnect = auto_reconnect.is_active();
            draft.reconnect_attempts = reconnect_attempts.text().to_string();
            draft.reconnect_delay = reconnect_delay.text().to_string();
            draft.tcp_keepalive = tcp_keepalive.is_active();
            draft.max_packet_size = max_packet_size.text().to_string();
            draft.window_size = window_size.text().to_string();

            let secret_text = secret.text().to_string();
            let credential = if draft.password_storage == PasswordStorage::Client {
                if secret_text.is_empty() {
                    if !initially_stored {
                        error.set_label("Enter a password or key passphrase for client storage.");
                        error.set_visible(true);
                        return;
                    }
                    CredentialUpdate::Keep
                } else {
                    CredentialUpdate::Store(SecretValue::new(secret_text))
                }
            } else if initial_storage == PasswordStorage::Client {
                CredentialUpdate::Remove
            } else {
                CredentialUpdate::Keep
            };

            let profile = match draft.build() {
                Ok(profile) => profile,
                Err(validation) => {
                    error.set_label(&validation.to_string());
                    error.set_visible(true);
                    return;
                }
            };

            error.set_visible(false);
            secret.set_text("");
            handler(AppCommand::SaveProfile {
                request: Box::new(ProfileSaveRequest {
                    profile,
                    overwrite: editor_mode == ProfileEditorMode::Edit,
                    credential,
                }),
            });
            dialog.close();
        });
    }

    dialog.present();
}

fn field(title: &str, child: &impl IsA<gtk::Widget>) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 7);
    let label = gtk::Label::new(Some(title));
    label.set_xalign(0.0);
    label.add_css_class("stm-field-label");
    container.append(&label);
    container.append(child);
    container
}

fn text_entry(placeholder: &str, value: &str) -> gtk::Entry {
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some(placeholder));
    entry.set_text(value);
    entry.update_property(&[gtk::accessible::Property::Label(placeholder)]);
    entry
}

fn wip_label(tooltip: &str) -> gtk::Label {
    let label = gtk::Label::new(Some("WIP"));
    label.add_css_class("stm-wip-badge");
    label.set_tooltip_text(Some(tooltip));
    label.update_property(&[gtk::accessible::Property::Description(tooltip)]);
    label
}

fn option_switch(
    container: &gtk::Box,
    title: &str,
    active: bool,
    wip: Option<&str>,
) -> gtk::Switch {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("stm-option-row");
    let label = gtk::Label::new(Some(title));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    row.append(&label);
    if let Some(reason) = wip {
        row.append(&wip_label(reason));
    }
    let switch = gtk::Switch::new();
    switch.set_active(active);
    switch.update_property(&[gtk::accessible::Property::Label(title)]);
    row.append(&switch);
    container.append(&row);
    switch
}

fn option_entry(container: &gtk::Box, title: &str, value: &str, wip: Option<&str>) -> gtk::Entry {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("stm-option-row");
    let label = gtk::Label::new(Some(title));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    row.append(&label);
    if let Some(reason) = wip {
        row.append(&wip_label(reason));
    }
    let entry = text_entry(title, value);
    entry.set_width_chars(10);
    row.append(&entry);
    container.append(&row);
    entry
}
