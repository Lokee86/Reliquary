use reliquary_memory_audit::{
    CandidateContract, Feature, Participant, Passage, Slots, SourceCitation, SourceId, Status,
    TimeScope,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const MAX_PROPOSITION_BYTES: usize = 4096;
const MAX_ROLE_BYTES: usize = 96;
pub(crate) const MAX_CITATIONS: usize = 8;
const MAX_PARTICIPANTS: usize = 4;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationProfile {
    #[default]
    Ordinary,
    Relational,
}

/// Relational discovery can also produce ordinary claims, identified by empty participants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PerceptionContract {
    pub profile: ObservationProfile,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ProposedClaim {
    pub proposition: String,
    pub profile: ObservationProfile,
    pub status: Status,
    pub time_scope: TimeScope,
    pub citations: Vec<SourceCitation>,
    pub participants: Vec<ClaimParticipant>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ClaimParticipant {
    pub participant: Participant,
    pub role: Option<String>,
}

impl CandidateContract for PerceptionContract {
    type Candidate = ProposedClaim;

    fn instruction(&self) -> &'static str {
        // Keep this policy part of the audit's versioned batch contract.
        match self.profile {
            ObservationProfile::Ordinary => concat!(
                "Propose a useful Observation from eligible Perception passages, including settled older sources. ",
                "Each item expresses one grounded proposition with one supported status and time scope. ",
                "Retain uncertainty and temporal qualifiers, distinguish plans and requests from outcomes, ",
                "and select direct support or the premises of a bounded conclusion. ",
                "Exclude unsupported generalizations and causal guesses. ",
                "Propose ordinary Observations and return empty participants.",
            ),
            ObservationProfile::Relational => concat!(
                "Propose a useful Observation from eligible Perception passages, including settled older sources. ",
                "Each item expresses one grounded proposition with one supported status and time scope. ",
                "Retain uncertainty and temporal qualifiers, distinguish plans and requests from outcomes, ",
                "and select direct support or the premises of a bounded conclusion. ",
                "Exclude unsupported generalizations and causal guesses. ",
                "Use empty participants for ordinary Observations. For relational Observations cite Memory ",
                "passages that ground two to four distinct resolved participant slots. An ordinary Observation ",
                "can supply additional evidence. Select grounded roles ",
                "or null. Co-occurrence, similar names and existing labels do not establish a relationship.",
            ),
        }
    }

    fn candidate_schema(&self) -> Value {
        claim_schema(self.profile, false)
    }

    fn parse_candidate(&self, value: Value, slots: &Slots<'_>) -> Option<ProposedClaim> {
        let fields: ClaimFields = serde_json::from_value(value).ok()?;
        if self.profile == ObservationProfile::Ordinary && !fields.participants.is_empty() {
            return None;
        }
        bind_claim(
            fields,
            |slot| slots.source(slot),
            |slot| slots.participant(slot),
        )
    }
}

pub(crate) fn object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object", "properties": properties,
        "required": required, "additionalProperties": false,
    })
}

pub(crate) fn claim_schema(profile: ObservationProfile, require_profile: bool) -> Value {
    let max = match profile {
        ObservationProfile::Ordinary => 0,
        ObservationProfile::Relational => MAX_PARTICIPANTS,
    };
    let min = usize::from(profile == ObservationProfile::Relational && require_profile) * 2;
    let slot = json!({"type":"integer", "minimum":0, "maximum":u16::MAX});
    let participant = object(
        json!({
            "participant_slot": slot,
            "role": {"type":["string","null"], "maxLength":MAX_ROLE_BYTES},
        }),
        &["participant_slot", "role"],
    );
    object(
        json!({
            "proposition": {"type":"string", "maxLength":MAX_PROPOSITION_BYTES},
            "status": {"type":"string", "enum":["observed","planned","requested","possible","expected"]},
            "time_scope": {"type":"string", "enum":["past","current","future","unspecified"]},
            "source_slots": {"type":"array", "minItems":1, "maxItems":MAX_CITATIONS, "items":slot},
            "participants": {"type":"array", "minItems":min, "maxItems":max, "items":participant},
        }),
        &[
            "proposition",
            "status",
            "time_scope",
            "source_slots",
            "participants",
        ],
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClaimFields {
    proposition: String,
    status: Status,
    time_scope: TimeScope,
    source_slots: Vec<u16>,
    participants: Vec<ParticipantFields>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParticipantFields {
    participant_slot: u16,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    role: Option<String>,
}

pub(crate) fn bind_claim<'a>(
    fields: ClaimFields,
    mut source: impl FnMut(u16) -> Option<&'a Passage>,
    mut participant: impl FnMut(u16) -> Option<&'a Participant>,
) -> Option<ProposedClaim> {
    let profile = if fields.participants.is_empty() {
        ObservationProfile::Ordinary
    } else {
        ObservationProfile::Relational
    };
    if !nonblank_bounded(&fields.proposition, MAX_PROPOSITION_BYTES)
        || !(1..=MAX_CITATIONS).contains(&fields.source_slots.len())
        || !participant_count(profile, fields.participants.len())
    {
        return None;
    }
    let mut citations = Vec::new();
    for slot in fields.source_slots {
        let source = source(slot)?;
        let citation = source.citation;
        if !source.allows(Feature::Perception) || citations.contains(&citation) {
            return None;
        }
        citations.push(citation);
    }
    if !has_participant_evidence(profile, &citations) {
        return None;
    }
    let mut participants = Vec::new();
    let mut entities = BTreeSet::new();
    for fields in fields.participants {
        let participant = participant(fields.participant_slot)?;
        if !entities.insert(participant.entity_id) || !valid_role(fields.role.as_deref()) {
            return None;
        }
        participants.push(ClaimParticipant {
            participant: participant.clone(),
            role: fields.role,
        });
    }
    Some(ProposedClaim {
        proposition: fields.proposition,
        profile,
        status: fields.status,
        time_scope: fields.time_scope,
        citations,
        participants,
    })
}

pub(crate) fn valid_claim(claim: &ProposedClaim) -> bool {
    let mut identities = BTreeSet::new();
    let mut citations = BTreeSet::new();
    nonblank_bounded(&claim.proposition, MAX_PROPOSITION_BYTES)
        && participant_count(claim.profile, claim.participants.len())
        && claim.citations.len() <= MAX_CITATIONS
        && has_participant_evidence(claim.profile, &claim.citations)
        && claim
            .citations
            .iter()
            .all(|citation| citations.insert(citation))
        && claim.participants.iter().all(|entry| {
            valid_role(entry.role.as_deref()) && identities.insert(entry.participant.entity_id)
        })
}

fn has_participant_evidence(profile: ObservationProfile, citations: &[SourceCitation]) -> bool {
    profile == ObservationProfile::Ordinary
        || citations
            .iter()
            .any(|cite| matches!(cite.source, SourceId::Memory(_)))
}

fn participant_count(profile: ObservationProfile, count: usize) -> bool {
    match profile {
        ObservationProfile::Ordinary => count == 0,
        ObservationProfile::Relational => (2..=MAX_PARTICIPANTS).contains(&count),
    }
}

fn valid_role(role: Option<&str>) -> bool {
    role.is_none_or(|role| nonblank_bounded(role, MAX_ROLE_BYTES))
}

pub(crate) fn nonblank_bounded(text: &str, max_bytes: usize) -> bool {
    !text.trim().is_empty() && text.len() <= max_bytes
}

pub(crate) fn model_claim(claim: &ProposedClaim) -> Value {
    json!({
        "proposition": claim.proposition, "profile": claim.profile,
        "status": claim.status, "time_scope": claim.time_scope,
    })
}

pub(crate) fn participant_identity(claim: &ProposedClaim, with_revision: bool) -> Value {
    let mut participants: Vec<_> = claim.participants.iter().collect();
    participants.sort_by_key(|entry| (entry.participant.owner_id, entry.participant.entity_id));
    Value::Array(
        participants
            .into_iter()
            .map(|entry| {
                json!([
                    entry.participant.owner_id,
                    entry.participant.entity_id,
                    with_revision.then_some(entry.participant.revision),
                    entry.role,
                ])
            })
            .collect(),
    )
}

pub(crate) fn model_participants(claim: &ProposedClaim) -> Vec<Value> {
    claim
        .participants
        .iter()
        .map(|entry| {
            json!({
                "label": entry.participant.label, "role": entry.role,
            })
        })
        .collect()
}

pub(crate) fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
