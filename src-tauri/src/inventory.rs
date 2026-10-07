// SPDX-License-Identifier: Apache-2.0

//! Typed inventory commands. The implementation stays in
//! `scoplen-client-core`; this module only supplies process state and IPC
//! adapters.

use crate::local_data::LocalDataState;
use scoplen_client_core::inventory::{
    AddHostError, Areas, Deletion, DeletionStore, EditHost, EditHostError, Failure, GroupError,
    GroupInput, GroupSummary, HostDetails, HostSource, HostSummary, Inventory, NewHost,
    OpenSshImportError, OpenSshImportPreview, OpenSshImportResult, RecentSession,
};

/// Process-local undo state shared by inventory command calls.
#[derive(Clone, Default)]
pub struct InventoryState {
    deletions: DeletionStore,
}

fn facade(local: &LocalDataState, state: &InventoryState) -> Result<Inventory, Failure> {
    let store = local
        .store()
        .ok_or_else(|| Failure::Failed { reference: "local data is locked".into() })?;
    Ok(Inventory::with_deletions(store, state.deletions.clone()))
}

async fn blocking<T>(
    operation: impl FnOnce() -> Result<T, Failure> + Send + 'static,
) -> Result<T, Failure>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| Failure::Failed { reference: error.to_string() })?
}

async fn blocking_add<T>(
    operation: impl FnOnce() -> Result<T, AddHostError> + Send + 'static,
) -> Result<T, AddHostError>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| AddHostError::Failed { reference: error.to_string() })?
}

/// Returns which inventory areas have content.
#[tauri::command]
#[specta::specta]
pub async fn inventory_areas(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
) -> Result<Areas, Failure> {
    let inventory = facade(&local, &state)?;
    blocking(move || inventory.areas()).await
}

/// Lists groups with live-host counts.
#[tauri::command]
#[specta::specta]
pub async fn inventory_groups(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
) -> Result<Vec<GroupSummary>, Failure> {
    let inventory = facade(&local, &state)?;
    blocking(move || inventory.groups()).await
}

/// Lists hosts from a source, filtered by the optional query.
#[tauri::command]
#[specta::specta]
pub async fn inventory_hosts(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
    source: HostSource,
    query: String,
) -> Result<Vec<HostSummary>, Failure> {
    let inventory = facade(&local, &state)?;
    blocking(move || inventory.hosts(source, query)).await
}

/// Lists the newest live entries in the device-local Recent session history.
#[tauri::command]
#[specta::specta]
pub async fn inventory_recent_sessions(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
) -> Result<Vec<RecentSession>, Failure> {
    let inventory = facade(&local, &state)?;
    blocking(move || inventory.recent_sessions()).await
}

/// Reads one host and its logins.
#[tauri::command]
#[specta::specta]
pub async fn inventory_host(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
    id: String,
) -> Result<Option<HostDetails>, Failure> {
    let inventory = facade(&local, &state)?;
    blocking(move || inventory.host(id)).await
}

/// Creates a host, credential, and default login as one operation.
#[tauri::command]
#[specta::specta]
pub async fn inventory_add_host(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
    host: NewHost,
) -> Result<HostDetails, AddHostError> {
    let store = local
        .store()
        .ok_or_else(|| AddHostError::Failed { reference: "local data is locked".into() })?;
    let inventory = Inventory::with_deletions(store, state.deletions.clone());
    blocking_add(move || inventory.add_host(host)).await
}

/// Changes a host's favorite marker.
#[tauri::command]
#[specta::specta]
pub async fn inventory_set_favorite(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
    id: String,
    favorite: bool,
) -> Result<(), Failure> {
    let inventory = facade(&local, &state)?;
    blocking(move || inventory.set_favorite(id, favorite)).await
}

/// Updates host metadata, tags, and group memberships in one core operation.
#[tauri::command]
#[specta::specta]
pub async fn inventory_update_host(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
    id: String,
    host: EditHost,
) -> Result<HostDetails, EditHostError> {
    let store = local
        .store()
        .ok_or_else(|| EditHostError::Failed { reference: "local data is locked".into() })?;
    let inventory = Inventory::with_deletions(store, state.deletions.clone());
    tauri::async_runtime::spawn_blocking(move || inventory.update_host(id, host))
        .await
        .map_err(|error| EditHostError::Failed { reference: error.to_string() })?
}

/// Creates a host group.
#[tauri::command]
#[specta::specta]
pub async fn inventory_create_group(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
    group: GroupInput,
) -> Result<GroupSummary, GroupError> {
    let inventory = facade(&local, &state)
        .map_err(|error| GroupError::Failed { reference: error.to_string() })?;
    tauri::async_runtime::spawn_blocking(move || inventory.create_group(group))
        .await
        .map_err(|error| GroupError::Failed { reference: error.to_string() })?
}

/// Updates a host group name and parent.
#[tauri::command]
#[specta::specta]
pub async fn inventory_update_group(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
    id: String,
    group: GroupInput,
) -> Result<GroupSummary, GroupError> {
    let inventory = facade(&local, &state)
        .map_err(|error| GroupError::Failed { reference: error.to_string() })?;
    tauri::async_runtime::spawn_blocking(move || inventory.update_group(id, group))
        .await
        .map_err(|error| GroupError::Failed { reference: error.to_string() })?
}

/// Tombstones a host and returns a short-lived undo token.
#[tauri::command]
#[specta::specta]
pub async fn inventory_delete_host(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
    id: String,
) -> Result<Deletion, Failure> {
    let inventory = facade(&local, &state)?;
    blocking(move || inventory.delete_host(id)).await
}

/// Restores a deletion made by this running desktop process.
#[tauri::command]
#[specta::specta]
pub async fn inventory_undo_delete(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
    token: String,
) -> Result<(), Failure> {
    let inventory = facade(&local, &state)?;
    blocking(move || inventory.undo_delete(token)).await
}

/// Reads an OpenSSH config for the empty-inventory preview without modifying
/// the source file or local store.
#[tauri::command]
#[specta::specta]
pub async fn inventory_preview_open_ssh_config(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
    path: String,
) -> Result<OpenSshImportPreview, OpenSshImportError> {
    let store = local
        .store()
        .ok_or_else(|| OpenSshImportError::Failed { reference: "local data is locked".into() })?;
    let inventory = Inventory::with_deletions(store, state.deletions.clone());
    tauri::async_runtime::spawn_blocking(move || inventory.preview_open_ssh_config(path))
        .await
        .map_err(|error| OpenSshImportError::Failed { reference: error.to_string() })?
}

/// Imports a previously previewed OpenSSH config into an empty inventory.
#[tauri::command]
#[specta::specta]
pub async fn inventory_import_open_ssh_config(
    local: tauri::State<'_, LocalDataState>,
    state: tauri::State<'_, InventoryState>,
    preview: OpenSshImportPreview,
) -> Result<OpenSshImportResult, OpenSshImportError> {
    let store = local
        .store()
        .ok_or_else(|| OpenSshImportError::Failed { reference: "local data is locked".into() })?;
    let inventory = Inventory::with_deletions(store, state.deletions.clone());
    tauri::async_runtime::spawn_blocking(move || inventory.import_open_ssh_config(preview))
        .await
        .map_err(|error| OpenSshImportError::Failed { reference: error.to_string() })?
}

/// Opens the conventional `~/.ssh/config` picker used by the empty-list
/// onboarding flow. Cancellation returns `null`.
#[tauri::command]
#[specta::specta]
pub async fn choose_open_ssh_config(app: tauri::AppHandle) -> Result<Option<String>, Failure> {
    let path = tauri::async_runtime::spawn_blocking(move || {
        use tauri_plugin_dialog::DialogExt as _;
        app.dialog()
            .file()
            .set_title("Choose an OpenSSH configuration")
            .add_filter("OpenSSH config", &["", "conf"])
            .blocking_pick_file()
    })
    .await
    .map_err(|error| Failure::Failed { reference: error.to_string() })?;
    Ok(path.and_then(|path| path.into_path().ok()).map(|path| path.to_string_lossy().into_owned()))
}

/// Opens the native private-key file picker; `null` means cancellation.
#[tauri::command]
#[specta::specta]
pub async fn choose_key_file(app: tauri::AppHandle) -> Result<Option<String>, Failure> {
    let path = tauri::async_runtime::spawn_blocking(move || {
        use tauri_plugin_dialog::DialogExt as _;
        app.dialog()
            .file()
            .set_title("Choose a private key")
            .add_filter("OpenSSH private key", &["", "key", "pem"])
            .blocking_pick_file()
    })
    .await
    .map_err(|error| Failure::Failed { reference: error.to_string() })?;
    Ok(path.and_then(|path| path.into_path().ok()).map(|path| path.to_string_lossy().into_owned()))
}
