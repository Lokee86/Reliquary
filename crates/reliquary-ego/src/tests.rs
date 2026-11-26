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
                    {"subject": "Pilot and launch", "source_slots": [0, 2, 0]},
                    {"subject": "Mixed source", "source_slots": [0, 1]}
                ],
                "lookups": []
            },
            "perception": { "candidates": [1], "lookups": [] }
        }))
    }
}

#[test]
fn topic_selects_distinct_exact_fresh_memory_references() {
    let fresh = passage(1, Grade::Fresh, Eligibility::BOTH);
    let stale = passage(2, Grade::Stale, Eligibility::PERCEPTION);
    let supporting = passage(3, Grade::Fresh, Eligibility::EGO);
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
            WorkItem {
                passage: supporting.clone(),
                mandatory: Eligibility::EGO,
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
    assert_eq!(exposure, vec![fresh.citation, supporting.citation]);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].subject, "Pilot and launch");
    assert_eq!(candidates[0].sources, [fresh.citation, supporting.citation]);
    let ListResult::Accepted { candidates, .. } = result.perception else {
        panic!("expected an independent Perception list");
    };
    assert_eq!(candidates, vec![stale.citation]);
}
