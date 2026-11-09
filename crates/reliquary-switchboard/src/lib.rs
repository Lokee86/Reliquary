//! Provider route selection and endpoint contracts.
//!
//! This crate owns model/provider configuration and transport seams. Callers
//! decide when to invoke a provider; Switchboard does not access REL/PHY state.

use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use zeroize::Zeroize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelProvider {
    OpenAiCodex,
    OpenAiReady,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelCapability {
    General,
    Decision,
    Embedding,
}

impl ModelProvider {
    pub fn supports(self, capability: ModelCapability) -> bool {
        match (self, capability) {
            (_, ModelCapability::General | ModelCapability::Decision) => true,
            (Self::OpenAiReady, ModelCapability::Embedding) => true,
            (Self::OpenAiCodex, ModelCapability::Embedding) => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralModelEndpoint {
    pub provider: ModelProvider,
    pub model: String,
    pub url: String,
    pub credential_id: CredentialId,
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
    pub url: String,
    pub credential_id: CredentialId,
    pub dimensions: u32,
    pub normalization: EmbeddingNormalization,
}

/// Provider-independent contract for embedding transports. Callers select
/// when to embed; providers only receive bounded batches and return vectors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddingMode {
    Query,
    Document,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddingNormalization {
    None,
    L2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmbeddingEndpointError {
    InvalidConfiguration(&'static str),
    InvalidResponse(&'static str),
    Failure(String),
    HttpStatus(u16),
}

impl fmt::Display for EmbeddingEndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "embedding endpoint error: {self:?}")
    }
}
impl std::error::Error for EmbeddingEndpointError {}

pub trait EmbeddingEndpoint: Send + Sync {
    fn dimensions(&self) -> u32;
    fn normalization(&self) -> EmbeddingNormalization;
    fn supports_bounded_embedding(&self) -> bool {
        false
    }
    fn embed_bounded(
        &self,
        _mode: EmbeddingMode,
        _inputs: &[String],
        _max_response_bytes: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        Err(EmbeddingEndpointError::InvalidConfiguration(
            "bounded embedding unsupported",
        ))
    }
    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError>;
}

/// Resolve only an explicitly supplied configured route adapter.
pub fn resolve_embedding_endpoint(
    route_configured: bool,
    endpoint: Option<Arc<dyn EmbeddingEndpoint>>,
) -> Result<Option<Arc<dyn EmbeddingEndpoint>>, EmbeddingEndpointError> {
    if !route_configured {
        return Ok(None);
    }
    endpoint
        .map(Some)
        .ok_or(EmbeddingEndpointError::InvalidConfiguration(
            "configured embedding route has no endpoint adapter",
        ))
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CredentialId(String);

impl CredentialId {
    pub fn new(value: impl Into<String>) -> Result<Self, CredentialError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(CredentialError::InvalidId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CredentialId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Result<Self, CredentialError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CredentialError::EmptySecret);
        }
        Ok(Self(value))
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString([REDACTED])")
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialsConfig {
    entries: BTreeMap<CredentialId, Credential>,
}

impl Default for CredentialsConfig {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl CredentialsConfig {
    pub fn insert_api_key(
        &mut self,
        id: CredentialId,
        secret: impl Into<String>,
    ) -> Result<(), CredentialError> {
        self.entries.insert(
            id,
            Credential::ApiKey {
                secret: SecretString::new(secret)?,
            },
        );
        Ok(())
    }

    pub fn insert_codex_oauth(
        &mut self,
        id: CredentialId,
        access_token: impl Into<String>,
        account_id: impl Into<String>,
    ) -> Result<(), CredentialError> {
        let account_id = account_id.into();
        if account_id.is_empty() || account_id.len() > 256 {
            return Err(CredentialError::InvalidAccountId);
        }
        self.entries.insert(
            id,
            Credential::CodexOAuth {
                access_token: SecretString::new(access_token)?,
                account_id,
                refresh: None,
            },
        );
        Ok(())
    }

    fn auth(
        &self,
        id: &CredentialId,
        provider: ModelProvider,
    ) -> Option<(&SecretString, Option<&str>)> {
        match (self.entries.get(id)?, provider) {
            (Credential::ApiKey { secret }, ModelProvider::OpenAiReady) => Some((secret, None)),
            (
                Credential::CodexOAuth {
                    access_token,
                    account_id,
                    ..
                },
                ModelProvider::OpenAiCodex,
            ) if !account_id.is_empty() && account_id.len() <= 256 => {
                Some((access_token, Some(account_id)))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
enum Credential {
    ApiKey {
        secret: SecretString,
    },
    CodexOAuth {
        access_token: SecretString,
        account_id: String,
        refresh: Option<CodexRefresh>,
    },
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ApiKey { .. } => "ApiKey([REDACTED])",
            Self::CodexOAuth { .. } => "CodexOAuth([REDACTED])",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexRefresh {
    id_token: SecretString,
    refresh_token: SecretString,
}

#[derive(Debug)]
pub enum CredentialError {
    InvalidId,
    EmptySecret,
    InvalidAccountId,
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId => f.write_str("invalid credential ID"),
            Self::EmptySecret => f.write_str("credential secret must not be empty"),
            Self::InvalidAccountId => f.write_str("invalid provider account ID"),
        }
    }
}
impl std::error::Error for CredentialError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModelSwitchboardConfig {
    pub general: Option<GeneralModelEndpoint>,
    pub general_decision: Option<DecisionModelEndpoint>,
    pub entity_resolution_decision: Option<DecisionModelEndpoint>,
    pub memory_retrieval_decision: Option<DecisionModelEndpoint>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionConsumer {
    EntityResolution,
    MemoryRetrieval,
    Chronos,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionRouteSelection<'a> {
    Dedicated(&'a DecisionModelEndpoint),
    Shared(&'a DecisionModelEndpoint),
    General(&'a GeneralModelEndpoint),
    Unavailable,
}

/// Selects operation-specific, shared decision, then general fallback routes.
pub fn select_decision_route<'a>(
    config: &'a ModelSwitchboardConfig,
    consumer: DecisionConsumer,
) -> DecisionRouteSelection<'a> {
    let dedicated = match consumer {
        DecisionConsumer::EntityResolution => config.entity_resolution_decision.as_ref(),
        DecisionConsumer::MemoryRetrieval => config.memory_retrieval_decision.as_ref(),
        DecisionConsumer::Chronos => None,
    };
    if let Some(route) = dedicated {
        return DecisionRouteSelection::Dedicated(route);
    }
    if let Some(route) = config.general_decision.as_ref() {
        return DecisionRouteSelection::Shared(route);
    }
    if let Some(route) = config.general.as_ref() {
        return DecisionRouteSelection::General(route);
    }
    DecisionRouteSelection::Unavailable
}

/// Binds a selected configuration route to the caller's provider adapters.
///
/// Transport implementations are injected so this crate can select policy
/// without owning a client runtime or importing a semantic owner.
pub fn resolve_decision_endpoint(
    config: &ModelSwitchboardConfig,
    consumer: DecisionConsumer,
    dedicated: Option<Arc<dyn DecisionEndpoint>>,
    shared: Option<Arc<dyn DecisionEndpoint>>,
    general: Option<Arc<dyn GeneralEndpoint>>,
) -> Result<Option<Arc<dyn DecisionEndpoint>>, DecisionEndpointError> {
    match select_decision_route(config, consumer) {
        DecisionRouteSelection::Dedicated(_) => {
            dedicated
                .map(Some)
                .ok_or(DecisionEndpointError::InvalidConfiguration(
                    "selected dedicated decision route has no endpoint adapter",
                ))
        }
        DecisionRouteSelection::Shared(_) => {
            shared
                .map(Some)
                .ok_or(DecisionEndpointError::InvalidConfiguration(
                    "selected shared decision route has no endpoint adapter",
                ))
        }
        DecisionRouteSelection::General(_) => {
            let general = general.ok_or(DecisionEndpointError::InvalidConfiguration(
                "selected general route has no endpoint adapter",
            ))?;
            Ok(Some(Arc::new(GeneralDecisionEndpoint::new(general))))
        }
        DecisionRouteSelection::Unavailable => Ok(None),
    }
}

#[derive(Debug)]
pub enum GeneralEndpointError {
    InvalidConfiguration(&'static str),
    Failure(String),
    HttpStatus(u16),
    Backpressure {
        message: String,
        retry_after: Option<Duration>,
    },
    InvalidResponse(&'static str),
}

impl GeneralEndpointError {
    pub fn is_backpressure(&self) -> bool {
        matches!(self, Self::Backpressure { .. })
    }
}

impl fmt::Display for GeneralEndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "general endpoint error: {self:?}")
    }
}
impl std::error::Error for GeneralEndpointError {}

pub trait GeneralEndpoint: Send + Sync {
    fn model(&self) -> &str;

    fn complete_json(
        &self,
        system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError>;
}

#[derive(Debug)]
pub enum DecisionEndpointError {
    InvalidConfiguration(&'static str),
    Failure(String),
    HttpStatus(u16),
    Backpressure {
        message: String,
        retry_after: Option<Duration>,
    },
    InvalidResponse(&'static str),
}

impl fmt::Display for DecisionEndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "decision endpoint error: {self:?}")
    }
}
impl std::error::Error for DecisionEndpointError {}

pub trait DecisionEndpoint: Send + Sync {
    fn model(&self) -> &str;
    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError>;
}

mod config_document;
mod configured;
mod device_auth;
mod model_config;
mod protected_config;
mod runtime_probe;
mod transport;
pub use config_document::ConfigObject;
pub use configured::{
    ConfiguredRoutes, ConfiguredRuntimeError, ProcessingRouteIdentity, RuntimeEndpointFailure,
};
pub use device_auth::{CodexDeviceCode, DeviceAuthError};
pub use model_config::{GeneralRoute, ModelRoute, ModelSlot};
pub use protected_config::master_key::MasterKeyError;
pub use protected_config::{ConfigFile, CredentialKind};

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod test_support {
    pub use crate::protected_config::master_key::mock::{
        MockMasterKeyStore, mock_master_key_store,
    };
}
pub use transport::ConfiguredEmbeddingEndpoint;
pub use transport::{ConfiguredDecisionEndpoint, ConfiguredGeneralEndpoint};

/// Uses a general structured-output endpoint for decision requests.
pub struct GeneralDecisionEndpoint {
    general: Arc<dyn GeneralEndpoint>,
}

impl GeneralDecisionEndpoint {
    pub fn new(general: Arc<dyn GeneralEndpoint>) -> Self {
        Self { general }
    }
}

impl DecisionEndpoint for GeneralDecisionEndpoint {
    fn model(&self) -> &str {
        self.general.model()
    }

    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError> {
        let request = DecisionRequest::parse(questions)?;
        let payload = serde_json::to_string(&json!({"state": state, "questions": questions}))
            .map_err(|error| DecisionEndpointError::Failure(error.to_string()))?;
        let response = self
            .general
            .complete_json(
                &request.prompt,
                &payload,
                &request.schema_name,
                &request.schema,
            )
            .map_err(map_general_error)?;
        match request.response_kind {
            ResponseKind::EmbeddedSchema => Ok(response),
            ResponseKind::ChoiceAnswers(expected) => validate_choice_answers(response, &expected),
        }
    }
}

enum ResponseKind {
    EmbeddedSchema,
    ChoiceAnswers(BTreeMap<String, Vec<String>>),
}

struct DecisionRequest {
    prompt: String,
    schema_name: String,
    schema: Value,
    response_kind: ResponseKind,
}

impl DecisionRequest {
    fn parse(questions: &Value) -> Result<Self, DecisionEndpointError> {
        if let Some(schema) = questions.get("schema") {
            if !schema.is_object() {
                return Err(invalid("embedded response schema must be an object"));
            }
            let prompt = questions
                .get("instruction")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or(invalid("embedded schema decision requires an instruction"))?;
            return Ok(Self {
                prompt: prompt.to_owned(),
                schema_name: "decision_response".into(),
                schema: normalize_embedded_schema(schema.clone()),
                response_kind: ResponseKind::EmbeddedSchema,
            });
        }

        let definitions = questions
            .as_object()
            .filter(|definitions| !definitions.is_empty())
            .ok_or(invalid(
                "choice decision questions must be a nonempty object",
            ))?;
        let mut answer_properties = Map::new();
        let mut required = Vec::with_capacity(definitions.len());
        let mut expected = BTreeMap::new();
        for (question_id, definition) in definitions {
            if question_id.trim().is_empty() {
                return Err(invalid("choice question ID must be nonempty"));
            }
            let definition = definition
                .as_object()
                .ok_or(invalid("choice question definition must be an object"))?;
            if definition.get("type").and_then(Value::as_str) != Some("choice") {
                return Err(invalid("unsupported decision question type"));
            }
            definition
                .get("instructions")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or(invalid("choice question instructions are required"))?;
            let criteria = definition
                .get("criteria")
                .and_then(Value::as_object)
                .filter(|criteria| !criteria.is_empty())
                .ok_or(invalid(
                    "choice question criteria must be a nonempty object",
                ))?;
            let mut choices = Vec::with_capacity(criteria.len());
            for (choice, description) in criteria {
                if choice.trim().is_empty() || !description.is_string() {
                    return Err(invalid(
                        "choice criteria require nonempty IDs and string descriptions",
                    ));
                }
                choices.push(choice.clone());
            }
            choices.sort();
            required.push(Value::String(question_id.clone()));
            expected.insert(question_id.clone(), choices.clone());
            answer_properties.insert(
                question_id.clone(),
                json!({
                    "type": "object",
                    "required": ["type", "choice"],
                    "additionalProperties": false,
                    "properties": {
                        "type": {"const": "choice"},
                        "choice": {"type": "string", "enum": choices}
                    }
                }),
            );
        }
        required.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
        let schema = json!({
            "type": "object",
            "required": ["answers"],
            "additionalProperties": false,
            "properties": {
                "answers": {
                    "type": "object",
                    "required": required,
                    "additionalProperties": false,
                    "properties": answer_properties
                }
            }
        });
        let prompt = definitions
            .iter()
            .map(|(id, definition)| {
                format!(
                    "Question {id}: {}\nAllowed choices: {}",
                    definition["instructions"].as_str().unwrap_or_default(),
                    definition["criteria"]
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        Ok(Self {
            prompt,
            schema_name: "decision_answers".into(),
            schema,
            response_kind: ResponseKind::ChoiceAnswers(expected),
        })
    }
}

fn normalize_embedded_schema(mut schema: Value) -> Value {
    fn normalize(value: &mut Value) {
        match value {
            Value::Object(object) => {
                if let Some(constant) = object.remove("const") {
                    if !object.contains_key("type") {
                        let kind = match &constant {
                            Value::Null => "null",
                            Value::Bool(_) => "boolean",
                            Value::Number(number) if number.is_i64() || number.is_u64() => {
                                "integer"
                            }
                            Value::Number(_) => "number",
                            Value::String(_) => "string",
                            Value::Array(_) => "array",
                            Value::Object(_) => "object",
                        };
                        object.insert("type".into(), Value::String(kind.into()));
                    }
                    object.insert("enum".into(), Value::Array(vec![constant]));
                }
                for child in object.values_mut() {
                    normalize(child);
                }
            }
            Value::Array(values) => {
                for child in values {
                    normalize(child);
                }
            }
            _ => {}
        }
    }
    normalize(&mut schema);
    schema
}

fn validate_choice_answers(
    response: Value,
    expected: &BTreeMap<String, Vec<String>>,
) -> Result<Value, DecisionEndpointError> {
    let response_object = response
        .as_object()
        .ok_or(invalid("choice decision response must be an object"))?;
    let answers = response_object
        .get("answers")
        .and_then(Value::as_object)
        .ok_or(invalid(
            "choice decision response requires an answers object",
        ))?;
    if response_object.len() != 1
        || answers.len() != expected.len()
        || answers.keys().any(|id| !expected.contains_key(id))
    {
        return Err(invalid(
            "choice decision response has missing or unknown answers",
        ));
    }
    for (id, allowed) in expected {
        let answer = answers
            .get(id)
            .and_then(Value::as_object)
            .ok_or(invalid("choice answer must be an object"))?;
        if answer.len() != 2 || answer.get("type").and_then(Value::as_str) != Some("choice") {
            return Err(invalid("choice answer has invalid type or extra fields"));
        }
        let choice = answer
            .get("choice")
            .and_then(Value::as_str)
            .ok_or(invalid("choice answer is missing its selected choice"))?;
        if !allowed.iter().any(|candidate| candidate == choice) {
            return Err(invalid("choice answer is outside the declared criteria"));
        }
    }
    Ok(response)
}

fn map_general_error(error: GeneralEndpointError) -> DecisionEndpointError {
    match error {
        GeneralEndpointError::Backpressure {
            message,
            retry_after,
        } => DecisionEndpointError::Backpressure {
            message,
            retry_after,
        },
        GeneralEndpointError::InvalidResponse(message) => {
            DecisionEndpointError::InvalidResponse(message)
        }
        GeneralEndpointError::InvalidConfiguration(message) => {
            DecisionEndpointError::InvalidConfiguration(message)
        }
        GeneralEndpointError::Failure(message) => DecisionEndpointError::Failure(message),
        GeneralEndpointError::HttpStatus(status) => DecisionEndpointError::HttpStatus(status),
    }
}

fn invalid(message: &'static str) -> DecisionEndpointError {
    DecisionEndpointError::InvalidResponse(message)
}

#[cfg(test)]
mod tests;
