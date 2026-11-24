//! Fresh-only, source-linked inputs for downstream Ego summary generation.

use reliquary_memory_audit::{
    CandidateContract, Feature, Slots, SourceCitation, Status, TimeScope,
};
use serde::Deserialize;
use serde_json::{Value, json};

const MAX_SUBJECT_BYTES: usize = 160;
const MAX_POINT_BYTES: usize = 256;
const MAX_EVIDENCE_ENTRIES: usize = 6;

#[derive(Clone, Copy, Default)]
pub struct EgoContract;

#[derive(Clone, Eq, PartialEq)]
pub struct EgoCandidate {
    pub subject: String,
    pub evidence: Vec<EgoEvidence>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct EgoEvidence {
    pub citation: SourceCitation,
    pub point: String,
    pub status: Status,
    pub time_scope: TimeScope,
    pub excerpt: String,
}

impl CandidateContract for EgoContract {
    type Candidate = EgoCandidate;

    fn instruction(&self) -> &'static str {
        "Extract subjects useful for a current summary. Use only Fresh Memory slots eligible for Ego. Each evidence entry states one source-supported point and its status and time scope. A source can ground distinct entries. Preserve uncertainty and temporal qualifiers. A detail found only in an older or ineligible source cannot be used."
    }

    fn candidate_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "subject": { "type": "string", "maxLength": MAX_SUBJECT_BYTES },
                "evidence": {
                    "type": "array", "minItems": 1, "maxItems": MAX_EVIDENCE_ENTRIES,
                    "items": {
                        "type": "object",
                        "properties": {
                            "source_slot": { "type": "integer", "minimum": 0, "maximum": u16::MAX },
                            "point": { "type": "string", "maxLength": MAX_POINT_BYTES },
                            "status": { "type": "string", "enum": ["observed", "planned", "requested", "possible", "expected"] },
                            "time_scope": { "type": "string", "enum": ["past", "current", "future", "unspecified"] }
                        },
                        "required": ["source_slot", "point", "status", "time_scope"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["subject", "evidence"],
            "additionalProperties": false
        })
    }

    fn parse_candidate(&self, value: Value, slots: &Slots<'_>) -> Option<EgoCandidate> {
        let wire: CandidateFields = serde_json::from_value(value).ok()?;
        let subject = bounded_text(wire.subject, MAX_SUBJECT_BYTES)?;
        if !(1..=MAX_EVIDENCE_ENTRIES).contains(&wire.evidence.len()) {
            return None;
        }
        let mut evidence = Vec::new();
        for entry in wire.evidence {
            let point = bounded_text(entry.point, MAX_POINT_BYTES)?;
            let source = slots
                .source(entry.source_slot)
                .filter(|source| source.allows(Feature::Ego))?;
            let entry = EgoEvidence {
                citation: source.citation,
                point,
                status: entry.status,
                time_scope: entry.time_scope,
                excerpt: source.text.clone(),
            };
            if !evidence.contains(&entry) {
                evidence.push(entry);
            }
        }
        Some(EgoCandidate { subject, evidence })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateFields {
    subject: String,
    evidence: Vec<EvidenceFields>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceFields {
    source_slot: u16,
    point: String,
    status: Status,
    time_scope: TimeScope,
}

fn bounded_text(text: String, limit: usize) -> Option<String> {
    let text = text.trim();
    (!text.is_empty() && text.len() <= limit).then(|| text.to_owned())
}

/// Authorized owner assessment of whether a previously accepted exact citation still qualifies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceStanding {
    CurrentFresh,
    NoLongerFresh,
    Replaced { replacement: SourceCitation },
    Unavailable,
}

#[derive(Clone, Eq, PartialEq)]
pub struct EvidenceChange {
    pub subject: String,
    pub point: String,
    pub status: Status,
    pub time_scope: TimeScope,
    pub previous: SourceCitation,
    pub standing: EvidenceStanding,
}

pub struct RecheckedEvidence {
    pub retained: Vec<EgoCandidate>,
    pub changed: Vec<EvidenceChange>,
}

/// Keep points backed by unchanged Fresh evidence and hand off exact changes for affected points.
pub fn recheck_evidence(
    prior: Vec<EgoCandidate>,
    mut standing: impl FnMut(SourceCitation) -> EvidenceStanding,
) -> RecheckedEvidence {
    use std::collections::BTreeMap;

    let mut retained = Vec::new();
    let mut changed = Vec::new();
    let mut checks = BTreeMap::new();
    for mut candidate in prior {
        candidate.evidence.retain(|entry| {
            let value = *checks
                .entry(entry.citation)
                .or_insert_with(|| standing(entry.citation));
            if value == EvidenceStanding::CurrentFresh {
                return true;
            }
            changed.push(EvidenceChange {
                subject: candidate.subject.clone(),
                point: entry.point.clone(),
                status: entry.status,
                time_scope: entry.time_scope,
                previous: entry.citation,
                standing: value,
            });
            false
        });
        if !candidate.evidence.is_empty() {
            retained.push(candidate);
        }
    }
    RecheckedEvidence { retained, changed }
}

#[cfg(test)]
mod tests;
