//! Ordinary Relationship writes use one allocation envelope per publication.
use reliquary_perception_store::relationship_compact_codec::publication::{self, RelationshipPublication};
use crate::{
    Cva, EntityId, EntityRef, MemoryId, MemoryRef, Phylactery, RelationshipDraft,
    RelationshipError, RelationshipId,
};
use reliquary_container::Container;
use reliquary_container::publication_envelope::{self, PublicationClock};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "relationship-publication-writer-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(mutation_id: &str, summary: &str, updated_at_ns: i64) -> RelationshipDraft {
    RelationshipDraft {
        kind: "supports".into(),
        participants: vec![
            EntityRef {
                owner_id: "remote-entity-owner-a".into(),
                entity_id: EntityId([0x41; 32]),
            },
            EntityRef {
                owner_id: "remote-entity-owner-b".into(),
                entity_id: EntityId([0x42; 32]),
            },
        ]
        .into_iter()
        .map(|entity| crate::RelationshipParticipant {
            entity,
            role: Some("source".into()),
        })
        .collect(),
        evidence: vec![MemoryRef {
            owner_id: "remote-memory-owner".into(),
            memory_id: MemoryId([0x43; 32]),
        }],
        summary: summary.into(),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns,
    }
}

fn payloads(container: &Container) -> Vec<Vec<u8>> {
    let mut values = Vec::new();
    let source = container.read_source().unwrap();
    let mut cursor = source.physical_cursor().unwrap();
    while let Some(frame) = cursor.next_frame().unwrap() {
        values.push(frame.payload);
    }
    values
}

fn assert_envelope_only_publications(container: &Container, expected: usize) {
    let values = payloads(container);
    let publications = values
        .iter()
        .filter_map(|payload| publication_envelope::decode(payload).unwrap())
        .map(|envelope| {
            assert!(matches!(envelope.clock, PublicationClock::Allocate { .. }));
            publication::decode(envelope.body, envelope.global_version)
                .unwrap()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(publications.len(), expected);
    assert_eq!(
        values
            .iter()
            .filter(|payload| payload.starts_with(b"CVAPUB01"))
            .count(),
        expected,
        "one physical envelope per accepted Relationship publication"
    );
    assert_eq!(
        publications
            .iter()
            .filter(|value| matches!(value, RelationshipPublication::Creation(_)))
            .count(),
        1
    );
    assert_eq!(
        publications
            .iter()
            .filter(|value| matches!(value, RelationshipPublication::Delta(_)))
            .count(),
        expected - 2
    );
    assert_eq!(
        publications
            .iter()
            .filter(|value| matches!(value, RelationshipPublication::Tombstone { .. }))
            .count(),
        1
    );
    assert!(values.iter().all(|payload| {
        !payload.starts_with(b"CVARNC01")
            && !payload.starts_with(b"CVARND01")
            && !payload.starts_with(b"CVARELT1")
            && !payload.starts_with(b"CVARELV1")
            && !payload.starts_with(b"CVARELR1")
            && !payload.starts_with(b"CVARNC02")
            && !payload.starts_with(b"CVARND02")
            && !payload.starts_with(b"CVARTD02")
            && !payload.starts_with(b"CVAVERS1")
            && !payload.starts_with(b"CVAVERS2")
    }));
}

fn assert_writer_contract_rel(path: &Path) {
    let mut owner = Cva::create_rel_with_uuid(path, [0x91; 16], None).unwrap();
    let (created, changed) = owner
        .publish_relationship(None, 0, draft("create", "initial summary", 10))
        .unwrap();
    assert!(changed);
    let before_retry = fs::metadata(path).unwrap().len();
    let before_global = owner.latest_global_version();
    let before_local = owner.relationship_version();
    let (retried, changed) = owner
        .publish_relationship(None, 0, draft("create", "initial summary", 10))
        .unwrap();
    assert!(!changed);
    assert_eq!(retried, created);
    assert_eq!(fs::metadata(path).unwrap().len(), before_retry);
    assert_eq!(owner.latest_global_version(), before_global);
    assert_eq!(owner.relationship_version(), before_local);

    let (revised, changed) = owner
        .publish_relationship(Some(created.id), 1, draft("revise", "updated summary", 11))
        .unwrap();
    assert!(changed);
    assert_eq!(revised.revision, 2);
    let before_retry = fs::metadata(path).unwrap().len();
    let before_global = owner.latest_global_version();
    let before_local = owner.relationship_version();
    let (retried, changed) = owner
        .publish_relationship(Some(created.id), 1, draft("revise", "updated summary", 11))
        .unwrap();
    assert!(!changed);
    assert_eq!(retried, revised);
    assert_eq!(fs::metadata(path).unwrap().len(), before_retry);
    assert_eq!(owner.latest_global_version(), before_global);
    assert_eq!(owner.relationship_version(), before_local);

    let before_len = fs::metadata(path).unwrap().len();
    let before_global = owner.latest_global_version();
    let before_local = owner.relationship_version();
    assert!(matches!(
        owner.publish_relationship(
            Some(created.id),
            2,
            RelationshipDraft {
                kind: String::new(),
                ..draft("invalid", "invalid kind", 12)
            }
        ),
        Err(RelationshipError::InvalidField("Relationship kind"))
    ));
    assert!(matches!(
        owner.publish_relationship(
            Some(created.id),
            0,
            draft("create", "conflicting retry", 10)
        ),
        Err(RelationshipError::MutationConflict)
    ));
    assert_eq!(fs::metadata(path).unwrap().len(), before_len);
    assert_eq!(owner.latest_global_version(), before_global);
    assert_eq!(owner.relationship_version(), before_local);

    owner
        .tombstone_relationship(created.id, revised.revision)
        .unwrap();
    let terminal_bytes = fs::metadata(path).unwrap().len();
    let terminal_global = owner.latest_global_version();
    let terminal_local = owner.relationship_version();
    owner
        .tombstone_relationship(created.id, revised.revision)
        .unwrap();
    assert_eq!(fs::metadata(path).unwrap().len(), terminal_bytes);
    assert_eq!(owner.latest_global_version(), terminal_global);
    assert_eq!(owner.relationship_version(), terminal_local);
    owner.sync().unwrap();
    assert_envelope_only_publications(&owner.container, 3);
    let records = owner.relationships.records().to_vec();
    let times = (1..=owner.latest_global_version())
        .map(|global| owner.transaction_time_ns(global))
        .collect::<Vec<_>>();
    drop(owner);
    let reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.relationships.records(), records.as_slice());
    assert_eq!(reopened.relationship_version(), 3);
    assert_eq!(reopened.latest_global_version(), 3);
    assert_eq!(
        reopened
            .relationship_retention_descriptor(created.id)
            .unwrap()
            .tombstone_revision,
        3
    );
    for (index, time) in times.into_iter().enumerate() {
        assert_eq!(reopened.transaction_time_ns(index as u64 + 1), time);
    }
}

fn assert_writer_contract_phy(path: &Path) {
    let mut owner = Phylactery::create_with_uuid(path, [0x92; 16]).unwrap();
    let (created, changed) = owner
        .publish_relationship(None, 0, draft("create", "initial summary", 10))
        .unwrap();
    assert!(changed);
    let before_retry = fs::metadata(path).unwrap().len();
    let before_global = owner.container.latest_version();
    let before_local = owner.relationship_version();
    let (retried, changed) = owner
        .publish_relationship(None, 0, draft("create", "initial summary", 10))
        .unwrap();
    assert!(!changed);
    assert_eq!(retried, created);
    assert_eq!(fs::metadata(path).unwrap().len(), before_retry);
    assert_eq!(owner.container.latest_version(), before_global);
    assert_eq!(owner.relationship_version(), before_local);

    let (revised, changed) = owner
        .publish_relationship(Some(created.id), 1, draft("revise", "updated summary", 11))
        .unwrap();
    assert!(changed);
    assert_eq!(revised.revision, 2);
    let before_retry = fs::metadata(path).unwrap().len();
    let before_global = owner.container.latest_version();
    let before_local = owner.relationship_version();
    let (retried, changed) = owner
        .publish_relationship(Some(created.id), 1, draft("revise", "updated summary", 11))
        .unwrap();
    assert!(!changed);
    assert_eq!(retried, revised);
    assert_eq!(fs::metadata(path).unwrap().len(), before_retry);
    assert_eq!(owner.container.latest_version(), before_global);
    assert_eq!(owner.relationship_version(), before_local);

    let before_len = fs::metadata(path).unwrap().len();
    let before_global = owner.container.latest_version();
    let before_local = owner.relationship_version();
    assert!(matches!(
        owner.publish_relationship(
            Some(created.id),
            2,
            RelationshipDraft {
                kind: String::new(),
                ..draft("invalid", "invalid kind", 12)
            }
        ),
        Err(RelationshipError::InvalidField("Relationship kind"))
    ));
    assert!(matches!(
        owner.publish_relationship(
            Some(created.id),
            0,
            draft("create", "conflicting retry", 10)
        ),
        Err(RelationshipError::MutationConflict)
    ));
    assert_eq!(fs::metadata(path).unwrap().len(), before_len);
    assert_eq!(owner.container.latest_version(), before_global);
    assert_eq!(owner.relationship_version(), before_local);

    owner
        .tombstone_relationship(created.id, revised.revision)
        .unwrap();
    let terminal_bytes = fs::metadata(path).unwrap().len();
    let terminal_global = owner.container.latest_version();
    let terminal_local = owner.relationship_version();
    owner
        .tombstone_relationship(created.id, revised.revision)
        .unwrap();
    assert_eq!(fs::metadata(path).unwrap().len(), terminal_bytes);
    assert_eq!(owner.container.latest_version(), terminal_global);
    assert_eq!(owner.relationship_version(), terminal_local);
    owner.sync().unwrap();
    assert_envelope_only_publications(&owner.container, 3);
    let records = owner.relationships.records().to_vec();
    let times = (1..=owner.container.latest_version())
        .map(|global| owner.transaction_time_ns(global))
        .collect::<Vec<_>>();
    drop(owner);
    let reopened = Phylactery::open(path).unwrap();
    assert_eq!(reopened.relationships.records(), records.as_slice());
    assert_eq!(reopened.relationship_version(), 3);
    assert_eq!(reopened.container.latest_version(), 3);
    assert_eq!(
        reopened
            .relationship_retention_descriptor(created.id)
            .unwrap()
            .tombstone_revision,
        3
    );
    for (index, time) in times.into_iter().enumerate() {
        assert_eq!(reopened.transaction_time_ns(index as u64 + 1), time);
    }
}

fn assert_truncated_terminal_recovers_rel(path: &Path) {
    let mut owner = Cva::create_rel_with_uuid(path, [0x93; 16], None).unwrap();
    let (created, _) = owner
        .publish_relationship(None, 0, draft("create", "initial summary", 10))
        .unwrap();
    owner
        .publish_relationship(Some(created.id), 1, draft("revise", "updated summary", 11))
        .unwrap();
    let accepted_end = owner.container.physical_length().unwrap();
    owner.tombstone_relationship(created.id, 2).unwrap();
    owner.sync().unwrap();
    let first_time = owner.transaction_time_ns(1);
    let second_time = owner.transaction_time_ns(2);
    drop(owner);

    let mut bytes = fs::read(path).unwrap();
    bytes.pop();
    fs::write(path, bytes).unwrap();
    crate::container_owner_tests::assert_normal_open_requires_repair(
        path,
        |path| {
            Cva::open(path)
                .map(|_| ())
                .map_err(|error| error.to_string())
        },
        accepted_end,
    );
    crate::container_owner_tests::repair_incomplete_physical_tail(path, accepted_end);
    let reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.relationship_version(), 2);
    assert_eq!(reopened.latest_global_version(), 2);
    assert_eq!(
        reopened.relationship(created.id).unwrap().summary,
        "updated summary"
    );
    assert_eq!(reopened.transaction_time_ns(1), first_time);
    assert_eq!(reopened.transaction_time_ns(2), second_time);
}

fn assert_truncated_terminal_recovers_phy(path: &Path) {
    let mut owner = Phylactery::create_with_uuid(path, [0x94; 16]).unwrap();
    let (created, _) = owner
        .publish_relationship(None, 0, draft("create", "initial summary", 10))
        .unwrap();
    owner
        .publish_relationship(Some(created.id), 1, draft("revise", "updated summary", 11))
        .unwrap();
    let accepted_end = owner.container.physical_length().unwrap();
    owner.tombstone_relationship(created.id, 2).unwrap();
    owner.sync().unwrap();
    let first_time = owner.transaction_time_ns(1);
    let second_time = owner.transaction_time_ns(2);
    drop(owner);

    let mut bytes = fs::read(path).unwrap();
    bytes.pop();
    fs::write(path, bytes).unwrap();
    crate::container_owner_tests::assert_normal_open_requires_repair(
        path,
        |path| {
            Phylactery::open(path)
                .map(|_| ())
                .map_err(|error| error.to_string())
        },
        accepted_end,
    );
    crate::container_owner_tests::repair_incomplete_physical_tail(path, accepted_end);
    let reopened = Phylactery::open(path).unwrap();
    assert_eq!(reopened.relationship_version(), 2);
    assert_eq!(reopened.container.latest_version(), 2);
    assert_eq!(
        reopened.relationship(created.id).unwrap().summary,
        "updated summary"
    );
    assert_eq!(reopened.transaction_time_ns(1), first_time);
    assert_eq!(reopened.transaction_time_ns(2), second_time);
}

#[test]
fn rel_relationship_writer_uses_allocation_envelopes_and_preserves_retry_contract() {
    let scratch = Scratch::new();
    assert_writer_contract_rel(&scratch.file("writer.rel"));
}

#[test]
fn phy_relationship_writer_uses_allocation_envelopes_and_preserves_retry_contract() {
    let scratch = Scratch::new();
    assert_writer_contract_phy(&scratch.file("writer.phy"));
}

#[test]
fn rel_relationship_writer_recovers_truncated_terminal_envelope() {
    let scratch = Scratch::new();
    assert_truncated_terminal_recovers_rel(&scratch.file("tail.rel"));
}

#[test]
fn phy_relationship_writer_recovers_truncated_terminal_envelope() {
    let scratch = Scratch::new();
    assert_truncated_terminal_recovers_phy(&scratch.file("tail.phy"));
}
