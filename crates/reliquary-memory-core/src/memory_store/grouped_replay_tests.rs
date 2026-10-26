use super::*;
use reliquary_container::{ContainerIdentity, FileKind};
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("grouped-memory-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        Self(directory.join("owner.rel"))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}

fn draft(n: usize) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: format!("grouped-{n}"),
        content: format!("body-{n}"),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: format!("grouped-mutation-{n}"),
        created_at_ns: n as i64,
        updated_at_ns: n as i64,
    }
}

fn prepared(
    store: &mut MemoryStore,
    container: &mut Container,
    indices: &[usize],
) -> (ObjectRef, Vec<u8>, MemoryGroupedBatch, u64) {
    let prepared = store
        .stage_grouped(
            container,
            indices.iter().map(|&i| (draft(i), None, None)).collect(),
        )
        .unwrap();
    let mut payload = Vec::new();
    let mut spans = Vec::new();
    for body in prepared.bodies {
        let start = payload.len();
        payload.extend_from_slice(&body.bytes);
        spans.push(MemoryGroupedBodySpan {
            id: body.id,
            range: start..payload.len(),
            encoding: MemoryGroupedBodyEncoding::Raw,
        });
    }
    let max_global = prepared.records.last().unwrap().global_version;
    let frame = container.append(&payload).unwrap();
    (
        frame,
        payload,
        MemoryGroupedBatch::new(prepared.records, spans, vec![]),
        max_global,
    )
}

fn empty() -> (Scratch, Container, MemoryStore) {
    let scratch = Scratch::new();
    let mut container = Container::create_with_identity(
        scratch.path(),
        ContainerIdentity {
            file_kind: FileKind::Reliquary,
            scope: None,
        },
    )
    .unwrap();
    let store = MemoryStore::empty();
    store.initialize(&mut container).unwrap();
    (scratch, container, store)
}

#[test]
fn accepted_grouped_records_and_routing_read_back_from_the_same_frame() {
    let (_scratch, mut container, mut store) = empty();
    let (frame, payload, mut batch, max_global) = prepared(&mut store, &mut container, &[1, 2]);
    let first = &batch.records[0];
    batch.routing_metadata.push(MemoryRoutingMetadata {
        memory_id: first.id,
        body_id: first.body_id,
        entity_mentions: vec![],
    });
    store
        .apply_grouped_batch(frame, &payload, batch.clone(), max_global)
        .unwrap();
    assert_eq!(store.stats().revisions, 2);
    assert_eq!(store.memory_version(), 2);
    for record in batch.records {
        let memory = store.memory(&mut container, record.id).unwrap();
        assert_eq!(memory.revision, 1);
        assert_eq!(memory.mutation_id, record.mutation_id);
    }
    assert!(
        store
            .routing_metadata(memory_id(&draft(1).mutation_id))
            .is_some()
    );
}

#[test]
fn later_invalid_record_rejects_without_publishing_earlier_batch_members() {
    let (scratch, mut container, mut store) = empty();
    let (frame, payload, mut batch, max_global) = prepared(&mut store, &mut container, &[1, 2]);
    batch.records[1].memory_version += 1;
    let bytes = std::fs::read(scratch.path()).unwrap();
    assert!(matches!(
        store.apply_grouped_batch(frame, &payload, batch, max_global),
        Err(MemoryError::InvalidVersion)
    ));
    assert_eq!(store.memory_version(), 0);
    assert!(store.records().is_empty());
    assert_eq!(store.stats().bodies, 0);
    assert_eq!(std::fs::read(scratch.path()).unwrap(), bytes);
}

#[test]
fn later_routing_conflict_rejects_without_inserting_valid_records_or_bodies() {
    let (_scratch, mut container, mut store) = empty();
    let (frame, payload, mut batch, max_global) = prepared(&mut store, &mut container, &[1, 2]);
    let first = batch.records[0].clone();
    batch.routing_metadata.push(MemoryRoutingMetadata {
        memory_id: first.id,
        body_id: first.body_id,
        entity_mentions: vec![],
    });
    batch.routing_metadata.push(MemoryRoutingMetadata {
        memory_id: first.id,
        body_id: MemoryBodyId([99; 32]),
        entity_mentions: vec![],
    });
    assert!(matches!(
        store.apply_grouped_batch(frame, &payload, batch, max_global),
        Err(MemoryError::InvalidField("Memory routing metadata binding"))
    ));
    assert_eq!(store.memory_version(), 0);
    assert!(store.records().is_empty());
    assert_eq!(store.stats().bodies, 0);
    assert!(store.routing_metadata(first.id).is_none());
}
