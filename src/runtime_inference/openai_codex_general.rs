use crate::model_config::{ModelProvider, ModelReasoningEffort};
use crate::{GeneralEndpoint, GeneralEndpointError, ModelRequestAuth, ModelSwitchboard};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER, USER_AGENT};
use serde_json::{Value, json};
use std::time::Duration;

pub const OPENAI_CODEX_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
pub const OPENAI_CODEX_COMPAT_VERSION: &str = "0.156.0";
const OPENAI_CODEX_ORIGINATOR: &str = "codex_cli_rs";
const REQUEST_TIMEOUT_SECS: u64 = 180;

#[derive(Clone)]
pub struct OpenAiCodexGeneralEndpoint {
    client: Client,
    url: String,
    model: String,
    reasoning_effort: ModelReasoningEffort,
    auth: ModelRequestAuth,
}

impl OpenAiCodexGeneralEndpoint {
    pub fn from_switchboard(switchboard: &ModelSwitchboard) -> Result<Self, GeneralEndpointError> {
        let route = switchboard
            .general()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "general route is not configured",
            ))?;
        let auth = switchboard
            .general_auth()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "general auth is missing",
            ))?;
        Self::from_route(route, auth)
    }

    pub fn from_insomnia_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let route = switchboard
            .insomnia()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "insomnia and general routes are not configured",
            ))?;
        let auth =
            switchboard
                .insomnia_auth()
                .ok_or(GeneralEndpointError::InvalidConfiguration(
                    "insomnia auth is missing",
                ))?;
        Self::from_route(route, auth)
    }

    pub fn from_insomnia_metadata_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let route =
            switchboard
                .insomnia_metadata()
                .ok_or(GeneralEndpointError::InvalidConfiguration(
                    "Insomnia metadata route is not configured",
                ))?;
        let auth = switchboard.insomnia_metadata_auth().ok_or(
            GeneralEndpointError::InvalidConfiguration("Insomnia metadata auth is missing"),
        )?;
        Self::from_route(route, auth)
    }

    pub fn from_entity_extraction_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let route =
            switchboard
                .entity_extraction()
                .ok_or(GeneralEndpointError::InvalidConfiguration(
                    "Entity extraction route is not configured",
                ))?;
        let auth = switchboard.entity_extraction_auth().ok_or(
            GeneralEndpointError::InvalidConfiguration("Entity extraction auth is missing"),
        )?;
        Self::from_route(route, auth)
    }

    pub fn from_entity_resolution_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let route =
            switchboard
                .entity_resolution()
                .ok_or(GeneralEndpointError::InvalidConfiguration(
                    "Entity resolution route is not configured",
                ))?;
        let auth = switchboard.entity_resolution_auth().ok_or(
            GeneralEndpointError::InvalidConfiguration("Entity resolution auth is missing"),
        )?;
        Self::from_route(route, auth)
    }

    pub fn from_insomnia_ownership_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let route =
            switchboard
                .insomnia_ownership()
                .ok_or(GeneralEndpointError::InvalidConfiguration(
                    "Insomnia ownership, metadata, and main routes are not configured",
                ))?;
        let auth = switchboard.insomnia_ownership_auth().ok_or(
            GeneralEndpointError::InvalidConfiguration("Insomnia ownership auth is missing"),
        )?;
        Self::from_route(route, auth)
    }

    pub fn from_chronos_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let route = switchboard
            .chronos()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "Chronos route is not configured",
            ))?;
        let auth = switchboard
            .chronos_auth()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "Chronos auth is missing",
            ))?;
        Self::from_route(route, auth)
    }

    pub fn from_dream_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, GeneralEndpointError> {
        let route = switchboard
            .dream()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "dream and general routes are not configured",
            ))?;
        let auth = switchboard
            .dream_auth()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "dream auth is missing",
            ))?;
        Self::from_route(route, auth)
    }

    fn from_route(
        route: &crate::GeneralModelEndpoint,
        auth: ModelRequestAuth,
    ) -> Result<Self, GeneralEndpointError> {
        if route.provider != ModelProvider::OpenAiCodex {
            return Err(GeneralEndpointError::InvalidConfiguration(
                "general route is not OpenAI Codex",
            ));
        }
        if auth.chatgpt_account_id.is_none() {
            return Err(GeneralEndpointError::InvalidConfiguration(
                "ChatGPT account ID is missing",
            ));
        }
        let reasoning_effort =
            route
                .reasoning_effort
                .ok_or(GeneralEndpointError::InvalidConfiguration(
                    "Codex reasoning effort is not configured",
                ))?;
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|error| GeneralEndpointError::Failure(error.to_string()))?;
        Ok(Self {
            client,
            url: OPENAI_CODEX_RESPONSES_URL.into(),
            model: route.model.clone(),
            reasoning_effort,
            auth,
        })
    }
}

impl GeneralEndpoint for OpenAiCodexGeneralEndpoint {
    fn model(&self) -> &str {
        &self.model
    }

    fn complete_json(
        &self,
        system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        let body = build_request_body(
            &self.model,
            self.reasoning_effort,
            system_prompt,
            user_payload,
            schema_name,
            schema,
        );
        let payload = serde_json::to_vec(&body)
            .map_err(|error| GeneralEndpointError::Failure(error.to_string()))?;
        let account_id = self.auth.chatgpt_account_id.as_deref().ok_or(
            GeneralEndpointError::InvalidConfiguration("ChatGPT account ID is missing"),
        )?;
        let mut request = self
            .client
            .post(&self.url)
            .header(AUTHORIZATION, self.auth.authorization_header())
            .header("ChatGPT-Account-ID", account_id)
            .header("originator", OPENAI_CODEX_ORIGINATOR)
            .header("version", OPENAI_CODEX_COMPAT_VERSION)
            .header(
                USER_AGENT,
                format!("codex_cli_rs/{OPENAI_CODEX_COMPAT_VERSION}"),
            )
            .header(ACCEPT, "text/event-stream")
            .header(CONTENT_TYPE, "application/json");
        if uses_responses_lite(&self.model) {
            request = request.header("x-openai-internal-codex-responses-lite", "true");
        }
        let response = request
            .body(payload)
            .send()
            .map_err(|error| GeneralEndpointError::Failure(error.to_string()))?;
        let status = response.status();
        let retry_after = retry_after_header(response.headers());
        let text = response
            .text()
            .map_err(|error| GeneralEndpointError::Failure(error.to_string()))?;
        if status.as_u16() == 429 {
            return Err(GeneralEndpointError::backpressure(
                format!("HTTP {status}: {}", truncate(&text, 2048)),
                retry_after.or_else(|| retry_after_body(&text)),
            ));
        }
        if !status.is_success() {
            return Err(GeneralEndpointError::Failure(format!(
                "HTTP {status}: {}",
                truncate(&text, 2048)
            )));
        }
        parse_sse_json(&text)
    }
}

pub(crate) fn build_request_body(
    model: &str,
    reasoning_effort: ModelReasoningEffort,
    system_prompt: &str,
    user_payload: &str,
    schema_name: &str,
    schema: &Value,
) -> Value {
    let mut reasoning = json!({
        "effort": reasoning_effort.as_str(),
        "summary": "auto"
    });
    if uses_responses_lite(model) {
        reasoning["context"] = json!("all_turns");
    }
    let mut body = json!({
        "model": model,
        "instructions": system_prompt,
        "input": [{
            "role": "user",
            "content": [{"type": "input_text", "text": user_payload}]
        }],
        "parallel_tool_calls": false,
        "reasoning": reasoning,
        "store": false,
        "stream": true,
        "include": [],
        "text": {
            "verbosity": "low",
            "format": {
                "type": "json_schema",
                "name": schema_name,
                "strict": true,
                "schema": schema
            }
        }
    });
    if uses_responses_lite(model) {
        body["service_tier"] = json!("priority");
    }
    body
}

pub(crate) fn parse_sse_json(stream: &str) -> Result<Value, GeneralEndpointError> {
    let mut output = String::new();
    let mut completed = false;
    let mut data = String::new();
    for line in stream.lines().chain(std::iter::once("")) {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            if !data.is_empty() {
                process_event(&data, &mut output, &mut completed)?;
                data.clear();
            }
            continue;
        }
        if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.trim_start());
        }
    }
    if !completed {
        return Err(GeneralEndpointError::InvalidResponse(
            "Codex stream ended before response.completed",
        ));
    }
    if output.trim().is_empty() {
        return Err(GeneralEndpointError::InvalidResponse(
            "Codex response contained no output text",
        ));
    }
    serde_json::from_str(output.trim())
        .map_err(|error| GeneralEndpointError::Failure(format!("invalid structured JSON: {error}")))
}

fn process_event(
    data: &str,
    output: &mut String,
    completed: &mut bool,
) -> Result<(), GeneralEndpointError> {
    if data == "[DONE]" {
        return Ok(());
    }
    let event: Value = serde_json::from_str(data)
        .map_err(|error| GeneralEndpointError::Failure(format!("invalid Codex SSE: {error}")))?;
    match event.get("type").and_then(Value::as_str) {
        Some("response.output_text.delta") => {
            if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                output.push_str(delta);
            }
        }
        Some("response.completed") => *completed = true,
        Some("response.failed") | Some("response.incomplete") => {
            return Err(GeneralEndpointError::Failure(format!(
                "Codex response failed: {}",
                truncate(data, 2048)
            )));
        }
        _ => {}
    }
    Ok(())
}

fn uses_responses_lite(model: &str) -> bool {
    model.starts_with("gpt-5.6-") || model.starts_with("gpt-6-")
}

fn retry_after_header(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let seconds = headers
        .get(RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    Some(Duration::from_secs(seconds))
}

fn retry_after_body(body: &str) -> Option<Duration> {
    let value: Value = serde_json::from_str(body).ok()?;
    let seconds = value
        .pointer("/error/resets_in_seconds")
        .and_then(Value::as_u64)
        .or_else(|| value.get("resets_in_seconds").and_then(Value::as_u64))?;
    Some(Duration::from_secs(seconds))
}

fn truncate(value: &str, max: usize) -> &str {
    value.get(..value.len().min(max)).unwrap_or(value)
}

#[cfg(test)]
#[path = "openai_codex_general_seam_tests.rs"]
mod seam_tests;
