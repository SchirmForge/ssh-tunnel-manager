// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! GTK 4/libadwaita presentation adapter for the second-generation GUI.
//!
//! GTK objects stay on the GLib main context. The toolkit-neutral runtime is
//! executed on a dedicated Tokio thread and communicates using typed core
//! effects and events.

mod action_router;
mod auth_dialog;
mod bridge;
mod components;
mod daemon_view;
mod profile_editor;
mod profile_list;
mod setup_wizard;
mod shell_state;
mod window;

pub use action_router::{ActionRoute, ShellAction, ShellPage};
pub use shell_state::{CapabilityPresentation, DaemonBadgeState, ShellViewState};

use adw::prelude::*;

const APPLICATION_ID: &str = "io.github.schirmforge.SshTunnelManagerGuiV2";
const STYLE_RESOURCE: &str = "/io/github/schirmforge/SshTunnelManagerGuiV2/style.css";

pub fn run() -> gtk::glib::ExitCode {
    if let Err(error) = gtk::gio::resources_register_include!("gui-v2.gresource") {
        eprintln!("Unable to register GUI resources: {error}");
        return gtk::glib::ExitCode::FAILURE;
    }

    let application = adw::Application::builder()
        .application_id(APPLICATION_ID)
        .build();

    application.connect_startup(|application| {
        install_styles();
        install_accelerators(application);
    });
    application.connect_activate(window::activate);
    application.run()
}

fn install_accelerators(application: &adw::Application) {
    application.set_accels_for_action("win.focus-search", &["<Primary>f"]);
    application.set_accels_for_action("win.new-profile", &["<Primary>n"]);
    application.set_accels_for_action("win.refresh", &["F5", "<Primary>r"]);
    application.set_accels_for_action("win.profiles", &["<Primary>1"]);
    application.set_accels_for_action("win.daemon", &["<Primary>2"]);
}

fn install_styles() {
    let provider = gtk::CssProvider::new();
    provider.load_from_resource(STYLE_RESOURCE);

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[cfg(test)]
mod tests {
    const STYLE: &str = include_str!("../data/style.css");

    #[test]
    fn application_css_preserves_system_fonts_and_semantic_colors() {
        assert!(!STYLE.contains("font-family"));
        assert!(
            !STYLE.contains('#'),
            "fixed color literals bypass light, dark, and high-contrast themes"
        );
    }
}
