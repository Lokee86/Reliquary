use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::Value;
use std::{fmt, time::Duration};

const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024;

/// Usable route budget in bytes. Perception always applies its own ceilings as well.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StageLimits {
    pub max_input_bytes: usize,
    pub max_response_bytes: usize,
}

impl Default for StageLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: MAX_INPUT_BYTES,
            max_response_bytes: MAX_RESPONSE_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageError {
    InvalidInput,
    InputLimit,
    InvalidReply,
    Endpoint { retry_after: Option<Duration> },
}

impl fmt::Display for StageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid Observation stage input",
            Self::InputLimit => "Observation stage exceeds the route input limit",
            Self::InvalidReply => "invalid Observation stage reply",
            Self::Endpoint { .. } => "Observation stage endpoint unavailable",
        })
    }
}

impl std::error::Error for StageError {}

pub(crate) fn complete(
    endpoint: &dyn GeneralEndpoint,
    prompt: &str,
    schema_name: &str,
    schema: &Value,
    state: &Value,
    limits: StageLimits,
) -> Result<Value, StageError> {
    let payload = state.to_string();
    if limits.max_response_bytes == 0
        || prompt.len() + schema.to_string().len() + payload.len()
            > limits.max_input_bytes.min(MAX_INPUT_BYTES)
    {
        return Err(StageError::InputLimit);
    }
    let reply = endpoint
        .complete_json(prompt, &payload, schema_name, schema)
        .map_err(|error| match error {
            GeneralEndpointError::InvalidResponse(_) => StageError::InvalidReply,
            GeneralEndpointError::Backpressure { retry_after, .. } => {
                StageError::Endpoint { retry_after }
            }
            _ => StageError::Endpoint { retry_after: None },
        })?;
    if reply.to_string().len() > limits.max_response_bytes.min(MAX_RESPONSE_BYTES) {
        return Err(StageError::InvalidReply);
    }
    Ok(reply)
}

pub(crate) fn fingerprint(value: Value) -> [u8; 32] {
    *blake3::hash(value.to_string().as_bytes()).as_bytes()
}
