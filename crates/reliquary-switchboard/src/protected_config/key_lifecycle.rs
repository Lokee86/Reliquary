use super::credentials::{self, PREFIX};
use super::master_key::{self, MasterKey, MasterKeyError};
use super::{ConfigDocument, credential_error};
use crate::config_document::read_bounded;
use crate::{ConfiguredRuntimeError, CredentialId, CredentialsConfig};
use std::borrow::Cow;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

pub(super) fn new_destination(path: &Path) -> Result<PathBuf, ConfiguredRuntimeError> {
    let name = path
        .file_name()
        .ok_or(ConfiguredRuntimeError::Write(io::ErrorKind::InvalidInput))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let canonical = fs::create_dir_all(parent)
        .and_then(|()| parent.canonicalize())
        .map_err(|error| ConfiguredRuntimeError::Write(error.kind()))?;
    Ok(canonical.join(name))
}

// Keyring writes are upserts. Use a stable, empty lock file so writers and
// migrations cannot install competing keys while replacing the config itself.
pub(super) fn lock(path: &Path) -> io::Result<File> {
    let name = format!(".reliquary-config-{}.lock", master_key::account(path));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_file_name(name))?;
    file.lock()?;
    Ok(file)
}

pub(super) fn key_for_write(
    path: &Path,
    expected: Option<&MasterKey>,
) -> Result<MasterKey, ConfiguredRuntimeError> {
    if let Some(key) = master_key::load(path)? {
        if expected.is_some_and(|expected| **expected != *key) {
            return Err(MasterKeyError::Changed.into());
        }
        return Ok(key);
    }
    if expected.is_some() {
        return Err(MasterKeyError::Missing.into());
    }
    // This handle may have opened an empty document before another process
    // saved its first credential. Never replace a missing key for that file.
    match ConfigDocument::read(path) {
        Ok(current) if current.0.keys().any(|name| name.starts_with(PREFIX)) => {
            return Err(MasterKeyError::Missing.into());
        }
        Ok(_) | Err(ConfiguredRuntimeError::Io(io::ErrorKind::NotFound)) => {}
        Err(error) => return Err(error),
    }
    Ok(master_key::store_new(path, &master_key::generate()?)?)
}

pub(super) fn open_credentials(
    path: &Path,
    document: &ConfigDocument,
) -> Result<(MasterKey, CredentialsConfig), ConfiguredRuntimeError> {
    let sidecar = legacy_path(path);
    match master_key::load(path)? {
        Some(key) => {
            let credentials = decode_credentials(document, &key)?;
            if !legacy_matches(&sidecar, &key) {
                return Ok((key, credentials));
            }
        }
        None if !has_legacy(&sidecar)? => {
            // A concurrent migration may have removed the companion after
            // the initial OS lookup.
            let key = master_key::load(path)?.ok_or(MasterKeyError::Missing)?;
            let credentials = decode_credentials(document, &key)?;
            return Ok((key, credentials));
        }
        None => {}
    }
    let _lock =
        lock(path).map_err(|_| MasterKeyError::Migration("could not lock the configuration"))?;
    migrate(path, &sidecar, document)
}

fn decode_credentials(
    document: &ConfigDocument,
    key: &MasterKey,
) -> Result<CredentialsConfig, ConfiguredRuntimeError> {
    let mut config = CredentialsConfig::default();
    for (name, object) in &document.0 {
        if let Some(id) = name.strip_prefix(PREFIX) {
            let id = CredentialId::new(id)
                .map_err(|_| credential_error("invalid credential object ID"))?;
            let credential = credentials::decode(name, object, key).map_err(credential_error)?;
            config.entries.insert(id, credential);
        }
    }
    Ok(config)
}

fn legacy_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .unwrap_or("reliquary".as_ref())
        .to_os_string();
    name.push(".master-key.json");
    path.with_file_name(name)
}

fn has_legacy(path: &Path) -> Result<bool, MasterKeyError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err(MasterKeyError::Migration(
            "legacy companion is not a regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(MasterKeyError::Migration(
            "could not inspect the legacy companion",
        )),
    }
}

#[derive(serde::Deserialize)]
struct KeyFile<'a> {
    version: u64,
    #[serde(borrow)]
    master_key_hex: Cow<'a, str>,
}

fn legacy_matches(path: &Path, key: &MasterKey) -> bool {
    // A companion with this stem may belong to another configuration.
    has_legacy(path).unwrap_or(false) && read_legacy(path).is_ok_and(|legacy| *legacy == **key)
}

fn read_legacy(path: &Path) -> Result<MasterKey, MasterKeyError> {
    let bytes = read_bounded(path, 4096)
        .map_err(|_| MasterKeyError::Migration("could not read the legacy companion"))?;
    if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
        return Err(MasterKeyError::Migration("invalid legacy key JSON"));
    }
    let file: KeyFile<'_> = serde_json::from_slice(&bytes)
        .map_err(|_| MasterKeyError::Migration("invalid legacy key JSON"))?;
    let encoded = Zeroizing::new(file.master_key_hex.into_owned());
    if file.version != 1 {
        return Err(MasterKeyError::Migration("unsupported legacy key version"));
    }
    let mut key = MasterKey::new([0; 32]);
    hex::decode_to_slice(encoded.as_bytes(), &mut *key)
        .map_err(|_| MasterKeyError::Migration("invalid legacy key material"))?;
    Ok(key)
}

fn migrate(
    path: &Path,
    sidecar: &Path,
    document: &ConfigDocument,
) -> Result<(MasterKey, CredentialsConfig), ConfiguredRuntimeError> {
    if let Some(key) = master_key::load(path)? {
        let credentials = decode_credentials(document, &key)?;
        if legacy_matches(sidecar, &key) {
            remove_legacy(sidecar)?;
        }
        return Ok((key, credentials));
    }
    if !has_legacy(sidecar)? {
        return Err(MasterKeyError::Missing.into());
    }

    let legacy = read_legacy(sidecar)?;
    let authenticate = |key| {
        decode_credentials(document, key).map_err(|_| {
            MasterKeyError::Migration("could not authenticate the existing credentials")
        })
    };
    // A wrong companion must never poison the OS entry.
    authenticate(&legacy)?;
    let key = master_key::store_new(path, &legacy).map_err(|_| {
        MasterKeyError::Migration(
            "could not persist and read back the key in the OS credential store",
        )
    })?;
    let credentials = authenticate(&key)?;
    remove_legacy(sidecar)?;
    Ok((key, credentials))
}

fn remove_legacy(path: &Path) -> Result<(), MasterKeyError> {
    fs::remove_file(path)
        .map_err(|_| MasterKeyError::Migration("could not remove the legacy companion"))
}
