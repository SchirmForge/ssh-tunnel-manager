// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Daemon communication modules
//!
//! These can be copied almost verbatim from gui-gtk:
//! - client.rs: REST API client
//! - sse.rs: SSE event listener
//!
//! Both are framework-agnostic and would work identically in Qt.

pub mod client;
pub mod sse;
