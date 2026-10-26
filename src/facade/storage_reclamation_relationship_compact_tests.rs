//! Scratch REL/PHY storage reclamation coverage for compact Relationship chains.
use crate::owner_lifecycle::{LifecycleState, TombstoneCoordinates};
fn encode_creation(c: &RelationshipCreation) -> Result<Vec<u8>, crate::RelationshipError> {
    crate::relationship_compact_codec::publication::encode(
        &crate::relationship_compact_codec::publication::RelationshipPublication::Creation(
            c.clone(),
        ),
    )
}
fn encode_delta(d: &RelationshipDelta) -> Result<Vec<u8>, crate::RelationshipError> {
    crate::relationship_compact_codec::publication::encode(
        &crate::relationship_compact_codec::publication::RelationshipPublication::Delta(d.clone()),
    )
}
use crate::relationship_compact_contract::{RelationshipCreation, RelationshipDelta};
use crate::relationship_model::RelationshipRecord;
use crate::{
    Cva, EntityId, EntityRef, MemoryId, MemoryRef, Phylactery, RelationshipId,
    RelationshipParticipant,
};
use reliquary_container::Container;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "reclaim-compact-relationship-{}",
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

fn record(id: u8, revision: u64, global: u64, local: u64, mutation: &str) -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([id; 32]),
        revision,
        kind: "  observes  ".into(),
        participants: vec![
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "  external-entity  ".into(),
                    entity_id: EntityId([id.wrapping_add(1); 32]),
                },
                role: Some("  exact role  ".into()),
            },
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "remote-entity".into(),
                    entity_id: EntityId([id.wrapping_add(2); 32]),
                },
                role: None,
            },
        ],
        evidence: vec![MemoryRef {
            owner_id: "  external-memory  ".into(),
            memory_id: MemoryId([id.wrapping_add(3); 32]),
        }],
        summary: format!("  relationship {id} revision {revision}  "),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 20 + revision as i64,
        global_version: global,
        relationship_version: local,
    }
}

fn append_publication(container: &mut Container, payload: &[u8], global: u64, _local: u64) {
    let bytes = reliquary_container::publication_envelope::encode(
        reliquary_container::publication_envelope::PublicationEnvelope {
            global_version: global,
            clock: reliquary_container::publication_envelope::PublicationClock::BindReserved,
            body: payload,
        },
    )
    .unwrap();
    container.append(&bytes).unwrap();
}

fn build_history(path: &Path) -> (Vec<RelationshipRecord>, Vec<i64>) {
    let mut container = Container::open(path).unwrap();
    let first = record(0x21, 1, 1, 1, "create-21");
    let first_bytes = encode_creation(&RelationshipCreation::new(first.clone()).unwrap()).unwrap();
    assert_eq!(
        container
            .allocate_version_with_transaction_time(Some(1_001))
            .unwrap(),
        1
    );
    append_publication(&mut container, &first_bytes, 1, 1);

    let mut second = first.clone();
    second.revision = 2;
    second.global_version = 2;
    second.relationship_version = 2;
    second.mutation_id = "revise-21".into();
    second.participants[0].role = Some("  revised exact role  ".into());
    second.summary = "  updated retained relationship  ".into();
    second.updated_at_ns = 15;
    let second_bytes = encode_delta(&RelationshipDelta::between(&first, &second).unwrap()).unwrap();
    assert_eq!(
        container
            .allocate_version_with_transaction_time(Some(1_002))
            .unwrap(),
        2
    );
    append_publication(&mut container, &second_bytes, 2, 2);

    assert_eq!(
        container
            .allocate_version_with_transaction_time(Some(1_003))
            .unwrap(),
        3
    );
    let body = crate::relationship_compact_codec::publication::encode(
        &crate::relationship_compact_codec::publication::RelationshipPublication::Tombstone {
            id: first.id,
            revision: 3,
            relationship_version: 3,
        },
    )
    .unwrap();
    append_publication(&mut container, &body, 3, 3);

    // Structurally valid orphan creation and delta have no version markers and
    // must be removed as unpublished backing carriers during reclamation.
    let orphan = record(0x71, 1, 71, 71, "unpublished-create");
    container
        .append(&encode_creation(&RelationshipCreation::new(orphan.clone()).unwrap()).unwrap())
        .unwrap();
    let mut orphan_next = orphan.clone();
    orphan_next.revision = 2;
    orphan_next.global_version = 72;
    orphan_next.relationship_version = 72;
    orphan_next.mutation_id = "unpublished-delta".into();
    orphan_next.summary = "never published".into();
    container
        .append(&encode_delta(&RelationshipDelta::between(&orphan, &orphan_next).unwrap()).unwrap())
        .unwrap();
    container.sync().unwrap();

    (vec![first, second], vec![1_001, 1_002, 1_003])
}

fn file_hash(path: &Path) -> [u8; 32] {
    Sha256::digest(fs::read(path).unwrap()).into()
}

fn payloads(container: &Container) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    let source = container.read_source().unwrap();
    let mut cursor = source.physical_cursor().unwrap();
    while let Some(frame) = cursor.next_frame().unwrap() {
        if let Some(envelope) =
            reliquary_container::publication_envelope::decode(&frame.payload).unwrap()
        {
            result.push(envelope.body.to_vec());
        } else {
            result.push(frame.payload);
        }
    }
    result
}

fn assert_compact_carrier_counts(container: &Container) {
    let payloads = payloads(container);
    assert_eq!(
        payloads
            .iter()
            .filter(|p| p.starts_with(b"CVARNC02"))
            .count(),
        1
    );
    assert_eq!(
        payloads
            .iter()
            .filter(|p| p.starts_with(b"CVARND02"))
            .count(),
        1
    );
    assert_eq!(
        payloads
            .iter()
            .filter(|p| p.starts_with(b"CVARTD02"))
            .count(),
        1
    );
    assert_eq!(
        payloads
            .iter()
            .filter(|p| p.starts_with(b"CVARELV1"))
            .count(),
        0
    );
    assert!(!payloads.iter().any(|p| p.starts_with(b"CVARELR1")));
    assert!(!payloads.iter().any(|p| {
        (p.starts_with(b"CVARNC02") || p.starts_with(b"CVARND02"))
            && p.get(8..40)
                .is_some_and(|id| id.iter().all(|byte| *byte == 0x71))
    }));
}

fn assert_history(
    expected: &[RelationshipRecord],
    times: &[i64],
    actual: &[RelationshipRecord],
    revision: impl Fn(RelationshipId, u64) -> crate::Relationship,
    lifecycle: impl Fn(RelationshipId) -> LifecycleState,
    transaction_time: impl Fn(u64) -> Option<i64>,
) {
    assert_eq!(actual, expected);
    assert_eq!(expected.len(), 2);
    for record in expected {
        assert_eq!(
            transaction_time(record.global_version),
            Some(times[(record.global_version - 1) as usize])
        );
    }
    for (index, expected_record) in expected.iter().enumerate() {
        let value = revision(expected_record.id, (index + 1) as u64);
        assert_eq!(value.id, expected_record.id);
        assert_eq!(value.revision, expected_record.revision);
        assert_eq!(value.global_version, expected_record.global_version);
        assert_eq!(
            value.relationship_version,
            expected_record.relationship_version
        );
        assert_eq!(value.kind, expected_record.kind);
        assert_eq!(value.participants, expected_record.participants);
        assert_eq!(value.evidence, expected_record.evidence);
        assert_eq!(value.summary, expected_record.summary);
        assert_eq!(value.mutation_id, expected_record.mutation_id);
        assert_eq!(value.created_at_ns, expected_record.created_at_ns);
        assert_eq!(value.updated_at_ns, expected_record.updated_at_ns);
    }
    assert_eq!(
        lifecycle(expected[0].id),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(3),
            domain_version: Some(3),
        })
    );
}

#[test]
fn rel_reclamation_retains_compact_relationship_history_and_discards_orphans() {
    let scratch = Scratch::new();
    let source = scratch.file("source.rel");
    let output = scratch.file("reclaimed.rel");
    drop(Cva::create(&source).unwrap());
    let (expected, times) = build_history(&source);
    let original_hash = file_hash(&source);
    let mut rel = Cva::open(&source).unwrap();
    assert_eq!(rel.relationships.records(), expected.as_slice());

    let report = rel.reclaim_storage(&output).unwrap();
    assert_eq!(report.unpublished_backing_chunks, 2);
    assert_eq!(file_hash(&source), original_hash);
    assert_eq!(rel.relationships.records(), expected.as_slice());
    let reopened = Cva::open(&output).unwrap();
    assert_eq!(reopened.relationship_version(), 3);
    assert_eq!(reopened.latest_global_version(), 3);
    assert_history(
        &expected,
        &times,
        reopened.relationships.records(),
        |id, revision| reopened.relationship_revision(id, revision).unwrap(),
        |id| reopened.relationship_lifecycle_state(id).unwrap(),
        |global| reopened.transaction_time_ns(global),
    );
    assert_compact_carrier_counts(&reopened.container);
}

#[test]
fn phy_reclamation_retains_compact_relationship_history_and_discards_orphans() {
    let scratch = Scratch::new();
    let source = scratch.file("source.phy");
    let output = scratch.file("reclaimed.phy");
    drop(Phylactery::create(&source).unwrap());
    let (expected, times) = build_history(&source);
    let original_hash = file_hash(&source);
    let mut phy = Phylactery::open(&source).unwrap();
    assert_eq!(phy.relationships.records(), expected.as_slice());

    let report = phy.reclaim_storage(&output).unwrap();
    assert_eq!(report.unpublished_backing_chunks, 2);
    assert_eq!(file_hash(&source), original_hash);
    assert_eq!(phy.relationships.records(), expected.as_slice());
    let reopened = Phylactery::open(&output).unwrap();
    assert_eq!(reopened.relationship_version(), 3);
    assert_eq!(reopened.latest_global_version(), 3);
    assert_history(
        &expected,
        &times,
        reopened.relationships.records(),
        |id, revision| reopened.relationship_revision(id, revision).unwrap(),
        |id| reopened.relationship_lifecycle_state(id).unwrap(),
        |global| reopened.transaction_time_ns(global),
    );
    assert_compact_carrier_counts(&reopened.container);
}
