// SPDX-License-Identifier: Apache-2.0

//! Test doubles shared by the core's tests.

use std::sync::{Arc, Mutex};

use scoplen_client_platform::keystore::{Keystore, KeystoreError};
use scoplen_crypto::Argon2idParams;
use zeroize::Zeroizing;

/// Cheap Argon2id parameters, so that passphrase tests run quickly.
pub(crate) const FAST: Argon2idParams =
    Argon2idParams { memory_kib: 64, time_cost: 1, parallelism: 1 };

/// An in-memory keystore standing in for the platform's; the platform
/// keystores have their own tests in `scoplen-client-platform`.
#[derive(Clone, Default)]
pub(crate) struct MemoryKeystore(pub(crate) Arc<Mutex<Option<Vec<u8>>>>);

impl Keystore for MemoryKeystore {
    fn name(&self) -> &'static str {
        "test keystore"
    }
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>, KeystoreError> {
        Ok(self.0.lock().unwrap().clone().map(Zeroizing::new))
    }
    fn store(&self, secret: &[u8]) -> Result<(), KeystoreError> {
        *self.0.lock().unwrap() = Some(secret.to_vec());
        Ok(())
    }
    fn delete(&self) -> Result<(), KeystoreError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}
