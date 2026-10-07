// SPDX-License-Identifier: Apache-2.0

//! The local inventory facade used by the desktop commands.
//!
//! This module is the boundary between the typed repositories and the
//! frontend. It returns only display data, never credential secrets, and uses
//! store transactions for operations that create or remove related objects.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::repository::{
    AccessProfile, Credential, CredentialBinding, CredentialKind, Edit, Host, HostChange,
    HostGroup, HostGroupChange, RecordError, Repository, Route, RouteChoice, RouteKind,
};
use crate::store::{NewObject, Store, StoreError};

const UNDO_LIFETIME: Duration = Duration::from_secs(5 * 60);

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
        let routes = Repository::<Route>::new(&self.store).list().map_err(failure)?;
        let profiles = Repository::<AccessProfile>::new(&self.store).list().map_err(failure)?;
        let recent = self.store.recent_sessions(1).map_err(failure)?.into_iter().next().is_some();
        Ok(Areas {
            favorites: hosts.iter().any(|host| host.favorite),
            recent,
            groups: !groups.is_empty(),
            keys: !credentials.is_empty(),
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

    /// Gets one host and all of its logins.
    pub fn host(&self, host_id: String) -> Result<Option<HostDetails>, Failure> {
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
        let credential_id = new_id().map_err(add_failed)?;
        let profile_id = new_id().map_err(add_failed)?;
        let host_fields = vec![
            (scoplen_model::FieldPath::Field(1), text(name)),
            (scoplen_model::FieldPath::Field(2), text(address)),
            (scoplen_model::FieldPath::Field(3), scoplen_model::cbor::Value::UInt(port as u64)),
            (scoplen_model::FieldPath::Field(8), uuid_value(profile_id)),
        ];
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
        let profile_fields = vec![
            (scoplen_model::FieldPath::Field(1), uuid_value(host_id)),
            (scoplen_model::FieldPath::Field(3), text(username)),
            (scoplen_model::FieldPath::Field(4), uuid_value(credential_id)),
            (scoplen_model::FieldPath::Field(5), uint(0)),
        ];
        self.store
            .create_batch(vec![
                NewObject {
                    id: host_id,
                    object_type: scoplen_model::ObjectType::HOST,
                    fields: host_fields,
                },
                NewObject {
                    id: credential_id,
                    object_type: scoplen_model::ObjectType::CREDENTIAL,
                    fields: credential_fields,
                },
                NewObject {
                    id: profile_id,
                    object_type: scoplen_model::ObjectType::ACCESS_PROFILE,
                    fields: profile_fields,
                },
            ])
            .map_err(add_failed)?;
        self.host(id(host_id))
            .map_err(|error| AddHostError::Failed { reference: error.to_string() })?
            .ok_or_else(|| add_failed("the host disappeared after creation"))
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
            let bytes = std::fs::read(path).map_err(|_| AddHostError::KeyNotFound)?;
            validate_private_key(&bytes)?;
            Ok((CredentialKind::PrivateKey, CredentialBinding::Shared, Some(bytes)))
        }
        SignIn::KeyText { key } => {
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
        let areas = inventory.areas().unwrap();
        assert!(areas.favorites && areas.keys);
        let hosts = inventory.hosts(HostSource::Favorites, "ROOT".into()).unwrap();
        assert_eq!(hosts.len(), 1);
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
}
