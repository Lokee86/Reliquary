use super::*;
use reliquary_application::{
    DreamClassifier, DreamVerificationPolicy, DreamVerifier, EntityOwnerSession,
    SwitchboardEmbeddingEndpoint, TextField, draft_dream_batch_organization,
};
use reliquary_container::{Container, ReliquaryScope};
use reliquary_memory_core::{MemoryDraft, MemoryStore};
use reliquary_switchboard::{
    EmbeddingEndpointError, EmbeddingMode, EmbeddingNormalization, GeneralEndpointError,
};
use serde_json::{Value, json};
use std::{fs, path::Path, sync::atomic::AtomicUsize, time::Duration};

struct Embedding {
    calls: AtomicUsize,
}
impl EmbeddingEndpoint for Embedding {
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
        _: EmbeddingMode,
        inputs: &[String],
        _: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(inputs.iter().map(|_| vec![1.0, 0.0]).collect())
    }
    fn embed(
        &self,
        _: EmbeddingMode,
        _: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        panic!("unbounded embedding")
    }
}

struct EntityEndpoint {
    calls: AtomicUsize,
}
impl GeneralEndpoint for EntityEndpoint {
    fn model(&self) -> &str {
        "entity-worker-test"
    }
    fn complete_json(
        &self,
        _: &str,
        _: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(match schema {
            "entity_resolution" => json!({
                "decision":"create_new", "reason":"named_referent", "target_candidate_index":-1,
                "identity_relation":"related_distinct", "mention_kind":"tool"
            }),
            "entity_admission" => json!({
                "decision":"create_new", "reason":"first_seen_identity", "promotion_policy":"immediate",
                "entity_kind":"tool", "entity_summary":"Atlas is the migration tool."
            }),
            other => panic!("unexpected schema {other}"),
        })
    }
}

fn create_owner(path: &Path, owner: [u8; 16]) -> MemoryId {
    drop(RelWriter::create_path(path, ReliquaryScope::Project, owner).unwrap());
    let mut container = Container::open_writer(path).unwrap();
    let mut memories = MemoryStore::open(&container).unwrap();
    memories
        .publish_versioned_creation(
            &mut container,
            MemoryDraft {
                title: "Atlas".into(),
                content: "Atlas is the migration tool.".into(),
                category: "fact".into(),
                memory_type: "semantic".into(),
                authority_kind: "user".into(),
                temporal_status: "current".into(),
                scope: "project".into(),
                lifecycle_state: "extracted".into(),
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
                source_time_ns: None,
                temporal_inference: None,
                mutation_id: "worker-source".into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
            1,
        )
        .unwrap()
        .revision
        .record
        .id
}

fn wait_for(counter: &AtomicUsize, minimum: usize) {
    for _ in 0..500 {
        if counter.load(Ordering::SeqCst) >= minimum {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("worker did not make progress");
}

#[test]
fn entity_and_vector_workers_publish_once_and_resume_from_owner_receipts() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!("workflow-workers-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("owner.rel");
    let raw = [177; 16];
    let memory = create_owner(&path, raw);
    let admission = GraphAdmission::for_replay(raw, [], [], [], [], []);

    let gateway = Gateway::new();
    let owner = gateway
        .mount_writable_reliquary(&path, &admission, [], [])
        .unwrap();
    let instance = Arc::new(gateway.instance("alice", [owner], []).unwrap());
    let embedding = Arc::new(Embedding {
        calls: AtomicUsize::new(0),
    });
    let adapter = SwitchboardEmbeddingEndpoint::new(embedding.clone());
    let profile = instance.publish_embedding_profile(owner, &adapter).unwrap();
    let before_vectors = embedding.calls.load(Ordering::SeqCst);
    let entity = Arc::new(EntityEndpoint {
        calls: AtomicUsize::new(0),
    });
    let work = OwnerWorkflowSet::new()
        .entity(EntityWorker::new(
            instance.clone(),
            EntityMemoryMention {
                owner_id: raw,
                memory_id: memory,
                field: TextField::Content,
                start_byte: 0,
                end_byte: 5,
                text: "Atlas".into(),
            },
            entity.clone(),
            10,
        ))
        .vector(VectorWorker::new(
            instance.clone(),
            owner,
            profile,
            vec![memory],
            embedding.clone(),
        ));
    let mut workers = gateway
        .start_owner_workflows(
            instance.clone(),
            owner,
            work,
            OwnerWorkerConfig {
                idle_poll: Duration::from_millis(2),
            },
        )
        .unwrap();
    wait_for(&entity.calls, 2);
    wait_for(&embedding.calls, before_vectors + 1);
    workers.stop().unwrap();
    drop(instance);
    drop(gateway);

    let gateway = Gateway::new();
    gateway
        .mount_writable_reliquary(&path, &admission, [], [])
        .unwrap();
    let instance = Arc::new(gateway.instance("alice", [owner], []).unwrap());
    let retry_entity = Arc::new(EntityEndpoint {
        calls: AtomicUsize::new(0),
    });
    let retry_embedding = Arc::new(Embedding {
        calls: AtomicUsize::new(0),
    });
    let work = OwnerWorkflowSet::new()
        .entity(EntityWorker::new(
            instance.clone(),
            EntityMemoryMention {
                owner_id: raw,
                memory_id: memory,
                field: TextField::Content,
                start_byte: 0,
                end_byte: 5,
                text: "Atlas".into(),
            },
            retry_entity.clone(),
            10,
        ))
        .vector(VectorWorker::new(
            instance.clone(),
            owner,
            profile,
            vec![memory],
            retry_embedding.clone(),
        ));
    let mut workers = gateway
        .start_owner_workflows(
            instance,
            owner,
            work,
            OwnerWorkerConfig {
                idle_poll: Duration::from_millis(2),
            },
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(30));
    workers.stop().unwrap();
    assert_eq!(retry_entity.calls.load(Ordering::SeqCst), 0);
    assert_eq!(retry_embedding.calls.load(Ordering::SeqCst), 0);
    drop(gateway);
    fs::remove_dir_all(root).unwrap();
}

#[derive(Clone)]
struct DreamEndpoint;
impl GeneralEndpoint for DreamEndpoint {
    fn model(&self) -> &str {
        "dream-worker-test"
    }
    fn complete_json(
        &self,
        _: &str,
        _: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        Ok(if schema == "dream_pair_verification" {
            json!({"relation_supported":"accept","direction_supported":"accept","evidence_supported":"accept"})
        } else {
            json!({"relation":"none","direction":"none","evidence":[]})
        })
    }
}

#[test]
fn dream_worker_publication_is_idempotent_after_gateway_reopen() {
    use reliquary_owner_session::DreamMemoryCut;
    let root = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!("dream-worker-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("owner.rel");
    let raw = [178; 16];
    drop(RelWriter::create_path(&path, ReliquaryScope::Project, raw).unwrap());
    let mut container = Container::open_writer(&path).unwrap();
    let mut memories = MemoryStore::open(&container).unwrap();
    let mut cuts = Vec::new();
    for index in 0..2 {
        let record = memories
            .publish_versioned_creation(
                &mut container,
                MemoryDraft {
                    title: "Rollout".into(),
                    content: format!("Rollout evidence {index}"),
                    category: "fact".into(),
                    memory_type: "semantic".into(),
                    authority_kind: "direct".into(),
                    temporal_status: "current".into(),
                    scope: "project".into(),
                    lifecycle_state: "extracted".into(),
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
                    source_time_ns: Some(index + 1),
                    temporal_inference: None,
                    mutation_id: format!("dream-{index}"),
                    created_at_ns: index + 1,
                    updated_at_ns: index + 1,
                },
                index + 1,
            )
            .unwrap()
            .revision
            .record;
        cuts.push(DreamMemoryCut {
            id: record.id,
            revision: record.revision,
            body_id: record.body_id,
        });
    }
    container.sync().unwrap();
    drop(container);
    RelWriter::open_path(
        &path,
        GraphAdmission::for_replay(raw, [], [], [], [], []),
        [],
        [],
    )
    .unwrap()
    .begin_initial_dream_frontier("worker-frontier".into(), cuts[0], vec![cuts[1]], 2)
    .unwrap();
    let session = RelSession::open_path(
        &path,
        &GraphAdmission::for_replay(raw, [], [], [], [], []),
        [],
        [],
    )
    .unwrap();
    let draft = draft_dream_batch_organization(
        &EntityOwnerSession::Rel(&session),
        raw,
        cuts[0].id,
        &[cuts[1].id],
        &DreamClassifier::new(DreamEndpoint),
        Some(&DreamVerifier::new(DreamEndpoint)),
        DreamVerificationPolicy::default(),
    )
    .unwrap();
    drop(session);
    let admission = GraphAdmission::for_replay(raw, [], [], [], [], []);
    for _ in 0..2 {
        let gateway = Gateway::new();
        let owner = gateway
            .mount_writable_reliquary(&path, &admission, [], [])
            .unwrap();
        let instance = Arc::new(gateway.instance("alice", [owner], []).unwrap());
        let work = OwnerWorkflowSet::new().dream(DreamWorker::new(
            instance.clone(),
            owner,
            [9; 32],
            draft.clone(),
            10,
        ));
        let mut workers = gateway
            .start_owner_workflows(
                instance,
                owner,
                work,
                OwnerWorkerConfig {
                    idle_poll: Duration::from_millis(2),
                },
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(20));
        workers.stop().unwrap();
        drop(gateway);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn freshness_worker_rebuilds_the_same_bounded_due_notice_after_reopen() {
    use reliquary_archive::{Archive, TurnInput};
    use reliquary_freshness_store::{FreshnessMemoryId, FreshnessStore};
    let root = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!("freshness-worker-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("owner.rel");
    let raw = [179; 16];
    let memory = create_owner(&path, raw);
    let mut container = Container::open_writer(&path).unwrap();
    FreshnessStore::open(&container)
        .unwrap()
        .create_birth(&mut container, FreshnessMemoryId(memory.0), 0, true)
        .unwrap();
    container.sync().unwrap();
    drop(container);
    let mut container = Container::open_writer(&path).unwrap();
    let mut archive = Archive::open(&container).unwrap();
    let mut parent = None;
    for turn in 0..1 {
        let id = format!("turn-{turn}");
        archive
            .accept_turn(
                &mut container,
                TurnInput {
                    id: id.clone(),
                    conversation_id: "freshness-clock".into(),
                    parent_id: parent,
                    role: "user".into(),
                    principal_id: Some("alice".into()),
                    timestamp_ns: turn,
                    content: "clock".into(),
                    attachments: vec![],
                    project_attachments: vec![],
                },
            )
            .unwrap();
        parent = Some(id);
    }
    container.sync().unwrap();
    drop(container);
    let admission = GraphAdmission::for_replay(raw, [], [], [], [], []);
    let mut first = None;
    for _ in 0..2 {
        let gateway = Gateway::new();
        let owner = gateway
            .mount_writable_reliquary(&path, &admission, [], [])
            .unwrap();
        let instance = Arc::new(gateway.instance("alice", [owner], []).unwrap());
        let freshness = Arc::new(FreshnessWorker::new(instance.clone(), owner, 8));
        let work = OwnerWorkflowSet::new().freshness(freshness.clone());
        let mut workers = gateway
            .start_owner_workflows(
                instance,
                owner,
                work,
                OwnerWorkerConfig {
                    idle_poll: Duration::from_millis(2),
                },
            )
            .unwrap();
        loop {
            if freshness.evaluated_turn().unwrap() == Some(1) {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let transitions = freshness.transitions().unwrap();
        workers.stop().unwrap();
        assert!(transitions.is_empty());
        if let Some(expected) = &first {
            assert_eq!(&transitions, expected);
        } else {
            first = Some(transitions);
        }
        drop(gateway);
    }
    fs::remove_dir_all(root).unwrap();
}
