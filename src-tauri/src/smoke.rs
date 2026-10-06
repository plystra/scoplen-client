// SPDX-License-Identifier: Apache-2.0

//! Launch verification: with `SPL_SMOKE_TEST=1` the application exits with
//! status 0 once the frontend reports that its first screen rendered, and with
//! status 1 if that has not happened within the time limit. CI uses this to
//! prove that a built application actually launches.

use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(90);

/// Whether this launch is a smoke test.
pub struct SmokeTest {
    enabled: bool,
}

impl SmokeTest {
    pub fn from_env() -> SmokeTest {
        SmokeTest { enabled: std::env::var_os("SPL_SMOKE_TEST").is_some_and(|v| v == "1") }
    }

    /// Starts the time limit.
    pub fn arm(&self, app: tauri::AppHandle) {
        if self.enabled {
            std::thread::spawn(move || {
                std::thread::sleep(LIMIT);
                eprintln!("spl-smoke: the frontend did not report ready within {LIMIT:?}");
                app.exit(1);
            });
        }
    }

    /// Records that the first screen rendered.
    pub fn ready(&self, app: &tauri::AppHandle) {
        if self.enabled {
            println!("spl-smoke: ready");
            app.exit(0);
        }
    }
}
