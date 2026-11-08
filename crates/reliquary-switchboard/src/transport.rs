use super::*;
use std::io::Read;

const CODEX_COMPAT_VERSION: &str = "0.156.0";

/// OpenAI-compatible embedding provider configured from an explicit route.
#[derive(Clone)]
pub struct ConfiguredEmbeddingEndpoint {
    client: reqwest::blocking::Client,
    url: reqwest::Url,
    model: String,
    dimensions: u32,
    normalization: EmbeddingNormalization,
    api_key: SecretString,
}

impl ConfiguredEmbeddingEndpoint {
    pub fn from_route(
        route: &EmbeddingModelEndpoint,
        credentials: &CredentialsConfig,
    ) -> Result<Self, EmbeddingEndpointError> {
        if route.provider != ModelProvider::OpenAiReady {
            return Err(EmbeddingEndpointError::InvalidConfiguration(
                "embedding provider is unsupported",
            ));
        }
        if route.model.trim().is_empty() || route.dimensions == 0 {
            return Err(EmbeddingEndpointError::InvalidConfiguration(
                "embedding model and dimensions are required",
            ));
        }
        let url =
            validated_http_url(&route.url).map_err(EmbeddingEndpointError::InvalidConfiguration)?;
        let (secret, _) = credentials
            .auth(&route.credential_id, route.provider)
            .ok_or(EmbeddingEndpointError::InvalidConfiguration(
                "embedding credential is missing or has the wrong provider type",
            ))?;
        let client = reqwest::blocking::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| EmbeddingEndpointError::Failure("provider client setup failed".into()))?;
        Ok(Self {
            client,
            url,
            model: route.model.clone(),
            dimensions: route.dimensions,
            normalization: route.normalization,
            api_key: secret.clone(),
        })
    }

    fn request(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
        limit: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        if inputs.is_empty() || inputs.len() > 16 || limit == 0 {
            return Err(EmbeddingEndpointError::InvalidConfiguration(
                "bounded request population",
            ));
        }
        let auth = SecretString::new(format!("Bearer {}", self.api_key.0))
            .expect("API key was validated as non-empty");
        let response = self.client.post(self.url.clone())
            .header(reqwest::header::AUTHORIZATION, auth.0.as_str())
            .json(&json!({"model": self.model, "input": inputs, "dimensions": self.dimensions,
                "encoding_format": "float", "input_type": match mode { EmbeddingMode::Query => "search_query", EmbeddingMode::Document => "search_document" }}))
            .send().map_err(|_| EmbeddingEndpointError::Failure("provider request failed".into()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(EmbeddingEndpointError::HttpStatus(status.as_u16()));
        }
        let cap = u64::try_from(limit)
            .ok()
            .and_then(|n| n.checked_add(1))
            .ok_or(EmbeddingEndpointError::InvalidConfiguration(
                "response bound overflow",
            ))?;
        let mut bytes = Vec::new();
        response
            .take(cap)
            .read_to_end(&mut bytes)
            .map_err(|_| EmbeddingEndpointError::Failure("provider response read failed".into()))?;
        if bytes.len() > limit {
            return Err(EmbeddingEndpointError::InvalidResponse(
                "bounded response exceeded",
            ));
        }
        decode_embedding_response(&bytes, inputs.len(), self.dimensions, self.normalization)
    }
}

impl EmbeddingEndpoint for ConfiguredEmbeddingEndpoint {
    fn dimensions(&self) -> u32 {
        self.dimensions
    }
    fn normalization(&self) -> EmbeddingNormalization {
        self.normalization
    }
    fn supports_bounded_embedding(&self) -> bool {
        true
    }
    fn embed_bounded(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
        max_response_bytes: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.request(mode, inputs, max_response_bytes)
    }
    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let mut result = Vec::with_capacity(inputs.len());
        for batch in inputs.chunks(16) {
            result.extend(self.request(mode, batch, MAX_RESPONSE_BYTES)?);
        }
        Ok(result)
    }
}

fn decode_embedding_response(
    bytes: &[u8],
    expected: usize,
    dimensions: u32,
    normalization: EmbeddingNormalization,
) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| EmbeddingEndpointError::InvalidResponse("invalid JSON response"))?;
    let data = value.get("data").and_then(Value::as_array).ok_or(
        EmbeddingEndpointError::InvalidResponse("missing embedding data"),
    )?;
    if data.len() != expected {
        return Err(EmbeddingEndpointError::InvalidResponse(
            "embedding count mismatch",
        ));
    }
    let mut output = vec![None; expected];
    for item in data {
        let index = item
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .filter(|i| *i < expected)
            .ok_or(EmbeddingEndpointError::InvalidResponse(
                "invalid embedding index",
            ))?;
        if output[index].is_some() {
            return Err(EmbeddingEndpointError::InvalidResponse(
                "duplicate embedding index",
            ));
        }
        let values = item.get("embedding").and_then(Value::as_array).ok_or(
            EmbeddingEndpointError::InvalidResponse("missing embedding vector"),
        )?;
        if values.len() != dimensions as usize {
            return Err(EmbeddingEndpointError::InvalidResponse(
                "embedding dimension mismatch",
            ));
        }
        let mut vector = Vec::with_capacity(values.len());
        for value in values {
            let number = value.as_f64().filter(|n| n.is_finite()).ok_or(
                EmbeddingEndpointError::InvalidResponse("non-finite embedding value"),
            )?;
            let number = number as f32;
            if !number.is_finite() {
                return Err(EmbeddingEndpointError::InvalidResponse(
                    "non-finite embedding value",
                ));
            }
            vector.push(number);
        }
        if normalization == EmbeddingNormalization::L2 {
            let norm = vector
                .iter()
                .map(|v| (*v as f64) * (*v as f64))
                .sum::<f64>()
                .sqrt();
            if norm == 0.0 || !norm.is_finite() {
                return Err(EmbeddingEndpointError::InvalidResponse(
                    "zero embedding vector",
                ));
            }
            for value in &mut vector {
                *value = (*value as f64 / norm) as f32;
            }
        }
        output[index] = Some(vector);
    }
    output
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or(EmbeddingEndpointError::InvalidResponse(
            "missing indexed embedding",
        ))
}

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

pub(super) fn validated_http_url(value: &str) -> Result<reqwest::Url, &'static str> {
    let url = reqwest::Url::parse(value).map_err(|_| "route URL is invalid")?;
    let local_http = url.scheme() == "http"
        && matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        );
    if !matches!(url.scheme(), "http" | "https")
        || (url.scheme() == "http" && !local_http)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "route URL must use HTTPS (or loopback HTTP) without embedded credentials or fragments",
        );
    }
    Ok(url)
}

fn response_bytes(response: reqwest::blocking::Response) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    response
        .take((MAX_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "provider response read failed".to_owned())?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err("provider response exceeded the 1 MiB limit".into());
    }
    Ok(bytes)
}

fn retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

fn parse_codex_stream(stream: &str) -> Result<Value, GeneralEndpointError> {
    let mut event_data = String::new();
    let mut output = String::new();
    let mut completed = false;
    for line in stream.lines().chain(std::iter::once("")) {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            if event_data == "[DONE]" {
                event_data.clear();
                continue;
            }
            if !event_data.is_empty() {
                let event: Value = serde_json::from_str(&event_data).map_err(|_| {
                    GeneralEndpointError::InvalidResponse("provider stream event is invalid")
                })?;
                match event.get("type").and_then(Value::as_str) {
                    Some("response.output_text.delta") => {
                        if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                            output.push_str(delta);
                        }
                    }
                    Some("response.completed") => completed = true,
                    Some("response.failed" | "response.incomplete") => {
                        return Err(GeneralEndpointError::Failure(
                            "provider stream did not complete successfully".into(),
                        ));
                    }
                    _ => {}
                }
                event_data.clear();
            }
            continue;
        }
        if let Some(value) = line.strip_prefix("data:") {
            if !event_data.is_empty() {
                event_data.push('\n');
            }
            event_data.push_str(value.trim_start());
        }
    }
    if !completed {
        return Err(GeneralEndpointError::InvalidResponse(
            "provider stream ended before completion",
        ));
    }
    serde_json::from_str(output.trim())
        .map_err(|_| GeneralEndpointError::InvalidResponse("provider structured output is invalid"))
}

/// Configured OpenAI-compatible structured-output provider endpoint.
#[derive(Clone)]
pub struct ConfiguredGeneralEndpoint {
    client: reqwest::blocking::Client,
    url: reqwest::Url,
    model: String,
    api_key: SecretString,
    provider: ModelProvider,
    account_id: Option<String>,
    reasoning_effort: Option<&'static str>,
}

impl ConfiguredGeneralEndpoint {
    pub fn from_route(
        route: &GeneralModelEndpoint,
        credentials: &CredentialsConfig,
    ) -> Result<Self, GeneralEndpointError> {
        Self::from_route_with_reasoning_effort(route, credentials, None)
    }

    pub(super) fn from_route_with_reasoning_effort(
        route: &GeneralModelEndpoint,
        credentials: &CredentialsConfig,
        reasoning_effort: Option<&'static str>,
    ) -> Result<Self, GeneralEndpointError> {
        if route.model.trim().is_empty() {
            return Err(GeneralEndpointError::InvalidConfiguration(
                "general model is empty",
            ));
        }
        let url =
            validated_http_url(&route.url).map_err(GeneralEndpointError::InvalidConfiguration)?;
        let (secret, account_id) = credentials
            .auth(&route.credential_id, route.provider)
            .ok_or(GeneralEndpointError::InvalidConfiguration(
                "general route credential is missing or has the wrong provider type",
            ))?;
        let api_key = secret.clone();
        let account_id = account_id.map(str::to_owned);
        let client = reqwest::blocking::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| GeneralEndpointError::Failure("provider client setup failed".into()))?;
        Ok(Self {
            client,
            url,
            model: route.model.clone(),
            api_key,
            provider: route.provider,
            account_id,
            reasoning_effort,
        })
    }
}

impl GeneralEndpoint for ConfiguredGeneralEndpoint {
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
        let (mut body, codex) = match self.provider {
            ModelProvider::OpenAiReady => (
                json!({
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
                false,
            ),
            ModelProvider::OpenAiCodex => {
                self.account_id
                    .as_deref()
                    .ok_or(GeneralEndpointError::InvalidConfiguration(
                        "provider account ID is missing",
                    ))?;
                (
                    json!({
                        "model": self.model,
                        "instructions": system_prompt,
                        "input": [{"role": "user", "content": [{"type": "input_text", "text": user_payload}]}],
                        "parallel_tool_calls": false,
                        "reasoning": {"effort": self.reasoning_effort.unwrap_or("low"), "summary": "auto"},
                        "store": false,
                        "stream": true,
                        "include": [],
                        "text": {"verbosity": "low", "format": {
                            "type": "json_schema", "name": schema_name, "strict": true, "schema": schema
                        }}
                    }),
                    true,
                )
            }
        };
        if !codex && let Some(effort) = self.reasoning_effort {
            body["reasoning_effort"] = json!(effort);
        }
        let responses_lite =
            codex && (self.model.starts_with("gpt-5.6-") || self.model.starts_with("gpt-6-"));
        if responses_lite {
            body["service_tier"] = json!("priority");
            body["reasoning"]["context"] = json!("all_turns");
        }
        let auth = SecretString::new(format!("Bearer {}", self.api_key.0))
            .expect("API key was validated as non-empty");
        let response = self
            .client
            .post(self.url.clone())
            .header(reqwest::header::AUTHORIZATION, auth.0.as_str())
            .header(
                reqwest::header::ACCEPT,
                if codex {
                    "text/event-stream"
                } else {
                    "application/json"
                },
            );
        let mut request = if codex {
            response
                .header(
                    "ChatGPT-Account-ID",
                    self.account_id.as_deref().unwrap_or_default(),
                )
                .header("originator", "codex_cli_rs")
                .header("version", CODEX_COMPAT_VERSION)
                .header(
                    reqwest::header::USER_AGENT,
                    format!("codex_cli_rs/{CODEX_COMPAT_VERSION}"),
                )
                .header(reqwest::header::CONTENT_TYPE, "application/json")
        } else {
            response
        };
        if responses_lite {
            request = request.header("x-openai-internal-codex-responses-lite", "true");
        }
        let response = request
            .json(&body)
            .send()
            .map_err(|_error| GeneralEndpointError::Failure("provider request failed".into()))?;
        let status = response.status();
        let retry_after = retry_after(response.headers());
        if status.as_u16() == 429 {
            return Err(GeneralEndpointError::Backpressure {
                message: "provider returned HTTP 429".into(),
                retry_after,
            });
        }
        if !status.is_success() {
            return Err(GeneralEndpointError::HttpStatus(status.as_u16()));
        }
        let bytes = response_bytes(response).map_err(GeneralEndpointError::Failure)?;
        if codex {
            parse_codex_stream(&String::from_utf8_lossy(&bytes))
        } else {
            let envelope: Value = serde_json::from_slice(&bytes).map_err(|_| {
                GeneralEndpointError::InvalidResponse("provider response is not JSON")
            })?;
            let content = envelope
                .pointer("/choices/0/message/content")
                .and_then(Value::as_str)
                .ok_or(GeneralEndpointError::InvalidResponse(
                    "provider response is missing structured content",
                ))?;
            serde_json::from_str(content).map_err(|_| {
                GeneralEndpointError::InvalidResponse("structured content is not JSON")
            })
        }
    }
}

/// Configured direct decision route using an API-key JSON endpoint.
#[derive(Clone)]
pub struct ConfiguredDecisionEndpoint {
    client: reqwest::blocking::Client,
    url: reqwest::Url,
    model: String,
    api_key: SecretString,
}

impl ConfiguredDecisionEndpoint {
    pub fn from_route(
        route: &DecisionModelEndpoint,
        credentials: &CredentialsConfig,
    ) -> Result<Self, DecisionEndpointError> {
        if route.model.trim().is_empty() {
            return Err(DecisionEndpointError::InvalidConfiguration(
                "decision model is empty",
            ));
        }
        let url =
            validated_http_url(&route.url).map_err(DecisionEndpointError::InvalidConfiguration)?;
        let (secret, _) = credentials
            .auth(&route.credential_id, ModelProvider::OpenAiReady)
            .ok_or(DecisionEndpointError::InvalidConfiguration(
                "decision route requires an API-key credential",
            ))?;
        let api_key = secret.clone();
        let client = reqwest::blocking::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| DecisionEndpointError::Failure("provider client setup failed".into()))?;
        Ok(Self {
            client,
            url,
            model: route.model.clone(),
            api_key,
        })
    }
}

impl DecisionEndpoint for ConfiguredDecisionEndpoint {
    fn model(&self) -> &str {
        &self.model
    }

    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError> {
        let auth = SecretString::new(format!("Bearer {}", self.api_key.0))
            .expect("API key was validated as non-empty");
        let response = self
            .client
            .post(self.url.clone())
            .header(reqwest::header::AUTHORIZATION, auth.0.as_str())
            .json(&json!({"model": self.model, "state": state, "questions": questions}))
            .send()
            .map_err(|_error| DecisionEndpointError::Failure("provider request failed".into()))?;
        let status = response.status();
        let retry_after = retry_after(response.headers());
        if status.as_u16() == 429 {
            return Err(DecisionEndpointError::Backpressure {
                message: "provider returned HTTP 429".into(),
                retry_after,
            });
        }
        if !status.is_success() {
            return Err(DecisionEndpointError::HttpStatus(status.as_u16()));
        }
        let bytes = response_bytes(response).map_err(DecisionEndpointError::Failure)?;
        serde_json::from_slice(&bytes)
            .map_err(|_| DecisionEndpointError::InvalidResponse("provider response is not JSON"))
    }
}
