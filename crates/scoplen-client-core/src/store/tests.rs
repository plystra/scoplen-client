// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, Mutex};

use scoplen_crypto::LocalDatabaseKey;
use scoplen_model::{FieldPath, MapKey, ObjectType, cbor::Value};

use super::*;

fn key(byte: u8) -> LocalDatabaseKey {
    LocalDatabaseKey::new([byte; 32])
}

fn host(name: &str, address: &str) -> LocalWrite {
    LocalWrite {
        id: None,
        object_type: ObjectType::HOST,
        fields: vec![
            (FieldPath::Field(1), Value::Text(name.into())),
            (FieldPath::Field(2), Value::Text(address.into())),
        ],
    }
}

fn text(object: &Object, field: u64) -> &str {
    object.required_text(field).unwrap()
}

#[test]
fn a_new_store_is_migrated_and_given_a_device_identity() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("local.db");
    let store = Store::open(&path, &key(1)).unwrap();
    assert_eq!(store.schema_version().unwrap(), MIGRATIONS.len() as u32);
    let device = store.device_id();
    assert_eq!(device.get_version_num(), 7);
    drop(store);

    let reopened = Store::open(&path, &key(1)).unwrap();
    assert_eq!(reopened.device_id(), device, "the device identity is stable");
}

#[test]
fn the_file_is_encrypted_and_only_the_right_key_opens_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("local.db");
    let store = Store::open(&path, &key(1)).unwrap();
    store.write(host("prod-db-01", "db.internal.example")).unwrap();
    drop(store);

    let bytes = std::fs::read(&path).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3"), "the header is encrypted");
    assert!(!bytes.windows(10).any(|w| w == b"prod-db-01"), "contents are encrypted");

    assert!(matches!(Store::open(&path, &key(2)), Err(StoreError::WrongKey)));
    assert_eq!(Store::open(&path, &key(1)).unwrap().list(ObjectType::HOST).unwrap().len(), 1);
}

#[test]
fn a_store_from_a_newer_version_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("local.db");
    let store = Store::open(&path, &key(1)).unwrap();
    store.conn().pragma_update(None, "user_version", MIGRATIONS.len() as u32 + 1).unwrap();
    drop(store);
    match Store::open(&path, &key(1)) {
        Err(StoreError::NewerSchema { found, supported }) => {
            assert_eq!(found, supported + 1);
        }
        other => panic!("expected NewerSchema, got {other:?}"),
    }
}

#[test]
fn writes_merge_field_by_field_with_increasing_clocks() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("local.db"), &key(1)).unwrap();
    let created = store.write(host("api", "10.0.0.5")).unwrap();
    let first_clock = created.field(1).unwrap().clock;
    assert_eq!(created.field(1).unwrap().origin, store.device_id());

    let renamed = store
        .write(LocalWrite {
            id: Some(created.id),
            object_type: ObjectType::HOST,
            fields: vec![(FieldPath::Field(1), Value::Text("api-1".into()))],
        })
        .unwrap();
    assert_eq!(text(&renamed, 1), "api-1");
    assert_eq!(text(&renamed, 2), "10.0.0.5", "untouched fields are kept");
    assert!(renamed.field(1).unwrap().clock > first_clock);
    assert_eq!(store.get(created.id).unwrap().unwrap(), renamed);
}

#[test]
fn map_entries_are_added_and_removed_individually() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("local.db"), &key(1)).unwrap();
    let tag = |k: &str| FieldPath::map_entry(4, MapKey::Text(k.into())).unwrap();
    let mut write = host("web", "web.example");
    write.fields.push((tag("env"), Value::Text("prod".into())));
    write.fields.push((tag("team"), Value::Text("edge".into())));
    let host = store.write(write).unwrap();

    let updated = store
        .write(LocalWrite {
            id: Some(host.id),
            object_type: ObjectType::HOST,
            fields: vec![(tag("team"), Value::Null)],
        })
        .unwrap();
    assert_eq!(updated.fields.get(&tag("env")).unwrap().value, Value::Text("prod".into()));
    assert_eq!(updated.fields.get(&tag("team")).unwrap().value, Value::Null);
}

#[test]
fn invalid_objects_are_rejected_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("local.db"), &key(1)).unwrap();
    let missing_address = LocalWrite {
        id: None,
        object_type: ObjectType::HOST,
        fields: vec![(FieldPath::Field(1), Value::Text("no-address".into()))],
    };
    assert!(matches!(store.write(missing_address), Err(StoreError::Model(_))));
    let too_long = host(&"x".repeat(scoplen_model::MAX_TEXT_BYTES + 1), "a.example");
    assert!(store.write(too_long).is_err());
    assert!(store.list(ObjectType::HOST).unwrap().is_empty());
}

#[test]
fn changing_a_missing_object_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("local.db"), &key(1)).unwrap();
    let id = scoplen_model::new_uuid_v7().unwrap();
    let write = LocalWrite { id: Some(id), object_type: ObjectType::HOST, fields: vec![] };
    assert!(matches!(store.write(write), Err(StoreError::NotFound(missing)) if missing == id));
    assert!(matches!(store.delete(id), Err(StoreError::NotFound(_))));
}

#[test]
fn deleted_objects_leave_lists_but_remain_restorable() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("local.db"), &key(1)).unwrap();
    let host = store.write(host("old", "old.example")).unwrap();
    store.delete(host.id).unwrap();
    assert!(store.list(ObjectType::HOST).unwrap().is_empty());
    assert!(!store.live_ids().unwrap().contains(&host.id));
    let kept = store.get(host.id).unwrap().unwrap();
    assert!(kept.is_tombstoned());

    // A later field write resurrects it (`04-object-model.md` §5 rule 3).
    let restored = store
        .write(LocalWrite {
            id: Some(host.id),
            object_type: ObjectType::HOST,
            fields: vec![(FieldPath::Field(1), Value::Text("old".into()))],
        })
        .unwrap();
    assert!(restored.is_resurrected());
    assert_eq!(store.list(ObjectType::HOST).unwrap().len(), 1);
}

#[test]
fn subscribers_hear_committed_changes_until_unsubscribed() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(&dir.path().join("local.db"), &key(1)).unwrap());
    let heard = Arc::new(Mutex::new(Vec::new()));
    let sink = heard.clone();
    let subscription = store.subscribe(move |change| sink.lock().unwrap().push(change.clone()));

    let host = store.write(host("a", "a.example")).unwrap();
    store.delete(host.id).unwrap();
    let _ = store.write(LocalWrite { id: None, object_type: ObjectType::HOST, fields: vec![] });
    assert_eq!(
        *heard.lock().unwrap(),
        vec![
            Change { object_type: ObjectType::HOST, ids: vec![host.id] },
            Change { object_type: ObjectType::HOST, ids: vec![host.id] },
        ],
        "a failed write is not announced"
    );

    drop(subscription);
    store.write(self::host("b", "b.example")).unwrap();
    assert_eq!(heard.lock().unwrap().len(), 2);
}

#[test]
fn the_vault_limit_stops_new_objects() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("local.db"), &key(1)).unwrap();
    let existing = store.write(host("a", "a.example")).unwrap();
    // Fill the collection directly; writing 100,000 objects one by one is slow.
    {
        let conn = store.conn();
        let mut insert = conn
            .prepare("INSERT INTO objects (id, vault, type, envelope, deleted, updated_at) VALUES (?1, NULL, 1, x'00', 0, 0)")
            .unwrap();
        for i in 1..MAX_OBJECTS_PER_VAULT {
            let mut id = [0u8; 16];
            id[..8].copy_from_slice(&i.to_be_bytes());
            insert.execute(params![id.as_slice()]).unwrap();
        }
    }
    assert!(matches!(store.write(host("b", "b.example")), Err(StoreError::VaultFull)));
    // Changing an existing object is still allowed.
    let rename = LocalWrite {
        id: Some(existing.id),
        object_type: ObjectType::HOST,
        fields: vec![(FieldPath::Field(1), Value::Text("a2".into()))],
    };
    assert!(store.write(rename).is_ok());
}
