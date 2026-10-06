// SPDX-License-Identifier: Apache-2.0

//! The main window opens where and as large as it was last closed. Its
//! geometry is a device-local record (`scoplen-docs/04-object-model.md` §4.9).

use scoplen_client_core::store::Store;
use serde::{Deserialize, Serialize};
use tauri::{LogicalPosition, LogicalSize, Manager as _, WebviewWindow};

const MAIN: &str = "main";

/// A window's size and position in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Geometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub maximized: bool,
}

/// A display's area in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Geometry {
    /// Whether enough of the window's title bar would be on one of the
    /// displays to grab it; a window saved on a display that is gone is not
    /// restored off screen.
    pub fn is_reachable(&self, displays: &[Area]) -> bool {
        const GRIP: f64 = 48.0;
        self.width >= 1.0
            && self.height >= 1.0
            && displays.iter().any(|d| {
                let left = self.x.max(d.x);
                let right = (self.x + self.width).min(d.x + d.width);
                right - left >= GRIP && self.y >= d.y && self.y + GRIP <= d.y + d.height
            })
    }
}

fn displays(window: &WebviewWindow) -> Vec<Area> {
    window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let scale = m.scale_factor();
            let position = m.position().to_logical::<f64>(scale);
            let size = m.size().to_logical::<f64>(scale);
            Area { x: position.x, y: position.y, width: size.width, height: size.height }
        })
        .collect()
}

/// Puts the main window back where it was, if that is still on a display.
pub fn restore(window: &WebviewWindow, store: &Store) {
    let Ok(Some(bytes)) = store.window_state(MAIN) else { return };
    let Ok(geometry) = serde_json::from_slice::<Geometry>(&bytes) else { return };
    if !geometry.is_reachable(&displays(window)) {
        return;
    }
    let _ = window.set_size(LogicalSize::new(geometry.width, geometry.height));
    let _ = window.set_position(LogicalPosition::new(geometry.x, geometry.y));
    if geometry.maximized {
        let _ = window.maximize();
    }
}

/// Saves the main window's geometry.
pub fn save(window: &WebviewWindow, store: &Store) {
    let Ok(scale) = window.scale_factor() else { return };
    let (Ok(position), Ok(size)) = (window.outer_position(), window.inner_size()) else { return };
    let position = position.to_logical::<f64>(scale);
    let size = size.to_logical::<f64>(scale);
    let geometry = Geometry {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized: window.is_maximized().unwrap_or(false),
    };
    if let Ok(bytes) = serde_json::to_vec(&geometry) {
        let _ = store.save_window_state(MAIN, &bytes);
    }
}

/// The main window, if it exists.
pub fn main_window(app: &tauri::AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MAIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISPLAY: Area = Area { x: 0.0, y: 0.0, width: 1440.0, height: 900.0 };

    fn at(x: f64, y: f64) -> Geometry {
        Geometry { x, y, width: 1200.0, height: 780.0, maximized: false }
    }

    #[test]
    fn a_window_on_a_display_is_restored() {
        assert!(at(100.0, 80.0).is_reachable(&[DISPLAY]));
        assert!(at(1300.0, 80.0).is_reachable(&[DISPLAY]), "partly off screen but grabbable");
    }

    #[test]
    fn a_window_whose_display_is_gone_is_not() {
        let second = Area { x: 1440.0, y: 0.0, width: 1920.0, height: 1080.0 };
        assert!(at(2000.0, 100.0).is_reachable(&[DISPLAY, second]));
        assert!(!at(2000.0, 100.0).is_reachable(&[DISPLAY]));
        assert!(!at(100.0, -500.0).is_reachable(&[DISPLAY]), "title bar above the display");
        assert!(!at(100.0, 100.0).is_reachable(&[]));
    }

    #[test]
    fn geometry_round_trips_as_json() {
        let geometry = Geometry { maximized: true, ..at(10.5, 20.0) };
        let bytes = serde_json::to_vec(&geometry).unwrap();
        assert_eq!(serde_json::from_slice::<Geometry>(&bytes).unwrap(), geometry);
    }
}
