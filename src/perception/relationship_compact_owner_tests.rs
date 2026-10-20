//! Scratch integration coverage for ordinary REL and PHY compact owner reopen.
use crate::relationship_codec::RelationshipVersion;
use crate::relationship_compact_codec::{encode_creation, encode_delta};
use crate::relationship_compact_contract::{RelationshipCreation, RelationshipDelta};
use crate::relationship_model::RelationshipRecord;
use crate::{
    Cva, EntityId, EntityRef, MemoryId, MemoryRef, Phylactery, RelationshipId,
    RelationshipParticipant,
};
use std::fs;
use std::path::{Path, PathBuf};

fn test_dir() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!(
            "relationship-compact-owner-{}",
            uuid::Uuid::new_v4()
        ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn initial_record() -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([0xA1; 32]),
        revision: 1,
        kind: "  observes  ".into(),
        participants: vec![
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "  external-entity  ".into(),
                    entity_id: EntityId([0x11; 32]),
                },
                role: Some("  source role  ".into()),
            },
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "remote-entity".into(),
                    entity_id: EntityId([0x22; 32]),
                },
                role: None,
            },
        ],
        evidence: vec![
            MemoryRef {
                owner_id: "  external-memory  ".into(),
                memory_id: MemoryId([0x33; 32]),
            },
            MemoryRef {
                owner_id: "remote-memory".into(),
                memory_id: MemoryId([0x44; 32]),
            },
        ],
        summary: "  original summary  ".into(),
        mutation_id: "  creation mutation  ".into(),
        created_at_ns: 10,
        updated_at_ns: 30,
        global_version: 2,
        relationship_version: 1,
    }
}

fn append_compact_history(container: &mut crate::Container) -> RelationshipRecord {
    // Reserve global tickets first, then bind the two owner frames in order
    // 2,1. Their transaction times remain tied to the ticket identities.
    assert_eq!(
        container
            .allocate_version_with_transaction_time(Some(200))
            .unwrap(),
        1
    );
    assert_eq!(
        container
            .allocate_version_with_transaction_time(Some(100))
            .unwrap(),
        2
    );

    let first = initial_record();
    let creation = RelationshipCreation::new(first.clone()).unwrap();
    let creation_ref = container
        .append(&encode_creation(&creation).unwrap())
        .unwrap();
    container
        .append(&crate::relationship_codec::encode_version(
            RelationshipVersion {
                global_version: 2,
                relationship_version: 1,
                record: creation_ref,
            },
        ))
        .unwrap();

    let mut second = first.clone();
    second.revision = 2;
    second.global_version = 1;
    second.relationship_version = 2;
    second.mutation_id = "  successor mutation  ".into();
    second.participants[0].role = Some("  reviewed role  ".into());
    second.summary = "  successor summary  ".into();
    // A backwards update time is valid because it still follows creation.
    second.updated_at_ns = 11;
    let delta = RelationshipDelta::between(&first, &second).unwrap();
    let delta_ref = container.append(&encode_delta(&delta).unwrap()).unwrap();
    container
        .append(&crate::relationship_codec::encode_version(
            RelationshipVersion {
                global_version: 1,
                relationship_version: 2,
                record: delta_ref,
            },
        ))
        .unwrap();
    second
}

fn assert_relationship_history(
    relationship_revision: impl Fn(RelationshipId, u64) -> crate::Relationship,
    transaction_time: impl Fn(u64) -> Option<i64>,
    first_expected: &RelationshipRecord,
    second_expected: &RelationshipRecord,
) {
    let id = first_expected.id;
    let first = relationship_revision(id, 1);
    let second = relationship_revision(id, 2);
    assert_eq!(first.id, id);
    assert_eq!(first.revision, 1);
    assert_eq!(second.id, id);
    assert_eq!(second.revision, 2);
    assert_eq!((first.global_version, first.relationship_version), (2, 1));
    assert_eq!((second.global_version, second.relationship_version), (1, 2));
    assert_eq!(transaction_time(1), Some(200));
    assert_eq!(transaction_time(2), Some(100));
    assert_eq!(transaction_time(first.global_version), Some(100));
    assert_eq!(transaction_time(second.global_version), Some(200));
    assert_eq!(first.kind, first_expected.kind);
    assert_eq!(first.participants, first_expected.participants);
    assert_eq!(first.evidence, first_expected.evidence);
    assert_eq!(first.summary, first_expected.summary);
    assert_eq!(first.mutation_id, first_expected.mutation_id);
    assert_eq!(first.created_at_ns, first_expected.created_at_ns);
    assert_eq!(first.updated_at_ns, first_expected.updated_at_ns);
    assert_eq!(second.kind, second_expected.kind);
    assert_eq!(second.participants, second_expected.participants);
    assert_eq!(second.evidence, second_expected.evidence);
    assert_eq!(second.summary, second_expected.summary);
    assert_eq!(second.mutation_id, second_expected.mutation_id);
    assert_eq!(second.created_at_ns, second_expected.created_at_ns);
    assert_eq!(second.updated_at_ns, second_expected.updated_at_ns);
    assert_eq!(first.participants[0].entity.owner_id, "  external-entity  ");
    assert_eq!(
        first.participants[0].role.as_deref(),
        Some("  source role  ")
    );
    assert_eq!(
        second.participants[0].role.as_deref(),
        Some("  reviewed role  ")
    );
    assert_eq!(first.participants[1].role, None);
    assert_eq!(first.evidence[0].owner_id, "  external-memory  ");
}

#[test]
fn compact_relationship_history_reopens_through_cva_owner_boundary() {
    let path = test_dir().join("compact-history.rel");
    let mut cva = Cva::create(&path).unwrap();
    let first = initial_record();
    let second = append_compact_history(&mut cva.container);
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.relationship_stats().revisions, 2);
    assert_eq!(reopened.relationship_version(), 2);
    assert_relationship_history(
        |id, revision| reopened.relationship_revision(id, revision).unwrap(),
        |version| reopened.container.transaction_time_ns(version),
        &first,
        &second,
    );
}

#[test]
fn compact_relationship_history_reopens_through_phylactery_owner_boundary() {
    let path = test_dir().join("compact-history.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let first = initial_record();
    let second = append_compact_history(&mut phy.container);
    phy.sync().unwrap();
    drop(phy);

    let reopened = Phylactery::open(&path).unwrap();
    assert_eq!(reopened.relationship_stats().revisions, 2);
    assert_eq!(reopened.relationship_version(), 2);
    assert_relationship_history(
        |id, revision| reopened.relationship_revision(id, revision).unwrap(),
        |version| reopened.container.transaction_time_ns(version),
        &first,
        &second,
    );
}
