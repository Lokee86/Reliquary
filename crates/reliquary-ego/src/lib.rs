//! Topics and exact Memory references for downstream Ego extraction.

pub mod rel_summary;

use reliquary_memory_audit::{CandidateContract, Feature, Slots, SourceCitation};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub mod package;

const MAX_SUBJECT_BYTES: usize = 160;
const MAX_SOURCE_SLOTS: usize = 6;

#[derive(Clone, Copy, Default)]
pub struct EgoContract;

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoCandidate {
    pub subject: String,
    pub sources: Vec<SourceCitation>,
}

impl CandidateContract for EgoContract {
    type Candidate = EgoCandidate;

    fn instruction(&self) -> &'static str {
        "Suggest topics useful for a current summary and select the Memory slots eligible for Ego worth reading for each topic. Return a short subject and its source slots."
    }

    fn candidate_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "subject": { "type": "string", "maxLength": MAX_SUBJECT_BYTES },
                "source_slots": {
                    "type": "array", "minItems": 1, "maxItems": MAX_SOURCE_SLOTS,
                    "items": { "type": "integer", "minimum": 0, "maximum": u16::MAX }
                }
            },
            "required": ["subject", "source_slots"],
            "additionalProperties": false
        })
    }

    fn parse_candidate(&self, value: Value, slots: &Slots<'_>) -> Option<EgoCandidate> {
        let wire: CandidateFields = serde_json::from_value(value).ok()?;
        let subject = wire.subject.trim();
        if subject.is_empty()
            || subject.len() > MAX_SUBJECT_BYTES
            || !(1..=MAX_SOURCE_SLOTS).contains(&wire.source_slots.len())
        {
            return None;
        }
        let mut sources = Vec::new();
        for slot in wire.source_slots {
            let citation = slots
                .source(slot)
                .filter(|source| source.allows(Feature::Ego))?
                .citation;
            if !sources.contains(&citation) {
                sources.push(citation);
            }
        }
        Some(EgoCandidate {
            subject: subject.to_owned(),
            sources,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateFields {
    subject: String,
    source_slots: Vec<u16>,
}

#[cfg(test)]
mod tests;
