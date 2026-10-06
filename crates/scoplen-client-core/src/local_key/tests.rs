// SPDX-License-Identifier: Apache-2.0

use scoplen_client_platform::keystore::Keystore;

use super::*;
use crate::testing::{FAST, MemoryKeystore};

fn with_keystore(dir: &Path, keystore: &MemoryKeystore) -> LocalKeys {
    LocalKeys::new(dir, Some(Box::new(keystore.clone()))).with_params(FAST)
}

fn without_keystore(dir: &Path) -> LocalKeys {
    LocalKeys::new(dir, None).with_params(FAST)
}

fn ready(unlock: Unlock) -> (LocalDatabaseKey, Protection) {
    match unlock {
        Unlock::Ready(key, protection) => (key, protection),
        other => panic!("expected a ready key, got {other:?}"),
    }
}

#[test]
fn first_launch_with_a_keystore_generates_and_keeps_a_key() {
    let dir = tempfile::tempdir().unwrap();
    let keystore = MemoryKeystore::default();
    let (key, protection) = ready(with_keystore(dir.path(), &keystore).begin().unwrap());
    assert_eq!(protection, Protection::Keystore);
    let (again, _) = ready(with_keystore(dir.path(), &keystore).begin().unwrap());
    assert_eq!(key, again, "later launches read the same key");
}

#[test]
fn a_passphrase_on_top_of_the_keystore_is_required_to_unlock() {
    let dir = tempfile::tempdir().unwrap();
    let keystore = MemoryKeystore::default();
    let keys = with_keystore(dir.path(), &keystore);
    let (key, _) = ready(keys.begin().unwrap());
    assert_eq!(
        keys.set_passphrase(&key, "correct horse").unwrap(),
        Protection::KeystoreAndPassphrase
    );

    let stored = keystore.0.lock().unwrap().clone().unwrap();
    assert!(!stored.windows(32).any(|w| w == key.as_ref()), "the raw key is no longer stored");

    assert!(matches!(
        keys.begin().unwrap(),
        Unlock::NeedsPassphrase(Protection::KeystoreAndPassphrase)
    ));
    assert!(matches!(keys.unlock("wrong"), Err(LocalKeyError::WrongPassphrase)));
    let (unlocked, protection) = keys.unlock("correct horse").unwrap();
    assert_eq!(unlocked, key, "the key is unchanged, so the store needs no re-encryption");
    assert_eq!(protection, Protection::KeystoreAndPassphrase);

    assert_eq!(keys.remove_passphrase(&key).unwrap(), Protection::Keystore);
    assert_eq!(ready(keys.begin().unwrap()).0, key);
}

#[test]
fn without_a_keystore_a_passphrase_is_chosen_first_and_cannot_be_removed() {
    let dir = tempfile::tempdir().unwrap();
    let keys = without_keystore(dir.path());
    assert!(matches!(keys.begin().unwrap(), Unlock::NeedsNewPassphrase));
    assert!(matches!(keys.create_with_passphrase(""), Err(LocalKeyError::EmptyPassphrase)));
    let key = keys.create_with_passphrase("only secret").unwrap();

    assert!(matches!(keys.begin().unwrap(), Unlock::NeedsPassphrase(Protection::PassphraseOnly)));
    assert!(matches!(keys.unlock("nope"), Err(LocalKeyError::WrongPassphrase)));
    assert_eq!(keys.unlock("only secret").unwrap(), (key.clone(), Protection::PassphraseOnly));

    keys.set_passphrase(&key, "changed").unwrap();
    assert!(matches!(keys.unlock("only secret"), Err(LocalKeyError::WrongPassphrase)));
    assert_eq!(keys.unlock("changed").unwrap().0, key);
    assert!(matches!(keys.remove_passphrase(&key), Err(LocalKeyError::PassphraseRequired)));
}

#[test]
fn a_lost_key_is_reported_and_the_store_can_be_set_aside() {
    let dir = tempfile::tempdir().unwrap();
    let keystore = MemoryKeystore::default();
    let keys = with_keystore(dir.path(), &keystore);
    let (key, _) = ready(keys.begin().unwrap());
    crate::store::Store::open(&keys.store_path(), &key).unwrap();
    keystore.delete().unwrap();

    assert!(matches!(keys.begin(), Err(LocalKeyError::KeyMissing("test keystore"))));
    let aside = keys.set_aside().unwrap().expect("the old store is kept");
    assert!(aside.exists());
    assert!(!keys.store_path().exists());
    let (fresh, _) = ready(keys.begin().unwrap());
    assert_ne!(fresh, key);
}

#[test]
fn malformed_stored_keys_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let keystore = MemoryKeystore::default();
    keystore.store(&[SLOT_KEY, 1, 2, 3]).unwrap();
    assert!(matches!(with_keystore(dir.path(), &keystore).begin(), Err(LocalKeyError::Malformed)));
    keystore.store(&[9]).unwrap();
    assert!(matches!(with_keystore(dir.path(), &keystore).begin(), Err(LocalKeyError::Malformed)));
    keystore.store(&[SLOT_ENVELOPE, 1, 2]).unwrap();
    assert!(matches!(
        with_keystore(dir.path(), &keystore).unlock("x"),
        Err(LocalKeyError::Malformed)
    ));
}
