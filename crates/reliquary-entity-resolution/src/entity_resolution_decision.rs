use crate::{
    DecisionEndpoint, DecisionEndpointError, EntityResolutionDecision, EntityResolutionEvaluation,
    EntityResolutionPrepared, EntityResolutionReason, EntityResolverOutput,
};
use serde_json::{Map, Value, json};

pub const DEFAULT_ENTITY_DECISION_RESOLVE_THRESHOLD: f64 = 0.95;
pub const DEFAULT_ENTITY_DECISION_MARGIN: f64 = 0.15;

pub(crate) fn try_decision_fast_path(
    endpoint: &dyn DecisionEndpoint,
    prepared: &EntityResolutionPrepared<'_>,
) -> Result<Option<EntityResolutionEvaluation>, DecisionEndpointError> {
    if prepared.candidates.candidates.is_empty() {
        return Ok(None);
    }

    let mut criteria = Map::new();
    for (index, _) in prepared.candidates.candidates.iter().enumerate() {
        criteria.insert(
            format!("candidate_{index}"),
            Value::String(format!(
                "The current mention refers to the same durable identity as candidate {index}."
            )),
        );
    }
    criteria.insert(
        "create_new".into(),
        Value::String(
            "The mention is a durable identity, but none of the supplied candidates is the same identity."
                .into(),
        ),
    );
    criteria.insert(
        "unresolved".into(),
        Value::String(
            "The evidence is insufficient or ambiguous, so no terminal identity decision is safe."
                .into(),
        ),
    );
    criteria.insert(
        "reject".into(),
        Value::String("The mention is not a durable Entity candidate at all.".into()),
    );

    let state = json!({
        "title": prepared.memory.title,
        "content": prepared.memory.content,
        "mention": {
            "text": prepared.candidates.mention.text,
            "start_byte": prepared.candidates.mention.start_byte,
            "end_byte": prepared.candidates.mention.end_byte,
        },
        "candidates": prepared.candidates.candidates.iter()
            .zip(prepared.evidence.iter())
            .enumerate()
            .map(|(index, (candidate, evidence))| json!({
                "candidate_index": index,
                "canonical_name": candidate.entity.canonical_name,
                "aliases": candidate.entity.aliases,
                "kind": candidate.entity.kind,
                "summary": candidate.entity.summary,
                "evidence": evidence.iter().take(3).map(|memory| json!({
                    "title": memory.title,
                    "content": truncate_utf8(&memory.content, 4096),
                })).collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>(),
    });
    let questions = json!({
        "resolution": {
            "type": "choice",
            "instructions": "Resolve only the supplied current mention. Lexical equality is not identity equality. Choose an existing candidate only when the current Memory context supports the same durable identity.",
            "criteria": criteria,
        }
    });

    let response = endpoint.evaluate(&state, &questions)?;
    let Some(index) = selected_candidate(&response, prepared.candidates.candidates.len())? else {
        return Ok(None);
    };
    let candidate = &prepared.candidates.candidates[index];
    if !candidate.exact_surface && !candidate.normalized_surface {
        return Ok(None);
    }
    let entity_id = candidate.entity.id;
    Ok(Some(EntityResolutionEvaluation {
        output: EntityResolverOutput {
            decision: EntityResolutionDecision::ResolveExisting(entity_id),
            reason: EntityResolutionReason::ContextMatch,
        },
        materialization: None,
    }))
}

fn selected_candidate(
    response: &Value,
    candidate_count: usize,
) -> Result<Option<usize>, DecisionEndpointError> {
    let answer = response
        .get("answers")
        .and_then(|value| value.get("resolution"))
        .ok_or(DecisionEndpointError::InvalidResponse(
            "missing resolution answer",
        ))?;
    if answer.get("type").and_then(Value::as_str) != Some("choice") {
        return Err(DecisionEndpointError::InvalidResponse(
            "resolution answer is not a choice",
        ));
    }
    let selected = answer.get("choice").and_then(Value::as_str).ok_or(
        DecisionEndpointError::InvalidResponse("resolution choice missing"),
    )?;
    let Some(index) = selected
        .strip_prefix("candidate_")
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return Ok(None);
    };
    if index >= candidate_count {
        return Err(DecisionEndpointError::InvalidResponse(
            "resolution candidate is out of range",
        ));
    }

    let probabilities = answer
        .get("probabilities")
        .and_then(Value::as_object)
        .ok_or(DecisionEndpointError::InvalidResponse(
            "resolution probabilities missing",
        ))?;
    let selected_probability = probabilities.get(selected).and_then(Value::as_f64).ok_or(
        DecisionEndpointError::InvalidResponse("selected probability missing"),
    )?;
    let second_probability = probabilities
        .iter()
        .filter(|(choice, _)| choice.as_str() != selected)
        .filter_map(|(_, value)| value.as_f64())
        .fold(0.0_f64, f64::max);

    if selected_probability < DEFAULT_ENTITY_DECISION_RESOLVE_THRESHOLD
        || selected_probability - second_probability < DEFAULT_ENTITY_DECISION_MARGIN
    {
        return Ok(None);
    }
    Ok(Some(index))
}

fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_high_confidence_candidate_with_margin() {
        let response = json!({
            "answers": {
                "resolution": {
                    "type": "choice",
                    "choice": "candidate_1",
                    "probabilities": {
                        "candidate_0": 0.02,
                        "candidate_1": 0.96,
                        "create_new": 0.01,
                        "unresolved": 0.01,
                        "reject": 0.0
                    }
                }
            }
        });
        assert_eq!(selected_candidate(&response, 2).unwrap(), Some(1));
    }

    #[test]
    fn rejects_low_confidence_candidate() {
        let response = json!({
            "answers": {
                "resolution": {
                    "type": "choice",
                    "choice": "candidate_0",
                    "probabilities": {
                        "candidate_0": 0.94,
                        "candidate_1": 0.03,
                        "create_new": 0.01,
                        "unresolved": 0.01,
                        "reject": 0.01
                    }
                }
            }
        });
        assert_eq!(selected_candidate(&response, 2).unwrap(), None);
    }

    #[test]
    fn rejects_high_probability_without_margin() {
        let response = json!({
            "answers": {
                "resolution": {
                    "type": "choice",
                    "choice": "candidate_0",
                    "probabilities": {
                        "candidate_0": 0.95,
                        "candidate_1": 0.84,
                        "create_new": 0.01,
                        "unresolved": 0.0,
                        "reject": 0.0
                    }
                }
            }
        });
        assert_eq!(selected_candidate(&response, 2).unwrap(), None);
    }

    #[test]
    fn never_terminally_accepts_non_candidate_choices() {
        for choice in ["create_new", "unresolved", "reject"] {
            let response = json!({
                "answers": {
                    "resolution": {
                        "type": "choice",
                        "choice": choice,
                        "probabilities": {
                            "candidate_0": 0.01,
                            "create_new": 0.97,
                            "unresolved": 0.01,
                            "reject": 0.01
                        }
                    }
                }
            });
            assert_eq!(selected_candidate(&response, 1).unwrap(), None);
        }
    }
}
