// SPDX-License-Identifier: Apache-2.0

//! The smallest live SSH IPC path: resolve one local profile, connect through
//! the shared engine, and stream channel bytes as binary Tauri frames.

use crate::frames::{FrameSender, RawFrame};
use crate::local_data::LocalDataState;
use scoplen_client_core::connection::{
    HostKeyPresentation, HostKeyStatus, HostKeyTrust, SshConnection, SshConnectionError,
    SshExecOutput, SshSession, StoredHostKeyVerifier, evaluate_host_key, probe_host_key,
};
use scoplen_client_core::inventory::{DeletionStore, Inventory};
use scoplen_client_core::repository::{
    Edit, Provenance, Repository, TrustRecord, TrustRecordChange,
};
use scoplen_ssh::ChannelEvent;
use serde::Serialize;
use specta::Type;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::ipc::Channel;

/// The result of comparing the probed key with this Host's active trust records.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum HostKeyTrustStatus {
    /// The key already matches an active host-specific trust record.
    Trusted,
    /// No active host-specific key exists yet.
    New,
    /// An active host-specific key exists, but it does not match.
    Changed,
}

/// A validated host key suitable for a confirmation dialog.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyDetails {
    /// The SSH algorithm identifier.
    pub algorithm: String,
    /// The complete OpenSSH public-key text.
    pub key: String,
    /// The OpenSSH SHA-256 fingerprint.
    pub fingerprint: String,
    /// How the key relates to the local Host trust records.
    pub status: HostKeyTrustStatus,
}

fn host_key_details(presentation: HostKeyPresentation, status: HostKeyStatus) -> HostKeyDetails {
    HostKeyDetails {
        algorithm: presentation.algorithm,
        key: presentation.key,
        fingerprint: presentation.fingerprint,
        status: match status {
            HostKeyStatus::Trusted => HostKeyTrustStatus::Trusted,
            HostKeyStatus::New => HostKeyTrustStatus::New,
            HostKeyStatus::Changed => HostKeyTrustStatus::Changed,
        },
    }
}

/// Probe and classify the SSH host key before opening an authenticated session.
///
/// New and changed keys are returned to the frontend for explicit confirmation;
/// this command never writes a trust record and never accepts a key for a later
/// connection by itself.
#[tauri::command]
#[specta::specta]
pub async fn session_host_key(
    local: tauri::State<'_, LocalDataState>,
    profile_id: String,
) -> Result<HostKeyDetails, String> {
    let store = local.store().ok_or_else(|| "local data is locked".to_owned())?;
    let inventory = Inventory::with_deletions(store.clone(), DeletionStore::default());
    let plan = inventory.connection_plan(profile_id).map_err(|error| error.to_string())?;
    let records =
        Repository::<TrustRecord>::new(&store).list().map_err(|error| error.to_string())?;
    let presentation = probe_host_key(&plan).await.map_err(display_connection_error)?;
    let status =
        evaluate_host_key(plan.host_id, &presentation.key, &presentation.fingerprint, &records);
    Ok(host_key_details(presentation, status))
}

/// Trust a previously displayed host key after probing it again.
///
/// The expected values bind the confirmation to the exact probe shown to the
/// user. A changed value aborts without writing anything. Older active direct
/// key records for the Host are revoked before the new manual record is
/// created, so a key rotation cannot leave two different keys trusted at once.
#[tauri::command]
#[specta::specta]
pub async fn session_trust_host_key(
    local: tauri::State<'_, LocalDataState>,
    profile_id: String,
    algorithm: String,
    key: String,
    fingerprint: String,
) -> Result<(), String> {
    let store = local.store().ok_or_else(|| "local data is locked".to_owned())?;
    let inventory = Inventory::with_deletions(store.clone(), DeletionStore::default());
    let plan = inventory.connection_plan(profile_id).map_err(|error| error.to_string())?;
    let current = probe_host_key(&plan).await.map_err(display_connection_error)?;
    if current.algorithm != algorithm || current.key != key || current.fingerprint != fingerprint {
        return Err("host key changed while waiting for confirmation; try again".to_owned());
    }

    let repository = Repository::<TrustRecord>::new(&store);
    let records = repository.list().map_err(|error| error.to_string())?;
    let status = evaluate_host_key(plan.host_id, &current.key, &current.fingerprint, &records);
    if status == HostKeyStatus::Trusted {
        return Ok(());
    }

    for record in records
        .iter()
        .filter(|record| record.host == Some(plan.host_id) && !record.revoked && !record.ca)
    {
        repository
            .update(record.meta.id, TrustRecordChange { revoked: Some(true), ..Default::default() })
            .map_err(|error| error.to_string())?;
    }
    repository
        .create(TrustRecordChange {
            host: Edit::Set(plan.host_id),
            key: Edit::Set(current.key),
            fingerprint: Edit::Set(current.fingerprint),
            provenance: Some(Provenance::Manual),
            accepted_at: Edit::Set(now_millis()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Connects one direct password profile and streams its exec or shell output.
///
/// Host-key verification uses active, host-specific trust records and remains
/// fail-closed for new, changed, revoked, wildcard-only, or CA records.
#[tauri::command]
#[specta::specta]
pub async fn session_connect(
    local: tauri::State<'_, LocalDataState>,
    profile_id: String,
    command: Option<String>,
    pty: bool,
    frames: Channel<RawFrame>,
) -> Result<(), String> {
    let store = local.store().ok_or_else(|| "local data is locked".to_owned())?;
    let inventory = Inventory::with_deletions(store.clone(), DeletionStore::default());
    let plan = inventory.connection_plan(profile_id).map_err(|error| error.to_string())?;
    let records =
        Repository::<TrustRecord>::new(&store).list().map_err(|error| error.to_string())?;
    let trust: HostKeyTrust = Arc::new(StoredHostKeyVerifier::new(plan.host_id, records));
    let mut connection =
        SshConnection::connect(&plan, Some(trust)).await.map_err(display_connection_error)?;
    connection.authenticate().await.map_err(display_connection_error)?;
    let mut session = connection.open_session().await.map_err(display_connection_error)?;
    let sender = FrameSender::new(frames);
    let pty_request = pty.then(default_pty);
    if let Some(command) = command {
        let output = session
            .exec(command.as_bytes(), pty_request.as_ref())
            .await
            .map_err(display_connection_error)?;
        send_exec_output(&sender, output)?;
    } else {
        if let Some(request) = pty_request.as_ref() {
            session.request_pty(request).await.map_err(display_connection_error)?;
        }
        session.request_shell().await.map_err(display_connection_error)?;
        stream_channel(&mut session, &sender).await?;
    }
    connection.close().await.map_err(display_connection_error)
}

fn default_pty() -> scoplen_ssh::PtyRequest {
    scoplen_ssh::PtyRequest {
        term: b"xterm-256color".to_vec(),
        columns: 80,
        rows: 24,
        pixel_width: 0,
        pixel_height: 0,
        modes: vec![0],
    }
}

fn send_exec_output(sender: &FrameSender, output: SshExecOutput) -> Result<(), String> {
    if !output.stdout.is_empty() {
        sender.send(output.stdout).map_err(|error| error.to_string())?;
    }
    if !output.stderr.is_empty() {
        sender.send(output.stderr).map_err(|error| error.to_string())?;
    }
    Ok(())
}

async fn stream_channel(session: &mut SshSession, sender: &FrameSender) -> Result<(), String> {
    while let Some(event) = session.next_event().await.map_err(display_connection_error)? {
        match event {
            ChannelEvent::Data(data) | ChannelEvent::ExtendedData { data, .. } => {
                sender.send(data).map_err(|error| error.to_string())?;
            }
            ChannelEvent::Close => break,
            _ => {}
        }
    }
    Ok(())
}

fn display_connection_error(error: SshConnectionError) -> String {
    error.to_string()
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
}
