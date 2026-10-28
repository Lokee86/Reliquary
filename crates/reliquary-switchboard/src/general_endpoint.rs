use serde_json::Value;
use std::fmt;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug)]
pub enum GeneralEndpointError {
    InvalidConfiguration(&'static str),
    Failure(String),
    Backpressure {
        message: String,
        retry_after: Option<Duration>,
    },
    InvalidResponse(&'static str),
}

impl GeneralEndpointError {
    pub fn backpressure(message: impl Into<String>, retry_after: Option<Duration>) -> Self {
        Self::Backpressure {
            message: message.into(),
            retry_after,
        }
    }

    pub fn backpressure_retry_after(&self) -> Option<Duration> {
        match self {
            Self::Backpressure { retry_after, .. } => *retry_after,
            _ => None,
        }
    }

    pub fn is_backpressure(&self) -> bool {
        matches!(self, Self::Backpressure { .. })
    }
}

impl fmt::Display for GeneralEndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => {
                write!(f, "general endpoint configuration: {message}")
            }
            Self::Failure(message) => write!(f, "general endpoint failure: {message}"),
            Self::Backpressure { message, .. } => {
                write!(f, "general endpoint backpressure: {message}")
            }
            Self::InvalidResponse(message) => write!(f, "general endpoint response: {message}"),
        }
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

impl<T: GeneralEndpoint + ?Sized> GeneralEndpoint for &T {
    fn model(&self) -> &str {
        (*self).model()
    }

    fn complete_json(
        &self,
        system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        (*self).complete_json(system_prompt, user_payload, schema_name, schema)
    }
}

pub struct SimulatedGeneralEndpoint {
    model: String,
    responses: Mutex<Vec<Value>>,
}

impl SimulatedGeneralEndpoint {
    pub fn new(model: impl Into<String>, responses: Vec<Value>) -> Self {
        let mut responses = responses;
        responses.reverse();
        Self {
            model: model.into(),
            responses: Mutex::new(responses),
        }
    }
}

impl GeneralEndpoint for SimulatedGeneralEndpoint {
    fn model(&self) -> &str {
        &self.model
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        _user_payload: &str,
        _schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.responses
            .lock()
            .map_err(|_| GeneralEndpointError::Failure("simulator lock poisoned".into()))?
            .pop()
            .ok_or_else(|| GeneralEndpointError::Failure("simulator has no response".into()))
    }
}
