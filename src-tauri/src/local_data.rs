// SPDX-License-Identifier: Apache-2.0

//! Commands for opening the local store and protecting its key.

use std::path::PathBuf;
use std::sync::Arc;

use scoplen_client_core::local_data::{LocalData, LocalDataError, Status};
use tauri::Manager as _;
use zeroize::Zeroizing;

/// The local data, or why it could not be prepared at startup.
pub struct LocalDataState(Result<Arc<LocalData>, LocalDataError>);

impl LocalDataState {
    /// Prepares the local data in the application's data directory, or in
    /// `SPL_DATA_DIR` when that is set (used by tests and launch checks to
    /// keep their data separate).
    pub fn start(app: &tauri::AppHandle) -> LocalDataState {
        let dir = match std::env::var_os("SPL_DATA_DIR") {
            Some(dir) => Ok(PathBuf::from(dir)),
            None => app
                .path()
                .app_data_dir()
                .map_err(|e| LocalDataError::Failed { reference: e.to_string() }),
        };
        let data = dir.and_then(|dir| {
            let keystore = scoplen_client_platform::keystore::local_key_store(&dir);
            LocalData::start(&dir, keystore)
        });
        LocalDataState(data.map(Arc::new))
    }

    fn get(&self) -> Result<Arc<LocalData>, LocalDataError> {
        self.0.clone()
    }
}

/// Runs a key operation off the interface thread: deriving a key from a
/// passphrase deliberately takes about a second.
async fn blocking<T: Send + 'static>(
    state: &LocalDataState,
    operation: impl FnOnce(&LocalData) -> Result<T, LocalDataError> + Send + 'static,
) -> Result<T, LocalDataError> {
    let data = state.get()?;
    tauri::async_runtime::spawn_blocking(move || operation(&data))
        .await
        .map_err(|e| LocalDataError::Failed { reference: e.to_string() })?
}

/// Where the local data stands.
#[tauri::command]
#[specta::specta]
pub fn local_data_status(
    state: tauri::State<'_, LocalDataState>,
) -> Result<Status, LocalDataError> {
    Ok(state.get()?.status())
}

/// Opens the store with the local passphrase.
#[tauri::command]
#[specta::specta]
pub async fn unlock_local_data(
    state: tauri::State<'_, LocalDataState>,
    passphrase: String,
) -> Result<Status, LocalDataError> {
    let passphrase = Zeroizing::new(passphrase);
    blocking(&state, move |data| data.unlock(&passphrase)).await
}

/// On a system without a keystore, chooses the passphrase for new local data.
#[tauri::command]
#[specta::specta]
pub async fn create_local_passphrase(
    state: tauri::State<'_, LocalDataState>,
    passphrase: String,
) -> Result<Status, LocalDataError> {
    let passphrase = Zeroizing::new(passphrase);
    blocking(&state, move |data| data.create(&passphrase)).await
}

/// Sets or changes the local passphrase.
#[tauri::command]
#[specta::specta]
pub async fn set_local_passphrase(
    state: tauri::State<'_, LocalDataState>,
    passphrase: String,
) -> Result<Status, LocalDataError> {
    let passphrase = Zeroizing::new(passphrase);
    blocking(&state, move |data| data.set_passphrase(&passphrase)).await
}

/// Removes the local passphrase, leaving the key in the keystore.
#[tauri::command]
#[specta::specta]
pub async fn remove_local_passphrase(
    state: tauri::State<'_, LocalDataState>,
) -> Result<Status, LocalDataError> {
    blocking(&state, |data| data.remove_passphrase()).await
}

/// Keeps unreadable local data aside and starts with empty local data.
#[tauri::command]
#[specta::specta]
pub async fn start_with_empty_local_data(
    state: tauri::State<'_, LocalDataState>,
) -> Result<Status, LocalDataError> {
    blocking(&state, |data| data.start_fresh()).await
}
