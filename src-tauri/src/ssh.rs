// SPDX-License-Identifier: Apache-2.0

//! The smallest live SSH IPC path: resolve one local profile, connect through
//! the shared engine, and stream channel bytes as binary Tauri frames.

use crate::frames::{FrameSender, RawFrame};
use crate::local_data::LocalDataState;
use scoplen_client_core::connection::{
    HostKeyTrust, SshConnection, SshConnectionError, SshExecOutput, SshSession,
    StoredHostKeyVerifier,
};
use scoplen_client_core::inventory::{DeletionStore, Inventory};
use scoplen_client_core::repository::{Repository, TrustRecord};
use scoplen_ssh::ChannelEvent;
use std::sync::Arc;
use tauri::ipc::Channel;

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
