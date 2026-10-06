// SPDX-License-Identifier: Apache-2.0

//! Launch verification: with `SPL_SMOKE_TEST=1` the application exits with
//! status 0 once the frontend reports that its first screen rendered, and with
//! status 1 if that has not happened within the time limit. CI uses this to
//! prove that a built application actually launches. `SPL_SMOKE_LINGER_SECS`
//! keeps it running that many seconds after the first screen, so that checks
//! of its behavior while idle, such as the offline check, cover that time.

use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(90);

/// Whether this launch is a smoke test.
pub struct SmokeTest {
    enabled: bool,
    linger: Duration,
}

impl SmokeTest {
    pub fn from_env() -> SmokeTest {
        let linger =
            std::env::var("SPL_SMOKE_LINGER_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        SmokeTest {
            enabled: std::env::var_os("SPL_SMOKE_TEST").is_some_and(|v| v == "1"),
            linger: Duration::from_secs(linger),
        }
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
            let (app, linger) = (app.clone(), self.linger);
            std::thread::spawn(move || {
                std::thread::sleep(linger);
                app.exit(0);
            });
        }
    }
}
