use crate::model_auth::{resolve_api_key_auth, resolve_auth};
use crate::model_config::{
    DecisionModelEndpoint, EmbeddingModelEndpoint, GeneralModelEndpoint, ModelProvider,
    ModelSwitchboardConfig, validate_credentials, validate_switchboard,
};
use crate::{ConfigError, CredentialsConfig, ModelRequestAuth};

#[derive(Clone, Debug)]
pub struct ModelSwitchboard {
    config: ModelSwitchboardConfig,
    credentials: CredentialsConfig,
}

impl ModelSwitchboard {
    pub fn new(
        config: ModelSwitchboardConfig,
        credentials: CredentialsConfig,
    ) -> Result<Self, ConfigError> {
        validate_switchboard(&config)?;
        validate_credentials(&config, &credentials)?;
        Ok(Self {
            config,
            credentials,
        })
    }

    pub fn general(&self) -> Option<&GeneralModelEndpoint> {
        self.config.general.as_ref()
    }

    pub fn insomnia(&self) -> Option<&GeneralModelEndpoint> {
        self.config
            .insomnia
            .as_ref()
            .or(self.config.general.as_ref())
    }

    pub fn insomnia_metadata(&self) -> Option<&GeneralModelEndpoint> {
        self.config.insomnia_metadata.as_ref()
    }

    pub fn entity_extraction(&self) -> Option<&GeneralModelEndpoint> {
        self.config.entity_extraction.as_ref()
    }

    pub fn entity_resolution_decision(&self) -> Option<&DecisionModelEndpoint> {
        self.config.entity_resolution_decision.as_ref()
    }

    pub fn general_decision(&self) -> Option<&DecisionModelEndpoint> {
        self.config.general_decision.as_ref()
    }

    pub fn memory_retrieval_decision(&self) -> Option<&DecisionModelEndpoint> {
        self.config.memory_retrieval_decision.as_ref()
    }

    pub fn entity_resolution(&self) -> Option<&GeneralModelEndpoint> {
        self.config.entity_resolution.as_ref()
    }

    pub fn insomnia_ownership(&self) -> Option<&GeneralModelEndpoint> {
        self.insomnia_metadata().or_else(|| self.insomnia())
    }

    pub fn chronos(&self) -> Option<&GeneralModelEndpoint> {
        self.config.chronos.as_ref()
    }

    pub fn dream(&self) -> Option<&GeneralModelEndpoint> {
        self.config.dream.as_ref().or(self.config.general.as_ref())
    }

    pub fn embedding(&self) -> Option<&EmbeddingModelEndpoint> {
        self.config.embedding.as_ref()
    }

    pub fn general_auth(&self) -> Option<ModelRequestAuth> {
        self.config.general.as_ref().map(|endpoint| {
            resolve_auth(
                endpoint.provider,
                &endpoint.credential_id,
                &self.credentials,
            )
        })
    }

    pub fn insomnia_auth(&self) -> Option<ModelRequestAuth> {
        self.insomnia().map(|endpoint| {
            resolve_auth(
                endpoint.provider,
                &endpoint.credential_id,
                &self.credentials,
            )
        })
    }

    pub fn insomnia_metadata_auth(&self) -> Option<ModelRequestAuth> {
        self.insomnia_metadata().map(|endpoint| {
            resolve_auth(
                endpoint.provider,
                &endpoint.credential_id,
                &self.credentials,
            )
        })
    }

    pub fn entity_extraction_auth(&self) -> Option<ModelRequestAuth> {
        self.entity_extraction()
            .map(|e| resolve_auth(e.provider, &e.credential_id, &self.credentials))
    }

    pub fn entity_resolution_decision_auth(&self) -> Option<ModelRequestAuth> {
        self.entity_resolution_decision()
            .map(|e| resolve_api_key_auth(&e.credential_id, &self.credentials))
    }

    pub fn general_decision_auth(&self) -> Option<ModelRequestAuth> {
        self.general_decision()
            .map(|e| resolve_api_key_auth(&e.credential_id, &self.credentials))
    }

    pub fn memory_retrieval_decision_auth(&self) -> Option<ModelRequestAuth> {
        self.memory_retrieval_decision()
            .map(|e| resolve_api_key_auth(&e.credential_id, &self.credentials))
    }

    pub fn entity_resolution_auth(&self) -> Option<ModelRequestAuth> {
        self.entity_resolution()
            .map(|e| resolve_auth(e.provider, &e.credential_id, &self.credentials))
    }

    pub fn insomnia_ownership_auth(&self) -> Option<ModelRequestAuth> {
        self.insomnia_ownership().map(|endpoint| {
            resolve_auth(
                endpoint.provider,
                &endpoint.credential_id,
                &self.credentials,
            )
        })
    }

    pub fn chronos_auth(&self) -> Option<ModelRequestAuth> {
        self.chronos().map(|endpoint| {
            resolve_auth(
                endpoint.provider,
                &endpoint.credential_id,
                &self.credentials,
            )
        })
    }

    pub fn dream_auth(&self) -> Option<ModelRequestAuth> {
        self.dream().map(|endpoint| {
            resolve_auth(
                endpoint.provider,
                &endpoint.credential_id,
                &self.credentials,
            )
        })
    }

    pub fn embedding_auth(&self) -> Option<ModelRequestAuth> {
        self.config.embedding.as_ref().map(|endpoint| {
            resolve_auth(
                endpoint.provider,
                &endpoint.credential_id,
                &self.credentials,
            )
        })
    }

    pub fn config(&self) -> &ModelSwitchboardConfig {
        &self.config
    }
}
