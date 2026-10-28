//! Model execution contracts and routing.

mod decision_endpoint;
mod general_decision;
mod general_endpoint;

pub use decision_endpoint::{DecisionEndpoint, DecisionEndpointError, SimulatedDecisionEndpoint};
pub use general_decision::GeneralDecisionEndpoint;
pub use general_endpoint::{GeneralEndpoint, GeneralEndpointError, SimulatedGeneralEndpoint};

mod config_credentials;
mod config_document;
mod credential;
mod credential_codec;
mod credential_crypto;
mod master_key;
mod master_key_entropy;
mod model_auth_kind;

pub use config_document::{
    ConfigDocument, ConfigDocumentError, ConfigObject, decode_config_document,
    encode_config_document, read_config_document, replace_config_document,
};
pub use credential::{Credential, CredentialError, CredentialId, CredentialsConfig, SecretString};
pub use master_key::{
    JsonMasterKeyStore, MASTER_KEY_BYTES, MasterKey, MasterKeyError, MasterKeyStore,
    master_key_path,
};
pub use model_auth_kind::ModelAuthKind;

#[cfg(test)]
mod master_key_tests;

#[cfg(test)]
mod config_document_tests;

pub use config_credentials::{CredentialDocumentError, insert_credentials, load_credentials};

mod model_auth;
pub mod model_switchboard;
pub mod model_switchboard_codec;
pub use model_auth::ModelRequestAuth;
pub use model_switchboard::{
    DecisionModelEndpoint, EmbeddingModelEndpoint, GeneralModelEndpoint, ModelCapability,
    ModelProvider, ModelReasoningEffort, ModelSwitchboard, ModelSwitchboardConfig,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddingNormalization {
    None,
    L2,
}
impl EmbeddingNormalization {
    pub fn tag(self) -> u8 {
        match self {
            Self::None => 0,
            Self::L2 => 1,
        }
    }
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::None),
            1 => Some(Self::L2),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub enum ModelConfigError {
    InvalidModelSwitchboard,
    Credential(CredentialError),
}
impl From<CredentialError> for ModelConfigError {
    fn from(value: CredentialError) -> Self {
        Self::Credential(value)
    }
}
impl std::fmt::Display for ModelConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidModelSwitchboard => f.write_str("invalid model switchboard configuration"),
            Self::Credential(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for ModelConfigError {}

mod configured_decision;
mod configured_general;
pub mod configured_runtime_decision;
mod openai_codex_device_auth;
mod openai_codex_general;
mod openai_ready_general;
pub use configured_decision::ConfiguredDecisionEndpoint;
pub use configured_general::ConfiguredGeneralEndpoint;
pub use openai_codex_device_auth::{
    OPENAI_CODEX_AUTH_ISSUER, OPENAI_CODEX_DEVICE_LOGIN_TIMEOUT_SECS, OPENAI_CODEX_OAUTH_CLIENT_ID,
    OpenAiCodexDeviceAuth, OpenAiCodexDeviceAuthError, OpenAiCodexDeviceCode,
};
pub use openai_codex_general::{
    OPENAI_CODEX_COMPAT_VERSION, OPENAI_CODEX_RESPONSES_URL, OpenAiCodexGeneralEndpoint,
};
pub use openai_ready_general::OpenAiReadyGeneralEndpoint;
#[cfg(test)]
mod openai_codex_device_auth_tests;
#[cfg(test)]
mod openai_codex_general_tests;
#[cfg(test)]
mod openai_ready_general_tests;

mod openai_ready_embedding;
mod openai_ready_embedding_response;
pub use openai_ready_embedding::{
    DEFAULT_REMOTE_EMBEDDING_BATCH_SIZE, DEFAULT_REMOTE_EMBEDDING_CONCURRENCY,
    OpenAiReadyEmbeddingEndpoint,
};
#[cfg(test)]
mod openai_ready_embedding_tests;
#[derive(Clone, Copy, Debug)]
pub enum EmbeddingMode {
    Query,
    Document,
}
#[derive(Debug)]
pub enum EmbeddingEndpointError {
    InvalidConfiguration(&'static str),
    InvalidResponse(&'static str),
    Failure(String),
}
impl std::fmt::Display for EmbeddingEndpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "embedding endpoint error: {self:?}")
    }
}
impl std::error::Error for EmbeddingEndpointError {}
