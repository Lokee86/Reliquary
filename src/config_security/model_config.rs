use crate::{ConfigError, CredentialError, CredentialId, CredentialsConfig, VectorNormalization};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelProvider {
    OpenAiCodex,
    OpenAiReady,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelCapability {
    General,
    Insomnia,
    InsomniaMetadata,
    EntityExtraction,
    EntityResolution,
    Chronos,
    Dream,
    Embedding,
}

impl ModelCapability {
    pub fn route_key(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Insomnia => "insomnia",
            Self::InsomniaMetadata => "insomnia_metadata",
            Self::EntityExtraction => "entity_extraction",
            Self::EntityResolution => "entity_resolution",
            Self::Chronos => "chronos",
            Self::Dream => "dream",
            Self::Embedding => "embedding",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelAuthKind {
    ChatGptDeviceCode,
    ApiKey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl ModelReasoningEffort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }

    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::None => 1,
            Self::Minimal => 2,
            Self::Low => 3,
            Self::Medium => 4,
            Self::High => 5,
            Self::XHigh => 6,
            Self::Max => 7,
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::None),
            2 => Some(Self::Minimal),
            3 => Some(Self::Low),
            4 => Some(Self::Medium),
            5 => Some(Self::High),
            6 => Some(Self::XHigh),
            7 => Some(Self::Max),
            _ => None,
        }
    }
}

impl ModelProvider {
    pub fn auth_kind(self) -> ModelAuthKind {
        match self {
            Self::OpenAiCodex => ModelAuthKind::ChatGptDeviceCode,
            Self::OpenAiReady => ModelAuthKind::ApiKey,
        }
    }

    pub fn supports(self, capability: ModelCapability) -> bool {
        match (self, capability) {
            (
                _,
                ModelCapability::General
                | ModelCapability::Insomnia
                | ModelCapability::InsomniaMetadata
                | ModelCapability::EntityExtraction
                | ModelCapability::EntityResolution
                | ModelCapability::Chronos
                | ModelCapability::Dream,
            ) => true,
            (Self::OpenAiReady, ModelCapability::Embedding) => true,
            (Self::OpenAiCodex, ModelCapability::Embedding) => false,
        }
    }

    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::OpenAiCodex => 1,
            Self::OpenAiReady => 2,
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::OpenAiCodex),
            2 => Some(Self::OpenAiReady),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralModelEndpoint {
    pub provider: ModelProvider,
    pub model: String,
    pub url: Option<String>,
    pub credential_id: CredentialId,
    pub reasoning_effort: Option<ModelReasoningEffort>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionModelEndpoint {
    pub model: String,
    pub url: String,
    pub credential_id: CredentialId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddingModelEndpoint {
    pub provider: ModelProvider,
    pub model: String,
    pub url: Option<String>,
    pub credential_id: CredentialId,
    pub dimensions: u32,
    pub normalization: VectorNormalization,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModelSwitchboardConfig {
    pub general: Option<GeneralModelEndpoint>,
    pub insomnia: Option<GeneralModelEndpoint>,
    pub insomnia_metadata: Option<GeneralModelEndpoint>,
    pub entity_extraction: Option<GeneralModelEndpoint>,
    pub general_decision: Option<DecisionModelEndpoint>,
    pub entity_resolution_decision: Option<DecisionModelEndpoint>,
    pub memory_retrieval_decision: Option<DecisionModelEndpoint>,
    pub entity_resolution: Option<GeneralModelEndpoint>,
    pub chronos: Option<GeneralModelEndpoint>,
    pub dream: Option<GeneralModelEndpoint>,
    pub embedding: Option<EmbeddingModelEndpoint>,
}

pub(crate) fn validate_switchboard(config: &ModelSwitchboardConfig) -> Result<(), ConfigError> {
    if let Some(endpoint) = &config.general {
        validate_general_endpoint(endpoint)?;
    }
    if let Some(endpoint) = &config.insomnia {
        validate_general_endpoint(endpoint)?;
    }
    if let Some(endpoint) = &config.insomnia_metadata {
        validate_general_endpoint(endpoint)?;
    }
    if let Some(endpoint) = &config.entity_extraction {
        validate_general_endpoint(endpoint)?;
    }
    if let Some(endpoint) = &config.general_decision {
        if endpoint.model.trim().is_empty() || !valid_http_endpoint_url(&endpoint.url) {
            return Err(ConfigError::InvalidModelSwitchboard);
        }
    }
    if let Some(endpoint) = &config.entity_resolution_decision {
        if endpoint.model.trim().is_empty() || !valid_url(&endpoint.url) {
            return Err(ConfigError::InvalidModelSwitchboard);
        }
    }
    if let Some(endpoint) = &config.memory_retrieval_decision {
        if endpoint.model.trim().is_empty() || !valid_http_endpoint_url(&endpoint.url) {
            return Err(ConfigError::InvalidModelSwitchboard);
        }
    }
    if let Some(endpoint) = &config.entity_resolution {
        validate_general_endpoint(endpoint)?;
    }
    if let Some(endpoint) = &config.chronos {
        validate_general_endpoint(endpoint)?;
    }
    if let Some(endpoint) = &config.dream {
        validate_general_endpoint(endpoint)?;
    }
    if let Some(endpoint) = &config.embedding {
        if !endpoint.provider.supports(ModelCapability::Embedding) || endpoint.dimensions == 0 {
            return Err(ConfigError::InvalidModelSwitchboard);
        }
        validate_endpoint(endpoint.provider, &endpoint.model, endpoint.url.as_deref())?;
    }
    Ok(())
}

fn validate_general_endpoint(endpoint: &GeneralModelEndpoint) -> Result<(), ConfigError> {
    validate_endpoint(endpoint.provider, &endpoint.model, endpoint.url.as_deref())
}

fn validate_endpoint(
    provider: ModelProvider,
    model: &str,
    url: Option<&str>,
) -> Result<(), ConfigError> {
    if model.trim().is_empty() {
        return Err(ConfigError::InvalidModelSwitchboard);
    }
    match provider {
        ModelProvider::OpenAiCodex if url.is_some() => Err(ConfigError::InvalidModelSwitchboard),
        ModelProvider::OpenAiReady => match url {
            Some(url) if valid_url(url) => Ok(()),
            _ => Err(ConfigError::InvalidModelSwitchboard),
        },
        ModelProvider::OpenAiCodex => Ok(()),
    }
}

fn valid_http_endpoint_url(url: &str) -> bool {
    reqwest::Url::parse(url.trim()).is_ok_and(|parsed| {
        matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some()
    })
}

fn valid_url(url: &str) -> bool {
    let trimmed = url.trim();
    trimmed.starts_with("https://") || trimmed.starts_with("http://")
}

pub(crate) fn validate_credentials(
    config: &ModelSwitchboardConfig,
    credentials: &CredentialsConfig,
) -> Result<(), CredentialError> {
    for endpoint in [
        config.general_decision.as_ref(),
        config.entity_resolution_decision.as_ref(),
        config.memory_retrieval_decision.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        let credential = credentials.get(&endpoint.credential_id).ok_or_else(|| {
            CredentialError::MissingCredential(endpoint.credential_id.as_str().into())
        })?;
        if credential.auth_kind() != ModelAuthKind::ApiKey {
            return Err(CredentialError::WrongAuthKind(
                endpoint.credential_id.as_str().into(),
            ));
        }
    }

    for (provider, id) in config
        .general
        .iter()
        .map(|endpoint| (endpoint.provider, &endpoint.credential_id))
        .chain(
            config
                .insomnia
                .iter()
                .map(|endpoint| (endpoint.provider, &endpoint.credential_id)),
        )
        .chain(
            config
                .insomnia_metadata
                .iter()
                .map(|endpoint| (endpoint.provider, &endpoint.credential_id)),
        )
        .chain(
            config
                .entity_extraction
                .iter()
                .map(|endpoint| (endpoint.provider, &endpoint.credential_id)),
        )
        .chain(
            config
                .entity_resolution
                .iter()
                .map(|endpoint| (endpoint.provider, &endpoint.credential_id)),
        )
        .chain(
            config
                .chronos
                .iter()
                .map(|endpoint| (endpoint.provider, &endpoint.credential_id)),
        )
        .chain(
            config
                .dream
                .iter()
                .map(|endpoint| (endpoint.provider, &endpoint.credential_id)),
        )
        .chain(
            config
                .embedding
                .iter()
                .map(|endpoint| (endpoint.provider, &endpoint.credential_id)),
        )
    {
        let credential = credentials
            .get(id)
            .ok_or_else(|| CredentialError::MissingCredential(id.as_str().into()))?;
        if credential.auth_kind() != provider.auth_kind() {
            return Err(CredentialError::WrongAuthKind(id.as_str().into()));
        }
    }
    Ok(())
}
