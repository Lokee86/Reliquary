use crate::model_config::ModelProvider;
use crate::{
    GeneralEndpoint, GeneralEndpointError, ModelSwitchboard, OpenAiCodexGeneralEndpoint,
    OpenAiReadyGeneralEndpoint,
};
use serde_json::Value;

#[derive(Clone)]
pub enum ConfiguredGeneralEndpoint {
    OpenAiCodex(OpenAiCodexGeneralEndpoint),
    OpenAiReady(OpenAiReadyGeneralEndpoint),
}

impl ConfiguredGeneralEndpoint {
    pub fn from_switchboard(switchboard: &ModelSwitchboard) -> Result<Self, GeneralEndpointError> {
        let provider = switchboard
            .general()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "general route is not configured",
            ))?
            .provider;
        match provider {
            ModelProvider::OpenAiCodex => Ok(Self::OpenAiCodex(
                OpenAiCodexGeneralEndpoint::from_switchboard(switchboard)?,
            )),
            ModelProvider::OpenAiReady => Ok(Self::OpenAiReady(
                OpenAiReadyGeneralEndpoint::from_switchboard(switchboard)?,
            )),
        }
    }

    pub fn from_insomnia_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let provider = switchboard
            .insomnia()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "insomnia and general routes are not configured",
            ))?
            .provider;
        match provider {
            ModelProvider::OpenAiCodex => Ok(Self::OpenAiCodex(
                OpenAiCodexGeneralEndpoint::from_insomnia_switchboard(switchboard)?,
            )),
            ModelProvider::OpenAiReady => Ok(Self::OpenAiReady(
                OpenAiReadyGeneralEndpoint::from_insomnia_switchboard(switchboard)?,
            )),
        }
    }

    pub fn from_insomnia_metadata_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let provider = switchboard
            .insomnia_metadata()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "Insomnia metadata route is not configured",
            ))?
            .provider;
        match provider {
            ModelProvider::OpenAiCodex => Ok(Self::OpenAiCodex(
                OpenAiCodexGeneralEndpoint::from_insomnia_metadata_switchboard(switchboard)?,
            )),
            ModelProvider::OpenAiReady => Ok(Self::OpenAiReady(
                OpenAiReadyGeneralEndpoint::from_insomnia_metadata_switchboard(switchboard)?,
            )),
        }
    }

    pub fn from_entity_extraction_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let provider = switchboard
            .entity_extraction()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "Entity extraction route is not configured",
            ))?
            .provider;
        match provider {
            ModelProvider::OpenAiCodex => Ok(Self::OpenAiCodex(
                OpenAiCodexGeneralEndpoint::from_entity_extraction_switchboard(switchboard)?,
            )),
            ModelProvider::OpenAiReady => Ok(Self::OpenAiReady(
                OpenAiReadyGeneralEndpoint::from_entity_extraction_switchboard(switchboard)?,
            )),
        }
    }

    pub fn from_entity_resolution_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let provider = switchboard
            .entity_resolution()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "Entity resolution route is not configured",
            ))?
            .provider;
        match provider {
            ModelProvider::OpenAiCodex => Ok(Self::OpenAiCodex(
                OpenAiCodexGeneralEndpoint::from_entity_resolution_switchboard(switchboard)?,
            )),
            ModelProvider::OpenAiReady => Ok(Self::OpenAiReady(
                OpenAiReadyGeneralEndpoint::from_entity_resolution_switchboard(switchboard)?,
            )),
        }
    }

    pub fn from_insomnia_ownership_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let provider = switchboard
            .insomnia_ownership()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "Insomnia ownership, metadata, and main routes are not configured",
            ))?
            .provider;
        match provider {
            ModelProvider::OpenAiCodex => Ok(Self::OpenAiCodex(
                OpenAiCodexGeneralEndpoint::from_insomnia_ownership_switchboard(switchboard)?,
            )),
            ModelProvider::OpenAiReady => Ok(Self::OpenAiReady(
                OpenAiReadyGeneralEndpoint::from_insomnia_ownership_switchboard(switchboard)?,
            )),
        }
    }

    pub fn from_chronos_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let provider = switchboard
            .chronos()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "Chronos route is not configured",
            ))?
            .provider;
        match provider {
            ModelProvider::OpenAiCodex => Ok(Self::OpenAiCodex(
                OpenAiCodexGeneralEndpoint::from_chronos_switchboard(switchboard)?,
            )),
            ModelProvider::OpenAiReady => Ok(Self::OpenAiReady(
                OpenAiReadyGeneralEndpoint::from_chronos_switchboard(switchboard)?,
            )),
        }
    }

    pub fn from_dream_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let provider = switchboard
            .dream()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "dream and general routes are not configured",
            ))?
            .provider;
        match provider {
            ModelProvider::OpenAiCodex => Ok(Self::OpenAiCodex(
                OpenAiCodexGeneralEndpoint::from_dream_switchboard(switchboard)?,
            )),
            ModelProvider::OpenAiReady => Ok(Self::OpenAiReady(
                OpenAiReadyGeneralEndpoint::from_dream_switchboard(switchboard)?,
            )),
        }
    }
}

impl GeneralEndpoint for ConfiguredGeneralEndpoint {
    fn model(&self) -> &str {
        match self {
            Self::OpenAiCodex(endpoint) => endpoint.model(),
            Self::OpenAiReady(endpoint) => endpoint.model(),
        }
    }

    fn complete_json(
        &self,
        system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        match self {
            Self::OpenAiCodex(endpoint) => {
                endpoint.complete_json(system_prompt, user_payload, schema_name, schema)
            }
            Self::OpenAiReady(endpoint) => {
                endpoint.complete_json(system_prompt, user_payload, schema_name, schema)
            }
        }
    }
}
