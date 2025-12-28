// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Profile list model for QML
//!
//! This demonstrates CODE REUSE: All data comes from gui-core's ProfileViewModel!
//! Only QML-specific glue code is Qt-specific.

use ssh_tunnel_common::{Profile, TunnelStatus};
use ssh_tunnel_gui_core::{load_profiles, ProfileViewModel, StatusColor};
use std::collections::HashMap;
use uuid::Uuid;

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(i32, count)]
        type ProfilesListModel = super::ProfilesListModelRust;
    }

    unsafe extern "RustQt" {
        #[qinvokable]
        fn refresh(self: Pin<&mut ProfilesListModel>);

        #[qinvokable]
        fn getName(self: &ProfilesListModel, index: i32) -> String;

        #[qinvokable]
        fn getHost(self: &ProfilesListModel, index: i32) -> String;

        #[qinvokable]
        fn getStatusText(self: &ProfilesListModel, index: i32) -> String;

        #[qinvokable]
        fn getStatusColor(self: &ProfilesListModel, index: i32) -> String;

        #[qinvokable]
        fn canStart(self: &ProfilesListModel, index: i32) -> bool;

        #[qinvokable]
        fn canStop(self: &ProfilesListModel, index: i32) -> bool;
    }
}

use std::pin::Pin;

/// Rust implementation of ProfilesListModel
#[derive(Default)]
pub struct ProfilesListModelRust {
    profiles: Vec<Profile>,
    statuses: HashMap<Uuid, TunnelStatus>,
    view_models: Vec<ProfileViewModel>,
    count: i32,
}

impl ffi::ProfilesListModel {
    pub fn refresh(mut self: Pin<&mut Self>) {
        let profiles = load_profiles().unwrap_or_default();
        let mut view_models = Vec::new();

        for profile in &profiles {
            let status = self
                .rust()
                .statuses
                .get(&profile.metadata.id)
                .cloned()
                .unwrap_or(TunnelStatus::NotConnected);

            let view_model = ProfileViewModel::from_profile(profile, status);
            view_models.push(view_model);
        }

        let count = profiles.len() as i32;
        self.rust_mut().profiles = profiles;
        self.rust_mut().view_models = view_models;
        self.as_mut().set_count(count);
    }

    pub fn getName(&self, index: i32) -> String {
        self.rust()
            .view_models
            .get(index as usize)
            .map(|vm| vm.name.clone())
            .unwrap_or_default()
    }

    pub fn getHost(&self, index: i32) -> String {
        self.rust()
            .view_models
            .get(index as usize)
            .map(|vm| vm.host.clone())
            .unwrap_or_default()
    }

    pub fn getStatusText(&self, index: i32) -> String {
        self.rust()
            .view_models
            .get(index as usize)
            .map(|vm| vm.status_text.clone())
            .unwrap_or_default()
    }

    pub fn getStatusColor(&self, index: i32) -> String {
        self.rust()
            .view_models
            .get(index as usize)
            .map(|vm| {
                match &vm.status_color {
                    StatusColor::Green => "#4caf50",
                    StatusColor::Orange => "#ff9800",
                    StatusColor::Red => "#f44336",
                    StatusColor::Gray => "#9e9e9e",
                }.to_string()
            })
            .unwrap_or_default()
    }

    pub fn canStart(&self, index: i32) -> bool {
        self.rust()
            .view_models
            .get(index as usize)
            .map(|vm| vm.can_start)
            .unwrap_or(false)
    }

    pub fn canStop(&self, index: i32) -> bool {
        self.rust()
            .view_models
            .get(index as usize)
            .map(|vm| vm.can_stop)
            .unwrap_or(false)
    }
}
