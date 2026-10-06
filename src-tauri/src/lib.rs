// SPDX-License-Identifier: Apache-2.0

//! The Scoplen desktop application shell: window management and the typed
//! IPC surface between the frontend and `scoplen-client-core`
//! (`scoplen-docs/11-client-architecture.md` §1, §4).

pub mod commands;
pub mod frames;
mod smoke;

use tauri::Manager as _;
use tauri_specta::{Builder, collect_commands};

/// Path of the generated frontend bindings, relative to this crate.
pub const BINDINGS_PATH: &str = "../web/app/src/ipc/bindings.ts";

/// The typed command surface. The frontend bindings are generated from it, so
/// the frontend and the core cannot drift.
pub fn ipc() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![commands::app_info, commands::shell_ready])
}

/// Writes the TypeScript bindings for [`ipc`] to `path`.
pub fn write_bindings(path: &std::path::Path) -> Result<(), specta_typescript::Error> {
    let header = "// SPDX-License-Identifier: Apache-2.0\n// Generated from src-tauri by `pnpm bindings`. Do not edit.\n";
    ipc().export(specta_typescript::Typescript::default().header(header), path)
}

/// Starts the application.
pub fn run() {
    let ipc = ipc();
    let smoke = smoke::SmokeTest::from_env();
    tauri::Builder::default()
        .manage(smoke)
        .invoke_handler(ipc.invoke_handler())
        .setup(move |app| {
            ipc.mount_events(app);
            app.state::<smoke::SmokeTest>().arm(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("the application failed to start");
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The committed bindings must be exactly what the command surface
    /// generates. Run `pnpm bindings` after changing a command.
    #[test]
    fn bindings_are_current() {
        let scratch = std::env::temp_dir().join(format!("spl-bindings-{}.ts", std::process::id()));
        super::write_bindings(&scratch).expect("bindings render");
        let generated = std::fs::read_to_string(&scratch).expect("generated bindings");
        let _ = std::fs::remove_file(&scratch);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(super::BINDINGS_PATH);
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(committed == generated, "{} is out of date; run `pnpm bindings`", path.display());
    }
}
