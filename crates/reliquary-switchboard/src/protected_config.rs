mod credentials;
mod routes;

use crate::config_document::{ConfigDocument, ConfigObject};
use crate::device_auth::DeviceAuth;
use crate::model_config::invalid;
use crate::{
    CodexDeviceCode, ConfiguredRoutes, ConfiguredRuntimeError, Credential, CredentialId,
    CredentialsConfig, DeviceAuthError, ModelRoute, ModelSlot, SecretString,
};
use credentials::{MasterKey, PREFIX};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialKind {
    ApiKey,
    ChatGptDeviceCode,
}

/// Switchboard-owned binary configuration and protected credential state.
pub struct ConfigFile {
    path: PathBuf,
    existing: bool,
    document: ConfigDocument,
    credentials: CredentialsConfig,
    master_key: Option<MasterKey>,
    dirty_credentials: BTreeSet<CredentialId>,
}

impl fmt::Debug for ConfigFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigFile")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl ConfigFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            existing: false,
            document: ConfigDocument::default(),
            credentials: CredentialsConfig::default(),
            master_key: None,
            dirty_credentials: BTreeSet::new(),
        }
    }

    pub fn open(path: &Path) -> Result<Self, ConfiguredRuntimeError> {
        let mut config = Self::new(path);
        config.document = ConfigDocument::read(path)?;
        config.existing = true;
        for (name, object) in &config.document.0 {
            let Some(id) = name.strip_prefix(PREFIX) else {
                continue;
            };
            let id = CredentialId::new(id)
                .map_err(|_| credential_error("invalid credential object ID"))?;
            let key = match &config.master_key {
                Some(key) => key,
                None => config.master_key.insert(
                    credentials::load_key(&config.master_key_path()).map_err(credential_error)?,
                ),
            };
            let credential = credentials::decode(name, object, key).map_err(credential_error)?;
            config.credentials.entries.insert(id, credential);
        }
        Ok(config)
    }

    #[cfg(test)]
    pub(super) fn credentials_for_test(&mut self) -> &mut CredentialsConfig {
        &mut self.credentials
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn master_key_path(&self) -> PathBuf {
        credentials::key_path(&self.path)
    }

    pub fn object(&self, name: &str) -> Option<&ConfigObject> {
        self.document.0.get(name)
    }

    /// Product owners set their own objects without interpreting Switchboard objects.
    pub fn set_product_object(
        &mut self,
        name: &str,
        object: ConfigObject,
    ) -> Result<(), ConfiguredRuntimeError> {
        if name.starts_with(PREFIX) || name.starts_with("models.") {
            return Err(invalid("configuration", "reserved Switchboard object key"));
        }
        self.document.0.insert(name.to_owned(), object);
        Ok(())
    }

    pub fn model(&self, slot: ModelSlot) -> Result<Option<ModelRoute>, ConfiguredRuntimeError> {
        routes::get(&self.document, slot)
    }

    pub fn set_model(
        &mut self,
        slot: ModelSlot,
        route: &ModelRoute,
    ) -> Result<(), ConfiguredRuntimeError> {
        let object = routes::encode(slot, route)?;
        self.document.0.insert(slot.object_key().to_owned(), object);
        Ok(())
    }

    pub fn clear_model(&mut self, slot: ModelSlot) {
        self.document.0.remove(slot.object_key());
    }

    pub fn credentials(&self) -> impl Iterator<Item = (&CredentialId, CredentialKind)> {
        self.credentials.entries.iter().map(|(id, credential)| {
            let kind = match credential {
                Credential::ApiKey { .. } => CredentialKind::ApiKey,
                Credential::CodexOAuth { .. } => CredentialKind::ChatGptDeviceCode,
            };
            (id, kind)
        })
    }

    pub fn add_api_key(&mut self, id: CredentialId, secret: SecretString) {
        self.dirty_credentials.insert(id.clone());
        self.credentials
            .entries
            .insert(id, Credential::ApiKey { secret });
    }

    pub fn remove_credential(&mut self, id: &CredentialId) -> Result<(), ConfiguredRuntimeError> {
        if !self.credentials.entries.contains_key(id) {
            return Err(credential_error("credential is missing"));
        }
        for slot in ModelSlot::ALL {
            if self
                .model(slot)?
                .is_some_and(|route| route.binding().0 == id)
            {
                return Err(ConfiguredRuntimeError::Credential {
                    route: slot.label(),
                    reason: "credential is in use",
                });
            }
        }
        self.credentials.entries.remove(id);
        self.dirty_credentials.remove(id);
        self.document.0.remove(&format!("{PREFIX}{id}"));
        Ok(())
    }

    pub fn login_codex(
        &mut self,
        id: CredentialId,
        on_code: impl FnOnce(&CodexDeviceCode),
    ) -> Result<(), ConfiguredRuntimeError> {
        self.login_codex_with(id, on_code, &DeviceAuth::new()?)
    }

    pub(super) fn login_codex_with(
        &mut self,
        id: CredentialId,
        on_code: impl FnOnce(&CodexDeviceCode),
        auth: &DeviceAuth,
    ) -> Result<(), ConfiguredRuntimeError> {
        let credential = auth.login(on_code)?;
        self.credentials.entries.insert(id.clone(), credential);
        self.dirty_credentials.insert(id);
        Ok(())
    }

    pub fn refresh_codex(&mut self, id: &CredentialId) -> Result<(), ConfiguredRuntimeError> {
        self.refresh_codex_with(id, |saved| DeviceAuth::new()?.refresh(saved))
    }

    pub(super) fn refresh_codex_with(
        &mut self,
        id: &CredentialId,
        refresh: impl FnOnce(&Credential) -> Result<Credential, DeviceAuthError>,
    ) -> Result<(), ConfiguredRuntimeError> {
        self.bind()?;
        let saved = self
            .credentials
            .entries
            .get(id)
            .ok_or_else(|| credential_error("credential is missing"))?;
        if !matches!(saved, Credential::CodexOAuth { .. }) {
            return Err(credential_error("credential is not Codex OAuth"));
        }
        let refreshed = refresh(saved)?;
        self.credentials.entries.insert(id.clone(), refreshed);
        self.dirty_credentials.insert(id.clone());
        Ok(())
    }

    pub fn bind(&self) -> Result<ConfiguredRoutes, ConfiguredRuntimeError> {
        for slot in ModelSlot::ALL {
            let Some(route) = self.model(slot)? else {
                continue;
            };
            let (id, provider) = route.binding();
            if self.credentials.auth(id, provider).is_none() {
                let reason = if self.credentials.entries.contains_key(id) {
                    "credential has the wrong auth kind or is missing account data"
                } else {
                    "credential is missing"
                };
                return Err(ConfiguredRuntimeError::Credential {
                    route: slot.label(),
                    reason,
                });
            }
        }
        ConfiguredRoutes::bind(routes::decode(&self.document)?, &self.credentials)
    }

    pub fn save(&mut self) -> Result<(), ConfiguredRuntimeError> {
        self.bind()?;
        if self.dirty_credentials.is_empty() {
            self.document.save(&self.path, self.existing)?;
            self.existing = true;
            return Ok(());
        }
        if !self.existing
            && self
                .path
                .try_exists()
                .map_err(|error| ConfiguredRuntimeError::Write(error.kind()))?
        {
            return Err(ConfiguredRuntimeError::Write(
                std::io::ErrorKind::AlreadyExists,
            ));
        }
        let key = credentials::key_for_write(&self.master_key_path(), &self.document)
            .map_err(credential_error)?;
        if self
            .master_key
            .as_ref()
            .is_some_and(|expected| **expected != *key)
        {
            return Err(credential_error(
                "companion master key changed after opening",
            ));
        }
        let mut document = self.document.clone();
        for id in &self.dirty_credentials {
            let name = format!("{PREFIX}{id}");
            let object = credentials::encode(&name, &self.credentials.entries[id], &key)
                .map_err(credential_error)?;
            document.0.insert(name, object);
        }
        document.save(&self.path, self.existing)?;
        self.document = document;
        self.existing = true;
        self.master_key = Some(key);
        self.dirty_credentials.clear();
        Ok(())
    }
}

fn credential_error(reason: &'static str) -> ConfiguredRuntimeError {
    ConfiguredRuntimeError::Credential {
        route: "credentials",
        reason,
    }
}

#[cfg(test)]
mod tests;
