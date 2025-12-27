// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Qt UI modules
//!
//! This module structure mirrors gui-gtk but uses Qt widgets instead.
//! Business logic is shared through gui-core.

pub mod window;
pub mod profiles_list;
pub mod profile_dialog;
pub mod event_handler;

// Planned modules (similar to GTK):
// pub mod profile_details;
// pub mod auth_dialog;
// pub mod client_config;
// pub mod navigation;
// pub mod help_dialog;
// pub mod about_dialog;
