use crate::{
    ConfigError, ConfiguredDecisionEndpoint, ConfiguredGeneralEndpoint, ModelSwitchboard,
    OpenAiReadyEmbeddingEndpoint, ReliquaryConfig, ReliquaryRuntimeRoutes,
};
use std::path::Path;
use std::sync::Arc;

#[path = "configured_runtime_historical.rs"]
mod historical;
#[path = "configured_runtime_insomnia.rs"]
mod insomnia;
#[path = "configured_runtime_vectors.rs"]
mod vectors;

pub use insomnia::{ConfiguredInsomniaOptions, ConfiguredInsomniaReport, InsomniaTerminalFailure};
pub use vectors::{ArchiveVectorBuildReport, EmbeddingProbeReport};

#[derive(Debug)]
pub struct ConfiguredRuntimeError(String);

impl std::fmt::Display for ConfiguredRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfiguredRuntimeError {}

impl From<ConfigError> for ConfiguredRuntimeError {
    fn from(value: ConfigError) -> Self {
        Self(value.to_string())
    }
}

pub struct ConfiguredRuntime {
    switchboard: ModelSwitchboard,
}

impl ConfiguredRuntime {
    pub fn open(path: &Path) -> Result<Self, ConfiguredRuntimeError> {
        let config = ReliquaryConfig::open(path)?;
        let switchboard = ModelSwitchboard::new(config.models, config.credentials)?;
        Ok(Self { switchboard })
    }

    pub fn runtime_routes(&self) -> Result<ReliquaryRuntimeRoutes, ConfiguredRuntimeError> {
        let config = self.switchboard.config();
        let general = config
            .general
            .as_ref()
            .map(|_| ConfiguredGeneralEndpoint::from_switchboard(&self.switchboard))
            .transpose()
            .map_err(operation)?
            .map(|endpoint| Arc::new(endpoint) as Arc<dyn crate::GeneralEndpoint>);
        let insomnia = config
            .insomnia
            .as_ref()
            .map(|_| ConfiguredGeneralEndpoint::from_insomnia_switchboard(&self.switchboard))
            .transpose()
            .map_err(operation)?
            .map(|endpoint| Arc::new(endpoint) as Arc<dyn crate::GeneralEndpoint>);
        let insomnia_metadata = config
            .insomnia_metadata
            .as_ref()
            .map(|_| {
                ConfiguredGeneralEndpoint::from_insomnia_metadata_switchboard(&self.switchboard)
            })
            .transpose()
            .map_err(operation)?
            .map(|endpoint| Arc::new(endpoint) as Arc<dyn crate::GeneralEndpoint>);
        let entity_extraction = config
            .entity_extraction
            .as_ref()
            .map(|_| {
                ConfiguredGeneralEndpoint::from_entity_extraction_switchboard(&self.switchboard)
            })
            .transpose()
            .map_err(operation)?
            .map(|endpoint| Arc::new(endpoint) as Arc<dyn crate::GeneralEndpoint>);
        let entity_resolution_decision = config
            .entity_resolution_decision
            .as_ref()
            .map(|_| {
                ConfiguredDecisionEndpoint::from_entity_resolution_decision_switchboard(
                    &self.switchboard,
                )
            })
            .transpose()
            .map_err(operation)?
            .map(|endpoint| Arc::new(endpoint) as Arc<dyn crate::DecisionEndpoint>);
        let memory_retrieval_decision = config
            .memory_retrieval_decision
            .as_ref()
            .map(|_| {
                ConfiguredDecisionEndpoint::from_memory_retrieval_decision_switchboard(
                    &self.switchboard,
                )
            })
            .transpose()
            .map_err(operation)?
            .map(|endpoint| Arc::new(endpoint) as Arc<dyn crate::DecisionEndpoint>);
        let entity_resolution = config
            .entity_resolution
            .as_ref()
            .map(|_| {
                ConfiguredGeneralEndpoint::from_entity_resolution_switchboard(&self.switchboard)
            })
            .transpose()
            .map_err(operation)?
            .map(|endpoint| Arc::new(endpoint) as Arc<dyn crate::GeneralEndpoint>);
        let chronos = config
            .chronos
            .as_ref()
            .map(|_| ConfiguredGeneralEndpoint::from_chronos_switchboard(&self.switchboard))
            .transpose()
            .map_err(operation)?
            .map(|endpoint| Arc::new(endpoint) as Arc<dyn crate::GeneralEndpoint>);
        let dream = config
            .dream
            .as_ref()
            .map(|_| ConfiguredGeneralEndpoint::from_dream_switchboard(&self.switchboard))
            .transpose()
            .map_err(operation)?
            .map(|endpoint| Arc::new(endpoint) as Arc<dyn crate::GeneralEndpoint>);
        let embedding = config
            .embedding
            .as_ref()
            .map(|_| OpenAiReadyEmbeddingEndpoint::from_switchboard(&self.switchboard))
            .transpose()
            .map_err(operation)?
            .map(|endpoint| Arc::new(endpoint) as Arc<dyn crate::EmbeddingEndpoint + Send + Sync>);
        Ok(
            ReliquaryRuntimeRoutes::new(general, insomnia, insomnia_metadata, dream, embedding)
                .with_entity_routes(entity_extraction, entity_resolution)
                .with_entity_resolution_decision(entity_resolution_decision)
                .with_memory_retrieval_decision(memory_retrieval_decision)
                .with_chronos(chronos),
        )
    }
}

pub(super) fn operation(error: impl std::fmt::Display) -> ConfiguredRuntimeError {
    ConfiguredRuntimeError(error.to_string())
}
