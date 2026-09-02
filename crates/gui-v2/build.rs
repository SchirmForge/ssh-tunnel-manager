// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

fn main() {
    glib_build_tools::compile_resources(&["data"], "data/gui-v2.gresource.xml", "gui-v2.gresource");
}
