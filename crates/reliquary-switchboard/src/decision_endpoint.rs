use serde_json::Value;
use std::fmt;
use std::sync::Mutex;

#[derive(Debug)]
pub enum DecisionEndpointError {
    InvalidConfiguration(&'static str),
    Failure(String),
    Backpressure {
        message: String,
        retry_after: Option<std::time::Duration>,
    },
    InvalidResponse(&'static str),
}

impl fmt::Display for DecisionEndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => {
                write!(f, "decision endpoint configuration: {message}")
            }
            Self::Failure(message) => write!(f, "decision endpoint failure: {message}"),
            Self::Backpressure { message, .. } => {
                write!(f, "decision endpoint backpressure: {message}")
            }
            Self::InvalidResponse(message) => write!(f, "decision endpoint response: {message}"),
        }
    }
}

impl std::error::Error for DecisionEndpointError {}

pub trait DecisionEndpoint: Send + Sync {
    fn model(&self) -> &str;

    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError>;
}

impl<T: DecisionEndpoint + ?Sized> DecisionEndpoint for &T {
    fn model(&self) -> &str {
        (*self).model()
    }

    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError> {
        (*self).evaluate(state, questions)
    }
}

pub struct SimulatedDecisionEndpoint {
    model: String,
    responses: Mutex<Vec<Value>>,
}

impl SimulatedDecisionEndpoint {
    pub fn new(model: impl Into<String>, responses: Vec<Value>) -> Self {
        let mut responses = responses;
        responses.reverse();
        Self {
            model: model.into(),
            responses: Mutex::new(responses),
        }
    }
}

impl DecisionEndpoint for SimulatedDecisionEndpoint {
    fn model(&self) -> &str {
        &self.model
    }

    fn evaluate(&self, _state: &Value, _questions: &Value) -> Result<Value, DecisionEndpointError> {
        self.responses
            .lock()
            .map_err(|_| DecisionEndpointError::Failure("simulator lock poisoned".into()))?
            .pop()
            .ok_or_else(|| DecisionEndpointError::Failure("simulator has no response".into()))
    }
}
