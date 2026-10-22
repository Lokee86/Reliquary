use crate::memory_codec::encode_body;
use crate::memory_compact_codec::publication::{self, MemoryPublication};
fn encode_creation(c: &MemoryCreation) -> Result<Vec<u8>, crate::MemoryError> {
    publication::encode(&MemoryPublication::Creation(c.clone()))
}
fn encode_delta(d: &MemoryDelta) -> Result<Vec<u8>, crate::MemoryError> {
    publication::encode(&MemoryPublication::Delta(d.clone()))
}
use crate::memory_compact_contract::{MemoryCreation, MemoryDelta};
use crate::memory_model::{MemoryRecord, memory_body_bytes, memory_body_id};
use crate::owner_lifecycle::{LifecycleState, TombstoneOutcome};
use crate::{Container, ContainerError, Cva, MemoryDraft, MemoryError, Phylactery};
use std::fs;
use std::path::{Path, PathBuf};

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "memory-compact-maintenance-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(mutation_id: &str, temporal_status: &str, title: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: temporal_status.into(),
        title: title.into(),
        content: format!("Body for {title}"),
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
        mutation_id: mutation_id.into(),
        created_at_ns: 100,
        updated_at_ns: 100,
    }
}

fn payloads(container: &mut Container) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    {
        let mut ingest_frame =
            |frame: crate::container::PhysicalFrame| -> Result<(), ContainerError> {
                let payload = frame.payload.as_slice();
                {
                    result.push(payload.to_vec());
                    Ok(())
                }
            };
        let mut cursor = container.physical_cursor().unwrap();
        (|| {
            while let Some(frame) = cursor.next_frame()? {
                ingest_frame(frame)?;
            }
            Ok::<(), ContainerError>(())
        })()
    }
    .unwrap();
    result
}

fn assert_compact_memory_payloads(container: &mut Container, records: &[MemoryRecord]) {
    let payloads = payloads(container);
    let publications: Vec<_> = payloads
        .iter()
        .filter_map(|payload| {
            let envelope = crate::container::publication_envelope::decode(payload).unwrap()?;
            publication::decode(envelope.body, envelope.global_version).unwrap()
        })
        .collect();
    assert_eq!(publications.len(), 3);
    let creations: Vec<_> = publications
        .iter()
        .filter_map(|publication| match publication {
            MemoryPublication::Creation(creation) => Some(creation),
            _ => None,
        })
        .collect();
    let deltas: Vec<_> = publications
        .iter()
        .filter_map(|publication| match publication {
            MemoryPublication::Delta(delta) => Some(delta),
            _ => None,
        })
        .collect();
    let tombstones: Vec<_> = publications
        .iter()
        .filter_map(|publication| match publication {
            MemoryPublication::Tombstone {
                id,
                revision,
                memory_version,
            } => Some((*id, *revision, *memory_version)),
            _ => None,
        })
        .collect();
    assert_eq!(creations.len(), 1);
    assert_eq!(deltas.len(), 1);
    assert_eq!(tombstones.len(), 1);
    assert_eq!(creations[0].record, records[0]);
    assert_eq!(deltas[0].id, records[1].id);
    assert_eq!(deltas[0].revision, records[1].revision);
    assert_eq!(deltas[0].global_version, records[1].global_version);
    assert_eq!(deltas[0].memory_version, records[1].memory_version);
    assert_eq!(
        tombstones[0],
        (
            records[0].id,
            records.last().unwrap().revision + 1,
            records.last().unwrap().memory_version + 1,
        )
    );
}

fn assert_reclaimed_records(
    actual: &[MemoryRecord],
    expected: &[MemoryRecord],
    tx_times: &[Option<i64>],
    transaction_time: impl Fn(u64) -> Option<i64>,
) {
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), tx_times.len());
    for (record, expected_time) in actual.iter().zip(tx_times) {
        assert_eq!(transaction_time(record.global_version), *expected_time);
    }
}

fn append_unversioned_compact_frames(container: &mut Container, accepted: &MemoryRecord, tag: u8) {
    let title = format!("Unpublished compact body {tag}");
    let content = format!("This body has no accepted publication ticket {tag}.");
    let body = memory_body_bytes(&title, &content);
    let body_id = memory_body_id(&title, &content);
    container
        .append(&encode_body(body_id, &body).unwrap())
        .unwrap();

    let mut creation_record = accepted.clone();
    creation_record.id = crate::MemoryId([tag; 32]);
    creation_record.revision = 1;
    creation_record.body_id = body_id;
    creation_record.mutation_id = format!("unpublished-creation-{tag}");
    creation_record.global_version = container.latest_version() + 1_000;
    creation_record.memory_version = accepted.memory_version + 1_000;
    container
        .append(&encode_creation(&MemoryCreation::new(creation_record).unwrap()).unwrap())
        .unwrap();

    let mut uncommitted_successor = accepted.clone();
    uncommitted_successor.revision = accepted.revision + 1;
    uncommitted_successor.global_version = container.latest_version() + 1_001;
    uncommitted_successor.memory_version = accepted.memory_version + 1_001;
    uncommitted_successor.mutation_id = format!("unpublished-delta-{tag}");
    uncommitted_successor.temporal_status = if accepted.temporal_status == "current" {
        "historical".into()
    } else {
        "current".into()
    };
    uncommitted_successor.updated_at_ns += 1;
    container
        .append(
            &encode_delta(&MemoryDelta::between(accepted, &uncommitted_successor).unwrap())
                .unwrap(),
        )
        .unwrap();
    container.sync().unwrap();
}

#[test]
fn divergent_rel_reconcile_retains_compact_source_history_and_replayed_transaction_time() {
    let scratch = ScratchDirectory::new();
    let left_path = scratch.file("left.rel");
    let right_path = scratch.file("right.rel");
    let merged_path = scratch.file("merged.rel");

    Cva::create_project(&left_path).unwrap().sync().unwrap();
    let mut base = Cva::open(&left_path).unwrap();
    let (shared, created) = base
        .publish_memory(None, 0, draft("shared-create", "current", "Shared"))
        .unwrap();
    assert!(created);
    base.sync().unwrap();
    drop(base);
    fs::copy(&left_path, &right_path).unwrap();

    let mut left = Cva::open(&left_path).unwrap();
    let mut left_revision = draft("shared-left-revision", "historical", "Shared");
    left_revision.updated_at_ns = 200;
    let (_, changed) = left
        .publish_memory(Some(shared.id), 1, left_revision)
        .unwrap();
    assert!(changed);
    left.sync().unwrap();
    let retained_left = left.memories.records().to_vec();
    let retained_left_times: Vec<_> = retained_left
        .iter()
        .map(|record| left.transaction_time_ns(record.global_version))
        .collect();
    drop(left);

    let mut right = Cva::open(&right_path).unwrap();
    let (right_memory, created) = right
        .publish_memory(None, 0, draft("right-create", "current", "Right branch"))
        .unwrap();
    assert!(created);
    right.sync().unwrap();
    let right_record = right
        .memories
        .records()
        .iter()
        .find(|record| record.id == right_memory.id)
        .unwrap()
        .clone();
    let right_time = right.transaction_time_ns(right_record.global_version);
    drop(right);

    let result = Cva::reconcile(&left_path, &right_path, &merged_path).unwrap();
    assert_eq!(result.replayed_memory_revisions, 1);
    let mut merged = Cva::open(&merged_path).unwrap();
    let merged_records = merged.memories.records().to_vec();
    assert_eq!(merged_records.len(), 3);
    assert_eq!(&merged_records[..2], retained_left.as_slice());
    assert_eq!(
        merged_records
            .iter()
            .find(|record| record.id == right_memory.id)
            .unwrap()
            .body_id,
        right_record.body_id
    );
    assert_eq!(
        merged
            .memory_revision(shared.id, 1)
            .unwrap()
            .temporal_status,
        "current"
    );
    assert_eq!(
        merged
            .memory_revision(shared.id, 2)
            .unwrap()
            .temporal_status,
        "historical"
    );
    assert_eq!(
        merged.memory_revision(right_memory.id, 1).unwrap().title,
        "Right branch"
    );
    for (record, expected_time) in merged_records[..2].iter().zip(retained_left_times) {
        assert_eq!(
            merged.transaction_time_ns(record.global_version),
            expected_time
        );
    }
    let replayed_right = merged_records
        .iter()
        .find(|record| record.id == right_memory.id)
        .unwrap();
    assert_eq!(
        merged.transaction_time_ns(replayed_right.global_version),
        right_time
    );
}

#[test]
fn rel_reclamation_preserves_compact_tombstoned_history_exactly() {
    let scratch = ScratchDirectory::new();
    let source = scratch.file("source.rel");
    let output = scratch.file("reclaimed.rel");
    let mut rel = Cva::create(&source).unwrap();
    let (first, created) = rel
        .publish_memory(None, 0, draft("rel-create", "current", "REL compact"))
        .unwrap();
    assert!(created);
    let mut second_draft = draft("rel-revision", "historical", "REL compact");
    second_draft.updated_at_ns = 200;
    let (second, changed) = rel.publish_memory(Some(first.id), 1, second_draft).unwrap();
    assert!(changed);
    let TombstoneOutcome::Published(coordinates) =
        rel.tombstone_memory(first.id, second.revision).unwrap()
    else {
        panic!("tombstone should publish");
    };
    rel.sync().unwrap();

    let expected = rel.memories.records().to_vec();
    let times: Vec<_> = expected
        .iter()
        .map(|record| rel.transaction_time_ns(record.global_version))
        .collect();
    let descriptor = rel.memory_retention_descriptor(first.id).unwrap();
    let version = rel.latest_global_version();
    let memory_version = rel.memory_version();
    assert_compact_memory_payloads(&mut rel.container, &expected);
    append_unversioned_compact_frames(&mut rel.container, expected.last().unwrap(), 81);
    assert_eq!(rel.memories.records(), expected.as_slice());
    assert_eq!(rel.latest_global_version(), version);
    assert_eq!(rel.memory_version(), memory_version);

    let report = rel.reclaim_storage(&output).unwrap();
    assert!(report.unpublished_backing_chunks >= 2);
    assert!(report.orphan_memory_body_chunks >= 1);
    let mut reopened = Cva::open(&output).unwrap();
    assert_eq!(reopened.latest_global_version(), version);
    assert_eq!(reopened.memory_version(), memory_version);
    assert_reclaimed_records(reopened.memories.records(), &expected, &times, |global| {
        reopened.transaction_time_ns(global)
    });
    assert_eq!(
        reopened.memory_retention_descriptor(first.id).unwrap(),
        descriptor
    );
    assert!(matches!(
        reopened.memory_lifecycle_state(first.id).unwrap(),
        LifecycleState::Tombstoned(value) if value == coordinates
    ));
    assert_compact_memory_payloads(&mut reopened.container, &expected);
    assert!(matches!(
        reopened.memory_body_id(first.id),
        Err(MemoryError::MissingMemory)
    ));
}

#[test]
fn phy_reclamation_preserves_compact_tombstoned_history_exactly() {
    let scratch = ScratchDirectory::new();
    let source = scratch.file("source.phy");
    let output = scratch.file("reclaimed.phy");
    let mut phy = Phylactery::create(&source).unwrap();
    let (first, created) = phy
        .publish_memory(None, 0, draft("phy-create", "current", "PHY compact"))
        .unwrap();
    assert!(created);
    let mut second_draft = draft("phy-revision", "historical", "PHY compact");
    second_draft.updated_at_ns = 200;
    let (second, changed) = phy.publish_memory(Some(first.id), 1, second_draft).unwrap();
    assert!(changed);
    let TombstoneOutcome::Published(coordinates) =
        phy.tombstone_memory(first.id, second.revision).unwrap()
    else {
        panic!("tombstone should publish");
    };
    phy.sync().unwrap();

    let expected = phy.memories.records().to_vec();
    let times: Vec<_> = expected
        .iter()
        .map(|record| phy.transaction_time_ns(record.global_version))
        .collect();
    let descriptor = phy.memory_retention_descriptor(first.id).unwrap();
    let version = phy.latest_global_version();
    let memory_version = phy.memory_version();
    assert_compact_memory_payloads(&mut phy.container, &expected);
    append_unversioned_compact_frames(&mut phy.container, expected.last().unwrap(), 82);
    assert_eq!(phy.memories.records(), expected.as_slice());
    assert_eq!(phy.latest_global_version(), version);
    assert_eq!(phy.memory_version(), memory_version);

    let report = phy.reclaim_storage(&output).unwrap();
    assert!(report.unpublished_backing_chunks >= 2);
    assert!(report.orphan_memory_body_chunks >= 1);
    let mut reopened = Phylactery::open(&output).unwrap();
    assert_eq!(reopened.latest_global_version(), version);
    assert_eq!(reopened.memory_version(), memory_version);
    assert_reclaimed_records(reopened.memories.records(), &expected, &times, |global| {
        reopened.transaction_time_ns(global)
    });
    assert_eq!(
        reopened.memory_retention_descriptor(first.id).unwrap(),
        descriptor
    );
    assert!(matches!(
        reopened.memory_lifecycle_state(first.id).unwrap(),
        LifecycleState::Tombstoned(value) if value == coordinates
    ));
    assert_compact_memory_payloads(&mut reopened.container, &expected);
    assert!(matches!(
        reopened.memory_body_id(first.id),
        Err(MemoryError::MissingMemory)
    ));
}
