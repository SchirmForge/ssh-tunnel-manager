// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// GObject wrapper for Profile

use glib::Object;
use gtk4::glib;
use gtk4::subclass::prelude::*;
use ssh_tunnel_common::config::Profile;
use ssh_tunnel_common::types::TunnelStatus;
use std::cell::RefCell;

// Implementation module
mod imp {
    use super::*;

    #[derive(Debug)]
    pub struct ProfileModel {
        pub profile: RefCell<Option<Profile>>,
        pub status: RefCell<TunnelStatus>,
    }

    impl Default for ProfileModel {
        fn default() -> Self {
            Self {
                profile: RefCell::new(None),
                status: RefCell::new(TunnelStatus::NotConnected),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ProfileModel {
        const NAME: &'static str = "ProfileModel";
        type Type = super::ProfileModel;
    }

    impl ObjectImpl for ProfileModel {}
}

// Public wrapper
glib::wrapper! {
    pub struct ProfileModel(ObjectSubclass<imp::ProfileModel>);
}

impl ProfileModel {
    /// Create a new ProfileModel from a Profile
    pub fn new(profile: Profile) -> Self {
        let obj: Self = Object::builder().build();
        obj.imp().profile.replace(Some(profile));
        obj
    }

    /// Get the profile name
    pub fn name(&self) -> String {
        self.imp()
            .profile
            .borrow()
            .as_ref()
            .map(|p| p.metadata.name.clone())
            .unwrap_or_default()
    }

    /// Get the profile ID as a string
    pub fn id(&self) -> String {
        self.imp()
            .profile
            .borrow()
            .as_ref()
            .map(|p| p.metadata.id.to_string())
            .unwrap_or_default()
    }

    /// Get the SSH host
    pub fn host(&self) -> String {
        self.imp()
            .profile
            .borrow()
            .as_ref()
            .map(|p| p.connection.host.clone())
            .unwrap_or_default()
    }

    /// Get the SSH port
    pub fn port(&self) -> u16 {
        self.imp()
            .profile
            .borrow()
            .as_ref()
            .map(|p| p.connection.port)
            .unwrap_or(22)
    }

    /// Get the SSH user
    pub fn user(&self) -> String {
        self.imp()
            .profile
            .borrow()
            .as_ref()
            .map(|p| p.connection.user.clone())
            .unwrap_or_default()
    }

    /// Get the local port
    pub fn local_port(&self) -> u16 {
        self.imp()
            .profile
            .borrow()
            .as_ref()
            .and_then(|p| p.forwarding.local_port)
            .unwrap_or(0)
    }

    /// Get the remote host
    pub fn remote_host(&self) -> String {
        self.imp()
            .profile
            .borrow()
            .as_ref()
            .and_then(|p| p.forwarding.remote_host.clone())
            .unwrap_or_default()
    }

    /// Get the remote port
    pub fn remote_port(&self) -> u16 {
        self.imp()
            .profile
            .borrow()
            .as_ref()
            .and_then(|p| p.forwarding.remote_port)
            .unwrap_or(0)
    }

    /// Get the bind address
    pub fn bind_address(&self) -> String {
        self.imp()
            .profile
            .borrow()
            .as_ref()
            .map(|p| p.forwarding.bind_address.clone())
            .unwrap_or_else(|| "127.0.0.1".to_string())
    }

    /// Get the auth type
    pub fn auth_type(&self) -> String {
        use ssh_tunnel_common::types::AuthType;
        self.imp()
            .profile
            .borrow()
            .as_ref()
            .map(|p| match p.connection.auth_type {
                AuthType::Password => "Password",
                AuthType::Key => "SSH Key",
                AuthType::PasswordWith2FA => "Password + 2FA",
            })
            .unwrap_or("Unknown")
            .to_string()
    }

    /// Get the SSH key path
    pub fn key_path(&self) -> String {
        self.imp()
            .profile
            .borrow()
            .as_ref()
            .and_then(|p| p.connection.key_path.as_ref())
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| "Not set".to_string())
    }

    /// Get a reference to the inner Profile
    pub fn profile(&self) -> Option<Profile> {
        self.imp().profile.borrow().clone()
    }

    /// Get the current tunnel status
    pub fn status(&self) -> TunnelStatus {
        self.imp().status.borrow().clone()
    }

    /// Update the tunnel status
    pub fn update_status(&self, status: TunnelStatus) {
        *self.imp().status.borrow_mut() = status;
    }
}
