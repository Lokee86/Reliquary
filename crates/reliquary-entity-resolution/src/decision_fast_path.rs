use crate::{EntityCandidate, EntityResolutionReason, ResolutionDecision, ResolutionOutcome};
use reliquary_switchboard::DecisionEndpoint;
use serde_json::{Map, Value, json};

/// A nonterminal or invalid answer leaves the general evaluator authoritative.
pub(crate) fn try_fast_path(
    endpoint: &dyn DecisionEndpoint,
    state: &Value,
    candidates: &[EntityCandidate],
) -> Option<ResolutionOutcome> {
    if candidates.is_empty() {
        return None;
    }
    let mut criteria = Map::new();
    for (index, _) in candidates.iter().enumerate() {
        criteria.insert(
            format!("candidate_{index}"),
            Value::String(format!(
                "The mention is the same durable identity as candidate {index}."
            )),
        );
    }
    criteria.insert(
        "create_new".into(),
        Value::String("The mention is a distinct durable identity.".into()),
    );
    criteria.insert(
        "unresolved".into(),
        Value::String("The evidence is insufficient or ambiguous.".into()),
    );
    criteria.insert(
        "reject".into(),
        Value::String("The mention is not a durable Entity.".into()),
    );
    let questions = json!({"resolution":{"type":"choice","instructions":"Resolve only the supplied mention; surface similarity alone does not prove identity.","criteria":criteria}});
    let response = endpoint.evaluate(state, &questions).ok()?;
    let answer = response.get("answers")?.get("resolution")?;
    if answer.get("type")?.as_str()? != "choice" {
        return None;
    }
    let choice = answer.get("choice")?.as_str()?;
    let index = choice.strip_prefix("candidate_")?.parse::<usize>().ok()?;
    let candidate = candidates.get(index)?;
    if !candidate.exact_surface && !candidate.normalized_surface {
        return None;
    }
    let probabilities = answer.get("probabilities")?.as_object()?;
    let probability = probabilities.get(choice)?.as_f64()?;
    if !probabilities.values().all(|value| {
        value
            .as_f64()
            .is_some_and(|p| p.is_finite() && (0.0..=1.0).contains(&p))
    }) {
        return None;
    }
    let second = probabilities
        .iter()
        .filter(|(name, _)| name.as_str() != choice)
        .filter_map(|(_, value)| value.as_f64())
        .fold(0.0_f64, f64::max);
    if probability < 0.95 || probability - second < 0.15 {
        return None;
    }
    Some(ResolutionOutcome {
        decision: ResolutionDecision::ResolveExisting(candidate.entity.id),
        reason: EntityResolutionReason::ContextMatch,
    })
}
