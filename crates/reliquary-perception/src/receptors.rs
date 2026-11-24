use crate::claim::{
    model_claim, model_participants, nonblank_bounded, normalize, object, participant_identity,
    valid_claim,
};
use crate::general::{complete, fingerprint};
use crate::{StageError, StageLimits, SynthesisDraft};
use reliquary_switchboard::GeneralEndpoint;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const RECEPTOR_POLICY_VERSION: u16 = 1;
const MAX_PER_CATEGORY: usize = 2;
const MAX_DESCRIPTION_BYTES: usize = 512;
const PROMPT: &str = concat!(
    "Describe specific evidence that would bear on the exact Observation. ",
    "Provide distinct search descriptions in all four categories. Support would corroborate its precise claim. ",
    "Challenge would disprove a material assertion about the same subject, scope, and applicable time. ",
    "Qualification would establish an exception or boundary to its wording. Other describes another kind ",
    "of evidence that could change reconsideration, such as a grounded clarification of identity or an ",
    "authoritative later record that establishes the earlier state. Keep separate conditions in separate ",
    "descriptions, connect each description to the actual claim and participants, and preserve distinctions ",
    "such as preliminary versus final measures and different components of an event. ",
    "Describe evidence to look for without asserting that it exists. Supply one or two descriptions per category.",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceptorBinding {
    pub route_identity: [u8; 32],
    pub embedding_profile: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceptorCategory {
    Support,
    Challenge,
    Qualification,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceptorDescription {
    pub category: ReceptorCategory,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceptorSet {
    pub binding: [u8; 32],
    pub receptors: Vec<ReceptorDescription>,
}

/// Compute the binding needed to reuse an already generated receptor set.
pub fn receptor_fingerprint(draft: &SynthesisDraft, binding: ReceptorBinding) -> [u8; 32] {
    fingerprint(json!([
        "reliquary-observation-receptors",
        RECEPTOR_POLICY_VERSION,
        binding.route_identity,
        binding.embedding_profile,
        model_claim(&draft.claim),
        participant_identity(&draft.claim, true),
        model_participants(&draft.claim),
    ]))
}

/// Produce separate descriptions for the application to embed independently.
pub fn generate_receptors(
    endpoint: &dyn GeneralEndpoint,
    draft: &SynthesisDraft,
    binding: ReceptorBinding,
    limits: StageLimits,
) -> Result<ReceptorSet, StageError> {
    if !valid_claim(&draft.claim) || draft.claim.citations.is_empty() {
        return Err(StageError::InvalidInput);
    }
    let state = json!({
        "claim": model_claim(&draft.claim),
        "participants": model_participants(&draft.claim),
    });
    let response = complete(
        endpoint,
        PROMPT,
        "observation_receptors_v1",
        &schema(),
        &state,
        limits,
    )?;
    let fields: ReceptorFields =
        serde_json::from_value(response).map_err(|_| StageError::InvalidReply)?;
    let mut distinct = BTreeSet::new();
    let mut receptors = Vec::new();
    for (category, descriptions) in [
        (ReceptorCategory::Support, fields.support),
        (ReceptorCategory::Challenge, fields.challenge),
        (ReceptorCategory::Qualification, fields.qualification),
        (ReceptorCategory::Other, fields.other),
    ] {
        if !(1..=MAX_PER_CATEGORY).contains(&descriptions.len()) {
            return Err(StageError::InvalidReply);
        }
        for description in descriptions {
            if !nonblank_bounded(&description, MAX_DESCRIPTION_BYTES)
                || !distinct.insert(normalize(&description))
            {
                return Err(StageError::InvalidReply);
            }
            receptors.push(ReceptorDescription {
                category,
                description,
            });
        }
    }
    Ok(ReceptorSet {
        binding: receptor_fingerprint(draft, binding),
        receptors,
    })
}

fn schema() -> Value {
    let category = json!({
        "type":"array", "minItems":1, "maxItems":MAX_PER_CATEGORY,
        "items":{"type":"string","maxLength":MAX_DESCRIPTION_BYTES},
    });
    object(
        json!({
            "support":category, "challenge":category, "qualification":category, "other":category,
        }),
        &["support", "challenge", "qualification", "other"],
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceptorFields {
    support: Vec<String>,
    challenge: Vec<String>,
    qualification: Vec<String>,
    other: Vec<String>,
}
