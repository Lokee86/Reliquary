use super::*;
use reliquary_memory_audit::{
    BatchLimits, Eligibility, Feature, Grade, ListResult, OwnerCut, Passage, SourceId, TextField,
    WorkItem, plan_batches, run_batch,
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};

const CUT: OwnerCut = OwnerCut {
    owner_id: [1; 16],
    history_generation: [2; 32],
    frontier: 40,
};

fn passage(id: u8, grade: Grade, eligible: Eligibility) -> Passage {
    let text = "The pilot was approved. The launch is planned.".to_owned();
    Passage {
        citation: SourceCitation {
            owner_id: CUT.owner_id,
            history_generation: CUT.history_generation,
            source: SourceId::Memory([id; 32]),
            revision: 1,
            body_id: [id; 32],
            field: TextField::Content,
            start_byte: 0,
            end_byte: text.len() as u32,
        },
        grade,
        eligible,
        text,
    }
}

struct Perception;

impl CandidateContract for Perception {
    type Candidate = SourceCitation;

    fn instruction(&self) -> &'static str {
        "Return the relevant source slot."
    }

    fn candidate_schema(&self) -> Value {
        json!({ "type": "integer" })
    }

    fn parse_candidate(&self, value: Value, slots: &Slots<'_>) -> Option<Self::Candidate> {
        Some(slots.source(u16::try_from(value.as_u64()?).ok()?)?.citation)
    }
}

struct Endpoint;

impl GeneralEndpoint for Endpoint {
    fn model(&self) -> &str {
        "scripted"
    }

    fn complete_json(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        Ok(json!({
            "ego": {
                "candidates": [
                    {"subject": "Pilot and launch", "evidence": [
                        {"source_slot": 0, "point": "Pilot approval", "status": "observed", "time_scope": "past"},
                        {"source_slot": 0, "point": "Launch", "status": "planned", "time_scope": "future"}
                    ]},
                    {"subject": "Mixed source", "evidence": [
                        {"source_slot": 0, "point": "Pilot approval", "status": "observed", "time_scope": "past"},
                        {"source_slot": 1, "point": "Old claim", "status": "possible", "time_scope": "current"}
                    ]}
                ],
                "lookups": []
            },
            "perception": { "candidates": [1], "lookups": [] }
        }))
    }
}

#[test]
fn fresh_subject_can_have_multiple_typed_entries_from_one_source() {
    let fresh = passage(1, Grade::Fresh, Eligibility::BOTH);
    let stale = passage(2, Grade::Stale, Eligibility::PERCEPTION);
    let batches = plan_batches(
        CUT,
        &[
            WorkItem {
                passage: fresh.clone(),
                mandatory: Eligibility::BOTH,
            },
            WorkItem {
                passage: stale.clone(),
                mandatory: Eligibility::PERCEPTION,
            },
        ],
        &[],
        BatchLimits::default(),
        &EgoContract,
        &Perception,
    )
    .unwrap();
    let mut lookup = |_: &OwnerCut, _: Feature, _: &str| panic!("unexpected lookup");
    let result = run_batch(
        &batches[0],
        &Endpoint,
        &EgoContract,
        &Perception,
        &mut lookup,
    )
    .unwrap();
    let ListResult::Accepted {
        candidates,
        exposure,
        rejected,
    } = result.ego
    else {
        panic!("expected a valid Ego list");
    };
    assert_eq!(rejected, 1);
    assert_eq!(exposure, vec![fresh.citation]);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].evidence.len(), 2);
    assert_eq!(candidates[0].evidence[0].excerpt, fresh.text);
    assert_eq!(candidates[0].evidence[0].status, Status::Observed);
    assert_eq!(candidates[0].evidence[1].status, Status::Planned);
    assert_eq!(candidates[0].evidence[1].time_scope, TimeScope::Future);
    let ListResult::Accepted { candidates, .. } = result.perception else {
        panic!("expected an independent Perception list");
    };
    assert_eq!(candidates, vec![stale.citation]);
}

#[test]
fn exact_evidence_change_preserves_unaffected_points() {
    let past = passage(1, Grade::Fresh, Eligibility::EGO);
    let current = passage(2, Grade::Fresh, Eligibility::EGO);
    let evidence = |passage: Passage, point: &str| EgoEvidence {
        citation: passage.citation,
        point: point.into(),
        excerpt: passage.text,
        status: Status::Observed,
        time_scope: TimeScope::Current,
    };
    let candidate = EgoCandidate {
        subject: "Pilot".into(),
        evidence: vec![
            evidence(past.clone(), "Past point"),
            evidence(current.clone(), "Current point"),
        ],
    };
    let checked = recheck_evidence(vec![candidate], |citation| {
        if citation == past.citation {
            EvidenceStanding::NoLongerFresh
        } else {
            EvidenceStanding::CurrentFresh
        }
    });
    assert_eq!(checked.retained.len(), 1);
    assert_eq!(checked.retained[0].evidence.len(), 1);
    assert_eq!(checked.retained[0].evidence[0].citation, current.citation);
    assert_eq!(checked.changed.len(), 1);
    assert_eq!(checked.changed[0].subject, "Pilot");
    assert_eq!(checked.changed[0].previous, past.citation);
    assert_eq!(checked.changed[0].standing, EvidenceStanding::NoLongerFresh);
}
