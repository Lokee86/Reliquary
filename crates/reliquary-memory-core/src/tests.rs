use super::*;
use crate::codec::encode_creation;
use reliquary_container::{Container, ContainerIdentity, FileKind, ReliquaryScope};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&base).unwrap();
        let path = base.join(format!(
            "memory-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn open_new(path: &std::path::Path) -> (Container, MemoryStore) {
    let mut c = Container::create(
        path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [1; 16],
    )
    .unwrap();
    let s = MemoryStore::create(&mut c).unwrap();
    (c, s)
}
fn draft(mutation: &str, title: &str, category: &str) -> MemoryDraft {
    MemoryDraft {
        title: title.into(),
        content: "immutable body".into(),
        category: category.into(),
        memory_type: "fact".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: Some("node-3".into()),
        content_source_conversation_id: Some("conversation-4".into()),
        content_source_node_id: Some("node-4".into()),
        grounding_source_conversation_id: Some("conversation-5".into()),
        grounding_source_node_id: Some("node-5".into()),
        source_episode_id: Some([2; 32]),
        source_ref: Some(MemorySourceRef {
            owner_id: "rel-owner-1".into(),
            principal_id: Some("principal-7".into()),
            source_episode_id: [2; 32],
            source_node_id: "node-3".into(),
            content_source_conversation_id: Some("conversation-4".into()),
            content_source_node_id: Some("node-4".into()),
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
        }),
        source_time_ns: Some(1_700_000_000),
        temporal_inference: Some(MemoryTemporalInference {
            body_id: memory_body_id(title, "immutable body"),
            source_time_ns: Some(1_700_000_000),
            inference: TemporalInference {
                model: "model-v1".into(),
                contract_version: "contract-v2".into(),
                resolutions: vec![TemporalInferenceResolution {
                    start_byte: 0,
                    end_byte: title.len(),
                    kind: TemporalIndicationKind::Explicit,
                    evidence: title.into(),
                    canonical_expression: "source time".into(),
                }],
            },
        }),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

#[test]
fn memory_identity_hashes_are_deterministic_and_stable() {
    let pinned = memory_id("identity-vector-1");
    assert_eq!(
        pinned.0,
        [
            0x76, 0xab, 0xfa, 0xff, 0x44, 0x29, 0xdd, 0x7c, 0xc8, 0x4f, 0xfd, 0x53, 0xe4, 0x73,
            0x57, 0x65, 0xe9, 0x62, 0xd7, 0x06, 0x42, 0xd6, 0x78, 0x50, 0xfd, 0x59, 0x2b, 0xcc,
            0x34, 0x65, 0x49, 0x51,
        ]
    );
    let a = memory_id("mutation-alpha");
    assert_eq!(a, memory_id("mutation-alpha"));
    assert_eq!(
        a.0,
        [
            0x67, 0x14, 0xb1, 0xe5, 0xd3, 0x15, 0xc3, 0xc9, 0x22, 0x38, 0x6e, 0x1a, 0x7f, 0x46,
            0x82, 0x7f, 0x42, 0xc8, 0xd4, 0xe3, 0xe7, 0xdd, 0x82, 0x6e, 0x4d, 0x72, 0xfe, 0xff,
            0x23, 0xda, 0xb0, 0x19,
        ]
    );
    assert_ne!(a, memory_id("mutation-beta"));
    let body = memory_body_id("title", "content");
    assert_eq!(body, memory_body_id("title", "content"));
    assert_eq!(
        body.0,
        [
            0x18, 0x81, 0x70, 0xb8, 0x0f, 0xd7, 0x65, 0x6b, 0xc3, 0xde, 0x0e, 0x48, 0x95, 0x3b,
            0x20, 0x5c, 0x75, 0xdd, 0x94, 0x30, 0xda, 0x57, 0x7f, 0xd4, 0x71, 0x67, 0xe7, 0xe5,
            0x85, 0x3e, 0x6d, 0x78,
        ]
    );
    assert_ne!(body, memory_body_id("title ", "content"));
}

#[test]
fn creation_revision_provenance_and_body_survive_reopen() {
    let d = TempDir::new();
    let p = d.0.join("owner.rel");
    let (mut c, mut store) = open_new(&p);
    let first = store
        .publish(&mut c, None, 0, draft("create-a", "Title", "fact"))
        .unwrap();
    assert!(first.published);
    let id = first.revision.record.id;
    assert_eq!(first.revision.record.revision, 1);
    let mut second_draft = draft("revise-a", "Title", "canonical");
    second_draft.memory_type = "episode_fact".into();
    second_draft.authority_kind = "correction".into();
    second_draft.temporal_status = "historical".into();
    second_draft.scope = "conversation".into();
    second_draft.lifecycle_state = "canonical".into();
    second_draft.archived = true;
    second_draft.parent_id = Some(MemoryId([8; 32]));
    second_draft.superseded_by = Some(MemoryId([9; 32]));
    second_draft.source_node_id = None;
    second_draft.content_source_conversation_id = Some("conversation-9".into());
    second_draft.content_source_node_id = None;
    second_draft.grounding_source_conversation_id = None;
    second_draft.grounding_source_node_id = Some("grounding-node-9".into());
    second_draft.source_episode_id = None;
    second_draft.updated_at_ns = 11;
    second_draft.created_at_ns = 9; // Existing replay allows this mutable V1 scalar.
    second_draft.source_time_ns = Some(1_700_000_001);
    second_draft.temporal_inference = None; // Old proof is bound to the prior source time.
    second_draft.source_ref = None; // Absent source input inherits the accepted reference.
    let second = store.publish(&mut c, Some(id), 1, second_draft).unwrap();
    assert_eq!(second.revision.record.revision, 2);
    assert_eq!(
        second.revision.record.body_id,
        first.revision.record.body_id
    );
    c.sync().unwrap();
    drop(store);
    drop(c);
    let mut c = Container::open_writer_as(&p, FileKind::Reliquary).unwrap();
    let store = MemoryStore::open(&c).unwrap();
    let history = store.history(id).unwrap();
    assert_eq!(history.revisions.len(), 2);
    assert_eq!(history.revisions[0].record.memory_version, 1);
    assert_eq!(history.revisions[1].record.memory_version, 2);
    assert_eq!(history.revisions[0].record.mutation_id, "create-a");
    assert_eq!(history.revisions[0].record.source_episode_id, Some([2; 32]));
    assert_eq!(
        history.revisions[0].record.source_node_id.as_deref(),
        Some("node-3")
    );
    assert_eq!(
        history.revisions[0].record.temporal_inference,
        draft("create-a", "Title", "fact").temporal_inference
    );
    assert_eq!(
        history.revisions[0].record.source_ref,
        draft("create-a", "Title", "fact").source_ref
    );
    assert_eq!(
        history.revisions[1]
            .record
            .source_ref
            .as_ref()
            .unwrap()
            .owner_id,
        "rel-owner-1"
    );
    assert_eq!(
        history.revisions[1]
            .record
            .source_ref
            .as_ref()
            .unwrap()
            .source_episode_id,
        [2; 32]
    );
    assert_eq!(
        history.revisions[0].payload,
        PayloadState::Available(MemoryBody {
            title: "Title".into(),
            content: "immutable body".into()
        })
    );
    assert_eq!(history.revisions[1].record.category, "canonical");
    let latest = &history.revisions[1].record;
    assert_eq!(latest.memory_type, "episode_fact");
    assert_eq!(latest.authority_kind, "correction");
    assert_eq!(latest.temporal_status, "historical");
    assert_eq!(latest.scope, "conversation");
    assert_eq!(latest.lifecycle_state, "canonical");
    assert!(latest.archived);
    assert_eq!(latest.parent_id, Some(MemoryId([8; 32])));
    assert_eq!(latest.superseded_by, Some(MemoryId([9; 32])));
    assert_eq!(latest.source_node_id, None);
    assert_eq!(
        latest.content_source_conversation_id.as_deref(),
        Some("conversation-9")
    );
    assert_eq!(latest.content_source_node_id, None);
    assert_eq!(latest.grounding_source_conversation_id, None);
    assert_eq!(
        latest.grounding_source_node_id.as_deref(),
        Some("grounding-node-9")
    );
    assert_eq!(latest.source_episode_id, None);
    assert_eq!(latest.source_ref.as_ref().unwrap().owner_id, "rel-owner-1");
    assert_eq!(latest.temporal_inference, None);
    assert_eq!(latest.created_at_ns, 9);
    assert_eq!(latest.updated_at_ns, 11);
    // The reopened writer remains usable; no semantic state is mutated by reads.
    c.sync().unwrap();
}

#[test]
fn retries_conflicts_immutable_body_and_terminal_deletion_are_explicit() {
    let d = TempDir::new();
    let p = d.0.join("lifecycle.rel");
    let (mut c, mut store) = open_new(&p);
    let create = draft("create-b", "Title", "fact");
    let created = store.publish(&mut c, None, 0, create.clone()).unwrap();
    let id = created.revision.record.id;
    assert!(!store.publish(&mut c, None, 0, create).unwrap().published);
    let mut changed_body = draft("bad-body", "Other", "fact");
    assert!(matches!(
        store.publish(&mut c, Some(id), 1, changed_body.clone()),
        Err(MemoryError::ImmutableBodyChanged)
    ));
    changed_body.title = "Title".into();
    changed_body.temporal_inference = draft("proof-refresh", "Title", "fact").temporal_inference;
    changed_body.mutation_id = "revise-b".into();
    changed_body.category = "canonical".into();
    store.publish(&mut c, Some(id), 1, changed_body).unwrap();
    let tombstone = store.tombstone(&mut c, id, 2).unwrap();
    assert_eq!(tombstone.revision, 3);
    assert_eq!(store.tombstone(&mut c, id, 2).unwrap(), tombstone);
    assert!(matches!(
        store.publish(&mut c, Some(id), 2, draft("resurrect", "Title", "fact")),
        Err(MemoryError::TerminalTombstone)
    ));
    c.sync().unwrap();
    drop(store);
    drop(c);
    let c = Container::open_read_only(&p).unwrap();
    let store = MemoryStore::open(&c).unwrap();
    let history = store.history(id).unwrap();
    assert_eq!(history.tombstone, Some(tombstone));
    assert_eq!(history.revisions.len(), 2);
    assert!(matches!(
        history.revisions[0].payload,
        PayloadState::Available(_)
    ));
}

#[test]
fn missing_memory_and_missing_body_backing_are_distinct() {
    let d = TempDir::new();
    let p = d.0.join("absent.rel");
    let (mut c, mut store) = open_new(&p);
    let value = draft("present-id", "Body", "fact");
    let id = memory_id(&value.mutation_id);
    let body_id = memory_body_id(&value.title, &value.content);
    let record = MemoryRecord {
        id,
        revision: 1,
        body_id,
        category: value.category,
        memory_type: value.memory_type,
        authority_kind: value.authority_kind,
        temporal_status: value.temporal_status,
        scope: value.scope,
        lifecycle_state: value.lifecycle_state,
        archived: value.archived,
        superseded_by: value.superseded_by,
        parent_id: value.parent_id,
        source_node_id: value.source_node_id,
        content_source_conversation_id: value.content_source_conversation_id,
        content_source_node_id: value.content_source_node_id,
        grounding_source_conversation_id: value.grounding_source_conversation_id,
        grounding_source_node_id: value.grounding_source_node_id,
        source_episode_id: value.source_episode_id,
        source_ref: value.source_ref,
        source_time_ns: value.source_time_ns,
        temporal_inference: value.temporal_inference,
        mutation_id: value.mutation_id,
        created_at_ns: value.created_at_ns,
        updated_at_ns: value.updated_at_ns,
        memory_version: 1,
    };
    c.append(&encode_creation(&record).unwrap()).unwrap();
    c.sync().unwrap();
    drop(store);
    drop(c);
    let c = Container::open_read_only(&p).unwrap();
    store = MemoryStore::open(&c).unwrap();
    assert!(matches!(
        store.revision(id, 1).unwrap().payload,
        PayloadState::Absent
    ));
    assert!(matches!(
        store.history(MemoryId([0; 32])),
        Err(MemoryError::MissingMemory)
    ));
}

#[test]
fn memory_mutations_reject_a_different_owner_without_writing() {
    let temp = TempDir::new();
    let (mut first, mut store) = open_new(&temp.0.join("first.rel"));
    let id = store
        .publish(&mut first, None, 0, draft("owner-test", "Title", "fact"))
        .unwrap()
        .revision
        .record
        .id;
    let mut second = Container::create(
        &temp.0.join("second.rel"),
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [2; 16],
    )
    .unwrap();
    let before = second.read_frames().unwrap().len();
    assert!(matches!(
        store.publish(&mut second, None, 0, draft("other", "Title", "fact")),
        Err(MemoryError::WrongOwner)
    ));
    assert!(matches!(
        store.tombstone(&mut second, id, 1),
        Err(MemoryError::WrongOwner)
    ));
    assert_eq!(second.read_frames().unwrap().len(), before);
    drop(store);
    let mut reopened = MemoryStore::open(&first).unwrap();
    assert!(matches!(
        reopened.tombstone(&mut second, id, 1),
        Err(MemoryError::WrongOwner)
    ));
    let mut phy = Container::create(
        &temp.0.join("private.phy"),
        ContainerIdentity::phylactery(),
        [1; 16],
    )
    .unwrap();
    assert!(matches!(
        reopened.tombstone(&mut phy, id, 1),
        Err(MemoryError::WrongOwner)
    ));
    assert!(phy.read_frames().unwrap().is_empty());
}

#[test]
fn creating_memory_twice_does_not_corrupt_existing_history() {
    let temp = TempDir::new();
    let (mut container, mut store) = open_new(&temp.0.join("existing.rel"));
    let id = store
        .publish(&mut container, None, 0, draft("existing", "Title", "fact"))
        .unwrap()
        .revision
        .record
        .id;
    let before = container.read_frames().unwrap().len();
    assert!(matches!(
        MemoryStore::create(&mut container),
        Err(MemoryError::Corrupt(_))
    ));
    assert_eq!(container.read_frames().unwrap().len(), before);
    assert_eq!(
        MemoryStore::open(&container)
            .unwrap()
            .history(id)
            .unwrap()
            .revisions
            .len(),
        1
    );
}

#[test]
fn invalid_creation_metadata_does_not_append_an_orphan_body() {
    let temp = TempDir::new();
    let (mut container, mut store) = open_new(&temp.0.join("invalid.rel"));
    let mut invalid = draft("invalid", "Title", "fact");
    invalid.category = "x".repeat(crate::codec::MAX_FIELD_BYTES + 1);
    let before = container.read_frames().unwrap().len();
    assert!(matches!(
        store.publish(&mut container, None, 0, invalid),
        Err(MemoryError::FieldTooLarge)
    ));
    assert_eq!(container.read_frames().unwrap().len(), before);
    assert_eq!(MemoryStore::open(&container).unwrap().memory_count(), 0);
}

#[test]
fn replay_rejects_creation_with_invalid_metadata() {
    let temp = TempDir::new();
    let (mut source, mut store) = open_new(&temp.0.join("source.rel"));
    let mut record = store
        .publish(&mut source, None, 0, draft("metadata", "Title", "fact"))
        .unwrap()
        .revision
        .record;
    record.category.clear();
    let (mut target, _) = open_new(&temp.0.join("forged.rel"));
    target.append(&encode_creation(&record).unwrap()).unwrap();
    assert!(matches!(
        MemoryStore::open(&target),
        Err(MemoryError::Corrupt("empty required Memory field"))
    ));
}

#[test]
fn replay_rechecks_temporal_spans_when_body_appears_later() {
    let temp = TempDir::new();
    let (mut source, mut store) = open_new(&temp.0.join("source.rel"));
    let mut record = store
        .publish(&mut source, None, 0, draft("late-body", "Title", "fact"))
        .unwrap()
        .revision
        .record;
    record
        .temporal_inference
        .as_mut()
        .unwrap()
        .inference
        .resolutions[0]
        .end_byte = 1000;
    let (mut target, _) = open_new(&temp.0.join("late.rel"));
    target.append(&encode_creation(&record).unwrap()).unwrap();
    let mut projected = MemoryStore::open(&target).unwrap();
    assert!(projected.references_body(record.body_id));
    assert!(!projected.references_body(MemoryBodyId([0; 32])));
    let before = target.read_frames().unwrap().len();
    assert!(matches!(
        projected.publish(&mut target, None, 0, draft("direct-late", "Title", "fact")),
        Err(MemoryError::InvalidField("temporal_inference_span"))
    ));
    assert_eq!(target.read_frames().unwrap().len(), before);
    let mut new_record = record.clone();
    new_record.mutation_id = "late-new".into();
    new_record.id = memory_id(&new_record.mutation_id);
    new_record.memory_version = 2;
    new_record.temporal_inference = None;
    assert!(matches!(
        projected.preflight_grouped_completion(
            vec![(record.body_id, memory_body_bytes("Title", "immutable body"))],
            vec![encode_creation(&new_record).unwrap()],
        ),
        Err(MemoryError::InvalidField("temporal_inference_span"))
    ));
    let mut body = b"RELMBOD1".to_vec();
    body.extend_from_slice(&record.body_id.0);
    body.extend_from_slice(&memory_body_bytes("Title", "immutable body"));
    target.append(&body).unwrap();
    assert!(matches!(
        MemoryStore::open(&target),
        Err(MemoryError::InvalidField("temporal_inference_span"))
    ));
}

#[test]
fn versioned_memory_creation_reopens_with_its_global_coordinate() {
    let temp = TempDir::new();
    let (mut container, mut store) = open_new(&temp.0.join("versioned.rel"));
    let draft = draft("versioned", "Title", "fact");
    let record = store
        .publish_versioned_creation(&mut container, draft.clone(), 33)
        .unwrap()
        .revision
        .record;
    let global = container.latest_version();
    assert_eq!(global, 1);
    assert!(
        !store
            .publish_versioned_creation(&mut container, draft, 44)
            .unwrap()
            .published
    );
    assert_eq!(container.latest_version(), global);
    container.sync().unwrap();
    drop(container);
    let mut container = Container::open_writer(&temp.0.join("versioned.rel")).unwrap();
    let store = MemoryStore::open(&container).unwrap();
    assert_eq!(
        store.history(record.id).unwrap().revisions[0].record,
        record
    );
    assert_eq!(container.transaction_time_ns(global), Some(33));
    let before = container.read_frames().unwrap().len();
    assert!(matches!(
        MemoryStore::create(&mut container),
        Err(MemoryError::Corrupt(_))
    ));
    assert_eq!(container.read_frames().unwrap().len(), before);
}
