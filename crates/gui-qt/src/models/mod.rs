// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Qt models for QML integration
//!
//! These models bridge QML UI with gui-core business logic, demonstrating
//! the ~60-70% code reuse architecture.

pub mod profile_list;

pub use profile_list::ProfilesListModel;
