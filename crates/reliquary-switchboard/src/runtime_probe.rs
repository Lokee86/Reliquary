use super::RuntimeEndpointFailure as Failure;
use super::transport::MAX_RESPONSE_BYTES;
use super::*;

const PROBE_INPUT: &str = "Reliquary provider readiness probe";
const PROBE_INSTRUCTION: &str = "Return exactly the JSON object {\"ok\":true}.";

fn probe_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "ok": { "type": "boolean", "enum": [true] } },
        "required": ["ok"],
        "additionalProperties": false
    })
}

fn check_probe_response(response: Value) -> Result<(), Failure> {
    if response == json!({"ok": true}) {
        Ok(())
    } else {
        Err(Failure::InvalidResponse)
    }
}

pub(super) fn probe_general(endpoint: &dyn GeneralEndpoint) -> Result<(), Failure> {
    let response = endpoint
        .complete_json(
            PROBE_INSTRUCTION,
            PROBE_INPUT,
            "readiness_probe",
            &probe_schema(),
        )
        .map_err(|error| match error {
            GeneralEndpointError::InvalidConfiguration(_) => Failure::InvalidConfiguration,
            GeneralEndpointError::Failure(_) => Failure::RequestFailed,
            GeneralEndpointError::HttpStatus(status) => Failure::HttpStatus(status),
            GeneralEndpointError::Backpressure { retry_after, .. } => {
                Failure::Backpressure { retry_after }
            }
            GeneralEndpointError::InvalidResponse(_) => Failure::InvalidResponse,
        })?;
    check_probe_response(response)
}

pub(super) fn probe_decision(endpoint: &dyn DecisionEndpoint) -> Result<(), Failure> {
    let response = endpoint
        .evaluate(
            &json!({"operation": "readiness_probe"}),
            &json!({
                "readiness": {
                    "type": "choice",
                    "instructions": "Select ready for this provider readiness probe.",
                    "criteria": {
                        "ready": "The provider is ready.",
                        "not_ready": "The provider is not ready."
                    }
                }
            }),
        )
        .map_err(|error| match error {
            DecisionEndpointError::InvalidConfiguration(_) => Failure::InvalidConfiguration,
            DecisionEndpointError::Failure(_) => Failure::RequestFailed,
            DecisionEndpointError::HttpStatus(status) => Failure::HttpStatus(status),
            DecisionEndpointError::Backpressure { retry_after, .. } => {
                Failure::Backpressure { retry_after }
            }
            DecisionEndpointError::InvalidResponse(_) => Failure::InvalidResponse,
        })?;
    match response
        .pointer("/answers/readiness")
        .and_then(Value::as_object)
    {
        Some(answer)
            if answer.get("type").and_then(Value::as_str) == Some("choice")
                && answer.get("choice").and_then(Value::as_str) == Some("ready") =>
        {
            Ok(())
        }
        _ => Err(Failure::InvalidResponse),
    }
}

pub(super) fn probe_embedding(endpoint: &dyn EmbeddingEndpoint) -> Result<(), Failure> {
    endpoint
        .embed_bounded(
            EmbeddingMode::Query,
            &[PROBE_INPUT.to_owned()],
            MAX_RESPONSE_BYTES,
        )
        .map_err(|error| match error {
            EmbeddingEndpointError::InvalidConfiguration(_) => Failure::InvalidConfiguration,
            EmbeddingEndpointError::InvalidResponse(_) => Failure::InvalidResponse,
            EmbeddingEndpointError::Failure(_) => Failure::RequestFailed,
            EmbeddingEndpointError::HttpStatus(status) => Failure::HttpStatus(status),
        })?;
    Ok(())
}
