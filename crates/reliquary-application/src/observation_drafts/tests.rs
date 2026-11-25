use super::*;
use reliquary_container::ReliquaryScope;
use reliquary_owner_session::model::{MemoryId, MemoryTextField};
use reliquary_owner_session::observation::{
    ObservationBatch, ObservationChange, ObservationPendingReason, ObservationSource,
    ObservationWithdrawalReason,
};
use reliquary_owner_session::{DreamMemoryCut, RelWriter};
use reliquary_perception::NATIVE_DECISION_MODEL;
use reliquary_switchboard::{
    DecisionEndpointError, EmbeddingEndpointError, EmbeddingNormalization, GeneralEndpointError,
};
use std::path::PathBuf;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

const CLAIM: &str = "Gallery access requires a staffed desk.";
const EXCEPTION: &str =
    "An exception allows members through the east entrance without a staffed desk.";
const NARROWED: &str =
    "Gallery access requires a staffed desk except for members using the east entrance.";

struct Path(PathBuf);

impl Path {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        std::fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "observation-drafts-{}-{}.rel",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for Path {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct Read<'a> {
    writer: &'a RelWriter,
    held: &'a AtomicBool,
}

impl ScopedObservationRead for Read<'_> {
    fn read<T>(&self, capture: impl FnOnce(&RelSession) -> Result<T>) -> Result<T> {
        assert!(!self.held.swap(true, Ordering::SeqCst));
        let result = capture(self.writer.read_session());
        self.held.store(false, Ordering::SeqCst);
        result
    }
}

struct Script<'a> {
    held: &'a AtomicBool,
    decisions: Mutex<Vec<Value>>,
    embeddings: Mutex<Vec<Vec<String>>>,
    replacement_support: Option<&'static str>,
}

impl GeneralEndpoint for Script<'_> {
    fn model(&self) -> &str {
        "scripted-general"
    }

    fn complete_json(
        &self,
        _: &str,
        packet: &str,
        schema_name: &str,
        _: &Value,
    ) -> std::result::Result<Value, GeneralEndpointError> {
        assert!(!self.held.load(Ordering::SeqCst));
        if schema_name == "observation_receptors_v1" {
            return Ok(json!({
                "support":["Gallery entrance staffed desk requirement"],
                "challenge":["Gallery entrance without a staffed desk"],
                "qualification":["Gallery exception for members at the east entrance"],
                "other":["Change to access schedule at the gallery"]
            }));
        }
        assert_eq!(schema_name, "observation_synthesis_v1");
        let state: Value = serde_json::from_str(packet).unwrap();
        let (claim, slots) = if let Some(replacement) = self.replacement_support {
            assert!(!state["revision"].is_null());
            let rows = state["source_slots"].as_array().unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["text"], replacement);
            (CLAIM, vec![0])
        } else if state["revision"].is_null() {
            (CLAIM, vec![0])
        } else {
            let slot = state["source_slots"]
                .as_array()
                .unwrap()
                .iter()
                .position(|row| row["text"].as_str().unwrap().contains(EXCEPTION))
                .unwrap();
            assert_eq!(
                state["revision"]["backcheck"][0]["disposition"],
                "material_qualification"
            );
            (NARROWED, vec![0, slot])
        };
        Ok(json!({
            "draft": {"proposition":claim,"status":"observed","time_scope":"current","source_slots":slots,"participants":[]},
            "duplicate_slot":null
        }))
    }
}

impl DecisionEndpoint for Script<'_> {
    fn model(&self) -> &str {
        NATIVE_DECISION_MODEL
    }

    fn evaluate(
        &self,
        state: &Value,
        questions: &Value,
    ) -> std::result::Result<Value, DecisionEndpointError> {
        assert!(!self.held.load(Ordering::SeqCst));
        let mut calls = self.decisions.lock().unwrap();
        calls.push(state.clone());
        let revising = calls.len() == 2;
        let mut answers = serde_json::Map::new();
        for id in questions.as_object().unwrap().keys() {
            let choice = if id == "verify" {
                if revising {
                    "needs_revision"
                } else {
                    "stands_as_written"
                }
            } else if revising {
                "material_qualification"
            } else {
                "no_material_effect"
            };
            answers.insert(id.clone(), json!({"type":"choice","choice":choice}));
        }
        Ok(json!({"answers":answers}))
    }
}

impl EmbeddingEndpoint for Script<'_> {
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> EmbeddingNormalization {
        EmbeddingNormalization::L2
    }
    fn supports_bounded_embedding(&self) -> bool {
        true
    }
    fn embed_bounded(
        &self,
        mode: EmbeddingMode,
        phrases: &[String],
        _: usize,
    ) -> std::result::Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        assert!(!self.held.load(Ordering::SeqCst));
        assert_eq!(mode, EmbeddingMode::Query);
        self.embeddings.lock().unwrap().push(phrases.to_vec());
        Ok(vec![vec![1.0, 0.0]; phrases.len()])
    }
    fn embed(
        &self,
        _: EmbeddingMode,
        _: &[String],
    ) -> std::result::Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        panic!("expected bounded embedding")
    }
}

#[test]
fn prepares_and_rechecks_a_verified_successor_outside_owner_read_closures() {
    let path = Path::new();
    let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, [13; 16]).unwrap();
    crate::commit_native_conversation_turn(
        &mut writer,
        crate::NativeConversationCommit {
            node_id: "turn".into(),
            conversation_id: "observation-fixture".into(),
            parent_node_id: None,
            branch_id: "main".into(),
            principal_id: "fixture".into(),
            committed_timestamp_ns: 1,
            content: "Accepted gallery policy source.".into(),
        },
    )
    .unwrap();
    let content = format!("{CLAIM}\n{EXCEPTION}");
    crate::create_direct_memory(
        &mut writer,
        "fixture",
        "Gallery policy".into(),
        content.clone(),
        "access".into(),
        "policy".into(),
        0,
    )
    .unwrap();
    let id: MemoryId = writer.read_session().perception_source_ids_page(None, 1)[0];
    let owner = writer.read_session().management_read();
    let row = owner.memory_record(id, None).unwrap();
    let source = DreamMemoryCut {
        id,
        revision: row.revision,
        body_id: row.body_id,
    };
    let turn = writer.read_session().rel_turn_count();
    writer
        .begin_first_empty_dream_pass("observation-fixture".into(), source, turn)
        .unwrap();
    let graph = writer.read_session().graph_version();
    assert!(
        writer
            .commit_empty_dream_pass("observation-fixture", source, graph, turn)
            .unwrap()
    );
    let capture = writer.read_session().perception_source_cut();
    let cut = OwnerCut {
        owner_id: capture.owner_id,
        history_generation: capture.history_generation,
        frontier: capture.memory,
    };
    let cite = ObservationCitation {
        owner_id: cut.owner_id,
        history_generation: cut.history_generation,
        source: ObservationSource::Memory(id.0),
        revision: source.revision,
        body_id: source.body_id.0,
        field: MemoryTextField::Content,
        start_byte: 0,
        end_byte: CLAIM.len() as u32,
    };
    let incoming = ObservationCitation {
        start_byte: (CLAIM.len() + 1) as u32,
        end_byte: content.len() as u32,
        ..cite
    };
    let proposal = ProposedClaim {
        proposition: CLAIM.into(),
        profile: reliquary_perception::ObservationProfile::Ordinary,
        status: Status::Observed,
        time_scope: TimeScope::Current,
        citations: vec![audit_citation(cite)],
        participants: Vec::new(),
    };
    let held = AtomicBool::new(false);
    let script = Script {
        held: &held,
        decisions: Mutex::default(),
        embeddings: Mutex::default(),
        replacement_support: None,
    };
    let routes = ObservationDraftRoutes {
        general: &script,
        verification: &script,
        embedding: &script,
        route_identity: [3; 32],
        embedding_profile: [5; 32],
        memory_vector_profile: None,
    };
    let ProposalEvaluation::Ready(prepared) = evaluate_proposal(
        &Read {
            writer: &writer,
            held: &held,
        },
        &routes,
        cut,
        &proposal,
    )
    .unwrap() else {
        panic!("expected a verified draft");
    };
    assert_eq!(writer.read_session().observation_publication_version(), 0);
    prepared
        .revalidate(writer.read_session(), routes.route_identity)
        .unwrap();
    let receipt = writer
        .publish_observation_batch(ObservationBatch {
            owner_id: cut.owner_id,
            history_generation: cut.history_generation,
            input_fingerprint: prepared.input_fingerprint,
            policy: prepared.policy,
            operations: vec![prepared.operation],
            progress: None,
        })
        .unwrap();
    let (id, revision) = receipt.revisions[0];
    writer
        .publish_observation_batch(ObservationBatch {
            owner_id: cut.owner_id,
            history_generation: cut.history_generation,
            input_fingerprint: [22; 32],
            policy: prepared.policy,
            operations: vec![ObservationOperation::Pending {
                id,
                expected_revision: revision,
                reason: ObservationPendingReason::NewEvidence,
                evidence: vec![incoming],
            }],
            progress: None,
        })
        .unwrap();
    let review = evaluate_review(
        &Read {
            writer: &writer,
            held: &held,
        },
        &routes,
        id,
    )
    .unwrap()
    .unwrap();
    review
        .revalidate(writer.read_session(), routes.route_identity)
        .unwrap();
    assert!(matches!(
        review.revalidate(writer.read_session(), [42; 32]),
        Err(Error::Stale)
    ));
    let ObservationOperation::Successor { draft, change, .. } = &review.operation else {
        panic!("expected successor")
    };
    assert_eq!(*change, ObservationChange::Qualification);
    assert_eq!(draft.proposition, NARROWED);
    assert!(draft.citations.contains(&incoming));
    let requests = script.decisions.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[1]["claim"], CLAIM);
    assert_eq!(requests[1]["backcheck_hits"][0]["text"], EXCEPTION);
    assert_eq!(
        requests[1]["citations"][0]["source_id"],
        requests[1]["backcheck_hits"][0]["source_id"]
    );
    assert_eq!(requests[2]["claim"], NARROWED);
    assert_eq!(requests[2]["backcheck_hits"].as_array().unwrap().len(), 0);
    drop(requests);
    assert_eq!(script.embeddings.lock().unwrap().len(), 2);
    let accepted = writer
        .publish_observation_batch(ObservationBatch {
            owner_id: cut.owner_id,
            history_generation: cut.history_generation,
            input_fingerprint: review.input_fingerprint,
            policy: review.policy,
            operations: vec![review.operation],
            progress: None,
        })
        .unwrap();
    assert_eq!(accepted.revisions, [(id, revision + 1)]);
    assert_eq!(
        writer
            .read_session()
            .observation(id)
            .unwrap()
            .revision
            .draft
            .proposition,
        NARROWED
    );

    let publish = |writer: &mut RelWriter, key: u8, operations| {
        writer
            .publish_observation_batch(ObservationBatch {
                owner_id: cut.owner_id,
                history_generation: cut.history_generation,
                input_fingerprint: [key; 32],
                policy: review.policy,
                operations,
                progress: None,
            })
            .unwrap()
    };
    let mut trigger = writer
        .read_session()
        .observation(id)
        .unwrap()
        .revision
        .draft
        .clone();
    trigger.proposition = "Members use the east entrance without a staffed desk.".into();
    trigger.citations = vec![incoming];
    trigger.reviewed.clear();
    let (trigger_id, trigger_revision) = publish(
        &mut writer,
        23,
        vec![ObservationOperation::Create { draft: trigger }],
    )
    .revisions[0];
    let historical_trigger = writer
        .read_session()
        .observation(trigger_id)
        .unwrap()
        .revision
        .full_citation();
    publish(
        &mut writer,
        24,
        vec![
            ObservationOperation::Pending {
                id,
                expected_revision: revision + 1,
                reason: ObservationPendingReason::NewEvidence,
                evidence: vec![historical_trigger],
            },
            ObservationOperation::Pending {
                id: trigger_id,
                expected_revision: trigger_revision,
                reason: ObservationPendingReason::ChangedSupport,
                evidence: Vec::new(),
            },
        ],
    );
    publish(
        &mut writer,
        25,
        vec![ObservationOperation::Withdraw {
            id: trigger_id,
            expected_revision: trigger_revision,
            reason: ObservationWithdrawalReason::UnresolvedEvidence,
            reviewed: Vec::new(),
        }],
    );
    let reaffirmed = evaluate_review(
        &Read {
            writer: &writer,
            held: &held,
        },
        &routes,
        id,
    )
    .unwrap()
    .unwrap();
    reaffirmed
        .revalidate(writer.read_session(), routes.route_identity)
        .unwrap();
    assert!(
        matches!(&reaffirmed.operation, ObservationOperation::Reaffirm { reviewed, .. }
        if reviewed.contains(&historical_trigger))
    );
    let calls = script.decisions.lock().unwrap();
    assert_eq!(calls.len(), 4);
    assert_eq!(calls[3]["backcheck_hits"].as_array().unwrap().len(), 0);
}

#[test]
fn review_after_source_removal_verifies_same_wording_with_exact_retained_support() {
    use reliquary_owner_session::model::{MemoryDraft, MemorySourceRef};

    const RETAINED: &str = "A concierge must be on duty to admit visitors.";
    let path = Path::new();
    let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, [13; 16]).unwrap();
    let prepare_source = |writer: &mut RelWriter, key: &str, content: &str| {
        crate::commit_native_conversation_turn(
            writer,
            crate::NativeConversationCommit {
                node_id: "turn".into(),
                conversation_id: key.into(),
                parent_node_id: None,
                branch_id: "main".into(),
                principal_id: "fixture".into(),
                committed_timestamp_ns: 1,
                content: content.into(),
            },
        )
        .unwrap();
        let memory = writer
            .publish_direct_memory(
                "fixture",
                MemoryDraft {
                    title: key.into(),
                    content: content.into(),
                    category: "fact".into(),
                    memory_type: "semantic".into(),
                    authority_kind: "direct".into(),
                    temporal_status: "current".into(),
                    scope: "local".into(),
                    lifecycle_state: "knowledge".into(),
                    archived: false,
                    parent_id: None,
                    superseded_by: None,
                    source_node_id: None,
                    content_source_conversation_id: None,
                    content_source_node_id: None,
                    grounding_source_conversation_id: None,
                    grounding_source_node_id: None,
                    source_episode_id: None,
                    source_ref: Some(MemorySourceRef {
                        owner_id: "0d".repeat(16),
                        principal_id: Some("fixture".into()),
                        source_episode_id: [0; 32],
                        source_node_id: "turn".into(),
                        content_source_conversation_id: Some(key.into()),
                        content_source_node_id: Some("turn".into()),
                        grounding_source_conversation_id: None,
                        grounding_source_node_id: None,
                    }),
                    source_time_ns: Some(1),
                    temporal_inference: None,
                    mutation_id: key.into(),
                    created_at_ns: 1,
                    updated_at_ns: 1,
                },
            )
            .unwrap()
            .record;
        let cut = writer.read_session().perception_source_cut();
        let observation = DreamMemoryCut {
            id: memory.id,
            revision: memory.revision,
            body_id: memory.body_id,
        };
        writer
            .begin_first_empty_dream_pass(key.into(), observation, cut.rel_turn)
            .unwrap();
        assert!(
            writer
                .commit_empty_dream_pass(key, observation, cut.graph, cut.rel_turn)
                .unwrap()
        );
        ObservationCitation {
            owner_id: cut.owner_id,
            history_generation: cut.history_generation,
            source: ObservationSource::Memory(memory.id.0),
            revision: memory.revision,
            body_id: memory.body_id.0,
            field: MemoryTextField::Content,
            start_byte: 0,
            end_byte: content.len() as u32,
        }
    };
    let removed = prepare_source(&mut writer, "copied", CLAIM);
    let supporting = prepare_source(&mut writer, "independent", RETAINED);
    let policy = ObservationPolicy {
        version: 1,
        general_route: [3; 32],
        decision_route: [3; 32],
    };
    let request = |input_fingerprint, operations| ObservationBatch {
        owner_id: removed.owner_id,
        history_generation: removed.history_generation,
        input_fingerprint,
        policy,
        operations,
        progress: None,
    };
    let row = writer
        .publish_observation_batch(request(
            [1; 32],
            vec![ObservationOperation::Create {
                draft: ObservationDraft {
                    proposition: CLAIM.into(),
                    profile: ObservationProfile::Ordinary,
                    status: ObservationStatus::Observed,
                    time_scope: ObservationTimeScope::Current,
                    validity: None,
                    citations: vec![removed],
                    reviewed: vec![supporting],
                    receptors: [
                        ObservationReceptorKind::Support,
                        ObservationReceptorKind::Challenge,
                        ObservationReceptorKind::Qualification,
                        ObservationReceptorKind::Change,
                    ]
                    .into_iter()
                    .map(|kind| ObservationReceptor {
                        kind,
                        description: "Gallery policy".into(),
                        embedding_profile: [5; 32],
                        vector: vec![1., 0.],
                    })
                    .collect(),
                },
            }],
        ))
        .unwrap();
    let (id, _) = row.revisions[0];
    assert!(writer.tombstone_conversation("copied").unwrap());
    assert_eq!(
        writer.read_session().observation(id).unwrap().standing,
        ObservationStanding::Pending(ObservationPendingReason::ChangedSupport)
    );
    let held = AtomicBool::new(false);
    let script = Script {
        held: &held,
        decisions: Mutex::default(),
        embeddings: Mutex::default(),
        replacement_support: Some(RETAINED),
    };
    let routes = ObservationDraftRoutes {
        general: &script,
        verification: &script,
        embedding: &script,
        route_identity: [3; 32],
        embedding_profile: [5; 32],
        memory_vector_profile: None,
    };
    let review = evaluate_review(
        &Read {
            writer: &writer,
            held: &held,
        },
        &routes,
        id,
    )
    .unwrap()
    .unwrap();
    review
        .revalidate(writer.read_session(), routes.route_identity)
        .unwrap();
    let ObservationOperation::Successor {
        draft,
        change: ObservationChange::Equivalent,
        ..
    } = &review.operation
    else {
        panic!("expected successor with unchanged wording");
    };
    assert_eq!(draft.proposition, CLAIM);
    assert_eq!(draft.citations, [supporting]);
    let calls = script.decisions.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["citations"][0]["text"], RETAINED);
    drop(calls);
    writer
        .publish_observation_batch(request(review.input_fingerprint, vec![review.operation]))
        .unwrap();
    assert_eq!(
        writer.read_session().observation(id).unwrap().standing,
        ObservationStanding::Current
    );
}
