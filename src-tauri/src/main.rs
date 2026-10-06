// SPDX-License-Identifier: Apache-2.0

//! The Scoplen desktop application.

// Release builds on Windows are GUI applications without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    scoplen_client_app::run();
}
