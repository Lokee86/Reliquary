use reliquary_memory_audit::{
    BatchLimits, Eligibility, Feature, Grade, ListResult, OwnerCut, Participant, Passage,
    SourceCitation, SourceId, TextField, WorkItem, plan_batches, run_batch,
};
use reliquary_perception::{
    ExistingObservation, ExistingObservationRef, HitVerdict, ObservationClaim, ObservationProfile,
    ObservationProposal, PerceptionContract, ReceptorBinding, ReceptorCategory, RevisionContext,
    RevisionEvidence, StageBinding, StageError, StageLimits, Status, SynthesisDecision,
    SynthesisRequest, TimeScope, generate_receptors, receptor_fingerprint, synthesize,
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};
use std::sync::Mutex;

const CUT: OwnerCut = OwnerCut {
    owner_id: [1; 16],
    history_generation: [2; 32],
    frontier: 10,
};

struct Endpoint {
    reply: Value,
    calls: Mutex<Vec<(String, Value)>>,
}

impl Endpoint {
    fn new(reply: Value) -> Self {
        Self {
            reply,
            calls: Mutex::default(),
        }
    }
}

impl GeneralEndpoint for Endpoint {
    fn model(&self) -> &str {
        "recording-general"
    }

    fn complete_json(
        &self,
        _: &str,
        payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert_eq!(schema["additionalProperties"], false);
        self.calls.lock().unwrap().push((
            schema_name.to_owned(),
            serde_json::from_str(payload).unwrap(),
        ));
        Ok(self.reply.clone())
    }
}

fn passage(source: SourceId, text: &str, eligible: Eligibility) -> Passage {
    Passage {
        citation: SourceCitation {
            owner_id: CUT.owner_id,
            history_generation: CUT.history_generation,
            source,
            revision: 1,
            body_id: [3; 32],
            field: TextField::Content,
            start_byte: 0,
            end_byte: text.len() as u32,
        },
        grade: if eligible.ego {
            Grade::Fresh
        } else {
            Grade::Stale
        },
        eligible,
        text: text.into(),
    }
}

fn participant(id: u8, label: &str) -> Participant {
    Participant {
        owner_id: CUT.owner_id,
        entity_id: [id; 32],
        revision: 1,
        label: label.into(),
    }
}

fn wire_proposal(text: &str, source: u16, participants: &[u16]) -> Value {
    json!({
        "proposition": text, "source_slots": [source], "participant_slots": participants,
    })
}

#[test]
fn discovery_binds_ordinary_and_relational_candidates_to_eligible_slots_in_one_batch() {
    let relational_source = passage(
        SourceId::Memory([1; 32]),
        "Facilities manages plant access.",
        Eligibility::PERCEPTION,
    );
    let derived_source = passage(
        SourceId::Observation([2; 32]),
        "Entry uses a badge.",
        Eligibility::PERCEPTION,
    );
    let participants = [participant(1, "Facilities"), participant(2, "Plant")];
    let work: Vec<_> = [relational_source.clone(), derived_source.clone()]
        .into_iter()
        .map(|passage| WorkItem {
            passage,
            mandatory: Eligibility::PERCEPTION,
        })
        .collect();
    let endpoint = Endpoint::new(json!({
        "perception":{"lookups":[], "candidates":[
            wire_proposal("Entry uses a badge.", 1, &[]),
            wire_proposal("Facilities manages plant access.", 0, &[0, 1]),
            wire_proposal("Facilities manages plant access.", 1, &[0, 1]),
            wire_proposal("Facilities manages plant access.", 0, &[0, 0])
        ]}
    }));
    let ego = PerceptionContract::default();
    let relational = PerceptionContract {
        profile: ObservationProfile::Relational,
    };
    let batch = plan_batches(
        CUT,
        &work,
        &participants,
        BatchLimits::default(),
        &ego,
        &relational,
    )
    .unwrap();
    let mut lookup = |_: &OwnerCut, _: Feature, _: &str| panic!("unexpected lookup");
    let result = run_batch(&batch[0], &endpoint, &ego, &relational, &mut lookup).unwrap();
    let ListResult::Accepted {
        candidates,
        rejected,
        ..
    } = result.perception
    else {
        panic!("expected accepted list")
    };
    assert_eq!(rejected, 2);
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].profile, ObservationProfile::Ordinary);
    assert_eq!(candidates[0].sources, [derived_source.citation]);
    assert_eq!(candidates[1].profile, ObservationProfile::Relational);
    assert_eq!(candidates[1].sources, [relational_source.citation]);
    assert!(candidates[1].participants == participants);
    assert_eq!(endpoint.calls.lock().unwrap().len(), 1);
}

#[test]
fn synthesis_preserves_the_exact_draft_rejects_repeated_work_and_routes_checked_duplicates() {
    let original = passage(
        SourceId::Memory([2; 32]),
        "Facilities plans to require visitor badges next month.",
        Eligibility::PERCEPTION,
    );
    let narrower = passage(
        SourceId::Memory([3; 32]),
        "The badge plan applies to weekday visitors next month.",
        Eligibility::PERCEPTION,
    );
    let proposal = ObservationProposal {
        proposition: "Visitor badges are required.".into(),
        profile: ObservationProfile::Ordinary,
        sources: vec![original.citation],
        participants: vec![],
    };
    let sources = [original.clone(), narrower.clone()];
    let mut request = SynthesisRequest {
        binding: StageBinding {
            cut: CUT,
            proposal_id: [4; 32],
            route_identity: [5; 32],
        },
        proposal: &proposal,
        sources: &sources,
        participants: &[],
        existing: &[],
        revision: None,
    };
    let mut planned_claim = json!({
        "proposition": original.text, "status": "planned", "time_scope": "future",
        "source_slots": [0], "participants": [],
    });
    let endpoint = Endpoint::new(json!({"draft": planned_claim, "duplicate_slot": null}));
    let SynthesisDecision::Draft(first) =
        synthesize(&endpoint, &request, StageLimits::default()).unwrap()
    else {
        panic!("expected exact draft");
    };
    assert_eq!(first.claim.proposition, original.text);
    assert_eq!(first.claim.citations, [original.citation]);
    assert_eq!(first.claim.status, Status::Planned);
    assert_eq!(first.claim.time_scope, TimeScope::Future);
    {
        let calls = endpoint.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "observation_synthesis_v2");
        assert_eq!(
            calls[0].1["proposal"],
            json!({"proposition": proposal.proposition, "profile": "ordinary"})
        );
        assert_eq!(calls[0].1["source_slots"][0]["text"], original.text);
        assert_eq!(calls[0].1["proposal_sources"], json!([0]));
    }

    let revision = RevisionContext {
        prior: first.clone(),
        backcheck: vec![RevisionEvidence {
            citation: narrower.citation,
            disposition: HitVerdict::MaterialQualification,
        }],
    };
    request.revision = Some(&revision);
    assert!(matches!(
        synthesize(&endpoint, &request, StageLimits::default()),
        Err(StageError::InvalidReply)
    ));
    planned_claim["proposition"] = json!(narrower.text);
    planned_claim["source_slots"] = json!([1]);
    let mut endpoint = Endpoint::new(json!({"draft": planned_claim, "duplicate_slot": null}));
    let SynthesisDecision::Draft(successor) =
        synthesize(&endpoint, &request, StageLimits::default()).unwrap()
    else {
        panic!("expected a changed draft");
    };
    assert_ne!(first.id, successor.id);
    assert_eq!(successor.claim.citations, [narrower.citation]);
    assert_eq!(
        endpoint.calls.lock().unwrap()[0].1["revision"]["backcheck"],
        json!([{"source_slot":1,"disposition":"material_qualification"}])
    );

    let target = ExistingObservationRef {
        owner_id: CUT.owner_id,
        history_generation: CUT.history_generation,
        id: [8; 32],
        revision: 2,
        body_id: [9; 32],
    };
    let existing = [ExistingObservation {
        target,
        claim: successor.claim.clone(),
    }];
    request.existing = &existing;
    endpoint.reply["duplicate_slot"] = json!(0);
    assert!(matches!(
        synthesize(&endpoint, &request, StageLimits::default()),
        Ok(SynthesisDecision::Duplicate { target: selected, .. }) if selected == target
    ));
    endpoint.reply["draft"]["time_scope"] = json!("past");
    assert!(matches!(
        synthesize(&endpoint, &request, StageLimits::default()),
        Err(StageError::InvalidReply)
    ));
    endpoint.reply = json!({"draft":null,"duplicate_slot":null});
    assert!(matches!(
        synthesize(&endpoint, &request, StageLimits::default()),
        Ok(SynthesisDecision::Abstained)
    ));
    let before = endpoint.calls.lock().unwrap().len();
    assert!(matches!(
        synthesize(
            &endpoint,
            &request,
            StageLimits {
                max_input_bytes: 128,
                ..StageLimits::default()
            }
        ),
        Err(StageError::InputLimit)
    ));
    assert_eq!(endpoint.calls.lock().unwrap().len(), before);
}

#[test]
fn receptors_bind_the_exact_claim_and_embedding_profile_with_separate_descriptions() {
    let source = passage(
        SourceId::Memory([2; 32]),
        "Visitors need badges on weekdays.",
        Eligibility::PERCEPTION,
    );
    let mut draft = reliquary_perception::SynthesisDraft {
        id: [1; 32],
        claim: ObservationClaim {
            proposition: source.text,
            profile: ObservationProfile::Ordinary,
            status: Status::Observed,
            time_scope: TimeScope::Current,
            citations: vec![source.citation],
            participants: vec![],
        },
    };
    let binding = ReceptorBinding {
        route_identity: [2; 32],
        embedding_profile: [3; 32],
    };
    let mut endpoint = Endpoint::new(json!({
        "support":["Weekday visitor badge requirement", "Weekday guard enforcing visitor badge checks"],
        "challenge":["Weekday unbadged entry permitted"],
        "qualification":["Exception for escorted weekday visits"],
        "other":["Authoritative definition of covered visitor groups"]
    }));
    let set = generate_receptors(&endpoint, &draft, binding, StageLimits::default()).unwrap();
    assert_eq!(endpoint.calls.lock().unwrap().len(), 1);
    assert_eq!(set.receptors.len(), 5);
    assert_eq!(set.receptors[1].category, ReceptorCategory::Support);
    assert_ne!(set.receptors[0].description, set.receptors[1].description);
    assert_eq!(set.receptors[3].category, ReceptorCategory::Qualification);
    draft.id = [4; 32];
    assert_eq!(set.binding, receptor_fingerprint(&draft, binding));
    let new_profile = ReceptorBinding {
        embedding_profile: [5; 32],
        ..binding
    };
    assert_ne!(set.binding, receptor_fingerprint(&draft, new_profile));
    draft
        .claim
        .proposition
        .push_str(" Escorted visits are exempt.");
    assert_ne!(set.binding, receptor_fingerprint(&draft, binding));
    endpoint.reply["qualification"] = json!(["Weekday visitor badge requirement"]);
    assert!(matches!(
        generate_receptors(&endpoint, &draft, binding, StageLimits::default()),
        Err(StageError::InvalidReply)
    ));
}
