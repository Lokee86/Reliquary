use crate::pair_context::{canonical_pair, context_json};
use crate::{DREAM_VERIFIER_SYSTEM_PROMPT, dream_verifier_schema};
use crate::{
    DreamContext, DreamEvidenceSide, DreamPairClassification, DreamPairVerification,
    DreamRelationDirection, DreamRelationKind, DreamVerificationError, DreamVerificationPolicy,
    DreamVerificationSignal, DreamVerificationVerdict, GeneralEndpoint,
};
use serde_json::{Value, json};

pub struct DreamVerifier<E> {
    endpoint: E,
}

impl<E: GeneralEndpoint> DreamVerifier<E> {
    pub fn new(endpoint: E) -> Self {
        Self { endpoint }
    }

    pub fn model(&self) -> &str {
        self.endpoint.model()
    }

    pub fn verify_if_required<C: DreamContext>(
        &self,
        policy: DreamVerificationPolicy,
        classification: &DreamPairClassification,
        left: &C,
        right: &C,
    ) -> Result<Option<DreamPairVerification>, DreamVerificationError> {
        if !policy.should_verify(classification.relation) {
            return Ok(None);
        }
        self.verify_pair(classification, left, right).map(Some)
    }

    pub fn verify_pair<C: DreamContext>(
        &self,
        classification: &DreamPairClassification,
        left: &C,
        right: &C,
    ) -> Result<DreamPairVerification, DreamVerificationError> {
        if classification.relation == DreamRelationKind::None {
            return Err(DreamVerificationError::NoRelation);
        }
        let (a, b) = canonical_pair(left, right).ok_or(DreamVerificationError::InvalidPair)?;
        if classification.a != a.memory().id || classification.b != b.memory().id {
            return Err(DreamVerificationError::InvalidPair);
        }
        let payload = serde_json::to_string(&json!({
            "contract_version": crate::DREAM_VERIFIER_CONTRACT_VERSION,
            "a": context_json(a),
            "b": context_json(b),
            "proposal": classification_json(classification),
        }))
        .map_err(|error| DreamVerificationError::InvalidOutput(error.to_string()))?;
        let output = self.endpoint.complete_json(
            DREAM_VERIFIER_SYSTEM_PROMPT,
            &payload,
            "dream_pair_verification",
            &dream_verifier_schema(),
        )?;
        parse_output(self.endpoint.model(), classification, &output)
    }
}

fn classification_json(classification: &DreamPairClassification) -> Value {
    let evidence: Vec<_> = classification
        .evidence
        .iter()
        .map(|item| {
            json!({
                "side": match item.side {
                    DreamEvidenceSide::A => "a",
                    DreamEvidenceSide::B => "b",
                },
                "quote": item.quote,
            })
        })
        .collect();
    json!({
        "classifier_model": classification.model,
        "relation": relation_name(classification.relation),
        "direction": direction_name(classification.direction),
        "evidence": evidence,
    })
}

fn parse_output(
    verifier_model: &str,
    classification: &DreamPairClassification,
    output: &Value,
) -> Result<DreamPairVerification, DreamVerificationError> {
    let relation_supported = parse_signal(required_string(output, "relation_supported")?)?;
    let direction_supported = parse_signal(required_string(output, "direction_supported")?)?;
    let evidence_supported = parse_signal(required_string(output, "evidence_supported")?)?;
    let verdict = derive_verdict(relation_supported, direction_supported, evidence_supported);
    Ok(DreamPairVerification {
        verifier_model: verifier_model.to_owned(),
        a: classification.a,
        b: classification.b,
        classification: classification.clone(),
        relation_supported,
        direction_supported,
        evidence_supported,
        verdict,
    })
}

fn derive_verdict(
    relation: DreamVerificationSignal,
    direction: DreamVerificationSignal,
    evidence: DreamVerificationSignal,
) -> DreamVerificationVerdict {
    let signals = [relation, direction, evidence];
    if signals.contains(&DreamVerificationSignal::No) {
        DreamVerificationVerdict::Reject
    } else if signals.contains(&DreamVerificationSignal::Uncertain) {
        DreamVerificationVerdict::Uncertain
    } else {
        DreamVerificationVerdict::Accept
    }
}

fn parse_signal(value: &str) -> Result<DreamVerificationSignal, DreamVerificationError> {
    match value {
        "yes" => Ok(DreamVerificationSignal::Yes),
        "no" => Ok(DreamVerificationSignal::No),
        "uncertain" => Ok(DreamVerificationSignal::Uncertain),
        _ => Err(invalid("unknown verification signal")),
    }
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, DreamVerificationError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(&format!("missing string field {key}")))
}

fn relation_name(relation: DreamRelationKind) -> &'static str {
    match relation {
        DreamRelationKind::None => "none",
        DreamRelationKind::Topical => "topical",
        DreamRelationKind::Factual => "factual",
        DreamRelationKind::Causal => "causal",
        DreamRelationKind::Recurrent => "recurrent",
        DreamRelationKind::DuplicateOf => "duplicate_of",
        DreamRelationKind::Supersedes => "supersedes",
    }
}

fn direction_name(direction: DreamRelationDirection) -> &'static str {
    match direction {
        DreamRelationDirection::None => "none",
        DreamRelationDirection::Undirected => "undirected",
        DreamRelationDirection::AToB => "a_to_b",
        DreamRelationDirection::BToA => "b_to_a",
    }
}

fn invalid(message: &str) -> DreamVerificationError {
    DreamVerificationError::InvalidOutput(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_signals_fail_and_no_overrides_uncertain() {
        assert!(parse_signal("maybe").is_err());
        assert_eq!(
            derive_verdict(
                DreamVerificationSignal::Uncertain,
                DreamVerificationSignal::No,
                DreamVerificationSignal::Yes
            ),
            DreamVerificationVerdict::Reject
        );
        assert_eq!(dream_verifier_schema()["additionalProperties"], false);
    }
}
