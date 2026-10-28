use crate::ConfigObject as RawConfigObject;
use crate::credential_codec::{
    decode_credential_object, encode_credential_object, id_from_object_key, object_key,
};
use crate::{
    CredentialError, CredentialsConfig, JsonMasterKeyStore, MasterKeyError, MasterKeyStore,
    master_key_path,
};
use std::collections::BTreeMap;
use std::path::Path;

pub fn load_credentials(
    config_path: &Path,
    objects: &mut BTreeMap<String, RawConfigObject>,
) -> Result<CredentialsConfig, CredentialDocumentError> {
    let credential_keys: Vec<String> = objects
        .keys()
        .filter(|key| key.starts_with("credential."))
        .cloned()
        .collect();
    if credential_keys.is_empty() {
        return Ok(CredentialsConfig::default());
    }

    let master_key = JsonMasterKeyStore::new(master_key_path(config_path)).load_existing()?;
    let mut credentials = CredentialsConfig::default();
    for key in credential_keys {
        let id = id_from_object_key(&key)?.ok_or(crate::CredentialError::InvalidCredentialId)?;
        let object = objects.remove(&key).expect("credential key came from map");
        credentials.insert(
            id.clone(),
            decode_credential_object(&id, &object, &master_key)?,
        );
    }
    Ok(credentials)
}

pub fn insert_credentials(
    config_path: &Path,
    credentials: &CredentialsConfig,
    objects: &mut BTreeMap<String, RawConfigObject>,
) -> Result<(), CredentialDocumentError> {
    if credentials.is_empty() {
        return Ok(());
    }
    let master_key = JsonMasterKeyStore::new(master_key_path(config_path)).load_or_create()?;
    for (id, credential) in credentials.entries() {
        objects.insert(
            object_key(id),
            encode_credential_object(id, credential, &master_key)?,
        );
    }
    Ok(())
}

#[derive(Debug)]
pub enum CredentialDocumentError {
    Credential(CredentialError),
    MasterKey(MasterKeyError),
}
impl From<CredentialError> for CredentialDocumentError {
    fn from(value: CredentialError) -> Self {
        Self::Credential(value)
    }
}
impl From<MasterKeyError> for CredentialDocumentError {
    fn from(value: MasterKeyError) -> Self {
        Self::MasterKey(value)
    }
}
impl std::fmt::Display for CredentialDocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Credential(error) => error.fmt(f),
            Self::MasterKey(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for CredentialDocumentError {}
