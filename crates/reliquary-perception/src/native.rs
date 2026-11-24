use crate::NATIVE_DECISION_MODEL;
use reliquary_switchboard::{DecisionEndpoint, DecisionEndpointError};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

const REQUEST_BYTES: usize = 64 * 1024;

#[derive(Debug)]
pub enum NativeDecisionError {
    WrongModel,
    InvalidRequest(&'static str),
    InvalidReply(&'static str),
    Endpoint(DecisionEndpointError),
}

impl fmt::Display for NativeDecisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongModel => write!(f, "Observation decision requires {NATIVE_DECISION_MODEL}"),
            Self::InvalidRequest(reason) => {
                write!(f, "invalid Observation decision request ({reason})")
            }
            Self::InvalidReply(reason) => {
                write!(f, "invalid Observation decision reply ({reason})")
            }
            Self::Endpoint(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for NativeDecisionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Endpoint(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HitVerdict {
    MaterialChallenge,
    MaterialQualification,
    WeakOrUncertain,
    NoMaterialEffect,
}

impl HitVerdict {
    pub(crate) fn is_material(self) -> bool {
        matches!(self, Self::MaterialChallenge | Self::MaterialQualification)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecisionDiagnostics {
    pub confidence: Option<f64>,
    pub probabilities: Option<BTreeMap<String, f64>>,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Question {
    #[serde(rename = "type")]
    kind: String,
    instructions: String,
    criteria: BTreeMap<String, String>,
}

impl Question {
    pub(crate) fn for_hit(&self, id: &str) -> Self {
        Self {
            kind: self.kind.clone(),
            instructions: self.instructions.replace("{hit}", id),
            criteria: self.criteria.clone(),
        }
    }
}

pub(crate) fn insert_question(
    questions: &mut BTreeMap<String, Question>,
    key: String,
    question: Question,
) -> Result<(), NativeDecisionError> {
    if questions.insert(key, question).is_some() {
        return Err(NativeDecisionError::InvalidRequest("question ID collision"));
    }
    Ok(())
}

#[derive(Serialize)]
struct Request<'a> {
    model: &'static str,
    state: &'a Value,
    questions: &'a Value,
}

pub(crate) fn dispatch(
    endpoint: &dyn DecisionEndpoint,
    state: &impl Serialize,
    questions: &BTreeMap<String, Question>,
    response_limit: Option<usize>,
) -> Result<Answers, NativeDecisionError> {
    if endpoint.model() != NATIVE_DECISION_MODEL {
        return Err(NativeDecisionError::WrongModel);
    }
    let state = serde_json::to_value(state)
        .map_err(|_| NativeDecisionError::InvalidRequest("state could not be serialized"))?;
    let encoded_questions = serde_json::to_value(questions)
        .map_err(|_| NativeDecisionError::InvalidRequest("questions could not be serialized"))?;
    let request = Request {
        model: NATIVE_DECISION_MODEL,
        state: &state,
        questions: &encoded_questions,
    };
    if serde_json::to_vec(&request)
        .map_err(|_| NativeDecisionError::InvalidRequest("request could not be serialized"))?
        .len()
        > REQUEST_BYTES
    {
        return Err(NativeDecisionError::InvalidRequest(
            "request exceeded the byte limit",
        ));
    }

    let reply = match response_limit {
        Some(bytes) => endpoint.evaluate_bounded(&state, &encoded_questions, bytes),
        None => endpoint.evaluate(&state, &encoded_questions),
    }
    .map_err(|error| match error {
        DecisionEndpointError::InvalidResponse(reason) => NativeDecisionError::InvalidReply(reason),
        error => NativeDecisionError::Endpoint(error),
    })?;
    Answers::decode(reply, questions)
}

pub(crate) struct Answers {
    choices: BTreeMap<String, String>,
    pub(crate) diagnostics: BTreeMap<String, DecisionDiagnostics>,
}

impl Answers {
    fn decode(
        reply: Value,
        questions: &BTreeMap<String, Question>,
    ) -> Result<Self, NativeDecisionError> {
        use NativeDecisionError::InvalidReply;

        let answers = reply
            .get("answers")
            .and_then(Value::as_object)
            .ok_or(InvalidReply("missing answer object"))?;
        if answers.len() != questions.len() || !questions.keys().all(|id| answers.contains_key(id))
        {
            return Err(InvalidReply("missing or unexpected answer IDs"));
        }
        let mut choices = BTreeMap::new();
        let mut diagnostics = BTreeMap::new();
        for (id, question) in questions {
            let answer = &answers[id];
            if answer.get("type").and_then(Value::as_str) != Some("choice") {
                return Err(InvalidReply("invalid answer type"));
            }
            let choice = answer
                .get("choice")
                .and_then(Value::as_str)
                .filter(|choice| question.criteria.contains_key(*choice))
                .ok_or(InvalidReply("missing or invalid choice"))?;
            let probabilities = answer
                .get("probabilities")
                .map(|scores| {
                    let scores = scores
                        .as_object()
                        .filter(|scores| scores.contains_key(choice))
                        .ok_or(InvalidReply(
                            "missing selected probability or invalid probability object",
                        ))?;
                    scores
                        .iter()
                        .map(|(key, value)| {
                            let value = value
                                .as_f64()
                                .filter(|value| {
                                    (0.0..=1.0).contains(value)
                                        && question.criteria.contains_key(key)
                                })
                                .ok_or(InvalidReply("invalid probability key or value"))?;
                            Ok((key.clone(), value))
                        })
                        .collect::<Result<_, _>>()
                })
                .transpose()?;
            let confidence = answer
                .get("confidence")
                .and_then(Value::as_f64)
                .filter(|value| (0.0..=1.0).contains(value));
            diagnostics.insert(
                id.clone(),
                DecisionDiagnostics {
                    confidence,
                    probabilities,
                },
            );
            choices.insert(id.clone(), choice.to_owned());
        }
        Ok(Self {
            choices,
            diagnostics,
        })
    }

    pub(crate) fn choice<T: DeserializeOwned>(&self, id: &str) -> Result<T, NativeDecisionError> {
        use serde::de::value::{Error, StrDeserializer};
        let choice = self
            .choices
            .get(id)
            .ok_or(NativeDecisionError::InvalidReply("missing answer ID"))?;
        T::deserialize(StrDeserializer::<Error>::new(choice))
            .map_err(|_| NativeDecisionError::InvalidReply("invalid typed choice"))
    }
}
