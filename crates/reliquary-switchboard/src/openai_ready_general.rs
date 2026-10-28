use crate::{
    GeneralEndpoint, GeneralEndpointError, ModelProvider, ModelReasoningEffort, ModelRequestAuth,
    ModelSwitchboard,
};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER};
use serde_json::{Value, json};
use std::thread::sleep;
use std::time::Duration;

const REQUEST_TIMEOUT_SECS: u64 = 180;
const NOUS_REQUEST_TIMEOUT_SECS: u64 = 240;
const BAI_REQUEST_TIMEOUT_SECS: u64 = 600;
const MAX_REQUEST_ATTEMPTS: usize = 4;

#[derive(Clone)]
pub struct OpenAiReadyGeneralEndpoint {
    client: Client,
    url: String,
    model: String,
    auth: ModelRequestAuth,
    structured_mode: StructuredMode,
    reasoning_effort: Option<ModelReasoningEffort>,
    stream_tool_calls: bool,
}

#[derive(Clone, Copy)]
enum StructuredMode {
    ResponseFormat,
    ForcedTool,
}

impl OpenAiReadyGeneralEndpoint {
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
        Self::from_route(route, auth, StructuredMode::ResponseFormat)
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
        Self::from_route(route, auth, StructuredMode::ForcedTool)
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
        Self::from_route(route, auth, StructuredMode::ForcedTool)
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
        Self::from_route(route, auth, StructuredMode::ForcedTool)
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
        Self::from_route(route, auth, StructuredMode::ForcedTool)
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
        Self::from_route(route, auth, StructuredMode::ForcedTool)
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
        Self::from_route(route, auth, StructuredMode::ForcedTool)
    }

    fn from_route(
        route: &crate::GeneralModelEndpoint,
        auth: ModelRequestAuth,
        structured_mode: StructuredMode,
    ) -> Result<Self, GeneralEndpointError> {
        if route.provider != ModelProvider::OpenAiReady {
            return Err(GeneralEndpointError::InvalidConfiguration(
                "general route is not OpenAI-ready",
            ));
        }
        let url = route
            .url
            .clone()
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "general URL is missing",
            ))?;
        let stream_tool_calls = url.contains("inference-api.nousresearch.com");
        let request_timeout = if stream_tool_calls {
            NOUS_REQUEST_TIMEOUT_SECS
        } else if url.contains("api.b.ai") {
            BAI_REQUEST_TIMEOUT_SECS
        } else {
            REQUEST_TIMEOUT_SECS
        };
        let client = Client::builder()
            .timeout(Duration::from_secs(request_timeout))
            .build()
            .map_err(|error| GeneralEndpointError::Failure(error.to_string()))?;
        Ok(Self {
            client,
            url,
            model: route.model.clone(),
            auth,
            structured_mode,
            reasoning_effort: route.reasoning_effort,
            stream_tool_calls,
        })
    }

    fn send_with_retry(&self, payload: &[u8]) -> Result<Vec<u8>, GeneralEndpointError> {
        for attempt in 0..MAX_REQUEST_ATTEMPTS {
            let mut request = self
                .client
                .post(&self.url)
                .header(AUTHORIZATION, self.auth.authorization_header())
                .header(CONTENT_TYPE, "application/json");
            if self.stream_tool_calls && matches!(self.structured_mode, StructuredMode::ForcedTool)
            {
                request = request.header(ACCEPT, "text/event-stream");
            }
            let response = request.body(payload.to_vec()).send();
            let response = match response {
                Ok(response) => response,
                Err(error) if attempt + 1 < MAX_REQUEST_ATTEMPTS && retryable_error(&error) => {
                    sleep(retry_delay(attempt));
                    continue;
                }
                Err(error) => return Err(GeneralEndpointError::Failure(error.to_string())),
            };
            let status = response.status();
            let retry_after = retry_after_header(response.headers());
            let bytes = response
                .bytes()
                .map_err(|error| GeneralEndpointError::Failure(error.to_string()))?
                .to_vec();
            if status.is_success() {
                return Ok(bytes);
            }
            if status.as_u16() == 429 {
                let body = String::from_utf8_lossy(&bytes);
                return Err(GeneralEndpointError::backpressure(
                    format!("HTTP {status}: {}", truncate(&body, 1024)),
                    retry_after.or_else(|| retry_after_body(&body)),
                ));
            }
            if attempt + 1 < MAX_REQUEST_ATTEMPTS && retryable_status(status.as_u16()) {
                sleep(retry_delay(attempt));
                continue;
            }
            return Err(GeneralEndpointError::Failure(format!(
                "HTTP {status}: {}",
                truncate(&String::from_utf8_lossy(&bytes), 1024)
            )));
        }
        Err(GeneralEndpointError::Failure(
            "request retries exhausted".into(),
        ))
    }
}

impl GeneralEndpoint for OpenAiReadyGeneralEndpoint {
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
        let mut body = match self.structured_mode {
            StructuredMode::ResponseFormat => json!({
                "model": self.model,
                "temperature": 0,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_payload}
                ],
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {
                        "name": schema_name,
                        "strict": true,
                        "schema": schema
                    }
                }
            }),
            StructuredMode::ForcedTool => json!({
                "model": self.model,
                "temperature": 0,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_payload}
                ],
                "tools": [{
                    "type": "function",
                    "function": {
                        "name": schema_name,
                        "description": "Return the required structured result.",
                        "parameters": schema,
                        "strict": true
                    }
                }],
                "tool_choice": {
                    "type": "function",
                    "function": {"name": schema_name}
                },
                "parallel_tool_calls": false
            }),
        };
        if let Some(reasoning_effort) = self.reasoning_effort {
            body["reasoning_effort"] = json!(reasoning_effort.as_str());
        }
        if self.stream_tool_calls && matches!(self.structured_mode, StructuredMode::ForcedTool) {
            body.as_object_mut().unwrap().remove("temperature");
            body["stream"] = json!(true);
            body["include_reasoning"] = json!(true);
            if self.reasoning_effort.is_none() {
                body["reasoning_effort"] = json!(ModelReasoningEffort::Low.as_str());
            }
        }
        let payload = serde_json::to_vec(&body)
            .map_err(|error| GeneralEndpointError::Failure(error.to_string()))?;
        let bytes = self.send_with_retry(&payload)?;
        if self.stream_tool_calls && matches!(self.structured_mode, StructuredMode::ForcedTool) {
            return parse_streamed_forced_tool_result(
                &String::from_utf8_lossy(&bytes),
                schema_name,
            );
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| GeneralEndpointError::Failure(error.to_string()))?;
        match self.structured_mode {
            StructuredMode::ResponseFormat => {
                let content = value
                    .pointer("/choices/0/message/content")
                    .and_then(Value::as_str)
                    .ok_or(GeneralEndpointError::InvalidResponse(
                        "missing choices[0].message.content",
                    ))?;
                parse_structured_content(content)
            }
            StructuredMode::ForcedTool => parse_forced_tool_result(&value, schema_name),
        }
    }
}

pub(crate) fn parse_streamed_forced_tool_result(
    stream: &str,
    expected_name: &str,
) -> Result<Value, GeneralEndpointError> {
    if let Ok(value) = serde_json::from_str::<Value>(stream.trim()) {
        return parse_forced_tool_result(&value, expected_name);
    }

    let mut call_index: Option<usize> = None;
    let mut call_name: Option<String> = None;
    let mut arguments = String::new();
    for line in stream.lines() {
        let Some(data) = line.trim_end_matches('\r').strip_prefix("data:") else {
            continue;
        };
        let data = data.trim_start();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let chunk: Value = serde_json::from_str(data).map_err(|error| {
            GeneralEndpointError::Failure(format!("invalid streamed JSON: {error}"))
        })?;
        if let Some(error) = chunk.get("error") {
            return Err(GeneralEndpointError::Failure(format!(
                "streamed endpoint error: {}",
                truncate(&error.to_string(), 1024)
            )));
        }
        let Some(choices) = chunk.get("choices").and_then(Value::as_array) else {
            continue;
        };
        for choice in choices {
            let Some(calls) = choice
                .get("delta")
                .and_then(|delta| delta.get("tool_calls"))
                .and_then(Value::as_array)
            else {
                continue;
            };
            for call in calls {
                let index = call.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                match call_index {
                    Some(existing) if existing != index => {
                        return Err(GeneralEndpointError::InvalidResponse(
                            "streamed structured response contained multiple tool calls",
                        ));
                    }
                    None => call_index = Some(index),
                    _ => {}
                }
                if let Some(function) = call.get("function") {
                    if let Some(name) = function.get("name").and_then(Value::as_str) {
                        match &call_name {
                            Some(existing) if existing != name => {
                                return Err(GeneralEndpointError::InvalidResponse(
                                    "streamed tool call function name changed",
                                ));
                            }
                            None => call_name = Some(name.to_owned()),
                            _ => {}
                        }
                    }
                    if let Some(part) = function.get("arguments").and_then(Value::as_str) {
                        arguments.push_str(part);
                    }
                }
            }
        }
    }

    if call_index.is_none() {
        return Err(GeneralEndpointError::InvalidResponse(
            "stream contained no tool call",
        ));
    }
    if call_name.as_deref() != Some(expected_name) {
        return Err(GeneralEndpointError::InvalidResponse(
            "streamed tool call function name does not match requested schema",
        ));
    }
    if arguments.trim().is_empty() {
        return Err(GeneralEndpointError::InvalidResponse(
            "streamed tool call contained no arguments",
        ));
    }
    serde_json::from_str(&arguments).map_err(|error| {
        GeneralEndpointError::Failure(format!(
            "invalid streamed tool-call arguments JSON: {error}; preview={:?}",
            truncate(&arguments, 512)
        ))
    })
}

pub(crate) fn parse_forced_tool_result(
    response: &Value,
    expected_name: &str,
) -> Result<Value, GeneralEndpointError> {
    let calls = response
        .pointer("/choices/0/message/tool_calls")
        .and_then(Value::as_array)
        .ok_or(GeneralEndpointError::InvalidResponse(
            "missing choices[0].message.tool_calls",
        ))?;
    if calls.len() != 1 {
        return Err(GeneralEndpointError::InvalidResponse(
            "forced structured response must contain exactly one tool call",
        ));
    }
    let function = calls[0]
        .get("function")
        .ok_or(GeneralEndpointError::InvalidResponse(
            "tool call is missing function",
        ))?;
    let name = function.get("name").and_then(Value::as_str).ok_or(
        GeneralEndpointError::InvalidResponse("tool call function is missing name"),
    )?;
    if name != expected_name {
        return Err(GeneralEndpointError::InvalidResponse(
            "tool call function name does not match requested schema",
        ));
    }
    let arguments = function.get("arguments").and_then(Value::as_str).ok_or(
        GeneralEndpointError::InvalidResponse("tool call function is missing string arguments"),
    )?;
    serde_json::from_str(arguments).map_err(|error| {
        GeneralEndpointError::Failure(format!(
            "invalid tool-call arguments JSON: {error}; preview={:?}",
            truncate(arguments, 512)
        ))
    })
}

pub(crate) fn parse_structured_content(content: &str) -> Result<Value, GeneralEndpointError> {
    let trimmed = content.trim();
    let candidate = if trimmed.starts_with("```") {
        let first_newline = trimmed.find('\n');
        let closing = trimmed.rfind("```");
        match (first_newline, closing) {
            (Some(first), Some(last)) if last > first => trimmed[first + 1..last].trim(),
            _ => trimmed,
        }
    } else {
        trimmed
    };
    serde_json::from_str(candidate).map_err(|error| {
        GeneralEndpointError::Failure(format!(
            "invalid structured JSON: {error}; preview={:?}",
            truncate(content, 512)
        ))
    })
}

fn retryable_error(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout()
}

fn retryable_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 500 | 502 | 503 | 504 | 524)
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

fn retry_delay(attempt: usize) -> Duration {
    Duration::from_secs(2_u64.saturating_pow(attempt as u32).min(30))
}

fn truncate(value: &str, max: usize) -> &str {
    value.get(..value.len().min(max)).unwrap_or(value)
}

#[cfg(test)]
#[path = "openai_ready_general_seam_tests.rs"]
mod seam_tests;
