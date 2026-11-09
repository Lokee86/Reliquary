use ring::digest::{Context, SHA256};
use ring::rand::{SecureRandom, SystemRandom};
use std::fmt;
use std::path::Path;
use zeroize::Zeroizing;

pub(crate) type MasterKey = Zeroizing<[u8; 32]>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MasterKeyError {
    Missing,
    StoreUnavailable,
    WriteRejected,
    InvalidMaterial,
    Changed,
    Entropy,
    Migration(&'static str),
}

impl fmt::Display for MasterKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("master key is missing for this encrypted configuration"),
            Self::StoreUnavailable => f.write_str("OS credential store is unavailable"),
            Self::WriteRejected => {
                f.write_str("OS credential store did not persist the master key")
            }
            Self::InvalidMaterial => {
                f.write_str("OS credential store contains an invalid master key")
            }
            Self::Changed => f.write_str("master key changed after opening the configuration"),
            Self::Entropy => f.write_str("master-key entropy is unavailable"),
            Self::Migration(reason) => write!(f, "legacy master-key migration failed ({reason})"),
        }
    }
}

impl std::error::Error for MasterKeyError {}

pub(super) fn account(path: &Path) -> String {
    let mut hash = Context::new(&SHA256);
    hash.update(b"reliquary/master-key/v1\0");
    hash.update(path.as_os_str().as_encoded_bytes());
    hex::encode(hash.finish())
}

pub(super) fn generate() -> Result<MasterKey, MasterKeyError> {
    let mut key = MasterKey::new([0; 32]);
    SystemRandom::new()
        .fill(&mut *key)
        .map_err(|_| MasterKeyError::Entropy)?;
    Ok(key)
}

pub(super) fn load(path: &Path) -> Result<Option<MasterKey>, MasterKeyError> {
    let account = account(path);
    #[cfg(any(test, feature = "test-support"))]
    if let Some(result) = mock::load(&account) {
        return decode(result?);
    }
    decode(os::load(&account)?)
}

fn decode(bytes: Option<Zeroizing<Vec<u8>>>) -> Result<Option<MasterKey>, MasterKeyError> {
    bytes
        .map(|bytes| {
            if bytes.len() != 32 {
                return Err(MasterKeyError::InvalidMaterial);
            }
            let mut key = MasterKey::new([0; 32]);
            key.copy_from_slice(&bytes);
            Ok(key)
        })
        .transpose()
}

/// The caller holds the configuration's write lock and has checked for absence.
pub(super) fn store_new(path: &Path, key: &MasterKey) -> Result<MasterKey, MasterKeyError> {
    let account = account(path);
    write(&account, key)?;
    match load(path)? {
        Some(stored) if *stored == **key => Ok(stored),
        _ => Err(MasterKeyError::WriteRejected),
    }
}

fn write(account: &str, key: &MasterKey) -> Result<(), MasterKeyError> {
    #[cfg(any(test, feature = "test-support"))]
    if let Some(result) = mock::write(account, key) {
        return result;
    }
    os::write(account, key)
}

#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
mod os {
    use super::{MasterKey, MasterKeyError, Zeroizing};

    const SERVICE: &str = "reliquary.master-key.v1";

    fn entry(account: &str) -> Result<keyring::Entry, MasterKeyError> {
        // Select the compiled platform backend directly, independent of global
        // keyring builder overrides by other libraries in the same process.
        keyring::default::default_credential_builder()
            .build(None, SERVICE, account)
            .map(keyring::Entry::new_with_credential)
            .map_err(|_| MasterKeyError::StoreUnavailable)
    }

    pub(super) fn load(account: &str) -> Result<Option<Zeroizing<Vec<u8>>>, MasterKeyError> {
        match entry(account)?.get_secret() {
            Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(MasterKeyError::StoreUnavailable),
        }
    }

    pub(super) fn write(account: &str, key: &MasterKey) -> Result<(), MasterKeyError> {
        entry(account)?
            .set_secret(&**key)
            .map_err(|_| MasterKeyError::WriteRejected)
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod os {
    use super::{MasterKey, MasterKeyError, Zeroizing};

    pub(super) fn load(_: &str) -> Result<Option<Zeroizing<Vec<u8>>>, MasterKeyError> {
        Err(MasterKeyError::StoreUnavailable)
    }

    pub(super) fn write(_: &str, _: &MasterKey) -> Result<(), MasterKeyError> {
        Err(MasterKeyError::StoreUnavailable)
    }
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) mod mock;
