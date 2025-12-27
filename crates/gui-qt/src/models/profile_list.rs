// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Profile list model for QML
//!
//! This demonstrates CODE REUSE: All data comes from gui-core's ProfileViewModel!
//! Only QML-specific glue code is Qt-specific.

use qmetaobject::prelude::*;
use ssh_tunnel_common::{Profile, TunnelStatus};
use ssh_tunnel_gui_core::{load_profiles, ProfileViewModel};
use std::collections::HashMap;
use uuid::Uuid;

/// Individual profile item for QML
#[derive(QGadget, Clone, Default)]
pub struct ProfileItem {
    // All these fields come from ProfileViewModel (SHARED CODE!)
    #[qproperty(QString)]
    pub id: qt_core::QString,

    #[qproperty(QString)]
    pub name: qt_core::QString,

    #[qproperty(QString)]
    pub description: qt_core::QString,

    #[qproperty(QString)]
    pub status_text: qt_core::QString,

    #[qproperty(QString)]
    pub status_color: qt_core::QString,

    #[qproperty(bool)]
    pub can_start: bool,

    #[qproperty(bool)]
    pub can_stop: bool,
}

impl ProfileItem {
    /// Create from ProfileViewModel (SHARED CODE!)
    pub fn from_view_model(view_model: &ProfileViewModel) -> Self {
        Self {
            id: view_model.id.to_string().into(),
            name: view_model.name.clone().into(),
            description: view_model.description.clone().into(),
            status_text: view_model.status_text.clone().into(),
            status_color: view_model.status_color.to_hex().into(),
            can_start: view_model.can_start,
            can_stop: view_model.can_stop,
        }
    }
}

/// Profiles list model for QML
///
/// ARCHITECTURE: This is Qt-specific glue, but data comes from gui-core!
#[derive(QObject, Default)]
pub struct ProfilesListModel {
    base: qt_base_class!(trait QObject),

    // Profiles data (loaded from gui-core)
    profiles: Vec<Profile>,

    // Tunnel statuses (from AppCore)
    statuses: HashMap<Uuid, TunnelStatus>,

    // QML-exposed list of profile items
    #[qproperty(QVariantList, READ)]
    items: qt_core::QVariantList,

    // Signals
    #[qsignal]
    items_changed: qt_core::Signal<()>,

    // Invokable methods
    #[qinvokable]
    refresh: qt_method!(fn(&mut self)),

    #[qinvokable]
    get_profile: qt_method!(fn(&self, id: QString) -> QString),
}

impl ProfilesListModel {
    pub fn new() -> Self {
        let mut model = Self::default();
        model.refresh();
        model
    }

    /// Load profiles from gui-core (SHARED CODE!)
    fn refresh(&mut self) {
        // Load profiles using gui-core function (SHARED!)
        self.profiles = load_profiles().unwrap_or_default();

        // Convert to ProfileViewModels and then to QML items
        let mut items = qt_core::QVariantList::default();

        for profile in &self.profiles {
            // Get status for this profile
            let status = self
                .statuses
                .get(&profile.metadata.id)
                .cloned()
                .unwrap_or(TunnelStatus::NotConnected);

            // Create ProfileViewModel using gui-core (SHARED!)
            let view_model = ProfileViewModel::from_profile(profile, status);

            // Convert to QML item (Qt-specific)
            let item = ProfileItem::from_view_model(&view_model);

            // Add to QML list
            items.push(QVariant::from(QGadgetBox::new(item)));
        }

        self.items = items;
        self.items_changed();
    }

    /// Get profile JSON by ID (for editing)
    fn get_profile(&self, id: QString) -> QString {
        let id_str = id.to_string();
        if let Ok(uuid) = Uuid::parse_str(&id_str) {
            if let Some(profile) = self.profiles.iter().find(|p| p.metadata.id == uuid) {
                if let Ok(json) = serde_json::to_string_pretty(profile) {
                    return json.into();
                }
            }
        }
        "{}".into()
    }

    /// Update tunnel status for a profile
    pub fn update_status(&mut self, profile_id: Uuid, status: TunnelStatus) {
        self.statuses.insert(profile_id, status);
        self.refresh();
    }
}
