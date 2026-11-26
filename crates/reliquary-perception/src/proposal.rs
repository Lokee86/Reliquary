use crate::ObservationProfile;
use crate::claim::{
    MAX_CITATIONS, MAX_PARTICIPANTS, MAX_PROPOSITION_BYTES, bind_sources, nonblank_bounded, object,
    participant_count, valid_source_references,
};
use reliquary_memory_audit::{CandidateContract, Participant, Slots, SourceCitation};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Eq, PartialEq)]
pub struct ObservationProposal {
    pub proposition: String,
    pub profile: ObservationProfile,
    pub sources: Vec<SourceCitation>,
    pub participants: Vec<Participant>,
}

impl ObservationProposal {
    pub(crate) fn valid(&self) -> bool {
        let mut entities = BTreeSet::new();
        nonblank_bounded(&self.proposition, MAX_PROPOSITION_BYTES)
            && participant_count(self.profile, self.participants.len())
            && valid_source_references(self.profile, &self.sources)
            && self
                .participants
                .iter()
                .all(|p| entities.insert(p.entity_id))
    }
}

/// Relational discovery also accepts ordinary proposals with an empty participant list.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PerceptionContract {
    pub profile: ObservationProfile,
}

impl CandidateContract for PerceptionContract {
    type Candidate = ObservationProposal;

    fn instruction(&self) -> &'static str {
        match self.profile {
            ObservationProfile::Ordinary => concat!(
                "Suggest useful Observations to investigate and select the eligible passages worth analyzing. ",
                "Use a short proposed statement and the source slots for each item. Leave participant_slots empty.",
            ),
            ObservationProfile::Relational => concat!(
                "Suggest useful Observations to investigate and select the eligible passages worth analyzing. ",
                "Use a short proposed statement and the source slots for each item. Use empty participant_slots ",
                "for ordinary Observations. For a Relationship, select two to four distinct resolved Entity slots ",
                "and Memory passages that establish their relevance. An eligible Observation can supply additional ",
                "evidence. Co-occurrence, similar names and existing labels do not establish a relationship.",
            ),
        }
    }

    fn candidate_schema(&self) -> Value {
        let max_participants = match self.profile {
            ObservationProfile::Ordinary => 0,
            ObservationProfile::Relational => MAX_PARTICIPANTS,
        };
        let slot = json!({"type":"integer", "minimum":0, "maximum":u16::MAX});
        object(
            json!({
                "proposition": {"type":"string", "maxLength":MAX_PROPOSITION_BYTES},
                "source_slots": {"type":"array", "minItems":1, "maxItems":MAX_CITATIONS, "items":slot},
                "participant_slots": {"type":"array", "minItems":0, "maxItems":max_participants, "items":slot},
            }),
            &["proposition", "source_slots", "participant_slots"],
        )
    }

    fn parse_candidate(&self, value: Value, slots: &Slots<'_>) -> Option<ObservationProposal> {
        let fields: ProposalFields = serde_json::from_value(value).ok()?;
        let profile = if fields.participant_slots.is_empty() {
            ObservationProfile::Ordinary
        } else if self.profile == ObservationProfile::Relational {
            ObservationProfile::Relational
        } else {
            return None;
        };
        if !participant_count(profile, fields.participant_slots.len()) {
            return None;
        }
        let sources = bind_sources(fields.source_slots, |slot| slots.source(slot))?;
        let participants = fields
            .participant_slots
            .into_iter()
            .map(|slot| slots.participant(slot).cloned())
            .collect::<Option<Vec<_>>>()?;
        let proposal = ObservationProposal {
            proposition: fields.proposition,
            profile,
            sources,
            participants,
        };
        proposal.valid().then_some(proposal)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposalFields {
    proposition: String,
    source_slots: Vec<u16>,
    participant_slots: Vec<u16>,
}
