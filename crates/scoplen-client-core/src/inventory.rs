// SPDX-License-Identifier: Apache-2.0

//! The local inventory facade used by the desktop commands.
//!
//! This module is the boundary between the typed repositories and the
//! frontend. It returns only display data, never credential secrets, and uses
//! store transactions for operations that create or remove related objects.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::repository::{
    AccessProfile, Credential, CredentialBinding, CredentialKind, Edit, Host, HostChange,
    HostGroup, HostGroupChange, RecordError, Repository, Route, RouteChoice, RouteKind,
    credential_secret,
};
use crate::store::device::{SESSION_HISTORY_LIMIT, SessionKind, SessionOutcome};
use crate::store::{NewObject, Store, StoreError};

const UNDO_LIFETIME: Duration = Duration::from_secs(5 * 60);
const MAX_OPEN_SSH_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_OPEN_SSH_HOSTS: usize = 256;
const MAX_OPEN_SSH_LINE_BYTES: usize = 4096;
const MAX_PRIVATE_KEY_BYTES: u64 = 1024 * 1024;
const MAX_PRIVATE_KEY_REUSE_SCAN: usize = 4096;

/// Which parts of the inventory have content to show.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Areas {
    /// At least one host is marked as a favorite.
    pub favorites: bool,
    /// At least one saved session refers to a host.
    pub recent: bool,
    /// At least one host group exists.
    pub groups: bool,
    /// At least one credential exists.
    pub keys: bool,
    /// At least one non-direct route is used or saved.
    pub routes: bool,
}

/// Which hosts to list.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HostSource {
    /// Every live host.
    All,
    /// Hosts marked as favorites.
    Favorites,
    /// Hosts referenced by recent sessions.
    Recent,
    /// Hosts in one group.
    Group {
        /// The group identifier.
        id: String,
    },
}

/// How a login reaches its host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RouteLabel {
    /// A direct SSH connection.
    Direct,
    /// Through one or more jump hosts.
    Jump {
        /// The route name.
        name: String,
    },
    /// Through a SOCKS or HTTP proxy.
    Proxy {
        /// The route name.
        name: String,
    },
    /// Through a ProxyCommand.
    Command {
        /// The route name.
        name: String,
    },
    /// Through a managed gateway network.
    Bastion {
        /// The route name.
        name: String,
    },
}

/// A credential's non-secret display data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CredentialLabel {
    /// The credential identifier.
    pub id: String,
    /// Its kind in interface vocabulary.
    pub kind: CredentialLabelKind,
    /// The optional user-assigned name.
    pub name: Option<String>,
    /// The OpenSSH fingerprint, when available.
    pub fingerprint: Option<String>,
    /// The key comment, when available.
    pub comment: Option<String>,
    /// The public half of a key, when available.
    pub public_key: Option<String>,
}

/// Credential kinds understood by the inventory interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CredentialLabelKind {
    /// A password.
    Password,
    /// A private key.
    Key,
    /// A system SSH agent.
    Agent,
    /// A hardware security key.
    SecurityKey,
    /// A non-exportable device key.
    DeviceKey,
    /// An external provider.
    External,
    /// A short-lived certificate.
    Certificate,
}

/// A row in the host list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HostSummary {
    /// The host identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// DNS name or IP literal.
    pub address: String,
    /// SSH port.
    pub port: u16,
    /// The default login's username.
    pub username: Option<String>,
    /// Number of logins for this host.
    pub login_count: u32,
    /// How its default login connects.
    pub route: RouteLabel,
    /// Whether it is a favorite.
    pub favorite: bool,
    /// User tags.
    pub tags: BTreeMap<String, String>,
    /// Group identifiers.
    pub groups: Vec<String>,
    /// Whether a later write resurrected it after deletion.
    pub restored: bool,
}

/// Details shown beside the host list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HostDetails {
    /// The shared summary fields.
    #[serde(flatten)]
    pub summary: HostSummary,
    /// Plain-text notes.
    pub notes: Option<String>,
    /// All logins for this host.
    pub logins: Vec<LoginSummary>,
}

/// One login shown in host details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LoginSummary {
    /// The access profile identifier.
    pub id: String,
    /// Optional display name.
    pub name: Option<String>,
    /// Remote username.
    pub username: String,
    /// Its non-secret credential, if one is selected.
    pub credential: Option<CredentialLabel>,
    /// How this login reaches its host.
    pub route: RouteLabel,
    /// Whether it is the host's default login.
    pub is_default: bool,
}

/// The kind of a device-local session shown in the Recent area.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RecentSessionKind {
    Terminal,
    Files,
    Forward,
}

/// How a device-local session ended, when it is no longer open.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RecentSessionOutcome {
    Closed,
    Failed,
}

/// A redacted entry in the device-local Recent session history.
/// This is intentionally a read model. It carries no credential or terminal
/// data, and recording or reconnecting a session remains the responsibility of
/// the connection and session managers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecentSession {
    /// The device-local session identifier.
    pub id: String,
    /// The live Host this session used.
    pub host_id: String,
    /// The Host display name at read time.
    pub host_name: String,
    /// The Host DNS name or IP literal.
    pub address: String,
    /// The Host SSH port.
    pub port: u16,
    /// The login username used by the session.
    pub username: String,
    /// The session kind.
    pub kind: RecentSessionKind,
    /// Start time in Unix milliseconds, encoded as decimal text at the IPC
    /// boundary so JavaScript cannot lose precision.
    pub started_at: String,
    /// End time in Unix milliseconds, if it has ended.
    pub ended_at: Option<String>,
    /// End status, if it has ended.
    pub outcome: Option<RecentSessionOutcome>,
}

/// A host group in the sidebar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GroupSummary {
    /// The group identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Parent group, if nested.
    pub parent: Option<String>,
    /// Number of live hosts in the group.
    pub host_count: u32,
}

/// The editable host metadata owned by the local inventory.
#[derive(Clone, Debug, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditHost {
    /// Display name; it must not be empty.
    pub name: String,
    /// DNS name or IP literal.
    pub address: String,
    /// SSH port, validated before the object is written.
    pub port: u32,
    /// Plain-text notes, or `null` to clear them.
    pub notes: Option<String>,
    /// Complete replacement for the tag map.
    pub tags: BTreeMap<String, String>,
    /// Complete replacement for host-group membership.
    pub groups: Vec<String>,
}

/// Why editing host metadata failed. The host is unchanged for every variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EditHostError {
    /// The display name is empty.
    #[error("the host name is empty")]
    EmptyName,
    /// The address is empty or contains whitespace.
    #[error("the address is not valid")]
    InvalidAddress,
    /// The port is outside the TCP range.
    #[error("the port is not valid")]
    InvalidPort,
    /// Notes exceed the object-model text limit.
    #[error("the notes are too long")]
    NotesTooLong,
    /// A tag key or value is empty or exceeds the object-model text limit.
    #[error("the tag is not valid")]
    InvalidTag,
    /// A requested group does not exist.
    #[error("the group was not found")]
    GroupNotFound,
    /// The local store or model failed without changing the host.
    #[error("{reference}")]
    Failed {
        /// A diagnostic reference safe to show for retryable failures.
        reference: String,
    },
}

/// Input for creating or editing a host group.
#[derive(Clone, Debug, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GroupInput {
    /// Display name; it must not be empty.
    pub name: String,
    /// Optional parent group.
    pub parent: Option<String>,
}

/// Why creating or editing a group failed. No group is changed for failures.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GroupError {
    /// The display name is empty.
    #[error("the group name is empty")]
    EmptyName,
    /// A requested parent group does not exist.
    #[error("the parent group was not found")]
    ParentNotFound,
    /// A group cannot be its own parent.
    #[error("a group cannot be its own parent")]
    SelfParent,
    /// The local store or model failed without changing the group.
    #[error("{reference}")]
    Failed {
        /// A diagnostic reference safe to show for retryable failures.
        reference: String,
    },
}

/// How a new host authenticates.
#[derive(Clone, Debug, Deserialize, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SignIn {
    /// Store a password in the encrypted personal object.
    Password {
        /// The password to store in the encrypted local object.
        password: String,
    },
    /// Read an OpenSSH private key from a path.
    KeyFile {
        /// The path to an OpenSSH private key file.
        path: String,
    },
    /// Use OpenSSH private-key text supplied by the user.
    KeyText {
        /// OpenSSH private-key text.
        key: String,
    },
    /// Generate a key (reserved for the key-generation flow).
    GenerateKey,
    /// Resolve a key from the system agent.
    Agent,
}

/// Input for the tier-0 add-host flow.
#[derive(Clone, Debug, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewHost {
    /// Display name; defaults to the address.
    pub name: Option<String>,
    /// DNS name or IP literal.
    pub address: String,
    /// SSH port; defaults to 22.
    pub port: Option<u16>,
    /// Remote username.
    pub username: String,
    /// Authentication material.
    pub sign_in: SignIn,
}

/// Why adding a host failed. Nothing is saved for any variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AddHostError {
    /// The address is empty or contains whitespace.
    #[error("the address is not valid")]
    InvalidAddress,
    /// The port is outside the TCP range.
    #[error("the port is not valid")]
    InvalidPort,
    /// The username is empty.
    #[error("the username is empty")]
    EmptyUsername,
    /// The password is empty.
    #[error("the password is empty")]
    EmptyPassword,
    /// The selected key file does not exist or could not be read.
    #[error("the selected key file was not found")]
    KeyNotFound,
    /// The selected key file exceeds the bounded local import size.
    #[error("the selected key file is too large")]
    KeyTooLarge,
    /// Reusing a private key would require scanning beyond the bounded local limit.
    #[error("the private-key reuse search reached its local limit")]
    KeyReuseLimit,
    /// The input is not a private key.
    #[error("the input is not a private key")]
    NotAPrivateKey,
    /// Public key text was supplied where private key text is needed.
    #[error("a public key cannot authenticate a login")]
    PublicKey,
    /// A PuTTY key is not accepted by the OpenSSH key reader.
    #[error("PuTTY keys must be converted to OpenSSH format first")]
    PuttyKey,
    /// A disk or model operation failed.
    #[error("{reference}")]
    Failed {
        /// A diagnostic reference safe to show for retryable failures.
        reference: String,
    },
}

/// A representable entry found in an OpenSSH configuration file. This is a
/// preview DTO: it never contains private key material.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpenSshImportEntry {
    /// The `Host` alias that becomes the display name.
    pub alias: String,
    /// The resolved host name or address.
    pub address: String,
    /// SSH port, defaulting to 22.
    pub port: u16,
    /// The configured user, or the current local user when omitted.
    pub username: String,
    /// An identity file to read at commit time, if configured.
    pub identity_file: Option<String>,
}

/// A directive the limited tier-0 importer cannot represent yet.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpenSshUnsupportedDirective {
    /// One-based source line.
    pub line: u32,
    /// The directive name as written.
    pub directive: String,
}

/// Why a literal `Host` alias cannot be imported without changing its meaning.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OpenSshSkipReason {
    /// A directive within this host block has no tier-0 representation.
    UnsupportedDirective {
        /// The directive name as written.
        directive: String,
    },
    /// A global rule could change any host in the source file.
    GlobalRules {
        /// The global directive responsible for the exclusion.
        directive: String,
    },
    /// Multiple `Host` blocks name the same alias.
    DuplicateAlias,
    /// A supported directive has a value outside the simple import subset.
    InvalidValue {
        /// The directive name as written.
        directive: String,
    },
}

/// A literal alias excluded from the tier-0 import preview.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpenSshSkippedHost {
    /// The `Host` alias.
    pub alias: String,
    /// One-based line of its `Host` block.
    pub line: u32,
    /// Why importing it would be misleading.
    pub reason: OpenSshSkipReason,
}

/// A preview of importing one OpenSSH configuration file.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpenSshImportPreview {
    /// The selected source path.
    pub path: String,
    /// Entries that can be represented by the tier-0 importer.
    pub entries: Vec<OpenSshImportEntry>,
    /// Literal aliases excluded from import, with a visible reason.
    pub skipped_hosts: Vec<OpenSshSkippedHost>,
    /// Directives that will be skipped and shown to the user.
    pub unsupported: Vec<OpenSshUnsupportedDirective>,
}

/// The result of importing a preview into an empty inventory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpenSshImportResult {
    /// Hosts created from the preview.
    pub hosts: Vec<HostDetails>,
    /// Literal aliases excluded from import, with a visible reason.
    pub skipped_hosts: Vec<OpenSshSkippedHost>,
    /// Unsupported directives retained for the report.
    pub unsupported: Vec<OpenSshUnsupportedDirective>,
}

/// Why the empty-list OpenSSH import could not proceed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OpenSshImportError {
    /// The selected file could not be read.
    #[error("the OpenSSH configuration could not be read")]
    FileNotFound,
    /// The source was not valid UTF-8.
    #[error("the OpenSSH configuration is not UTF-8 text")]
    NotText,
    /// The config or its simple-host list exceeds the bounded onboarding import.
    #[error("the OpenSSH configuration exceeds the onboarding import limit")]
    TooLarge,
    /// No representable host block was found.
    #[error("the OpenSSH configuration has no representable hosts")]
    NoHosts,
    /// The source changed after the user reviewed its preview.
    #[error("the OpenSSH configuration changed after preview")]
    SourceChanged,
    /// Import is intentionally limited to the first-launch empty inventory.
    #[error("OpenSSH import is available only when there are no hosts")]
    InventoryNotEmpty,
    /// An identity file named by a representable entry could not be read.
    #[error("the configured identity file could not be read")]
    IdentityNotFound {
        /// The path, which is not secret.
        path: String,
    },
    /// The selected file or local store failed without a partial import.
    #[error("{reference}")]
    Failed {
        /// A diagnostic reference safe to show for retryable failures.
        reference: String,
    },
}

/// A failure that can be retried by the interface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Failure {
    /// The local store or model failed without changing the requested data.
    #[error("{reference}")]
    Failed {
        /// A diagnostic reference safe to show for retryable failures.
        reference: String,
    },
}

/// A deletion token returned to the interface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Deletion {
    /// An opaque token accepted by [`Inventory::undo_delete`].
    pub token: String,
}

#[derive(Clone)]
struct PendingDelete {
    ids: Vec<Uuid>,
    expires: Instant,
}

/// Shared, in-memory undo token state owned by the desktop process.
#[derive(Clone, Default)]
pub struct DeletionStore {
    pending: Arc<Mutex<HashMap<Uuid, PendingDelete>>>,
}

/// Inventory operations over one open local store.
#[derive(Clone)]
pub struct Inventory {
    store: Arc<Store>,
    deletions: DeletionStore,
}

impl Inventory {
    /// Creates an inventory facade with process-local undo state.
    pub fn new(store: Arc<Store>) -> Self {
        Self::with_deletions(store, DeletionStore::default())
    }

    /// Creates a facade sharing undo state with other command invocations.
    pub fn with_deletions(store: Arc<Store>, deletions: DeletionStore) -> Self {
        Inventory { store, deletions }
    }

    /// Returns which navigation areas have content.
    pub fn areas(&self) -> Result<Areas, Failure> {
        let hosts = self.host_records()?;
        let groups = Repository::<HostGroup>::new(&self.store).list().map_err(failure)?;
        let credentials = Repository::<Credential>::new(&self.store).list().map_err(failure)?;
        let profiles = Repository::<AccessProfile>::new(&self.store).list().map_err(failure)?;
        let routes = Repository::<Route>::new(&self.store).list().map_err(failure)?;
        let recent = !self.recent_sessions()?.is_empty();
        let credential_uses = profiles.iter().filter_map(|profile| profile.credential).fold(
            HashMap::<Uuid, u32>::new(),
            |mut uses, credential| {
                *uses.entry(credential).or_default() += 1;
                uses
            },
        );
        let mut explicit_key = false;
        for credential in &credentials {
            if credential.kind != CredentialKind::PrivateKey {
                continue;
            }
            let implicit = self.store.is_implicit(credential.meta.id).map_err(failure)?;
            if !implicit
                || credential_uses.get(&credential.meta.id).copied().unwrap_or_default() > 1
            {
                explicit_key = true;
                break;
            }
        }
        Ok(Areas {
            favorites: hosts.iter().any(|host| host.favorite),
            recent,
            groups: !groups.is_empty(),
            keys: explicit_key,
            routes: !routes.is_empty()
                || profiles.iter().any(|p| matches!(p.route, RouteChoice::Route(_))),
        })
    }

    /// Lists groups with live-host counts.
    pub fn groups(&self) -> Result<Vec<GroupSummary>, Failure> {
        let groups = Repository::<HostGroup>::new(&self.store).list().map_err(failure)?;
        let hosts = self.host_records()?;
        Ok(groups
            .into_iter()
            .map(|group| GroupSummary {
                id: id(group.meta.id),
                name: group.name,
                parent: group.parent.map(id),
                host_count: hosts.iter().filter(|host| host.groups.contains(&group.meta.id)).count()
                    as u32,
            })
            .collect())
    }

    /// Lists hosts from a source, filtered by name, address, username, or tag.
    pub fn hosts(&self, source: HostSource, query: String) -> Result<Vec<HostSummary>, Failure> {
        let hosts = self.host_records()?;
        let profiles = self.profiles()?;
        let routes = self.routes()?;
        let recent = self.recent_hosts(&profiles)?;
        let query = query.trim().to_lowercase();
        let group = match &source {
            HostSource::Group { id } => {
                Some(parse_id(id.as_str()).map_err(|_| failed("the group identifier is invalid"))?)
            }
            _ => None,
        };
        let kind = source;
        let mut summaries = hosts
            .into_iter()
            .filter(|host| match (&kind, group) {
                (HostSource::All, _) => true,
                (HostSource::Favorites, _) => host.favorite,
                (HostSource::Recent, _) => recent.contains(&host.meta.id),
                (HostSource::Group { .. }, Some(group)) => host.groups.contains(&group),
                (HostSource::Group { .. }, None) => false,
            })
            .filter(|host| {
                if query.is_empty() {
                    return true;
                }
                let profile_text = profiles
                    .iter()
                    .filter(|profile| profile.host == host.meta.id)
                    .map(|profile| profile.username.to_lowercase())
                    .collect::<Vec<_>>()
                    .join(" ");
                host.name.to_lowercase().contains(&query)
                    || host.address.to_lowercase().contains(&query)
                    || profile_text.contains(&query)
                    || host.tags.iter().any(|(key, value)| {
                        key.to_lowercase().contains(&query) || value.to_lowercase().contains(&query)
                    })
            })
            .map(|host| summary(&host, &profiles, &routes))
            .collect::<Vec<_>>();
        summaries.sort_by(|left, right| {
            right.favorite.cmp(&left.favorite).then_with(|| left.name.cmp(&right.name))
        });
        Ok(summaries)
    }

    /// Lists the newest device-local sessions with their current Host labels.
    ///
    /// Session history can outlive an object deletion or a profile change. An
    /// entry is therefore exposed only while both its login and Host are live;
    /// stale entries remain in the local history for retention but never become
    /// a misleading navigation target.
    pub fn recent_sessions(&self) -> Result<Vec<RecentSession>, Failure> {
        let hosts = self.host_records()?;
        let profiles = self.profiles()?;
        let sessions = self.store.recent_sessions(SESSION_HISTORY_LIMIT).map_err(failure)?;
        Ok(sessions
            .into_iter()
            .filter_map(|session| {
                let profile = profiles.iter().find(|profile| profile.meta.id == session.profile)?;
                let host = hosts.iter().find(|host| host.meta.id == profile.host)?;
                Some(RecentSession {
                    id: id(session.id),
                    host_id: id(host.meta.id),
                    host_name: host.name.clone(),
                    address: host.address.clone(),
                    port: host.port,
                    username: profile.username.clone(),
                    kind: recent_session_kind(session.kind),
                    started_at: session.started_at.to_string(),
                    ended_at: session.ended_at.map(|value| value.to_string()),
                    outcome: session.outcome.map(recent_session_outcome),
                })
            })
            .collect())
    }

    /// Gets one host and all of its logins.
    pub fn host(&self, host_id: String) -> Result<Option<HostDetails>, Failure> {
        let host_uuid = parse_id(&host_id).map_err(|_| failed("the host identifier is invalid"))?;
        let details = self.host_without_promotion(host_id)?;
        let Some(details) = details else { return Ok(None) };
        let profiles = self.profiles()?;
        let credentials = self.credentials()?;
        let mut inspected = Vec::new();
        for profile in profiles.iter().filter(|profile| profile.host == host_uuid) {
            inspected.push((scoplen_model::ObjectType::ACCESS_PROFILE, profile.meta.id));
            if let Some(credential) = profile.credential
                && credentials.iter().any(|item| {
                    item.meta.id == credential && item.kind == CredentialKind::PrivateKey
                })
            {
                inspected.push((scoplen_model::ObjectType::CREDENTIAL, credential));
            }
        }
        self.store.promote_batch(inspected).map_err(failure)?;
        Ok(Some(details))
    }

    fn host_without_promotion(&self, host_id: String) -> Result<Option<HostDetails>, Failure> {
        let host_uuid = parse_id(&host_id).map_err(|_| failed("the host identifier is invalid"))?;
        let Some(host) = Repository::<Host>::new(&self.store).get(host_uuid).map_err(failure)?
        else {
            return Ok(None);
        };
        let profiles = self.profiles()?;
        let routes = self.routes()?;
        let credentials = self.credentials()?;
        let summary = summary(&host, &profiles, &routes);
        let mut logins = profiles
            .iter()
            .filter(|profile| profile.host == host_uuid)
            .map(|profile| LoginSummary {
                id: id(profile.meta.id),
                name: profile.name.clone(),
                username: profile.username.clone(),
                credential: profile
                    .credential
                    .and_then(|credential| {
                        credentials.iter().find(|item| item.meta.id == credential)
                    })
                    .map(credential_label),
                route: route_label(profile, &routes),
                is_default: host.default_profile == Some(profile.meta.id),
            })
            .collect::<Vec<_>>();
        logins.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(Some(HostDetails { summary, notes: host.notes, logins }))
    }

    /// Replaces editable host metadata, tags, and group memberships in one
    /// model write. The host remains unchanged when validation fails.
    pub fn update_host(
        &self,
        host_id: String,
        input: EditHost,
    ) -> Result<HostDetails, EditHostError> {
        let host_uuid =
            parse_id(&host_id).map_err(|_| edit_failed("the host identifier is invalid"))?;
        let hosts = Repository::<Host>::new(&self.store);
        let existing = hosts
            .get(host_uuid)
            .map_err(edit_failure)?
            .ok_or_else(|| edit_failed("the host was not found"))?;
        let name = input.name.trim().to_owned();
        if name.is_empty() {
            return Err(EditHostError::EmptyName);
        }
        let address = input.address.trim().to_owned();
        if address.is_empty() || address.chars().any(char::is_whitespace) {
            return Err(EditHostError::InvalidAddress);
        }
        if input.port == 0 || input.port > u16::MAX as u32 {
            return Err(EditHostError::InvalidPort);
        }
        if input.notes.as_ref().is_some_and(|notes| notes.len() > scoplen_model::MAX_TEXT_BYTES) {
            return Err(EditHostError::NotesTooLong);
        }
        if input.tags.iter().any(|(key, value)| {
            key.is_empty()
                || key.len() > scoplen_model::MAX_TEXT_BYTES
                || value.len() > scoplen_model::MAX_TEXT_BYTES
        }) {
            return Err(EditHostError::InvalidTag);
        }

        let mut groups = BTreeSet::new();
        let group_repo = Repository::<HostGroup>::new(&self.store);
        let known_groups = group_repo.list().map_err(edit_failure)?;
        for group in input.groups {
            let group_id = parse_id(&group).map_err(|_| EditHostError::GroupNotFound)?;
            if !known_groups.iter().any(|known| known.meta.id == group_id) {
                return Err(EditHostError::GroupNotFound);
            }
            groups.insert(group_id);
        }
        let tags = input.tags;
        let mut tag_changes = BTreeMap::new();
        for key in existing.tags.keys().chain(tags.keys()) {
            if tag_changes.contains_key(key) {
                continue;
            }
            tag_changes.insert(key.clone(), tags.get(key).cloned());
        }
        let mut group_changes = BTreeMap::new();
        for group in existing.groups.iter().chain(groups.iter()) {
            if group_changes.contains_key(group) {
                continue;
            }
            group_changes.insert(*group, groups.contains(group));
        }
        hosts
            .update(
                host_uuid,
                HostChange {
                    name: Some(name),
                    address: Some(address),
                    port: Edit::Set(input.port as u16),
                    tags: tag_changes,
                    groups: group_changes,
                    notes: Edit::from_option(input.notes),
                    ..Default::default()
                },
            )
            .map_err(edit_failure)?;
        self.host(host_id)
            .map_err(|error| EditHostError::Failed { reference: error.to_string() })?
            .ok_or_else(|| edit_failed("the host disappeared after editing"))
    }

    /// Creates a host group after validating its optional parent.
    pub fn create_group(&self, input: GroupInput) -> Result<GroupSummary, GroupError> {
        let name = input.name.trim().to_owned();
        if name.is_empty() {
            return Err(GroupError::EmptyName);
        }
        let parent = self.validate_parent(None, input.parent)?;
        let group = Repository::<HostGroup>::new(&self.store)
            .create(HostGroupChange {
                name: Some(name),
                parent: Edit::from_option(parent),
                ..Default::default()
            })
            .map_err(group_failure)?;
        self.group_summary(group)
    }

    /// Updates a host group's name and optional parent.
    pub fn update_group(
        &self,
        group_id: String,
        input: GroupInput,
    ) -> Result<GroupSummary, GroupError> {
        let id =
            parse_id(&group_id).map_err(|_| group_failed("the group identifier is invalid"))?;
        let groups = Repository::<HostGroup>::new(&self.store);
        if groups.get(id).map_err(group_failure)?.is_none() {
            return Err(group_failed("the group was not found"));
        }
        let name = input.name.trim().to_owned();
        if name.is_empty() {
            return Err(GroupError::EmptyName);
        }
        let parent = self.validate_parent(Some(id), input.parent)?;
        let group = groups
            .update(
                id,
                HostGroupChange {
                    name: Some(name),
                    parent: Edit::from_option(parent),
                    ..Default::default()
                },
            )
            .map_err(group_failure)?;
        self.group_summary(group)
    }

    fn validate_parent(
        &self,
        self_id: Option<Uuid>,
        parent: Option<String>,
    ) -> Result<Option<Uuid>, GroupError> {
        let Some(parent) = parent else { return Ok(None) };
        let parent = parse_id(&parent).map_err(|_| GroupError::ParentNotFound)?;
        if self_id == Some(parent) {
            return Err(GroupError::SelfParent);
        }
        let groups = Repository::<HostGroup>::new(&self.store);
        if groups.get(parent).map_err(group_failure)?.is_none() {
            return Err(GroupError::ParentNotFound);
        }
        Ok(Some(parent))
    }

    fn group_summary(&self, group: HostGroup) -> Result<GroupSummary, GroupError> {
        let host_count = self
            .host_records()
            .map_err(|error| GroupError::Failed { reference: error.to_string() })?
            .iter()
            .filter(|host| host.groups.contains(&group.meta.id))
            .count() as u32;
        Ok(GroupSummary {
            id: id(group.meta.id),
            name: group.name,
            parent: group.parent.map(id),
            host_count,
        })
    }

    /// Adds a host, its credential, and its default login atomically.
    pub fn add_host(&self, input: NewHost) -> Result<HostDetails, AddHostError> {
        let address = input.address.trim().to_owned();
        if address.is_empty() || address.chars().any(char::is_whitespace) {
            return Err(AddHostError::InvalidAddress);
        }
        let port = input.port.unwrap_or(22);
        if port == 0 {
            return Err(AddHostError::InvalidPort);
        }
        let username = input.username.trim().to_owned();
        if username.is_empty() {
            return Err(AddHostError::EmptyUsername);
        }
        let name =
            input.name.filter(|name| !name.trim().is_empty()).unwrap_or_else(|| address.clone());
        let (kind, binding, secret) = credential_input(input.sign_in)?;
        let host_id = new_id().map_err(add_failed)?;
        let profile_id = new_id().map_err(add_failed)?;
        let reusable = self.reusable_key(kind, binding, secret.as_deref())?;
        let reused = reusable.is_some();
        let credential_id = match reusable {
            Some(credential) => credential.meta.id,
            None => new_id().map_err(add_failed)?,
        };
        let host_fields = vec![
            (scoplen_model::FieldPath::Field(1), text(name)),
            (scoplen_model::FieldPath::Field(2), text(address)),
            (scoplen_model::FieldPath::Field(3), scoplen_model::cbor::Value::UInt(port as u64)),
            (scoplen_model::FieldPath::Field(8), uuid_value(profile_id)),
        ];
        let profile_fields = vec![
            (scoplen_model::FieldPath::Field(1), uuid_value(host_id)),
            (scoplen_model::FieldPath::Field(3), text(username)),
            (scoplen_model::FieldPath::Field(4), uuid_value(credential_id)),
            (scoplen_model::FieldPath::Field(5), uint(0)),
        ];
        let mut creates = vec![NewObject {
            id: host_id,
            object_type: scoplen_model::ObjectType::HOST,
            fields: host_fields,
        }];
        if !reused {
            let mut credential_fields = vec![
                (scoplen_model::FieldPath::Field(2), uint(kind as u64)),
                (scoplen_model::FieldPath::Field(3), uint(binding as u64)),
            ];
            if let Some(secret) = secret {
                credential_fields.push((
                    scoplen_model::FieldPath::Field(4),
                    scoplen_model::cbor::Value::Bytes(secret),
                ));
            }
            creates.push(NewObject {
                id: credential_id,
                object_type: scoplen_model::ObjectType::CREDENTIAL,
                fields: credential_fields,
            });
        }
        creates.push(NewObject {
            id: profile_id,
            object_type: scoplen_model::ObjectType::ACCESS_PROFILE,
            fields: profile_fields,
        });
        let mut implicit = vec![(scoplen_model::ObjectType::ACCESS_PROFILE, profile_id)];
        if !reused {
            implicit.push((scoplen_model::ObjectType::CREDENTIAL, credential_id));
        }
        let promotions = if reused {
            vec![(scoplen_model::ObjectType::CREDENTIAL, credential_id)]
        } else {
            Vec::new()
        };
        self.store
            .create_batch_with_implicit_and_promotions(creates, implicit, promotions)
            .map_err(add_failed)?;
        self.host_without_promotion(id(host_id))
            .map_err(|error| AddHostError::Failed { reference: error.to_string() })?
            .ok_or_else(|| add_failed("the host disappeared after creation"))
    }

    fn reusable_key(
        &self,
        kind: CredentialKind,
        binding: CredentialBinding,
        secret: Option<&[u8]>,
    ) -> Result<Option<Credential>, AddHostError> {
        if kind != CredentialKind::PrivateKey || binding != CredentialBinding::Shared {
            return Ok(None);
        }
        let Some(secret) = secret else { return Ok(None) };
        let credentials = Repository::<Credential>::new(&self.store)
            .list()
            .map_err(|error| AddHostError::Failed { reference: diagnostic(&error) })?;
        let mut scanned = 0;
        for credential in credentials {
            if credential.kind != kind || credential.binding != binding || !credential.has_secret {
                continue;
            }
            scanned += 1;
            if scanned > MAX_PRIVATE_KEY_REUSE_SCAN {
                return Err(AddHostError::KeyReuseLimit);
            }
            if credential_secret(&self.store, credential.meta.id)
                .map_err(|error| AddHostError::Failed { reference: diagnostic(&error) })?
                .is_some_and(|stored| stored.as_bytes() == secret)
            {
                return Ok(Some(credential));
            }
        }
        Ok(None)
    }

    /// Reads a simple OpenSSH configuration for the empty-inventory onboarding
    /// flow. The source is never modified and the result contains no secret
    /// material. Full `Include`, `Match`, wildcard, proxy, and forwarding
    /// semantics remain owned by the later OpenSSH interoperability gate.
    pub fn preview_open_ssh_config(
        &self,
        path: String,
    ) -> Result<OpenSshImportPreview, OpenSshImportError> {
        let parsed = parse_open_ssh_config(Path::new(&path))?;
        if parsed.entries.is_empty()
            && parsed.skipped_hosts.is_empty()
            && parsed.unsupported.is_empty()
        {
            return Err(OpenSshImportError::NoHosts);
        }
        Ok(OpenSshImportPreview {
            path,
            entries: parsed.entries,
            skipped_hosts: parsed.skipped_hosts,
            unsupported: parsed.unsupported,
        })
    }

    /// Imports representable `Host` blocks only when the inventory is empty.
    /// All entries are validated before the first write; a malformed source or
    /// missing identity therefore leaves the store unchanged.
    pub fn import_open_ssh_config(
        &self,
        preview: OpenSshImportPreview,
    ) -> Result<OpenSshImportResult, OpenSshImportError> {
        if !self.host_records().map_err(import_failure)?.is_empty() {
            return Err(OpenSshImportError::InventoryNotEmpty);
        }
        let parsed = parse_open_ssh_config(Path::new(&preview.path))?;
        if parsed.entries != preview.entries
            || parsed.skipped_hosts != preview.skipped_hosts
            || parsed.unsupported != preview.unsupported
        {
            return Err(OpenSshImportError::SourceChanged);
        }
        if parsed.entries.is_empty() {
            return Err(OpenSshImportError::NoHosts);
        }
        let mut inputs = Vec::with_capacity(parsed.entries.len());
        for entry in &parsed.entries {
            let sign_in = match &entry.identity_file {
                Some(identity_file) => {
                    let path = expand_config_path(identity_file);
                    if !path.is_file() {
                        return Err(OpenSshImportError::IdentityNotFound {
                            path: path.to_string_lossy().into_owned(),
                        });
                    }
                    SignIn::KeyFile { path: path.to_string_lossy().into_owned() }
                }
                None => SignIn::Agent,
            };
            inputs.push(NewHost {
                name: Some(entry.alias.clone()),
                address: entry.address.clone(),
                port: Some(entry.port),
                username: entry.username.clone(),
                sign_in,
            });
        }

        let (creates, implicit, host_ids, promote_credentials) =
            self.prepare_import_batch(&inputs).map_err(import_add_failure)?;
        self.store
            .create_initial_inventory_batch(
                creates,
                implicit,
                promote_credentials
                    .into_iter()
                    .map(|id| (scoplen_model::ObjectType::CREDENTIAL, id))
                    .collect(),
            )
            .map_err(|error| match error {
                StoreError::InventoryNotEmpty => OpenSshImportError::InventoryNotEmpty,
                other => import_failure(other),
            })?;
        let mut hosts = Vec::with_capacity(host_ids.len());
        for host_id in host_ids {
            hosts.push(
                self.host_without_promotion(id(host_id))
                    .map_err(|error| OpenSshImportError::Failed { reference: error.to_string() })?
                    .ok_or_else(|| OpenSshImportError::Failed {
                        reference: "the imported host disappeared after creation".into(),
                    })?,
            );
        }
        Ok(OpenSshImportResult {
            hosts,
            skipped_hosts: parsed.skipped_hosts,
            unsupported: parsed.unsupported,
        })
    }

    fn prepare_import_batch(
        &self,
        inputs: &[NewHost],
    ) -> Result<PreparedImportBatch, AddHostError> {
        let mut creates = Vec::with_capacity(inputs.len() * 3);
        let mut implicit = Vec::with_capacity(inputs.len() * 2);
        let mut host_ids = Vec::with_capacity(inputs.len());
        let mut promote_credentials = Vec::new();
        let mut new_keys = HashMap::<Vec<u8>, Uuid>::new();
        let mut key_uses = HashMap::<Uuid, u32>::new();
        for input in inputs {
            let address = input.address.trim().to_owned();
            if address.is_empty() || address.chars().any(char::is_whitespace) {
                return Err(AddHostError::InvalidAddress);
            }
            let port = input.port.unwrap_or(22);
            if port == 0 {
                return Err(AddHostError::InvalidPort);
            }
            let username = input.username.trim().to_owned();
            if username.is_empty() {
                return Err(AddHostError::EmptyUsername);
            }
            let name = input
                .name
                .clone()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| address.clone());
            let (kind, binding, secret) = credential_input(input.sign_in.clone())?;
            let reusable =
                if kind == CredentialKind::PrivateKey && binding == CredentialBinding::Shared {
                    if let Some(secret) = secret.as_deref() {
                        if let Some(id) = new_keys.get(secret) {
                            Some(*id)
                        } else {
                            self.reusable_key(kind, binding, Some(secret))?
                                .map(|credential| credential.meta.id)
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
            let host_id = new_id().map_err(add_failed)?;
            let profile_id = new_id().map_err(add_failed)?;
            let credential_id = reusable.unwrap_or(new_id().map_err(add_failed)?);
            host_ids.push(host_id);
            creates.push(NewObject {
                id: host_id,
                object_type: scoplen_model::ObjectType::HOST,
                fields: vec![
                    (scoplen_model::FieldPath::Field(1), text(name)),
                    (scoplen_model::FieldPath::Field(2), text(address)),
                    (scoplen_model::FieldPath::Field(3), uint(port as u64)),
                    (scoplen_model::FieldPath::Field(8), uuid_value(profile_id)),
                ],
            });
            if reusable.is_none() {
                let mut fields = vec![
                    (scoplen_model::FieldPath::Field(2), uint(kind as u64)),
                    (scoplen_model::FieldPath::Field(3), uint(binding as u64)),
                ];
                if let Some(secret) = secret {
                    if kind == CredentialKind::PrivateKey {
                        new_keys.insert(secret.clone(), credential_id);
                    }
                    fields.push((
                        scoplen_model::FieldPath::Field(4),
                        scoplen_model::cbor::Value::Bytes(secret),
                    ));
                }
                creates.push(NewObject {
                    id: credential_id,
                    object_type: scoplen_model::ObjectType::CREDENTIAL,
                    fields,
                });
                implicit.push((scoplen_model::ObjectType::CREDENTIAL, credential_id));
            } else {
                promote_credentials.push(credential_id);
            }
            *key_uses.entry(credential_id).or_default() += 1;
            creates.push(NewObject {
                id: profile_id,
                object_type: scoplen_model::ObjectType::ACCESS_PROFILE,
                fields: vec![
                    (scoplen_model::FieldPath::Field(1), uuid_value(host_id)),
                    (scoplen_model::FieldPath::Field(3), text(username)),
                    (scoplen_model::FieldPath::Field(4), uuid_value(credential_id)),
                    (scoplen_model::FieldPath::Field(5), uint(0)),
                ],
            });
            implicit.push((scoplen_model::ObjectType::ACCESS_PROFILE, profile_id));
        }
        for (credential_id, uses) in key_uses {
            if uses > 1 && !promote_credentials.contains(&credential_id) {
                promote_credentials.push(credential_id);
            }
        }
        Ok((creates, implicit, host_ids, promote_credentials))
    }

    /// Sets or clears the favorite marker.
    pub fn set_favorite(&self, host_id: String, favorite: bool) -> Result<(), Failure> {
        let id = parse_id(&host_id).map_err(|_| failed("the host identifier is invalid"))?;
        Repository::<Host>::new(&self.store)
            .update(id, HostChange { favorite: Some(favorite), ..Default::default() })
            .map(|_| ())
            .map_err(failure)
    }

    /// Deletes a host and its logins, retaining an in-process undo token.
    pub fn delete_host(&self, host_id: String) -> Result<Deletion, Failure> {
        let host_uuid = parse_id(&host_id).map_err(|_| failed("the host identifier is invalid"))?;
        let hosts = Repository::<Host>::new(&self.store);
        let Some(_host) = hosts.get(host_uuid).map_err(failure)? else {
            return Err(failed("the host was not found"));
        };
        let profiles = self.profiles()?;
        let credentials = self.credentials()?;
        let host_profiles =
            profiles.iter().filter(|profile| profile.host == host_uuid).collect::<Vec<_>>();
        let mut ids = vec![host_uuid];
        ids.extend(host_profiles.iter().map(|profile| profile.meta.id));
        let mut profile_uses: HashMap<Uuid, usize> = HashMap::new();
        for profile in &profiles {
            if let Some(credential) = profile.credential {
                *profile_uses.entry(credential).or_default() += 1;
            }
        }
        for profile in host_profiles {
            let Some(credential_id) = profile.credential else { continue };
            let Some(credential) =
                credentials.iter().find(|credential| credential.meta.id == credential_id)
            else {
                continue;
            };
            if credential.name.is_none() && profile_uses.get(&credential_id) == Some(&1) {
                ids.push(credential_id);
            }
        }
        self.store.delete_batch(ids.clone()).map_err(failure)?;
        let token = new_id().map_err(failure)?;
        self.deletions
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|_, pending| pending.expires > Instant::now());
        self.deletions
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(token, PendingDelete { ids, expires: Instant::now() + UNDO_LIFETIME });
        Ok(Deletion { token: id(token) })
    }

    /// Restores a recent deletion token.
    pub fn undo_delete(&self, token: String) -> Result<(), Failure> {
        let token_id = parse_id(&token).map_err(|_| failed("the deletion token is invalid"))?;
        let pending = {
            let mut pending =
                self.deletions.pending.lock().unwrap_or_else(|error| error.into_inner());
            pending.retain(|_, item| item.expires > Instant::now());
            pending.remove(&token_id).ok_or_else(|| failed("the deletion token has expired"))?
        };
        self.store.restore_batch(pending.ids).map_err(failure)
    }

    fn host_records(&self) -> Result<Vec<Host>, Failure> {
        Repository::<Host>::new(&self.store).list().map_err(failure)
    }

    fn profiles(&self) -> Result<Vec<AccessProfile>, Failure> {
        Repository::<AccessProfile>::new(&self.store).list().map_err(failure)
    }

    fn credentials(&self) -> Result<Vec<Credential>, Failure> {
        Repository::<Credential>::new(&self.store).list().map_err(failure)
    }

    fn routes(&self) -> Result<Vec<Route>, Failure> {
        Repository::<Route>::new(&self.store).list().map_err(failure)
    }

    fn recent_hosts(&self, profiles: &[AccessProfile]) -> Result<BTreeSet<Uuid>, Failure> {
        let recent = self.store.recent_sessions(1000).map_err(failure)?;
        Ok(recent
            .into_iter()
            .filter_map(|session| {
                profiles.iter().find(|profile| profile.meta.id == session.profile)
            })
            .map(|profile| profile.host)
            .collect())
    }
}

fn summary(host: &Host, profiles: &[AccessProfile], routes: &[Route]) -> HostSummary {
    let host_profiles =
        profiles.iter().filter(|profile| profile.host == host.meta.id).collect::<Vec<_>>();
    let default = host
        .default_profile
        .and_then(|id| host_profiles.iter().find(|profile| profile.meta.id == id).copied())
        .or_else(|| host_profiles.first().copied());
    HostSummary {
        id: id(host.meta.id),
        name: host.name.clone(),
        address: host.address.clone(),
        port: host.port,
        username: default.map(|profile| profile.username.clone()),
        login_count: host_profiles.len() as u32,
        route: default.map_or(RouteLabel::Direct, |profile| route_label(profile, routes)),
        favorite: host.favorite,
        tags: host.tags.clone(),
        groups: host.groups.iter().copied().map(id).collect(),
        restored: host.meta.restored,
    }
}

fn route_label(profile: &AccessProfile, routes: &[Route]) -> RouteLabel {
    let RouteChoice::Route(route_id) = profile.route else { return RouteLabel::Direct };
    let Some(route) = routes.iter().find(|route| route.meta.id == route_id) else {
        return RouteLabel::Jump { name: "Missing route".into() };
    };
    match route.kind {
        RouteKind::Jump { .. } => RouteLabel::Jump { name: route.name.clone() },
        RouteKind::Socks5 { .. } | RouteKind::HttpConnect { .. } => {
            RouteLabel::Proxy { name: route.name.clone() }
        }
        RouteKind::Command { .. } => RouteLabel::Command { name: route.name.clone() },
        RouteKind::Managed { .. } => RouteLabel::Bastion { name: route.name.clone() },
    }
}

fn recent_session_kind(kind: SessionKind) -> RecentSessionKind {
    match kind {
        SessionKind::Terminal => RecentSessionKind::Terminal,
        SessionKind::Files => RecentSessionKind::Files,
        SessionKind::Forward => RecentSessionKind::Forward,
    }
}

fn recent_session_outcome(outcome: SessionOutcome) -> RecentSessionOutcome {
    match outcome {
        SessionOutcome::Closed => RecentSessionOutcome::Closed,
        SessionOutcome::Failed => RecentSessionOutcome::Failed,
    }
}

fn credential_label(credential: &Credential) -> CredentialLabel {
    CredentialLabel {
        id: id(credential.meta.id),
        kind: match credential.kind {
            CredentialKind::Password => CredentialLabelKind::Password,
            CredentialKind::PrivateKey => CredentialLabelKind::Key,
            CredentialKind::Certificate => CredentialLabelKind::Certificate,
            CredentialKind::Agent => CredentialLabelKind::Agent,
            CredentialKind::SecurityKey => CredentialLabelKind::SecurityKey,
            CredentialKind::DeviceBoundKey => CredentialLabelKind::DeviceKey,
            CredentialKind::ExternalProvider => CredentialLabelKind::External,
        },
        name: credential.name.clone(),
        fingerprint: None,
        comment: None,
        public_key: credential.public_key.clone(),
    }
}

fn credential_input(
    sign_in: SignIn,
) -> Result<(CredentialKind, CredentialBinding, Option<Vec<u8>>), AddHostError> {
    match sign_in {
        SignIn::Password { password } => {
            if password.is_empty() {
                return Err(AddHostError::EmptyPassword);
            }
            Ok((CredentialKind::Password, CredentialBinding::Shared, Some(password.into_bytes())))
        }
        SignIn::KeyFile { path } => {
            let bytes = read_private_key_file(&path)?;
            validate_private_key(&bytes)?;
            Ok((CredentialKind::PrivateKey, CredentialBinding::Shared, Some(bytes)))
        }
        SignIn::KeyText { key } => {
            if key.len() as u64 > MAX_PRIVATE_KEY_BYTES {
                return Err(AddHostError::KeyTooLarge);
            }
            let bytes = key.into_bytes();
            validate_private_key(&bytes)?;
            Ok((CredentialKind::PrivateKey, CredentialBinding::Shared, Some(bytes)))
        }
        SignIn::GenerateKey => Err(AddHostError::Failed {
            reference: "key generation is not available in the inventory flow".into(),
        }),
        SignIn::Agent => Ok((CredentialKind::Agent, CredentialBinding::None, None)),
    }
}

fn read_private_key_file(path: &str) -> Result<Vec<u8>, AddHostError> {
    let file = std::fs::File::open(path).map_err(|_| AddHostError::KeyNotFound)?;
    let mut bytes = Vec::new();
    file.take(MAX_PRIVATE_KEY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AddHostError::KeyNotFound)?;
    if bytes.len() as u64 > MAX_PRIVATE_KEY_BYTES {
        return Err(AddHostError::KeyTooLarge);
    }
    Ok(bytes)
}

struct ParsedOpenSshConfig {
    entries: Vec<OpenSshImportEntry>,
    skipped_hosts: Vec<OpenSshSkippedHost>,
    unsupported: Vec<OpenSshUnsupportedDirective>,
}

type PreparedImportBatch =
    (Vec<NewObject>, Vec<(scoplen_model::ObjectType, Uuid)>, Vec<Uuid>, Vec<Uuid>);

struct OpenSshCandidate {
    line: u32,
    entry: OpenSshImportEntry,
    reason: Option<OpenSshSkipReason>,
}

fn parse_open_ssh_config(path: &Path) -> Result<ParsedOpenSshConfig, OpenSshImportError> {
    let file = std::fs::File::open(path).map_err(|_| OpenSshImportError::FileNotFound)?;
    let mut bytes = Vec::new();
    file.take(MAX_OPEN_SSH_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| OpenSshImportError::FileNotFound)?;
    if bytes.len() as u64 > MAX_OPEN_SSH_CONFIG_BYTES {
        return Err(OpenSshImportError::TooLarge);
    }
    let text = String::from_utf8(bytes).map_err(|_| OpenSshImportError::NotText)?;
    let default_user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".into());
    let mut current = Vec::new();
    let mut current_reason = None;
    let mut candidates = Vec::new();
    let mut global_reason = None;
    let mut seen_directives = HashSet::new();
    let mut unsupported = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        if raw.len() > MAX_OPEN_SSH_LINE_BYTES {
            return Err(OpenSshImportError::TooLarge);
        }
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        let Some(directive) = words.next() else { continue };
        let value = words.collect::<Vec<_>>().join(" ");
        let directive_lower = directive.to_ascii_lowercase();
        match directive_lower.as_str() {
            "host" => {
                flush_open_ssh_block(&mut current, &mut current_reason, &mut candidates);
                seen_directives.clear();
                let aliases = value.split_whitespace().collect::<Vec<_>>();
                if aliases.is_empty() || aliases.iter().any(|alias| !simple_ssh_value(alias)) {
                    global_reason.get_or_insert_with(|| "Host".to_owned());
                    unsupported.push(OpenSshUnsupportedDirective {
                        line: (index + 1) as u32,
                        directive: directive.to_owned(),
                    });
                }
                for alias in aliases {
                    if alias.chars().any(|character| matches!(character, '*' | '?' | '!')) {
                        global_reason.get_or_insert_with(|| "Host".to_owned());
                        unsupported.push(OpenSshUnsupportedDirective {
                            line: (index + 1) as u32,
                            directive: directive.to_owned(),
                        });
                        continue;
                    }
                    if !simple_ssh_value(alias) {
                        continue;
                    }
                    current.push(OpenSshCandidate {
                        line: (index + 1) as u32,
                        entry: OpenSshImportEntry {
                            alias: alias.to_owned(),
                            address: alias.to_owned(),
                            port: 22,
                            username: default_user.clone(),
                            identity_file: None,
                        },
                        reason: None,
                    });
                    if candidates.len() + current.len() > MAX_OPEN_SSH_HOSTS {
                        return Err(OpenSshImportError::TooLarge);
                    }
                }
            }
            "include" | "match" => {
                flush_open_ssh_block(&mut current, &mut current_reason, &mut candidates);
                global_reason.get_or_insert_with(|| directive.to_owned());
                unsupported.push(OpenSshUnsupportedDirective {
                    line: (index + 1) as u32,
                    directive: directive.to_owned(),
                });
            }
            "hostname" | "user" | "port" | "identityfile" => {
                if current.is_empty() {
                    global_reason.get_or_insert_with(|| directive.to_owned());
                    unsupported.push(OpenSshUnsupportedDirective {
                        line: (index + 1) as u32,
                        directive: directive.to_owned(),
                    });
                    continue;
                }
                let repeated = !seen_directives.insert(directive_lower.clone());
                let valid = simple_ssh_value(&value)
                    && match directive_lower.as_str() {
                        "port" => value.parse::<u16>().is_ok_and(|port| port != 0),
                        "identityfile" => {
                            value.starts_with("~/") || Path::new(&value).is_absolute()
                        }
                        _ => true,
                    };
                if repeated || !valid {
                    current_reason.get_or_insert(OpenSshSkipReason::InvalidValue {
                        directive: directive.to_owned(),
                    });
                    unsupported.push(OpenSshUnsupportedDirective {
                        line: (index + 1) as u32,
                        directive: directive.to_owned(),
                    });
                    continue;
                }
                for candidate in &mut current {
                    match directive_lower.as_str() {
                        "hostname" => candidate.entry.address = value.clone(),
                        "user" => candidate.entry.username = value.clone(),
                        "port" => candidate.entry.port = value.parse().unwrap_or(22),
                        "identityfile" => candidate.entry.identity_file = Some(value.clone()),
                        _ => unreachable!(),
                    }
                }
            }
            _ => {
                if current.is_empty() {
                    global_reason.get_or_insert_with(|| directive.to_owned());
                } else {
                    current_reason.get_or_insert(OpenSshSkipReason::UnsupportedDirective {
                        directive: directive.to_owned(),
                    });
                }
                unsupported.push(OpenSshUnsupportedDirective {
                    line: (index + 1) as u32,
                    directive: directive.to_owned(),
                });
            }
        }
    }
    flush_open_ssh_block(&mut current, &mut current_reason, &mut candidates);
    let mut counts = HashMap::<String, usize>::new();
    for candidate in &candidates {
        *counts.entry(candidate.entry.alias.to_ascii_lowercase()).or_default() += 1;
    }
    let mut entries = Vec::new();
    let mut skipped_hosts = Vec::new();
    for candidate in candidates {
        let reason = global_reason
            .as_ref()
            .map(|directive| OpenSshSkipReason::GlobalRules { directive: directive.clone() })
            .or_else(|| {
                (counts.get(&candidate.entry.alias.to_ascii_lowercase()).copied().unwrap_or(0) > 1)
                    .then_some(OpenSshSkipReason::DuplicateAlias)
            })
            .or(candidate.reason);
        if let Some(reason) = reason {
            skipped_hosts.push(OpenSshSkippedHost {
                alias: candidate.entry.alias,
                line: candidate.line,
                reason,
            });
        } else {
            entries.push(candidate.entry);
        }
    }
    Ok(ParsedOpenSshConfig { entries, skipped_hosts, unsupported })
}

fn flush_open_ssh_block(
    current: &mut Vec<OpenSshCandidate>,
    reason: &mut Option<OpenSshSkipReason>,
    candidates: &mut Vec<OpenSshCandidate>,
) {
    for mut candidate in current.drain(..) {
        candidate.reason = reason.clone();
        candidates.push(candidate);
    }
    *reason = None;
}

fn simple_ssh_value(value: &str) -> bool {
    !value.is_empty()
        && !value.chars().any(|character| {
            character.is_whitespace()
                || matches!(character, '\\' | '\'' | '"' | '#' | '%' | '*' | '?' | '!')
        })
}

fn expand_config_path(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME"))
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(path)
}

fn import_failure(error: impl std::error::Error) -> OpenSshImportError {
    OpenSshImportError::Failed { reference: diagnostic(&error) }
}

fn import_add_failure(error: AddHostError) -> OpenSshImportError {
    OpenSshImportError::Failed { reference: error.to_string() }
}

fn validate_private_key(bytes: &[u8]) -> Result<(), AddHostError> {
    let text = String::from_utf8_lossy(bytes);
    if text.trim().is_empty() {
        return Err(AddHostError::NotAPrivateKey);
    }
    if text.contains("PuTTY-User-Key-File") {
        return Err(AddHostError::PuttyKey);
    }
    if text.contains("BEGIN PUBLIC KEY") || text.trim_start().starts_with("ssh-") {
        return Err(AddHostError::PublicKey);
    }
    if text.contains("PRIVATE KEY") { Ok(()) } else { Err(AddHostError::NotAPrivateKey) }
}

fn text(value: String) -> scoplen_model::cbor::Value {
    scoplen_model::cbor::Value::Text(value)
}

fn uint(value: u64) -> scoplen_model::cbor::Value {
    scoplen_model::cbor::Value::UInt(value)
}

fn uuid_value(value: Uuid) -> scoplen_model::cbor::Value {
    scoplen_model::cbor::Value::Bytes(value.as_bytes().to_vec())
}

fn new_id() -> Result<Uuid, StoreError> {
    scoplen_model::new_uuid_v7().map_err(StoreError::from)
}

fn id(value: Uuid) -> String {
    value.to_string()
}

fn parse_id(value: &str) -> Result<Uuid, uuid::Error> {
    Uuid::parse_str(value)
}

fn failure(error: impl std::error::Error) -> Failure {
    failed(&diagnostic(&error))
}

fn add_failed(error: impl std::fmt::Display) -> AddHostError {
    AddHostError::Failed { reference: error.to_string() }
}

fn edit_failure(error: impl std::error::Error) -> EditHostError {
    EditHostError::Failed { reference: diagnostic(&error) }
}

fn edit_failed(reference: &str) -> EditHostError {
    EditHostError::Failed { reference: reference.into() }
}

fn group_failure(error: impl std::error::Error) -> GroupError {
    GroupError::Failed { reference: diagnostic(&error) }
}

fn group_failed(reference: &str) -> GroupError {
    GroupError::Failed { reference: reference.into() }
}

fn failed(reference: &str) -> Failure {
    Failure::Failed { reference: reference.into() }
}

fn diagnostic(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

impl From<RecordError> for Failure {
    fn from(error: RecordError) -> Self {
        failure(error)
    }
}

impl From<StoreError> for Failure {
    fn from(error: StoreError) -> Self {
        failure(error)
    }
}

impl From<RecordError> for AddHostError {
    fn from(error: RecordError) -> Self {
        add_failed(error)
    }
}

impl From<StoreError> for AddHostError {
    fn from(error: StoreError) -> Self {
        add_failed(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::{CredentialChange, Edit};
    use crate::store::Store;
    use crate::store::device::{SessionKind, SessionOutcome};
    use scoplen_crypto::LocalDatabaseKey;

    fn inventory() -> Inventory {
        let dir = tempfile::tempdir().unwrap();
        // Leak the temporary directory for the duration of this test facade;
        // the encrypted store owns the file and no production path leaks.
        let dir = Box::leak(Box::new(dir));
        let store =
            Store::open(&dir.path().join("local.db"), &LocalDatabaseKey::new([7; 32])).unwrap();
        Inventory::new(Arc::new(store))
    }

    #[test]
    fn add_host_is_visible_with_a_secret_free_details_shape() {
        let inventory = inventory();
        let details = inventory
            .add_host(NewHost {
                name: None,
                address: "db.example".into(),
                port: None,
                username: "deploy".into(),
                sign_in: SignIn::Password { password: "secret".into() },
            })
            .unwrap();
        assert_eq!(details.summary.name, "db.example");
        assert_eq!(details.summary.username.as_deref(), Some("deploy"));
        assert_eq!(details.logins.len(), 1);
        assert_eq!(
            details.logins[0].credential.as_ref().unwrap().kind,
            CredentialLabelKind::Password
        );
        assert!(inventory.hosts(HostSource::All, String::new()).unwrap().len() == 1);
        let credential_id = parse_id(&details.logins[0].credential.as_ref().unwrap().id).unwrap();
        let secret = inventory.store.device_credential(credential_id).unwrap();
        assert!(secret.is_none(), "shared credentials stay in the encrypted object");
        assert_eq!(inventory.store.list(scoplen_model::ObjectType::HOST).unwrap().len(), 1);
        assert_eq!(
            inventory.store.list(scoplen_model::ObjectType::ACCESS_PROFILE).unwrap().len(),
            1
        );
        assert_eq!(inventory.store.list(scoplen_model::ObjectType::CREDENTIAL).unwrap().len(), 1);
    }

    #[test]
    fn adding_a_bad_key_does_not_leave_a_host() {
        let inventory = inventory();
        let error = inventory
            .add_host(NewHost {
                name: None,
                address: "db.example".into(),
                port: None,
                username: "deploy".into(),
                sign_in: SignIn::KeyText { key: "ssh-rsa AAAA".into() },
            })
            .unwrap_err();
        assert_eq!(error, AddHostError::PublicKey);
        assert!(inventory.hosts(HostSource::All, String::new()).unwrap().is_empty());
    }

    #[test]
    fn delete_and_undo_restore_the_host_and_its_unnamed_credential() {
        let inventory = inventory();
        let details = inventory
            .add_host(NewHost {
                name: Some("api".into()),
                address: "api.example".into(),
                port: Some(2222),
                username: "root".into(),
                sign_in: SignIn::Password { password: "secret".into() },
            })
            .unwrap();
        let token = inventory.delete_host(details.summary.id.clone()).unwrap();
        assert!(inventory.host(details.summary.id.clone()).unwrap().is_none());
        inventory.undo_delete(token.token).unwrap();
        let restored = inventory.host(details.summary.id).unwrap().unwrap();
        assert!(restored.summary.restored);
        assert_eq!(restored.logins.len(), 1);
    }

    #[test]
    fn named_credentials_survive_host_delete() {
        let inventory = inventory();
        let details = inventory
            .add_host(NewHost {
                name: Some("api".into()),
                address: "api.example".into(),
                port: None,
                username: "root".into(),
                sign_in: SignIn::Password { password: "secret".into() },
            })
            .unwrap();
        let credential_id = parse_id(&details.logins[0].credential.as_ref().unwrap().id).unwrap();
        Repository::<Credential>::new(&inventory.store)
            .update(
                credential_id,
                CredentialChange { name: Edit::Set("shared".into()), ..Default::default() },
            )
            .unwrap();

        let token = inventory.delete_host(details.summary.id.clone()).unwrap();
        let credential = inventory.store.get(credential_id).unwrap().unwrap();
        assert!(!credential.is_tombstoned());
        assert!(inventory.host(details.summary.id).unwrap().is_none());
        inventory.undo_delete(token.token).unwrap();
    }

    #[test]
    fn undo_tokens_are_single_use_and_reject_invalid_values() {
        let inventory = inventory();
        assert!(matches!(inventory.undo_delete("not-a-token".into()), Err(Failure::Failed { .. })));
        let details = inventory
            .add_host(NewHost {
                name: None,
                address: "api.example".into(),
                port: None,
                username: "root".into(),
                sign_in: SignIn::Agent,
            })
            .unwrap();
        let token = inventory.delete_host(details.summary.id).unwrap().token;
        inventory.undo_delete(token.clone()).unwrap();
        assert!(matches!(inventory.undo_delete(token), Err(Failure::Failed { .. })));
    }

    #[test]
    fn favorite_and_search_feed_the_host_list() {
        let inventory = inventory();
        let details = inventory
            .add_host(NewHost {
                name: Some("api".into()),
                address: "api.example".into(),
                port: None,
                username: "root".into(),
                sign_in: SignIn::Agent,
            })
            .unwrap();
        inventory.set_favorite(details.summary.id.clone(), true).unwrap();
        // The first-launch login and credential remain implicit until the
        // user inspects the host or reuses the credential.
        assert!(!inventory.areas().unwrap().keys);
        inventory.host(details.summary.id).unwrap();
        let areas = inventory.areas().unwrap();
        assert!(areas.favorites && !areas.keys);
        let hosts = inventory.hosts(HostSource::Favorites, "ROOT".into()).unwrap();
        assert_eq!(hosts.len(), 1);
    }

    #[test]
    fn recent_sessions_are_redacted_ordered_and_skip_orphans() {
        let inventory = inventory();
        let details = inventory
            .add_host(NewHost {
                name: Some("api".into()),
                address: "api.example".into(),
                port: Some(2222),
                username: "ops".into(),
                sign_in: SignIn::Agent,
            })
            .unwrap();
        let profile = parse_id(&details.logins[0].id).unwrap();
        let first = inventory.store.record_session_start(profile, SessionKind::Terminal).unwrap();
        let second = inventory.store.record_session_start(profile, SessionKind::Files).unwrap();
        inventory.store.record_session_end(first, SessionOutcome::Failed).unwrap();

        let sessions = inventory.recent_sessions().unwrap();
        assert_eq!(
            sessions.iter().map(|session| session.id.clone()).collect::<Vec<_>>(),
            [id(second), id(first)]
        );
        assert_eq!(sessions[0].host_name, "api");
        assert_eq!(sessions[0].address, "api.example");
        assert_eq!(sessions[0].username, "ops");
        assert_eq!(sessions[0].kind, RecentSessionKind::Files);
        assert_eq!(sessions[0].outcome, None);
        assert_eq!(sessions[1].kind, RecentSessionKind::Terminal);
        assert_eq!(sessions[1].outcome, Some(RecentSessionOutcome::Failed));
        assert!(sessions[1].ended_at.is_some());

        let orphan_profile = scoplen_model::new_uuid_v7().unwrap();
        inventory.store.record_session_start(orphan_profile, SessionKind::Forward).unwrap();
        assert_eq!(inventory.recent_sessions().unwrap().len(), 2);

        inventory.delete_host(details.summary.id).unwrap();
        assert!(inventory.recent_sessions().unwrap().is_empty());
        assert!(
            !inventory.areas().unwrap().recent,
            "orphan history does not expose an empty Recent area"
        );
    }

    #[test]
    fn edits_host_metadata_tags_and_groups_as_one_validated_change() {
        let inventory = inventory();
        let group =
            inventory.create_group(GroupInput { name: "Production".into(), parent: None }).unwrap();
        let details = inventory
            .add_host(NewHost {
                name: Some("api".into()),
                address: "api.example".into(),
                port: None,
                username: "ops".into(),
                sign_in: SignIn::Agent,
            })
            .unwrap();
        let edited = inventory
            .update_host(
                details.summary.id.clone(),
                EditHost {
                    name: "API production".into(),
                    address: "10.0.0.7".into(),
                    port: 2222,
                    notes: Some("owned by platform".into()),
                    tags: BTreeMap::from([("env".into(), "prod".into())]),
                    groups: vec![group.id.clone()],
                },
            )
            .unwrap();
        assert_eq!(edited.summary.name, "API production");
        assert_eq!(edited.summary.address, "10.0.0.7");
        assert_eq!(edited.summary.port, 2222);
        assert_eq!(edited.summary.tags["env"], "prod");
        assert_eq!(edited.summary.groups, vec![group.id]);
        assert_eq!(edited.notes.as_deref(), Some("owned by platform"));
    }

    #[test]
    fn editing_rejects_invalid_values_without_changing_the_host() {
        let inventory = inventory();
        let details = inventory
            .add_host(NewHost {
                name: Some("api".into()),
                address: "api.example".into(),
                port: None,
                username: "ops".into(),
                sign_in: SignIn::Agent,
            })
            .unwrap();
        let result = inventory.update_host(
            details.summary.id.clone(),
            EditHost {
                name: "".into(),
                address: "bad address".into(),
                port: 0,
                notes: None,
                tags: BTreeMap::new(),
                groups: Vec::new(),
            },
        );
        assert_eq!(result, Err(EditHostError::EmptyName));
        let unchanged = inventory.host(details.summary.id).unwrap().unwrap();
        assert_eq!(unchanged.summary.name, "api");
        assert_eq!(unchanged.summary.address, "api.example");
    }

    #[test]
    fn group_parent_validation_and_editing_keep_invalid_changes_out() {
        let inventory = inventory();
        let group =
            inventory.create_group(GroupInput { name: "Production".into(), parent: None }).unwrap();
        assert_eq!(
            inventory.create_group(GroupInput { name: "".into(), parent: None }),
            Err(GroupError::EmptyName)
        );
        assert_eq!(
            inventory.update_group(
                group.id.clone(),
                GroupInput { name: "Production".into(), parent: Some(group.id.clone()) },
            ),
            Err(GroupError::SelfParent)
        );
        let updated = inventory
            .update_group(group.id, GroupInput { name: "Prod".into(), parent: None })
            .unwrap();
        assert_eq!(updated.name, "Prod");
    }

    #[test]
    fn implicit_key_and_login_promote_when_host_is_inspected() {
        let inventory = inventory();
        let key =
            "-----BEGIN OPENSSH PRIVATE KEY-----\nplaceholder\n-----END OPENSSH PRIVATE KEY-----";
        let details = inventory
            .add_host(NewHost {
                name: Some("api".into()),
                address: "api.example".into(),
                port: None,
                username: "ops".into(),
                sign_in: SignIn::KeyText { key: key.into() },
            })
            .unwrap();
        let credential = parse_id(&details.logins[0].credential.as_ref().unwrap().id).unwrap();
        let profile = parse_id(&details.logins[0].id).unwrap();
        assert!(inventory.store.is_implicit(credential).unwrap());
        assert!(inventory.store.is_implicit(profile).unwrap());
        assert!(!inventory.areas().unwrap().keys);

        let inspected = inventory.host(details.summary.id).unwrap().unwrap();
        assert_eq!(inspected.summary.name, "api");
        assert!(!inventory.store.is_implicit(credential).unwrap());
        assert!(!inventory.store.is_implicit(profile).unwrap());
        assert!(inventory.areas().unwrap().keys);
    }

    #[test]
    fn inspecting_a_password_host_does_not_promote_a_password_or_show_keys() {
        let inventory = inventory();
        let details = inventory
            .add_host(NewHost {
                name: Some("password-host".into()),
                address: "password.example".into(),
                port: None,
                username: "ops".into(),
                sign_in: SignIn::Password { password: "secret".into() },
            })
            .unwrap();
        let credential = parse_id(&details.logins[0].credential.as_ref().unwrap().id).unwrap();
        assert!(inventory.store.is_implicit(credential).unwrap());
        assert!(!inventory.areas().unwrap().keys);

        inventory.host(details.summary.id).unwrap().unwrap();
        assert!(inventory.store.is_implicit(credential).unwrap());
        assert!(!inventory.areas().unwrap().keys);
    }

    #[test]
    fn oversized_key_files_are_rejected_before_reading_into_the_store() {
        let inventory = inventory();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oversized-key");
        let mut bytes = vec![b'x'; MAX_PRIVATE_KEY_BYTES as usize + 1];
        bytes[..35].copy_from_slice(b"-----BEGIN OPENSSH PRIVATE KEY-----");
        std::fs::write(&path, bytes).unwrap();
        let result = inventory.add_host(NewHost {
            name: Some("too-large".into()),
            address: "large.example".into(),
            port: None,
            username: "ops".into(),
            sign_in: SignIn::KeyFile { path: path.to_string_lossy().into_owned() },
        });
        assert_eq!(result, Err(AddHostError::KeyTooLarge));
        assert!(inventory.hosts(HostSource::All, String::new()).unwrap().is_empty());
    }

    #[test]
    fn reusing_a_key_on_a_second_host_promotes_the_credential() {
        let inventory = inventory();
        let key =
            "-----BEGIN OPENSSH PRIVATE KEY-----\nplaceholder\n-----END OPENSSH PRIVATE KEY-----";
        let first = inventory
            .add_host(NewHost {
                name: Some("one".into()),
                address: "one.example".into(),
                port: None,
                username: "ops".into(),
                sign_in: SignIn::KeyText { key: key.into() },
            })
            .unwrap();
        let first_credential = first.logins[0].credential.as_ref().unwrap().id.clone();
        let second = inventory
            .add_host(NewHost {
                name: Some("two".into()),
                address: "two.example".into(),
                port: None,
                username: "ops".into(),
                sign_in: SignIn::KeyText { key: key.into() },
            })
            .unwrap();
        assert_eq!(second.logins[0].credential.as_ref().unwrap().id, first_credential);
        assert!(inventory.areas().unwrap().keys);
        assert_eq!(inventory.store.list(scoplen_model::ObjectType::CREDENTIAL).unwrap().len(), 1);
    }

    #[test]
    fn open_ssh_preview_reports_unsupported_directives_without_writing() {
        let inventory = inventory();
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        std::fs::write(
            &config,
            "Host prod\n  HostName prod.example\n  User deploy\n  Port 2222\n  ProxyJump bastion\nHost safe\n  HostName safe.example\n  User deploy\n",
        )
        .unwrap();
        let preview = inventory.preview_open_ssh_config(config.to_string_lossy().into()).unwrap();
        assert_eq!(preview.entries.len(), 1);
        assert_eq!(preview.entries[0].address, "safe.example");
        assert_eq!(preview.entries[0].port, 22);
        assert_eq!(preview.entries[0].username, "deploy");
        assert_eq!(preview.skipped_hosts.len(), 1);
        assert_eq!(preview.skipped_hosts[0].alias, "prod");
        assert_eq!(
            preview.skipped_hosts[0].reason,
            OpenSshSkipReason::UnsupportedDirective { directive: "ProxyJump".into() }
        );
        assert_eq!(preview.unsupported[0].directive, "ProxyJump");
        assert!(inventory.hosts(HostSource::All, String::new()).unwrap().is_empty());
        let imported = inventory.import_open_ssh_config(preview.clone()).unwrap();
        assert_eq!(imported.hosts.len(), 1);
        assert_eq!(imported.hosts[0].summary.name, "safe");
        assert_eq!(imported.skipped_hosts, preview.skipped_hosts);
    }

    #[test]
    fn open_ssh_global_rules_and_duplicate_aliases_fail_closed() {
        let inventory = inventory();
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        std::fs::write(
            &config,
            "Host first\n  HostName first.example\nMatch host first\n  HostName changed.example\nHost second\n  HostName second.example\n",
        )
        .unwrap();
        let preview = inventory.preview_open_ssh_config(config.to_string_lossy().into()).unwrap();
        assert!(preview.entries.is_empty());
        assert_eq!(preview.skipped_hosts.len(), 2);
        assert!(preview.skipped_hosts.iter().all(|host| matches!(
            host.reason,
            OpenSshSkipReason::GlobalRules { ref directive } if directive == "Match"
        )));
        assert_eq!(
            inventory.import_open_ssh_config(preview.clone()),
            Err(OpenSshImportError::NoHosts)
        );
        assert!(inventory.hosts(HostSource::All, String::new()).unwrap().is_empty());

        std::fs::write(&config, "Host one\n  HostName one.example\nHost *\n  ProxyJump gateway\n")
            .unwrap();
        let preview = inventory.preview_open_ssh_config(config.to_string_lossy().into()).unwrap();
        assert!(preview.entries.is_empty());
        assert_eq!(preview.skipped_hosts[0].alias, "one");
        assert!(matches!(
            preview.skipped_hosts[0].reason,
            OpenSshSkipReason::GlobalRules { ref directive } if directive == "Host"
        ));

        std::fs::write(
            &config,
            "Host duplicate\n  HostName first.example\nHost duplicate\n  HostName second.example\nHost unique\n  HostName unique.example\n",
        )
        .unwrap();
        let preview = inventory.preview_open_ssh_config(config.to_string_lossy().into()).unwrap();
        assert_eq!(preview.entries.len(), 1);
        assert_eq!(preview.entries[0].alias, "unique");
        assert_eq!(preview.skipped_hosts.len(), 2);
        assert!(
            preview
                .skipped_hosts
                .iter()
                .all(|host| host.reason == OpenSshSkipReason::DuplicateAlias)
        );
    }

    #[test]
    fn open_ssh_config_and_host_counts_are_bounded() {
        let inventory = inventory();
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        std::fs::write(&config, "#".repeat(MAX_OPEN_SSH_CONFIG_BYTES as usize + 1)).unwrap();
        assert_eq!(
            inventory.preview_open_ssh_config(config.to_string_lossy().into()),
            Err(OpenSshImportError::TooLarge)
        );
        let hosts =
            (0..=MAX_OPEN_SSH_HOSTS).map(|index| format!("Host h{index}\n")).collect::<String>();
        std::fs::write(&config, hosts).unwrap();
        assert_eq!(
            inventory.preview_open_ssh_config(config.to_string_lossy().into()),
            Err(OpenSshImportError::TooLarge)
        );
    }

    #[test]
    fn open_ssh_import_rejects_a_source_changed_after_preview() {
        let inventory = inventory();
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        std::fs::write(&config, "Host one\n  HostName one.example\n").unwrap();
        let preview = inventory.preview_open_ssh_config(config.to_string_lossy().into()).unwrap();
        std::fs::write(&config, "Host two\n  HostName two.example\n").unwrap();
        assert_eq!(
            inventory.import_open_ssh_config(preview),
            Err(OpenSshImportError::SourceChanged)
        );
        assert!(inventory.hosts(HostSource::All, String::new()).unwrap().is_empty());
    }

    #[test]
    fn open_ssh_import_is_empty_inventory_only_and_prevalidates_identity_files() {
        let inventory = inventory();
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let identity = dir.path().join("id_ed25519");
        std::fs::write(
            &identity,
            "-----BEGIN OPENSSH PRIVATE KEY-----\nkey\n-----END OPENSSH PRIVATE KEY-----",
        )
        .unwrap();
        std::fs::write(
            &config,
            format!(
                "Host prod\n  HostName prod.example\n  User deploy\n  IdentityFile {}\nHost missing\n  HostName missing.example\n  IdentityFile {}/not-there\n",
                identity.to_string_lossy().replace('\\', "/"),
                dir.path().to_string_lossy().replace('\\', "/")
            ),
        )
        .unwrap();
        let preview = inventory.preview_open_ssh_config(config.to_string_lossy().into()).unwrap();
        let error = inventory.import_open_ssh_config(preview).unwrap_err();
        assert!(matches!(error, OpenSshImportError::IdentityNotFound { .. }));
        assert!(inventory.hosts(HostSource::All, String::new()).unwrap().is_empty());

        let invalid_identity = dir.path().join("not_private");
        std::fs::write(&invalid_identity, "ssh-ed25519 public-key").unwrap();
        std::fs::write(
            &config,
            format!(
                "Host prod\n  HostName prod.example\n  IdentityFile {}\nHost bad\n  HostName bad.example\n  IdentityFile {}\n",
                identity.to_string_lossy().replace('\\', "/"),
                invalid_identity.to_string_lossy().replace('\\', "/")
            ),
        )
        .unwrap();
        let preview = inventory.preview_open_ssh_config(config.to_string_lossy().into()).unwrap();
        let error = inventory.import_open_ssh_config(preview).unwrap_err();
        assert!(matches!(error, OpenSshImportError::Failed { .. }));
        assert!(inventory.hosts(HostSource::All, String::new()).unwrap().is_empty());

        std::fs::write(
            &config,
            format!(
                "Host prod\n  HostName prod.example\n  User deploy\n  IdentityFile {}\n",
                identity.to_string_lossy().replace('\\', "/")
            ),
        )
        .unwrap();
        let preview = inventory.preview_open_ssh_config(config.to_string_lossy().into()).unwrap();
        let imported = inventory.import_open_ssh_config(preview.clone()).unwrap();
        assert_eq!(imported.hosts.len(), 1);
        assert!(inventory.hosts(HostSource::All, String::new()).unwrap().len() == 1);
        let error = inventory.import_open_ssh_config(preview).unwrap_err();
        assert_eq!(error, OpenSshImportError::InventoryNotEmpty);
    }
}
