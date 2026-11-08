use crate::*;

pub(super) const GENERAL: &str = "general";
pub(super) const SHARED: &str = "decision";
pub(super) const INSOMNIA: &str = "insomnia";
pub(super) const INSOMNIA_METADATA: &str = "insomnia_metadata";
pub(super) const ENTITY_EXTRACTION: &str = "entity_extraction";
pub(super) const DREAM: &str = "dream";
pub(super) const ENTITY: &str = "entity_resolution";
pub(super) const ENTITY_DECISION: &str = "entity_resolution_decision";
pub(super) const RETRIEVAL: &str = "memory_retrieval_decision";
pub(super) const EMBEDDING: &str = "embedding";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelSlot {
    General,
    Insomnia,
    InsomniaMetadata,
    EntityExtraction,
    GeneralDecision,
    EntityResolutionDecision,
    MemoryRetrievalDecision,
    EntityResolution,
    Dream,
    Embedding,
}

impl ModelSlot {
    pub const ALL: [Self; 10] = [
        Self::General,
        Self::Insomnia,
        Self::InsomniaMetadata,
        Self::EntityExtraction,
        Self::GeneralDecision,
        Self::EntityResolutionDecision,
        Self::MemoryRetrievalDecision,
        Self::EntityResolution,
        Self::Dream,
        Self::Embedding,
    ];

    pub fn object_key(self) -> &'static str {
        match self {
            Self::General => "models.general",
            Self::Insomnia => "models.insomnia",
            Self::InsomniaMetadata => "models.insomnia_metadata",
            Self::EntityExtraction => "models.entity_extraction",
            Self::GeneralDecision => "models.general_decision",
            Self::EntityResolutionDecision => "models.entity_resolution_decision",
            Self::MemoryRetrievalDecision => "models.memory_retrieval_decision",
            Self::EntityResolution => "models.entity_resolution",
            Self::Dream => "models.dream",
            Self::Embedding => "models.embedding",
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::GeneralDecision => SHARED,
            _ => self.object_key().strip_prefix("models.").unwrap(),
        }
    }

    pub fn is_decision(self) -> bool {
        matches!(
            self,
            Self::GeneralDecision | Self::EntityResolutionDecision | Self::MemoryRetrievalDecision
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralRoute {
    pub endpoint: GeneralModelEndpoint,
    pub reasoning_effort: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelRoute {
    General(GeneralRoute),
    Decision(DecisionModelEndpoint),
    Embedding(EmbeddingModelEndpoint),
}

impl ModelRoute {
    pub(super) fn binding(&self) -> (&CredentialId, ModelProvider) {
        match self {
            Self::General(route) => (&route.endpoint.credential_id, route.endpoint.provider),
            Self::Decision(route) => (&route.credential_id, ModelProvider::OpenAiReady),
            Self::Embedding(route) => (&route.credential_id, route.provider),
        }
    }
}

#[derive(Default)]
pub(super) struct RuntimeConfig {
    pub general: Option<GeneralRoute>,
    pub insomnia: Option<GeneralRoute>,
    pub insomnia_metadata: Option<GeneralRoute>,
    pub entity_extraction: Option<GeneralRoute>,
    pub dream: Option<GeneralRoute>,
    pub entity_resolution: Option<GeneralRoute>,
    pub general_decision: Option<DecisionModelEndpoint>,
    pub entity_resolution_decision: Option<DecisionModelEndpoint>,
    pub memory_retrieval_decision: Option<DecisionModelEndpoint>,
    pub embedding: Option<EmbeddingModelEndpoint>,
}

pub(super) fn invalid(route: &'static str, reason: &'static str) -> ConfiguredRuntimeError {
    ConfiguredRuntimeError::Invalid { route, reason }
}
