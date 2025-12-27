// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Daemon REST API client
//!
//! NOTE: This file can be copied almost exactly from gui-gtk/src/daemon/client.rs
//! It's framework-agnostic and only depends on reqwest, not Qt or GTK.
//!
//! This demonstrates code reuse - the entire HTTP client layer is shared!

// TODO: Copy implementation from gui-gtk/src/daemon/client.rs
// The code is 100% reusable because it doesn't depend on UI framework
