use super::*;
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::json;
use std::{cell::RefCell, collections::VecDeque, sync::Mutex};

const CUT: OwnerCut = OwnerCut {
    owner_id: [1; 16],
    history_generation: [2; 32],
    frontier: 40,
};

struct Contract;

impl CandidateContract for Contract {
    type Candidate = SourceCitation;

    fn instruction(&self) -> &'static str {
        "Return the slot of a relevant source."
    }

    fn candidate_schema(&self) -> Value {
        json!({"type": "integer"})
    }

    fn parse_candidate(&self, value: Value, slots: &Slots<'_>) -> Option<Self::Candidate> {
        let slot = u16::try_from(value.as_u64()?).ok()?;
        Some(slots.source(slot)?.citation)
    }
}

struct Endpoint {
    replies: Mutex<VecDeque<Result<Value, GeneralEndpointError>>>,
    requests: Mutex<Vec<(usize, Value)>>,
}

impl Endpoint {
    fn new(responses: impl IntoIterator<Item = Value>) -> Self {
        Self {
            replies: Mutex::new(responses.into_iter().map(Ok).collect()),
            requests: Mutex::new(Vec::new()),
        }
    }
}

impl GeneralEndpoint for Endpoint {
    fn model(&self) -> &str {
        "scripted"
    }

    fn complete_json(
        &self,
        system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert_eq!(schema_name, "memory_web_audit_v1");
        self.requests.lock().unwrap().push((
            system_prompt.len() + user_payload.len() + schema.to_string().len(),
            serde_json::from_str(user_payload).unwrap(),
        ));
        self.replies.lock().unwrap().pop_front().unwrap()
    }
}

fn item(id: u8, grade: Grade, eligible: Eligibility, text: &str) -> WorkItem {
    WorkItem {
        passage: Passage {
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
            text: text.into(),
        },
        mandatory: eligible,
    }
}

fn plan(work: &[WorkItem], limits: BatchLimits) -> Vec<Batch> {
    plan_batches(CUT, work, &[], limits, &Contract, &Contract).unwrap()
}

fn no_lookup(_: &OwnerCut, _: Feature, _: &str) -> Result<Vec<Passage>, LookupError> {
    panic!("unexpected lexical lookup")
}

fn reply(candidates: Value, lookups: &[&str]) -> Value {
    json!({ "candidates": candidates, "lookups": lookups })
}

fn accepted(
    result: ListResult<SourceCitation>,
) -> (Vec<SourceCitation>, Vec<SourceCitation>, usize) {
    let ListResult::Accepted {
        candidates,
        exposure,
        rejected,
    } = result
    else {
        panic!("expected accepted result");
    };
    (candidates, exposure, rejected)
}

#[test]
fn fitting_routine_and_max_memory_sets_fit_one_combined_request() {
    for count in [45, 125, 126] {
        let work: Vec<_> = (0..count)
            .map(|index| {
                let (grade, eligibility) = match index {
                    0..25 => (Grade::Fresh, Eligibility::BOTH),
                    25..40 => (Grade::Stale, Eligibility::PERCEPTION),
                    40..45 => (Grade::Dormant, Eligibility::PERCEPTION),
                    _ => (Grade::Fresh, Eligibility::BOTH),
                };
                item(index, grade, eligibility, "One bounded passage.")
            })
            .collect();
        let batches = plan(&work, BatchLimits::default());
        assert_eq!(batches.len(), if count > 125 { 2 } else { 1 });
        assert_eq!(
            batches[0].mandatory_exposure(Feature::Perception).count(),
            usize::from(count).min(125)
        );
        let endpoint = Endpoint::new([json!({
            "ego": reply(json!([0, 25]), &[]),
            "perception": reply(json!([25, 42]), &[])
        })]);
        let result =
            run_batch(&batches[0], &endpoint, &Contract, &Contract, &mut no_lookup).unwrap();
        assert_eq!(accepted(result.ego).2, 1);
        assert_eq!(accepted(result.perception).0.len(), 2);
        assert_eq!(endpoint.requests.lock().unwrap().len(), 1);
    }
}

#[test]
fn escaped_utf8_passages_preserve_exact_mandatory_ranges() {
    let contents = "🪨\n\tα\"\\\u{0000}".repeat(200);
    let mut input = item(1, Grade::Fresh, Eligibility::BOTH, &contents);
    input.passage.citation.start_byte = 5;
    input.passage.citation.end_byte += 5;
    let limits = BatchLimits {
        max_input_bytes: 1700,
        ..BatchLimits::default()
    };
    let batches = plan(std::slice::from_ref(&input), limits);
    assert!(batches.len() > 1);
    let endpoint = Endpoint::new((0..batches.len()).map(|_| {
        json!({
            "ego": reply(json!([]), &[]),
            "perception": reply(json!([]), &[]),
        })
    }));
    let mut previous_end = 5;
    for batch in &batches {
        let result = run_batch(batch, &endpoint, &Contract, &Contract, &mut no_lookup).unwrap();
        let exposure = accepted(result.ego).1;
        assert_eq!(accepted(result.perception).1, exposure);
        assert_eq!(exposure.len(), 1);
        assert_eq!(exposure[0].start_byte, previous_end);
        previous_end = exposure[0].end_byte;
        assert!(contents.is_char_boundary((previous_end - 5) as usize));
    }
    assert_eq!(previous_end, input.passage.citation.end_byte);
    let requests = endpoint.requests.lock().unwrap();
    let reconstructed: String = requests
        .iter()
        .map(|(len, packet)| {
            assert!(*len <= limits.max_input_bytes);
            packet["sources"][0]["text"].as_str().unwrap()
        })
        .collect();
    assert_eq!(reconstructed, contents);
}

#[test]
fn missing_or_malformed_list_leaves_valid_sibling_exposed() {
    let work = [item(1, Grade::Fresh, Eligibility::BOTH, "Fresh fact.")];
    let batches = plan(&work, BatchLimits::default());
    let endpoint = Endpoint::new([
        json!({ "ego": null, "perception": reply(json!([0, 800, "0"]), &[]) }),
        json!({ "ego": reply(json!([]), &[]) }),
        json!(["unreadable"]),
    ]);
    let run = || run_batch(&batches[0], &endpoint, &Contract, &Contract, &mut no_lookup);
    let first = run().unwrap();
    assert!(matches!(
        first.ego,
        ListResult::Incomplete(ListFailure::Malformed)
    ));
    let (candidates, exposure, rejected) = accepted(first.perception);
    assert_eq!(candidates, vec![work[0].passage.citation]);
    assert_eq!(exposure, candidates);
    assert_eq!(rejected, 2);
    let retry = run().unwrap();
    assert_eq!(accepted(retry.ego).1, exposure);
    assert!(matches!(
        retry.perception,
        ListResult::Incomplete(ListFailure::Malformed)
    ));
    assert!(matches!(run(), Err(AuditError::UnreadableResponse)));
}

#[test]
fn optional_lookups_are_scoped_and_never_count_as_exposure() {
    let work = [item(1, Grade::Fresh, Eligibility::BOTH, "Original fact.")];
    let batches = plan(&work, BatchLimits::default());
    let endpoint = Endpoint::new([
        json!({
            "ego": reply(json!([]), &["fresh", "over budget"]),
            "perception": reply(json!([]), &["older", "over budget"])
        }),
        json!({
            "ego": reply(json!([1, 2]), &[]),
            "perception": reply(json!([1, 2]), &[])
        }),
    ]);
    let queries = RefCell::new(Vec::new());
    let mut lookup = |cut: &OwnerCut, feature, query: &str| {
        assert_eq!(*cut, CUT);
        queries.borrow_mut().push((feature, query.to_string()));
        let (id, grade, eligible) = match feature {
            Feature::Ego => (2, Grade::Fresh, Eligibility::BOTH),
            Feature::Perception => (3, Grade::Dormant, Eligibility::PERCEPTION),
        };
        let mut foreign = item(4, grade, eligible, "Foreign text.").passage;
        foreign.citation.owner_id = [99; 16];
        Ok(vec![
            item(id, grade, eligible, "Optional hit.").passage,
            foreign,
        ])
    };
    let result = run_batch(&batches[0], &endpoint, &Contract, &Contract, &mut lookup).unwrap();
    let (ego, ego_exposure, ego_rejected) = accepted(result.ego);
    let (perception, perception_exposure, perception_rejected) = accepted(result.perception);
    assert_eq!(ego[0].source, SourceId::Memory([2; 32]));
    assert_eq!(perception[0].source, SourceId::Memory([3; 32]));
    assert_eq!(ego_exposure, vec![work[0].passage.citation]);
    assert_eq!(ego_exposure, perception_exposure);
    assert_eq!((ego_rejected, perception_rejected), (1, 1));
    assert_eq!(
        queries.into_inner(),
        vec![
            (Feature::Ego, "fresh".into()),
            (Feature::Perception, "older".into())
        ]
    );
    let requests = endpoint.requests.lock().unwrap();
    let follow_up = &requests[1].1;
    assert_eq!(follow_up["phase"], "follow_up");
    assert_eq!(follow_up["sources"].as_array().unwrap().len(), 3);
    assert_eq!(
        follow_up["sources"][1]["eligible"],
        json!({"ego": true, "perception": false})
    );
    assert_eq!(
        follow_up["sources"][2]["mandatory"],
        json!({"ego": false, "perception": false})
    );
}

#[test]
fn lookup_invalidation_and_response_caps_fail_without_sibling_loss() {
    let work = [item(1, Grade::Fresh, Eligibility::BOTH, "Original fact.")];
    let batches = plan(&work, BatchLimits::default());
    let endpoint = Endpoint::new([json!({
        "ego": reply(json!([]), &["fresh"]),
        "perception": reply(json!([]), &[])
    })]);
    let mut lookup = |_: &OwnerCut, _, _: &str| Err(LookupError::Invalidated);
    let result = run_batch(&batches[0], &endpoint, &Contract, &Contract, &mut lookup).unwrap();
    assert!(matches!(
        result.ego,
        ListResult::Incomplete(ListFailure::Lookup(LookupError::Invalidated))
    ));
    assert_eq!(
        accepted(result.perception).1,
        vec![work[0].passage.citation]
    );
    assert_eq!(endpoint.requests.lock().unwrap().len(), 1);

    let limits = BatchLimits {
        max_response_bytes: 32,
        ..BatchLimits::default()
    };
    let batches = plan(&work, limits);
    let endpoint = Endpoint::new([json!({
        "ego": reply(json!([]), &[]),
        "perception": reply(json!([]), &[])
    })]);
    assert!(matches!(
        run_batch(&batches[0], &endpoint, &Contract, &Contract, &mut no_lookup),
        Err(AuditError::UnreadableResponse)
    ));
}

#[test]
fn derived_sources_never_earn_or_serve_ego_exposure() {
    let memory = item(1, Grade::Fresh, Eligibility::BOTH, "A source Memory.");
    let mut derived = item(
        2,
        Grade::Fresh,
        Eligibility::BOTH,
        "A verified Observation.",
    );
    derived.passage.citation.source = SourceId::Observation([2; 32]);
    assert!(matches!(
        plan_batches(
            CUT,
            &[derived.clone()],
            &[],
            BatchLimits::default(),
            &Contract,
            &Contract
        ),
        Err(AuditError::InvalidInput)
    ));
    derived.mandatory = Eligibility::PERCEPTION;
    let batches = plan(&[memory, derived], BatchLimits::default());
    let endpoint = Endpoint::new([json!({
        "ego": reply(json!([1]), &[]),
        "perception": reply(json!([1]), &[])
    })]);
    let result = run_batch(&batches[0], &endpoint, &Contract, &Contract, &mut no_lookup).unwrap();
    assert_eq!(accepted(result.ego).2, 1);
    assert_eq!(
        accepted(result.perception).0[0].source,
        SourceId::Observation([2; 32])
    );
}
