use crate::ModelAuthKind;
use std::collections::BTreeMap;
use std::fmt;
use zeroize::Zeroize;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CredentialId(String);

impl CredentialId {
    pub fn new(value: impl Into<String>) -> Result<Self, CredentialError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(CredentialError::InvalidCredentialId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Result<Self, CredentialError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CredentialError::InvalidCredential);
        }
        Ok(Self(value))
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString([REDACTED])")
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Credential {
    ApiKey {
        api_key: SecretString,
    },
    ChatGptOAuth {
        id_token: SecretString,
        access_token: SecretString,
        refresh_token: SecretString,
        account_id: Option<String>,
    },
}

impl Credential {
    pub fn auth_kind(&self) -> ModelAuthKind {
        match self {
            Self::ApiKey { .. } => ModelAuthKind::ApiKey,
            Self::ChatGptOAuth { .. } => ModelAuthKind::ChatGptDeviceCode,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CredentialsConfig {
    entries: BTreeMap<CredentialId, Credential>,
}

impl CredentialsConfig {
    pub fn insert_api_key(
        &mut self,
        id: CredentialId,
        api_key: impl Into<String>,
    ) -> Result<(), CredentialError> {
        let api_key = SecretString::new(api_key)?;
        self.entries.insert(id, Credential::ApiKey { api_key });
        Ok(())
    }

    pub fn insert_chatgpt_oauth(
        &mut self,
        id: CredentialId,
        id_token: impl Into<String>,
        access_token: impl Into<String>,
        refresh_token: impl Into<String>,
        account_id: Option<String>,
    ) -> Result<(), CredentialError> {
        if account_id.as_ref().is_some_and(|value| value.is_empty()) {
            return Err(CredentialError::InvalidCredential);
        }
        self.entries.insert(
            id,
            Credential::ChatGptOAuth {
                id_token: SecretString::new(id_token)?,
                access_token: SecretString::new(access_token)?,
                refresh_token: SecretString::new(refresh_token)?,
                account_id,
            },
        );
        Ok(())
    }

    pub fn get(&self, id: &CredentialId) -> Option<&Credential> {
        self.entries.get(id)
    }

    pub fn remove(&mut self, id: &CredentialId) -> Option<Credential> {
        self.entries.remove(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &CredentialId> {
        self.entries.keys()
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = (&CredentialId, &Credential)> {
        self.entries.iter()
    }

    pub(crate) fn insert(&mut self, id: CredentialId, credential: Credential) {
        self.entries.insert(id, credential);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Debug)]
pub enum CredentialError {
    InvalidCredentialId,
    InvalidCredential,
    InvalidEncryptedObject,
    EncryptionFailed,
    DecryptionFailed,
    MissingCredential(String),
    WrongAuthKind(String),
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCredentialId => write!(f, "invalid credential id"),
            Self::InvalidCredential => write!(f, "invalid credential"),
            Self::InvalidEncryptedObject => write!(f, "invalid encrypted credential object"),
            Self::EncryptionFailed => write!(f, "credential encryption failed"),
            Self::DecryptionFailed => write!(f, "credential decryption failed"),
            Self::MissingCredential(id) => write!(f, "missing credential: {id}"),
            Self::WrongAuthKind(id) => write!(f, "credential has wrong auth kind: {id}"),
        }
    }
}

impl std::error::Error for CredentialError {}
