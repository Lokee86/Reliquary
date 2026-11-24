use super::*;
use reliquary_container::ReliquaryScope;
use reliquary_owner_session::model::{MemoryId, MemoryTextField};
use reliquary_owner_session::observation::{
    ObservationBatch, ObservationChange, ObservationPendingReason, ObservationSource,
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
        let (claim, slots) = if state["revision"].is_null() {
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
    };
    let routes = ObservationDraftRoutes {
        general: &script,
        verification: &script,
        embedding: &script,
        route_identity: [3; 32],
        embedding_profile: [5; 32],
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
}
