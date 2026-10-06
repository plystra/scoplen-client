// SPDX-License-Identifier: Apache-2.0

//! Tauri build script: embeds the configuration, icons, and capabilities.
//!
//! On Windows, Tauri needs the Common Controls v6 manifest in every executable
//! that links it, or the process fails to start with
//! `STATUS_ENTRYPOINT_NOT_FOUND`. `tauri-build` adds the manifest only to the
//! application binary, so it is embedded here through the linker instead, which
//! covers the application, its unit tests, and examples alike.

fn main() {
    let mut attributes = tauri_build::Attributes::new();
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    }
    tauri_build::try_build(attributes).expect("the Tauri build step failed");
}
