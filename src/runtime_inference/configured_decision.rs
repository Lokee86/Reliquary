use crate::{DecisionEndpoint, DecisionEndpointError, ModelRequestAuth, ModelSwitchboard};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::time::Duration;

const REQUEST_TIMEOUT_SECS: u64 = 30;

#[derive(Clone)]
pub struct ConfiguredDecisionEndpoint {
    client: Client,
    model: String,
    url: String,
    auth: ModelRequestAuth,
}

impl ConfiguredDecisionEndpoint {
    fn from_route(
        endpoint: Option<&crate::DecisionModelEndpoint>,
        auth: Option<ModelRequestAuth>,
        route: &'static str,
    ) -> Result<Self, DecisionEndpointError> {
        let endpoint = endpoint.ok_or(DecisionEndpointError::InvalidConfiguration(route))?;
        let auth = auth.ok_or(DecisionEndpointError::InvalidConfiguration(route))?;
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|error| DecisionEndpointError::Failure(error.to_string()))?;
        Ok(Self {
            client,
            model: endpoint.model.clone(),
            url: endpoint.url.clone(),
            auth,
        })
    }

    pub fn from_entity_resolution_decision_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, DecisionEndpointError> {
        Self::from_route(
            switchboard.entity_resolution_decision(),
            switchboard.entity_resolution_decision_auth(),
            "Entity resolution decision route is not configured",
        )
    }

    pub fn from_memory_retrieval_decision_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, DecisionEndpointError> {
        Self::from_route(
            switchboard.memory_retrieval_decision(),
            switchboard.memory_retrieval_decision_auth(),
            "Memory retrieval decision route is not configured",
        )
    }
}

impl DecisionEndpoint for ConfiguredDecisionEndpoint {
    fn model(&self) -> &str {
        &self.model
    }

    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError> {
        let response = self
            .client
            .post(&self.url)
            .header("Authorization", self.auth.authorization_header())
            .header("Content-Type", "application/json")
            .json(&json!({
                "model": self.model,
                "state": state,
                "questions": questions,
            }))
            .send()
            .map_err(|error| DecisionEndpointError::Failure(error.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| DecisionEndpointError::Failure(error.to_string()))?;
        if !status.is_success() {
            return Err(DecisionEndpointError::Failure(format!(
                "HTTP {status}: {body}"
            )));
        }
        serde_json::from_str(&body).map_err(|error| {
            DecisionEndpointError::Failure(format!("response JSON failed: {error}: {body}"))
        })
    }
}
