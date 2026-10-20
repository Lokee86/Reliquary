use crate::container::publication_envelope;
use crate::memory_compact_codec::publication::{self, MemoryPublication};
use crate::memory_compact_contract::MemoryDelta;
use crate::memory_model::MemoryRecord;
use crate::owner_lifecycle::{LifecycleState, TombstoneOutcome};
use crate::{
    Container, ContainerError, Cva, Memory, MemoryDraft, MemoryError, MemoryId, Phylactery,
};
use std::path::{Path, PathBuf};

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("memory-compact-writer-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn creation_draft() -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Compact writer title".into(),
        content: "The body is shared by both compact revisions.".into(),
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
        mutation_id: "compact-writer-create".into(),
        created_at_ns: 100,
        updated_at_ns: 100,
    }
}

fn revision_draft() -> MemoryDraft {
    let mut draft = creation_draft();
    draft.temporal_status = "historical".into();
    draft.mutation_id = "compact-writer-revise".into();
    draft.updated_at_ns = 200;
    draft
}

fn bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

fn payloads(path: &Path) -> Vec<Vec<u8>> {
    let mut container = Container::open(path).unwrap();
    let mut result = Vec::new();
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            result.push(payload.to_vec());
            Ok(())
        })
        .unwrap();
    result
}

fn assert_compact_frames(path: &Path, records: &[MemoryRecord]) {
    let payloads = payloads(path);
    let envelopes: Vec<_> = payloads
        .iter()
        .filter_map(|payload| publication_envelope::decode(payload).unwrap())
        .collect();
    let creations: Vec<_> = envelopes
        .iter()
        .filter_map(|envelope| {
            match publication::decode(envelope.body, envelope.global_version).unwrap() {
                Some(MemoryPublication::Creation(creation)) => Some(creation),
                _ => None,
            }
        })
        .collect();
    let deltas: Vec<_> = envelopes
        .iter()
        .filter_map(|envelope| {
            match publication::decode(envelope.body, envelope.global_version).unwrap() {
                Some(MemoryPublication::Delta(delta)) => Some(delta),
                _ => None,
            }
        })
        .collect();
    let terminals = envelopes
        .iter()
        .filter(|envelope| envelope.body.starts_with(b"CVAMT002"))
        .count();
    assert_eq!(envelopes.len(), 2 + terminals);
    assert!(terminals <= 1);
    assert_eq!(creations.len(), 1);
    assert_eq!(deltas.len(), 1);
    assert_eq!(creations[0].record, records[0]);
    assert_eq!(
        deltas[0],
        MemoryDelta::between(&records[0], &records[1]).unwrap()
    );

    for legacy_magic in [b"CVAMC001".as_slice(), b"CVAMD001", b"CVAMEMV1"] {
        assert!(
            !payloads
                .iter()
                .any(|payload| payload.starts_with(legacy_magic)),
            "found legacy Memory publication sidecar {}",
            String::from_utf8_lossy(legacy_magic)
        );
    }

    for legacy_version in 2..=8 {
        let magic = format!("CVAMEMR{legacy_version}");
        assert!(
            !payloads
                .iter()
                .any(|payload| payload.starts_with(magic.as_bytes())),
            "found legacy full-record frame {magic}"
        );
    }
}

fn assert_memory_snapshot(memory: &Memory, record: &MemoryRecord) {
    assert_eq!(memory.id, record.id);
    assert_eq!(memory.revision, record.revision);
    assert_eq!(memory.category, record.category);
    assert_eq!(memory.temporal_status, record.temporal_status);
    assert_eq!(memory.title, "Compact writer title");
    assert_eq!(
        memory.content,
        "The body is shared by both compact revisions."
    );
    assert_eq!(memory.mutation_id, record.mutation_id);
    assert_eq!(memory.created_at_ns, record.created_at_ns);
    assert_eq!(memory.updated_at_ns, record.updated_at_ns);
    assert_eq!(memory.global_version, record.global_version);
    assert_eq!(memory.memory_version, record.memory_version);
}

#[test]
fn rel_writes_compact_creation_and_delta_and_reopens_exact_tombstoned_history() {
    let scratch = ScratchDirectory::new();
    let path = scratch.file("compact.rel");
    let mut rel = Cva::create(&path).unwrap();
    let draft = creation_draft();
    let (first, created) = rel.publish_memory(None, 0, draft.clone()).unwrap();
    assert!(created);
    let id = first.id;
    let body_id = rel.memory_body_id(id).unwrap();
    let creation_bytes = bytes(&path);

    let (creation_retry, created) = rel.publish_memory(Some(id), 0, draft).unwrap();
    assert!(!created);
    assert_eq!(creation_retry, first);
    assert_eq!(bytes(&path), creation_bytes);

    let (second, created) = rel.publish_memory(Some(id), 1, revision_draft()).unwrap();
    assert!(created);
    assert_eq!(second.revision, 2);
    assert_eq!(second.memory_version, first.memory_version + 1);
    assert!(second.global_version > first.global_version);
    assert_eq!(second.created_at_ns, first.created_at_ns);
    assert_eq!(second.updated_at_ns, 200);
    assert_eq!(rel.memory_body_id(id).unwrap(), body_id);
    let revision_bytes = bytes(&path);
    let (revision_retry, created) = rel.publish_memory(Some(id), 1, revision_draft()).unwrap();
    assert!(!created);
    assert_eq!(revision_retry, second);
    assert_eq!(bytes(&path), revision_bytes);

    let records = rel.memories.records().to_vec();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].body_id, body_id);
    assert_eq!(records[1].body_id, body_id);
    assert_eq!(records[0].revision, 1);
    assert_eq!(records[1].revision, 2);
    assert_compact_frames(&path, &records);

    let tombstone = rel.tombstone_memory(id, second.revision).unwrap();
    let TombstoneOutcome::Published(coordinates) = tombstone else {
        panic!("first compact-history tombstone should publish");
    };
    let tombstone_bytes = bytes(&path);
    let tombstone_version = rel.memory_version();
    assert_eq!(tombstone_version, second.memory_version + 1);
    assert_eq!(
        rel.tombstone_memory(id, second.revision).unwrap(),
        TombstoneOutcome::AlreadyTombstoned(coordinates)
    );
    assert_eq!(rel.memory_version(), tombstone_version);
    assert_eq!(bytes(&path), tombstone_bytes);

    let payloads = payloads(&path);
    let envelopes: Vec<_> = payloads
        .iter()
        .filter_map(|payload| publication_envelope::decode(payload).unwrap())
        .collect();
    assert_eq!(envelopes.len(), 3);
    let tombstones: Vec<_> = envelopes
        .iter()
        .filter_map(|envelope| {
            match publication::decode(envelope.body, envelope.global_version).unwrap() {
                Some(MemoryPublication::Tombstone {
                    id,
                    revision,
                    memory_version,
                }) => Some((id, revision, memory_version)),
                _ => None,
            }
        })
        .collect();
    assert_eq!(tombstones.len(), 1);
    assert_eq!(tombstones[0].0, id);
    assert_eq!(tombstones[0].1, second.revision + 1);
    assert_eq!(tombstones[0].2, tombstone_version);
    assert_compact_frames(&path, &records);
    rel.sync().unwrap();
    drop(rel);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.memories.records(), records);
    assert_eq!(reopened.memory_version(), tombstone_version);
    assert!(matches!(
        reopened.memory_body_id(id),
        Err(MemoryError::MissingMemory)
    ));
    assert!(matches!(
        reopened.memory_lifecycle_state(id).unwrap(),
        LifecycleState::Tombstoned(value) if value == coordinates
    ));
    assert_memory_snapshot(&reopened.memory_revision(id, 1).unwrap(), &records[0]);
    assert_memory_snapshot(&reopened.memory_revision(id, 2).unwrap(), &records[1]);
    let retained = reopened.memory_retention_descriptor(id).unwrap();
    assert_eq!(
        retained.tombstone_global_version,
        coordinates.global_version.unwrap()
    );
    assert_eq!(
        retained.tombstone_memory_version,
        coordinates.domain_version.unwrap()
    );
    assert_eq!(retained.revisions.len(), 2);
}

#[test]
fn phy_writes_compact_creation_and_delta_and_reopens_exact_tombstoned_history() {
    let scratch = ScratchDirectory::new();
    let path = scratch.file("compact.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let draft = creation_draft();
    let (first, created) = phy.publish_memory(None, 0, draft.clone()).unwrap();
    assert!(created);
    let id = first.id;
    let body_id = phy.memory_body_id(id).unwrap();
    let creation_bytes = bytes(&path);

    let (creation_retry, created) = phy.publish_memory(Some(id), 0, draft).unwrap();
    assert!(!created);
    assert_eq!(creation_retry, first);
    assert_eq!(bytes(&path), creation_bytes);

    let (second, created) = phy.publish_memory(Some(id), 1, revision_draft()).unwrap();
    assert!(created);
    assert_eq!(second.revision, 2);
    assert_eq!(second.memory_version, first.memory_version + 1);
    assert!(second.global_version > first.global_version);
    assert_eq!(second.created_at_ns, first.created_at_ns);
    assert_eq!(second.updated_at_ns, 200);
    assert_eq!(phy.memory_body_id(id).unwrap(), body_id);
    let revision_bytes = bytes(&path);
    let (revision_retry, created) = phy.publish_memory(Some(id), 1, revision_draft()).unwrap();
    assert!(!created);
    assert_eq!(revision_retry, second);
    assert_eq!(bytes(&path), revision_bytes);

    let records = phy.memories.records().to_vec();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].body_id, body_id);
    assert_eq!(records[1].body_id, body_id);
    assert_eq!(records[0].revision, 1);
    assert_eq!(records[1].revision, 2);
    assert_compact_frames(&path, &records);

    let tombstone = phy.tombstone_memory(id, second.revision).unwrap();
    let TombstoneOutcome::Published(coordinates) = tombstone else {
        panic!("first compact-history tombstone should publish");
    };
    let tombstone_bytes = bytes(&path);
    let tombstone_version = phy.memory_version();
    assert_eq!(tombstone_version, second.memory_version + 1);
    assert_eq!(
        phy.tombstone_memory(id, second.revision).unwrap(),
        TombstoneOutcome::AlreadyTombstoned(coordinates)
    );
    assert_eq!(phy.memory_version(), tombstone_version);
    assert_eq!(bytes(&path), tombstone_bytes);

    let payloads = payloads(&path);
    let envelopes: Vec<_> = payloads
        .iter()
        .filter_map(|payload| publication_envelope::decode(payload).unwrap())
        .collect();
    assert_eq!(envelopes.len(), 3);
    let tombstones: Vec<_> = envelopes
        .iter()
        .filter_map(|envelope| {
            match publication::decode(envelope.body, envelope.global_version).unwrap() {
                Some(MemoryPublication::Tombstone {
                    id,
                    revision,
                    memory_version,
                }) => Some((id, revision, memory_version)),
                _ => None,
            }
        })
        .collect();
    assert_eq!(tombstones.len(), 1);
    assert_eq!(tombstones[0].0, id);
    assert_eq!(tombstones[0].1, second.revision + 1);
    assert_eq!(tombstones[0].2, tombstone_version);
    assert_compact_frames(&path, &records);
    phy.sync().unwrap();
    drop(phy);

    let mut reopened = Phylactery::open(&path).unwrap();
    assert_eq!(reopened.memories.records(), records);
    assert_eq!(reopened.memory_version(), tombstone_version);
    assert!(matches!(
        reopened.memory_body_id(id),
        Err(MemoryError::MissingMemory)
    ));
    assert!(matches!(
        reopened.memory_lifecycle_state(id).unwrap(),
        LifecycleState::Tombstoned(value) if value == coordinates
    ));
    assert_memory_snapshot(&reopened.memory_revision(id, 1).unwrap(), &records[0]);
    assert_memory_snapshot(&reopened.memory_revision(id, 2).unwrap(), &records[1]);
    let retained = reopened.memory_retention_descriptor(id).unwrap();
    assert_eq!(
        retained.tombstone_global_version,
        coordinates.global_version.unwrap()
    );
    assert_eq!(
        retained.tombstone_memory_version,
        coordinates.domain_version.unwrap()
    );
    assert_eq!(retained.revisions.len(), 2);
}
