use crate::*;
use reliquary_container::Container;
use reliquary_dream::{DreamClassifier, DreamVerificationPolicy, DreamVerifier};
use reliquary_memory_core::{MemoryDraft, MemoryId, MemoryStore};
use reliquary_owner_session::model::RelationKind;
use reliquary_owner_session::{
    DreamMemoryCut, GraphAdmission, PhySession, PhyWriter, RelSession, RelWriter,
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
static NEXT: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&dir).unwrap();
        Self(dir.join(format!(
            "batch-{}-{}.rel",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
#[derive(Clone)]
struct Endpoint {
    relation: &'static str,
    verdict: &'static str,
}
impl GeneralEndpoint for Endpoint {
    fn model(&self) -> &str {
        "batch-model"
    }
    fn complete_json(
        &self,
        _: &str,
        payload: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        if schema == "dream_pair_verification" {
            return Ok(
                json!({"relation_supported":self.verdict,"direction_supported":self.verdict,"evidence_supported":self.verdict}),
            );
        }
        let data: Value = serde_json::from_str(payload).unwrap();
        let direction = if self.relation == "supersedes" {
            if data["a"]["source_timestamp_ns"].as_i64() > data["b"]["source_timestamp_ns"].as_i64()
            {
                "a_to_b"
            } else {
                "b_to_a"
            }
        } else if self.relation == "none" {
            "none"
        } else {
            "undirected"
        };
        Ok(
            json!({"relation":self.relation,"direction":direction,"evidence":if self.relation=="none" {json!([])}else{json!([{"side":"a","quote":"Rollout"},{"side":"b","quote":"Rollout"}])}}),
        )
    }
}
fn memory(mutation: &str) -> MemoryDraft {
    MemoryDraft {
        title: "Rollout plan".into(),
        content: "Rollout evidence for this owner.".into(),
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "user".into(),
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
        source_ref: None,
        source_time_ns: Some(1),
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn graph(owner: [u8; 16]) -> GraphAdmission {
    GraphAdmission::for_replay(owner, [], [], [], [], [])
}
fn fixture(rel: bool) -> (Temp, [u8; 16], DreamMemoryCut, Vec<DreamMemoryCut>) {
    let path = Temp::new();
    let owner = [219; 16];
    if rel {
        drop(
            RelWriter::create_path(&path.0, reliquary_container::ReliquaryScope::Project, owner)
                .unwrap(),
        );
    } else {
        drop(PhyWriter::create_path(&path.0, owner).unwrap());
    }
    let mut c = Container::open_writer(&path.0).unwrap();
    let mut m = MemoryStore::open(&c).unwrap();
    let mut records = Vec::new();
    for n in 0..3 {
        let mut draft = memory(&format!("batch-{n}"));
        draft.source_time_ns = Some(n + 1);
        draft.updated_at_ns = n + 1;
        if n == 2 {
            draft.lifecycle_state = "extracted".into();
        }
        records.push(m.publish(&mut c, None, 0, draft).unwrap().revision.record);
    }
    if rel {
        let mut f = reliquary_freshness_store::FreshnessStore::open(&c).unwrap();
        for (n, r) in records.iter().enumerate() {
            f.create_birth(
                &mut c,
                reliquary_freshness_store::FreshnessMemoryId(r.id.0),
                if n == 2 { 3 } else { 1 },
                n != 2,
            )
            .unwrap();
        }
    }
    c.sync().unwrap();
    let cuts: Vec<_> = records
        .iter()
        .map(|r| DreamMemoryCut {
            id: r.id,
            revision: r.revision,
            body_id: r.body_id,
        })
        .collect();
    (path, owner, cuts[2], cuts[..2].to_vec())
}
fn begin(
    path: &Temp,
    owner: [u8; 16],
    rel: bool,
    source: DreamMemoryCut,
    candidates: Vec<DreamMemoryCut>,
) {
    if rel {
        RelWriter::open_path(&path.0, graph(owner), [], [])
            .unwrap()
            .begin_initial_dream_frontier("initial".into(), source, candidates, 501)
            .unwrap();
    } else {
        PhyWriter::open_path(&path.0, &graph(owner), [])
            .unwrap()
            .begin_initial_dream_frontier("initial".into(), source, candidates, 501)
            .unwrap();
    }
}
fn draft(
    path: &Temp,
    owner: [u8; 16],
    rel: bool,
    source: MemoryId,
    candidates: &[MemoryId],
    relation: &'static str,
    verdict: &'static str,
) -> DreamOrganizationDraft {
    let r;
    let p;
    let view = if rel {
        r = RelSession::open_path(&path.0, &graph(owner), [], []).unwrap();
        EntityOwnerSession::Rel(&r)
    } else {
        p = PhySession::open_path(&path.0, &graph(owner), []).unwrap();
        EntityOwnerSession::Phy(&p)
    };
    draft_dream_batch_organization(
        &view,
        owner,
        source,
        candidates,
        &DreamClassifier::new(Endpoint { relation, verdict }),
        Some(&DreamVerifier::new(Endpoint { relation, verdict })),
        DreamVerificationPolicy::default(),
    )
    .unwrap()
}
fn publish(
    path: &Temp,
    owner: [u8; 16],
    rel: bool,
    key: [u8; 32],
    draft: &DreamOrganizationDraft,
) -> Result<bool, DreamWorkflowError> {
    if rel {
        let mut w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
        commit_dream_batch(&mut DreamOwnerWriter::Rel(&mut w), owner, key, draft, 100)
    } else {
        let mut w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
        commit_dream_batch(&mut DreamOwnerWriter::Phy(&mut w), owner, key, draft, 100)
    }
}
fn settle(
    path: &Temp,
    owner: [u8; 16],
    rel: bool,
    source: MemoryId,
) -> Result<bool, DreamWorkflowError> {
    let empty = draft(path, owner, rel, source, &[], "none", "yes");
    if rel {
        let mut w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
        settle_initial_dream_cycle(
            &mut DreamOwnerWriter::Rel(&mut w),
            owner,
            "initial",
            &empty,
            501,
        )
    } else {
        let mut w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
        settle_initial_dream_cycle(
            &mut DreamOwnerWriter::Phy(&mut w),
            owner,
            "initial",
            &empty,
            501,
        )
    }
}

#[test]
fn native_import_retries_lifecycle_conflicts_in_candidate_order() {
    struct ConflictingProposal {
        calls: Arc<Mutex<Vec<i64>>>,
        persists: bool,
    }

    impl GeneralEndpoint for ConflictingProposal {
        fn model(&self) -> &str {
            "batch-model"
        }

        fn complete_json(
            &self,
            system: &str,
            payload: &str,
            schema: &str,
            definition: &Value,
        ) -> Result<Value, GeneralEndpointError> {
            let data: Value = serde_json::from_str(payload).unwrap();
            let candidate = data["a"]["source_timestamp_ns"]
                .as_i64()
                .unwrap()
                .min(data["b"]["source_timestamp_ns"].as_i64().unwrap());
            let mut calls = self.calls.lock().unwrap();
            let relation = match candidate {
                1 => "supersedes",
                2 if self.persists || calls.len() < 2 => "duplicate_of",
                2 => "none",
                _ => panic!("unexpected candidate"),
            };
            calls.push(candidate);
            Endpoint {
                relation,
                verdict: "yes",
            }
            .complete_json(system, payload, schema, definition)
        }
    }

    for persists in [false, true] {
        let (path, owner, source, candidates) = fixture(true);
        let ids: Vec<_> = candidates.iter().map(|candidate| candidate.id).collect();
        begin(&path, owner, true, source, candidates);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let draft = {
            let rel = RelSession::open_path(&path.0, &graph(owner), [], []).unwrap();
            super::prepare_dream_batch(&EntityOwnerSession::Rel(&rel), owner, source.id, &ids)
                .unwrap()
                .evaluate_import(
                    &DreamClassifier::new(ConflictingProposal {
                        calls: calls.clone(),
                        persists,
                    }),
                    Some(&DreamVerifier::new(Endpoint {
                        relation: "none",
                        verdict: "yes",
                    })),
                    DreamVerificationPolicy::default(),
                )
                .unwrap()
        };

        let expected = if persists {
            &[1, 2, 1, 2, 1, 2][..]
        } else {
            &[1, 2, 1, 2][..]
        };
        assert_eq!(calls.lock().unwrap().as_slice(), expected);
        let publication = [15; 32];
        let cut = super::prepare_dream_batch_cut(owner, publication, &draft, 100).unwrap();
        assert_eq!(cut.pairs.len(), 2);
        assert_eq!(cut.pairs[0].kind, Some(RelationKind::Supersedes));
        assert!(!cut.pairs[0].rejected);
        assert_eq!(cut.pairs[1].kind, None);
        assert_eq!(cut.pairs[1].rejected, persists);
        assert!(publish(&path, owner, true, publication, &draft).unwrap());
        assert!(!publish(&path, owner, true, publication, &draft).unwrap());
    }
}
#[path = "completion.rs"]
mod completion;
#[path = "publication.rs"]
mod publication;

#[path = "read_cost/tests.rs"]
mod read_cost;

#[path = "user_protection/tests.rs"]
mod user_protection;

#[path = "temporal_binding.rs"]
mod temporal_binding;
