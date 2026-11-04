use crate::pair_context::{canonical_pair, context_json, memory_text};
use crate::{DREAM_CLASSIFIER_SYSTEM_PROMPT, dream_classifier_schema};
use crate::{
    DreamCandidateContexts, DreamClassificationError, DreamContext, DreamEvidenceSide,
    DreamPairClassification, DreamPairEvidence, DreamRelationDirection, DreamRelationKind,
    GeneralEndpoint,
};
use serde_json::{Value, json};

pub struct DreamClassifier<E> {
    endpoint: E,
    system_prompt: String,
}

impl<E: GeneralEndpoint> DreamClassifier<E> {
    pub fn new(endpoint: E) -> Self {
        Self {
            endpoint,
            system_prompt: DREAM_CLASSIFIER_SYSTEM_PROMPT.to_owned(),
        }
    }

    pub fn with_system_prompt(endpoint: E, system_prompt: impl Into<String>) -> Self {
        Self {
            endpoint,
            system_prompt: system_prompt.into(),
        }
    }

    pub fn model(&self) -> &str {
        self.endpoint.model()
    }

    pub fn classify_candidates(
        &self,
        set: &impl DreamCandidateContexts,
    ) -> Result<Vec<DreamPairClassification>, DreamClassificationError> {
        set.candidates()
            .into_iter()
            .map(|candidate| self.classify_pair(set.source(), candidate))
            .collect()
    }

    pub fn classify_pair<C: DreamContext>(
        &self,
        left: &C,
        right: &C,
    ) -> Result<DreamPairClassification, DreamClassificationError> {
        let (a, b) = canonical_pair(left, right).ok_or(DreamClassificationError::InvalidPair)?;
        let payload = serde_json::to_string(&json!({
            "contract_version": crate::DREAM_CLASSIFIER_CONTRACT_VERSION,
            "a": context_json(a),
            "b": context_json(b),
        }))
        .map_err(|error| DreamClassificationError::InvalidOutput(error.to_string()))?;
        let output = self.endpoint.complete_json(
            &self.system_prompt,
            &payload,
            "dream_pair_classification",
            &dream_classifier_schema(),
        )?;
        parse_output(self.endpoint.model(), a, b, &output)
    }
}

fn parse_output<C: DreamContext>(
    model: &str,
    a: &C,
    b: &C,
    output: &Value,
) -> Result<DreamPairClassification, DreamClassificationError> {
    let relation = parse_relation(required_string(output, "relation")?)?;
    let reported_direction = parse_direction(required_string(output, "direction")?)?;
    let direction = canonical_relation_direction(relation, reported_direction)?;
    let evidence = parse_evidence(output, relation, a, b)?;
    Ok(DreamPairClassification {
        model: model.to_owned(),
        a: a.memory().id,
        b: b.memory().id,
        relation,
        direction,
        evidence,
    })
}

fn parse_evidence<C: DreamContext>(
    output: &Value,
    relation: DreamRelationKind,
    a: &C,
    b: &C,
) -> Result<Vec<DreamPairEvidence>, DreamClassificationError> {
    let values = output
        .get("evidence")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("evidence must be an array"))?;
    if relation == DreamRelationKind::None {
        return if values.is_empty() {
            Ok(Vec::new())
        } else {
            Err(invalid("none must not include evidence"))
        };
    }
    if values.len() != 2 {
        return Err(invalid(
            "non-none relation requires exactly two evidence quotes",
        ));
    }
    let mut parsed = Vec::with_capacity(2);
    for value in values {
        let side = match required_string(value, "side")? {
            "a" => DreamEvidenceSide::A,
            "b" => DreamEvidenceSide::B,
            _ => return Err(invalid("invalid evidence side")),
        };
        if parsed
            .iter()
            .any(|item: &DreamPairEvidence| item.side == side)
        {
            return Err(invalid("evidence must contain one quote per side"));
        }
        let quote = required_string(value, "quote")?.trim().to_owned();
        let context = if side == DreamEvidenceSide::A { a } else { b };
        if quote.is_empty() || !memory_text(context).contains(&quote) {
            return Err(invalid("evidence quote is not verbatim Memory text"));
        }
        parsed.push(DreamPairEvidence { side, quote });
    }
    if parsed.len() != 2 {
        return Err(invalid("evidence must contain A and B"));
    }
    Ok(parsed)
}

fn canonical_relation_direction(
    relation: DreamRelationKind,
    reported: DreamRelationDirection,
) -> Result<DreamRelationDirection, DreamClassificationError> {
    match relation {
        DreamRelationKind::None => Ok(DreamRelationDirection::None),
        DreamRelationKind::Topical
        | DreamRelationKind::Recurrent
        | DreamRelationKind::DuplicateOf => Ok(DreamRelationDirection::Undirected),
        DreamRelationKind::Factual | DreamRelationKind::Causal | DreamRelationKind::Supersedes => {
            match reported {
                DreamRelationDirection::AToB | DreamRelationDirection::BToA => Ok(reported),
                DreamRelationDirection::None | DreamRelationDirection::Undirected => {
                    Err(invalid("directed relation requires a_to_b or b_to_a"))
                }
            }
        }
    }
}

fn parse_relation(value: &str) -> Result<DreamRelationKind, DreamClassificationError> {
    Ok(match value {
        "none" => DreamRelationKind::None,
        "topical" => DreamRelationKind::Topical,
        "factual" => DreamRelationKind::Factual,
        "causal" => DreamRelationKind::Causal,
        "recurrent" => DreamRelationKind::Recurrent,
        "duplicate_of" => DreamRelationKind::DuplicateOf,
        "supersedes" => DreamRelationKind::Supersedes,
        _ => return Err(invalid("unknown relation")),
    })
}

fn parse_direction(value: &str) -> Result<DreamRelationDirection, DreamClassificationError> {
    Ok(match value {
        "none" => DreamRelationDirection::None,
        "undirected" => DreamRelationDirection::Undirected,
        "a_to_b" => DreamRelationDirection::AToB,
        "b_to_a" => DreamRelationDirection::BToA,
        _ => return Err(invalid("unknown direction")),
    })
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, DreamClassificationError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(&format!("missing string field {key}")))
}

fn invalid(message: &str) -> DreamClassificationError {
    DreamClassificationError::InvalidOutput(message.into())
}
