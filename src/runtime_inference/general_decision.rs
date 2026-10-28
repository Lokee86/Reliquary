//! Structured decision requests backed by the configured general model route.
use crate::{DecisionEndpoint, DecisionEndpointError, GeneralEndpoint, GeneralEndpointError};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;

pub(crate) struct GeneralDecisionEndpoint {
    general: Arc<dyn GeneralEndpoint>,
}

impl GeneralDecisionEndpoint {
    pub(crate) fn new(general: Arc<dyn GeneralEndpoint>) -> Self {
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
                schema: schema.clone(),
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
                let instructions = definition["instructions"].as_str().unwrap_or_default();
                let criteria = &definition["criteria"];
                format!("Question {id}: {instructions}\nAllowed choices: {criteria}")
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

fn validate_choice_answers(
    response: Value,
    expected: &BTreeMap<String, Vec<String>>,
) -> Result<Value, DecisionEndpointError> {
    let answers = response
        .get("answers")
        .and_then(Value::as_object)
        .ok_or(invalid(
            "choice decision response requires an answers object",
        ))?;
    let response_object = response
        .as_object()
        .ok_or(invalid("choice decision response must be an object"))?;
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
        if !allowed.iter().any(|allowed| allowed == choice) {
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
    }
}

fn invalid(message: &'static str) -> DecisionEndpointError {
    DecisionEndpointError::InvalidResponse(message)
}

#[cfg(test)]
#[path = "general_decision_tests.rs"]
mod tests;
