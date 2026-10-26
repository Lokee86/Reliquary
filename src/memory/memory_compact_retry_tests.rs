use crate::memory_codec::{MemoryVersion, encode_body, encode_version};
use crate::memory_compact_codec::{encode_creation, encode_delta};
use crate::memory_compact_contract::{MemoryCreation, MemoryDelta};
use crate::memory_model::{MemoryRecord, memory_body_bytes, memory_body_id, memory_id};
use crate::memory_rebuild::MemoryOpenState;
use crate::memory_store::MemoryStore;
use crate::{Memory, MemoryDraft, MemoryError, MemoryId};
use reliquary_container::{Container, ContainerIdentity, FileKind};
use std::path::{Path, PathBuf};

struct ScratchFile(PathBuf);

impl ScratchFile {
    fn new() -> Self {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("memory-compact-retry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        Self(directory.join("owner.rel"))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchFile {
    fn drop(&mut self) {
        if let Some(directory) = self.0.parent() {
            let _ = std::fs::remove_dir_all(directory);
        }
    }
}

fn creation_draft() -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Retry title".into(),
        content: "Retry content remains immutable.".into(),
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
        mutation_id: "compact-create-retry".into(),
        created_at_ns: 100,
        updated_at_ns: 100,
    }
}

fn revision_draft() -> MemoryDraft {
    let mut draft = creation_draft();
    draft.category = "belief".into();
    draft.temporal_status = "historical".into();
    draft.mutation_id = "compact-update-retry".into();
    draft.updated_at_ns = 200;
    draft
}

fn record_from_draft(
    draft: &MemoryDraft,
    id: MemoryId,
    revision: u64,
    body_id: crate::MemoryBodyId,
    global_version: u64,
    memory_version: u64,
) -> MemoryRecord {
    MemoryRecord {
        id,
        revision,
        body_id,
        category: draft.category.clone(),
        memory_type: draft.memory_type.clone(),
        authority_kind: draft.authority_kind.clone(),
        temporal_status: draft.temporal_status.clone(),
        scope: draft.scope.clone(),
        lifecycle_state: draft.lifecycle_state.clone(),
        archived: draft.archived,
        superseded_by: draft.superseded_by,
        parent_id: draft.parent_id,
        source_node_id: draft.source_node_id.clone(),
        content_source_conversation_id: draft.content_source_conversation_id.clone(),
        content_source_node_id: draft.content_source_node_id.clone(),
        grounding_source_conversation_id: draft.grounding_source_conversation_id.clone(),
        grounding_source_node_id: draft.grounding_source_node_id.clone(),
        source_episode_id: draft.source_episode_id,
        source_time_ns: draft.source_time_ns,
        source_ref: None,
        temporal_inference: None,
        mutation_id: draft.mutation_id.clone(),
        created_at_ns: draft.created_at_ns,
        updated_at_ns: draft.updated_at_ns,
        global_version,
        memory_version,
    }
}

fn compact_populated_owner(path: &Path) -> Result<(Container, MemoryStore, MemoryId), MemoryError> {
    let identity = ContainerIdentity {
        file_kind: FileKind::Reliquary,
        scope: None,
    };
    let mut writer = Container::create_with_identity(path, identity)?;
    let empty = MemoryStore::empty();
    empty.initialize(&mut writer)?;

    let first_draft = creation_draft();
    let second_draft = revision_draft();
    let id = memory_id(&first_draft.mutation_id);
    let body_id = memory_body_id(&first_draft.title, &first_draft.content);
    writer.append(&encode_body(
        body_id,
        &memory_body_bytes(&first_draft.title, &first_draft.content),
    )?)?;

    let first_global = writer.latest_version() + 1;
    let first = record_from_draft(&first_draft, id, 1, body_id, first_global, 1);
    let body = crate::memory_compact_codec::publication::encode(
        &crate::memory_compact_codec::publication::MemoryPublication::Creation(
            MemoryCreation::new(first.clone())?,
        ),
    )?;
    writer.append_publication_envelope(&body)?;
    let second_global = writer.latest_version() + 1;
    let second = record_from_draft(&second_draft, id, 2, body_id, second_global, 2);
    let body = crate::memory_compact_codec::publication::encode(
        &crate::memory_compact_codec::publication::MemoryPublication::Delta(MemoryDelta::between(
            &first, &second,
        )?),
    )?;
    writer.append_publication_envelope(&body)?;
    drop(writer);

    let mut container = Container::open(path)?;
    let latest_global = container.latest_version();
    let chunks = container.chunks()?;
    let mut state = MemoryOpenState::new();
    for chunk in chunks {
        let payload = container.read(chunk)?;
        state.ingest(chunk, &payload, latest_global)?;
    }
    let memory = state.finish(&mut container)?;
    Ok((container, memory, id))
}

fn resolved(store: &MemoryStore, container: &mut Container, id: MemoryId) -> Memory {
    store.memory(container, id).unwrap()
}

fn bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

#[test]
fn compact_creation_and_latest_revision_retries_are_read_only_even_with_stale_expected_revision() {
    let scratch = ScratchFile::new();
    let (mut container, mut store, id) = compact_populated_owner(scratch.path()).unwrap();
    let first_draft = creation_draft();
    let latest_draft = revision_draft();

    for (expected_revision, draft, revision) in [(0, first_draft, 1), (0, latest_draft, 2)] {
        let expected_memory = store.memory_revision(&mut container, id, revision).unwrap();
        let before_records = store.records().to_vec();
        let before_bytes = bytes(scratch.path());
        let before_global = container.latest_version();
        let before_memory = store.memory_version();

        let (retried, published) = store
            .publish(&mut container, Some(id), expected_revision, draft)
            .unwrap();
        assert!(!published);
        assert_eq!(retried.revision, revision);
        assert_eq!(retried, expected_memory);
        assert_eq!(resolved(&store, &mut container, id).revision, 2);
        assert_eq!(store.records(), before_records);
        assert_eq!(container.latest_version(), before_global);
        assert_eq!(store.memory_version(), before_memory);
        assert_eq!(bytes(scratch.path()), before_bytes);
    }
}

#[test]
fn changed_metadata_with_same_mutation_conflicts_and_stale_new_mutation_rejects() {
    let scratch = ScratchFile::new();
    let (mut container, mut store, id) = compact_populated_owner(scratch.path()).unwrap();
    let before_records = store.records().to_vec();
    let before_bytes = bytes(scratch.path());
    let before_global = container.latest_version();
    let before_memory = store.memory_version();

    let mut conflicting_retry = revision_draft();
    conflicting_retry.category = "different meaning".into();
    assert!(matches!(
        store.publish(&mut container, Some(id), 0, conflicting_retry),
        Err(MemoryError::MutationConflict)
    ));
    assert_eq!(store.records(), before_records);
    assert_eq!(container.latest_version(), before_global);
    assert_eq!(store.memory_version(), before_memory);
    assert_eq!(bytes(scratch.path()), before_bytes);

    let mut stale_new_mutation = revision_draft();
    stale_new_mutation.mutation_id = "new-stale-mutation".into();
    stale_new_mutation.category = "new content".into();
    assert!(matches!(
        store.publish(&mut container, Some(id), 1, stale_new_mutation),
        Err(MemoryError::RevisionConflict)
    ));
    assert_eq!(store.records(), before_records);
    assert_eq!(container.latest_version(), before_global);
    assert_eq!(store.memory_version(), before_memory);
    assert_eq!(bytes(scratch.path()), before_bytes);
}

#[test]
fn compact_populated_tombstone_exact_retry_preserves_original_coordinates_and_bytes() {
    let scratch = ScratchFile::new();
    let (mut container, mut store, id) = compact_populated_owner(scratch.path()).unwrap();
    let records_before = store.records().to_vec();
    let first = store.tombstone(&mut container, id, 2).unwrap();
    let crate::owner_lifecycle::TombstoneOutcome::Published(coordinates) = first else {
        panic!("first tombstone must publish");
    };
    let before_bytes = bytes(scratch.path());
    let before_global = container.latest_version();
    let before_memory = store.memory_version();
    let before_records = store.records().to_vec();
    assert_eq!(before_records, records_before);

    assert_eq!(
        store.tombstone(&mut container, id, 2).unwrap(),
        crate::owner_lifecycle::TombstoneOutcome::AlreadyTombstoned(coordinates)
    );
    assert_eq!(store.records(), before_records);
    assert_eq!(container.latest_version(), before_global);
    assert_eq!(store.memory_version(), before_memory);
    assert_eq!(bytes(scratch.path()), before_bytes);
}
