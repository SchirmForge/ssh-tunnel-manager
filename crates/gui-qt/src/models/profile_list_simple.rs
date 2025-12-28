// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Simplified profile list model to test cxx-qt compilation

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "RustQt" {
        #[qobject]
        type SimpleModel = super::SimpleModelRust;

        #[qinvokable]
        fn test(self: &SimpleModel) -> i32;
    }
}

#[derive(Default)]
pub struct SimpleModelRust {
    value: i32,
}

impl ffi::SimpleModel {
    pub fn test(&self) -> i32 {
        42
    }
}
