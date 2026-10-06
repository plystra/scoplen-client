// SPDX-License-Identifier: Apache-2.0

//! Commands the frontend invokes. Each is a thin adapter over
//! `scoplen-client-core`; no behavior lives here.

use scoplen_client_core::locale::Locale;
use serde::Serialize;
use specta::Type;

/// The desktop platform the application runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    /// macOS.
    Macos,
    /// Windows.
    Windows,
    /// Any other system; Phase 1 ships only macOS and Windows builds.
    Other,
}

impl Platform {
    /// The platform this binary was built for.
    pub const fn current() -> Platform {
        if cfg!(target_os = "macos") {
            Platform::Macos
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Other
        }
    }
}

/// What the interface needs to know about the running application.
#[derive(Clone, Debug, Serialize, Type)]
pub struct AppInfo {
    /// The application version.
    pub version: String,
    /// The platform the application runs on.
    pub platform: Platform,
    /// The interface language chosen from the system's preferences.
    pub locale: Locale,
}

/// Returns the application version, platform, and interface language.
#[tauri::command]
#[specta::specta]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: Platform::current(),
        locale: Locale::negotiate(&scoplen_client_platform::preferred_languages()),
    }
}

/// Called by the frontend once its first screen has rendered.
#[tauri::command]
#[specta::specta]
pub fn shell_ready(app: tauri::AppHandle, smoke: tauri::State<'_, crate::smoke::SmokeTest>) {
    smoke.ready(&app);
}
