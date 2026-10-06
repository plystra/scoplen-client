// SPDX-License-Identifier: Apache-2.0

//! Windows: the key is wrapped by an RSA key held in the TPM through the
//! Microsoft Platform Crypto Provider where a TPM is available, and by DPAPI
//! for the current user otherwise. The wrapped bytes are kept in a file next
//! to the store, prefixed by the method that wrapped them.

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    BCRYPT_OAEP_PADDING_INFO, BCRYPT_RSA_ALGORITHM, BCRYPT_SHA256_ALGORITHM, CERT_KEY_SPEC,
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    MS_PLATFORM_CRYPTO_PROVIDER, NCRYPT_FLAGS, NCRYPT_HANDLE, NCRYPT_KEY_HANDLE,
    NCRYPT_PAD_OAEP_FLAG, NCRYPT_PROV_HANDLE, NCryptCreatePersistedKey, NCryptDecrypt,
    NCryptDeleteKey, NCryptEncrypt, NCryptFinalizeKey, NCryptFreeObject, NCryptOpenKey,
    NCryptOpenStorageProvider,
};
use windows::core::{HSTRING, PCWSTR, w};
use zeroize::Zeroizing;

use super::{Keystore, KeystoreError};

const NAME: &str = "Windows keystore";
const FILE: &str = "local-key.protected";
const METHOD_DPAPI: u8 = 1;
const METHOD_TPM: u8 = 2;
/// Mixed into DPAPI so that another application's protected blob for the
/// same user cannot be substituted.
const DPAPI_ENTROPY: &[u8] = b"spl-local-db-key-dpapi-v1";

pub(super) struct WindowsKeystore {
    file: PathBuf,
    tpm_key_name: HSTRING,
}

impl WindowsKeystore {
    pub(super) fn for_data_dir(data_dir: &Path) -> WindowsKeystore {
        // One TPM key per store location; the name is derived from the path so
        // that development and release installs do not share a key.
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in data_dir.as_os_str().as_encoded_bytes() {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
        }
        WindowsKeystore {
            file: data_dir.join(FILE),
            tpm_key_name: HSTRING::from(format!("Scoplen local database key {hash:016x}")),
        }
    }

    fn file_error(operation: &'static str) -> impl FnOnce(std::io::Error) -> KeystoreError {
        move |source| KeystoreError::File { operation, source }
    }
}

impl Keystore for WindowsKeystore {
    fn name(&self) -> &'static str {
        NAME
    }

    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>, KeystoreError> {
        let bytes = match std::fs::read(&self.file) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Self::file_error("read")(e)),
        };
        match bytes.split_first() {
            Some((&METHOD_DPAPI, wrapped)) => dpapi_unprotect(wrapped).map(Some),
            Some((&METHOD_TPM, wrapped)) => tpm_unwrap(&self.tpm_key_name, wrapped).map(Some),
            _ => Err(KeystoreError::Unrecoverable { keystore: NAME }),
        }
    }

    fn store(&self, secret: &[u8]) -> Result<(), KeystoreError> {
        let mut file = match tpm_wrap(&self.tpm_key_name, secret) {
            Ok(wrapped) => [&[METHOD_TPM][..], &wrapped].concat(),
            // No TPM, or the provider refused: DPAPI still binds the key to
            // this user on this machine.
            Err(_) => [&[METHOD_DPAPI][..], &dpapi_protect(secret)?].concat(),
        };
        let partial = self.file.with_extension("protected.partial");
        std::fs::write(&partial, &file).map_err(Self::file_error("written"))?;
        std::fs::rename(&partial, &self.file).map_err(Self::file_error("written"))?;
        file.iter_mut().for_each(|b| *b = 0);
        Ok(())
    }

    fn delete(&self) -> Result<(), KeystoreError> {
        match std::fs::remove_file(&self.file) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(Self::file_error("removed")(e)),
        }
        tpm_delete(&self.tpm_key_name);
        Ok(())
    }
}

fn platform(operation: &'static str, error: windows::core::Error) -> KeystoreError {
    KeystoreError::Platform { keystore: NAME, operation, detail: error.message() }
}

fn blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_ptr().cast_mut() }
}

/// Copies and frees a blob allocated by DPAPI.
fn take_blob(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    // SAFETY: on success DPAPI returns `cbData` readable bytes at `pbData`,
    // allocated with LocalAlloc; they are copied before being freed once.
    unsafe {
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        std::ptr::write_bytes(out.pbData, 0, out.cbData as usize);
        LocalFree(Some(HLOCAL(out.pbData.cast())));
        bytes
    }
}

fn dpapi_protect(secret: &[u8]) -> Result<Vec<u8>, KeystoreError> {
    let input = blob(secret);
    let entropy = blob(DPAPI_ENTROPY);
    let mut out = CRYPT_INTEGER_BLOB::default();
    // SAFETY: the input and entropy blobs point to live slices for the
    // duration of the call, and `out` is a valid place for the result.
    unsafe {
        CryptProtectData(
            &input,
            w!("Scoplen local database key"),
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )
    }
    .map_err(|e| platform("protect", e))?;
    Ok(take_blob(out))
}

fn dpapi_unprotect(wrapped: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeystoreError> {
    let input = blob(wrapped);
    let entropy = blob(DPAPI_ENTROPY);
    let mut out = CRYPT_INTEGER_BLOB::default();
    // SAFETY: as in `dpapi_protect`.
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )
    }
    .map_err(|_| KeystoreError::Unrecoverable { keystore: NAME })?;
    Ok(Zeroizing::new(take_blob(out)))
}

/// An NCrypt handle freed when dropped.
struct Handle(NCRYPT_HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: the handle came from NCrypt and is freed exactly once.
            let _ = unsafe { NCryptFreeObject(self.0) };
        }
    }
}

fn open_provider() -> Result<Handle, windows::core::Error> {
    let mut provider = NCRYPT_PROV_HANDLE::default();
    // SAFETY: `provider` is a valid place for the returned handle.
    unsafe { NCryptOpenStorageProvider(&mut provider, MS_PLATFORM_CRYPTO_PROVIDER, 0) }?;
    Ok(Handle(NCRYPT_HANDLE(provider.0)))
}

fn open_key(
    provider: &Handle,
    name: &HSTRING,
    create: bool,
) -> Result<Handle, windows::core::Error> {
    let provider = NCRYPT_PROV_HANDLE(provider.0.0);
    let mut key = NCRYPT_KEY_HANDLE::default();
    // SAFETY: `provider` is open, `name` outlives the call, and `key` is a
    // valid place for the returned handle.
    let opened = unsafe {
        NCryptOpenKey(provider, &mut key, PCWSTR(name.as_ptr()), CERT_KEY_SPEC(0), NCRYPT_FLAGS(0))
    };
    if opened.is_ok() || !create {
        opened?;
        return Ok(Handle(NCRYPT_HANDLE(key.0)));
    }
    // SAFETY: as above; the new key is finalized before use.
    unsafe {
        NCryptCreatePersistedKey(
            provider,
            &mut key,
            BCRYPT_RSA_ALGORITHM,
            PCWSTR(name.as_ptr()),
            CERT_KEY_SPEC(0),
            NCRYPT_FLAGS(0),
        )?;
        let handle = Handle(NCRYPT_HANDLE(key.0));
        NCryptFinalizeKey(key, NCRYPT_FLAGS(0))?;
        Ok(handle)
    }
}

fn oaep() -> BCRYPT_OAEP_PADDING_INFO {
    BCRYPT_OAEP_PADDING_INFO {
        pszAlgId: BCRYPT_SHA256_ALGORITHM,
        pbLabel: std::ptr::null_mut(),
        cbLabel: 0,
    }
}

/// The shape shared by `NCryptEncrypt` and `NCryptDecrypt`.
type NcryptOperation = unsafe fn(
    NCRYPT_KEY_HANDLE,
    Option<&[u8]>,
    Option<*const core::ffi::c_void>,
    Option<&mut [u8]>,
    *mut u32,
    NCRYPT_FLAGS,
) -> windows::core::Result<()>;

/// Runs an NCrypt encrypt or decrypt: first to size the output, then for real.
fn ncrypt(
    operation: NcryptOperation,
    key: &Handle,
    input: &[u8],
) -> Result<Vec<u8>, windows::core::Error> {
    let key = NCRYPT_KEY_HANDLE(key.0.0);
    let padding = oaep();
    let padding_ptr = Some(std::ptr::from_ref(&padding).cast());
    let mut size = 0u32;
    // SAFETY: `key` is open, `input` and `padding` outlive both calls, and the
    // output buffer has the size NCrypt reported.
    unsafe {
        operation(key, Some(input), padding_ptr, None, &mut size, NCRYPT_PAD_OAEP_FLAG)?;
        let mut output = vec![0u8; size as usize];
        operation(
            key,
            Some(input),
            padding_ptr,
            Some(&mut output),
            &mut size,
            NCRYPT_PAD_OAEP_FLAG,
        )?;
        output.truncate(size as usize);
        Ok(output)
    }
}

fn tpm_wrap(name: &HSTRING, secret: &[u8]) -> Result<Vec<u8>, windows::core::Error> {
    let provider = open_provider()?;
    let key = open_key(&provider, name, true)?;
    ncrypt(NCryptEncrypt, &key, secret)
}

fn tpm_unwrap(name: &HSTRING, wrapped: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeystoreError> {
    let unrecoverable = |_| KeystoreError::Unrecoverable { keystore: NAME };
    let provider = open_provider().map_err(unrecoverable)?;
    let key = open_key(&provider, name, false).map_err(unrecoverable)?;
    ncrypt(NCryptDecrypt, &key, wrapped).map(Zeroizing::new).map_err(unrecoverable)
}

fn tpm_delete(name: &HSTRING) {
    let Ok(provider) = open_provider() else { return };
    let Ok(key) = open_key(&provider, name, false) else { return };
    let raw = NCRYPT_KEY_HANDLE(key.0.0);
    // SAFETY: the key handle is open. NCryptDeleteKey frees it on success, so
    // the guard is then forgotten; on failure the guard frees it.
    let deleted = unsafe { NCryptDeleteKey(raw, 0) };
    if deleted.is_ok() {
        std::mem::forget(key);
    }
}
