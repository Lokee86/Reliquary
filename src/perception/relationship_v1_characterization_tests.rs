//! Exact legacy Relationship owner replay used to freeze the compact contract.

use crate::relationship_codec::{self, RelationshipVersion};
use crate::relationship_model::RelationshipRecord;
use crate::{
    Container, Cva, EntityId, EntityRef, MemoryId, MemoryRef, Phylactery, RelationshipId,
    RelationshipParticipant,
};
use std::fs;
use std::path::{Path, PathBuf};

fn test_dir() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!(
            "relationship-v1-characterization-{}",
            uuid::Uuid::new_v4()
        ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn downgrade_header_to_v1(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn record(
    revision: u64,
    updated_at_ns: i64,
    mutation_id: &str,
    summary: &str,
) -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([0x41; 32]),
        revision,
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
        summary: summary.into(),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns,
        global_version: 0,
        relationship_version: 0,
    }
}

fn append_history(container: &mut Container) -> RelationshipId {
    // The global tickets are reserved monotonically, then the V1 markers bind
    // them in owner-history order 2, 1. This is a late-bound global reference,
    // not a descending sequence of physical ticket records.
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

    let first = record(1, 30, "  mutation-one  ", "  prior summary  ");
    let first_ref = container
        .append(&relationship_codec::encode_record(&first).unwrap())
        .unwrap();
    container
        .append(&relationship_codec::encode_version(RelationshipVersion {
            global_version: 2,
            relationship_version: 1,
            record: first_ref,
        }))
        .unwrap();

    // The historical owner accepts a backwards updated_at value when it
    // remains no earlier than created_at. Exact whitespace is also preserved.
    let second = record(2, 11, "  mutation-two  ", "  later summary  ");
    let second_ref = container
        .append(&relationship_codec::encode_record(&second).unwrap())
        .unwrap();
    container
        .append(&relationship_codec::encode_version(RelationshipVersion {
            global_version: 1,
            relationship_version: 2,
            record: second_ref,
        }))
        .unwrap();

    first.id
}

fn assert_history(path: &Path) {
    let before = fs::read(path).unwrap();
    let cva = Cva::open_v1_read_only(path).unwrap();
    assert_eq!(cva.relationship_stats().revisions, 2);
    assert_eq!(cva.relationship_version(), 2);
    assert_eq!(cva.container.transaction_time_ns(1), Some(200));
    assert_eq!(cva.container.transaction_time_ns(2), Some(100));
    let id = RelationshipId([0x41; 32]);
    let first = cva.relationship_revision(id, 1).unwrap();
    let second = cva.relationship_revision(id, 2).unwrap();
    assert_eq!((first.global_version, first.relationship_version), (2, 1));
    assert_eq!((second.global_version, second.relationship_version), (1, 2));
    assert_eq!(
        cva.container.transaction_time_ns(first.global_version),
        Some(100)
    );
    assert_eq!(
        cva.container.transaction_time_ns(second.global_version),
        Some(200)
    );
    assert_eq!(first.kind, "  observes  ");
    assert_eq!(first.participants[0].entity.owner_id, "  external-entity  ");
    assert_eq!(first.participants[0].entity.entity_id, EntityId([0x11; 32]));
    assert_eq!(
        first.participants[0].role.as_deref(),
        Some("  source role  ")
    );
    assert_eq!(first.participants[1].entity.owner_id, "remote-entity");
    assert_eq!(first.participants[1].entity.entity_id, EntityId([0x22; 32]));
    assert_eq!(first.participants[1].role, None);
    assert_eq!(first.evidence[0].owner_id, "  external-memory  ");
    assert_eq!(first.evidence[0].memory_id, MemoryId([0x33; 32]));
    assert_eq!(first.evidence[1].owner_id, "remote-memory");
    assert_eq!(first.evidence[1].memory_id, MemoryId([0x44; 32]));
    assert_eq!(first.summary, "  prior summary  ");
    assert_eq!(first.mutation_id, "  mutation-one  ");
    assert_eq!(second.summary, "  later summary  ");
    assert_eq!(second.mutation_id, "  mutation-two  ");
    assert_eq!((first.created_at_ns, first.updated_at_ns), (10, 30));
    assert_eq!((second.created_at_ns, second.updated_at_ns), (10, 11));
    drop(cva);
    assert_eq!(
        fs::read(path).unwrap(),
        before,
        "read-only V1 open must not rewrite bytes"
    );
}

#[test]
fn v1_rel_reopen_preserves_legacy_relationship_values_and_late_tickets() {
    let path = test_dir().join("relationship-history.rel");
    let mut rel = Cva::create(&path).unwrap();
    append_history(&mut rel.container);
    rel.sync().unwrap();
    drop(rel);
    downgrade_header_to_v1(&path);
    assert_history(&path);
}

#[test]
fn v1_phy_reopen_preserves_legacy_relationship_values_and_late_tickets() {
    let path = test_dir().join("relationship-history.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    append_history(&mut phy.container);
    phy.sync().unwrap();
    drop(phy);
    downgrade_header_to_v1(&path);

    let before = fs::read(&path).unwrap();
    let phy = Phylactery::open_v1_read_only(&path).unwrap();
    assert_eq!(phy.relationship_stats().revisions, 2);
    assert_eq!(phy.relationship_version(), 2);
    assert_eq!(phy.container.transaction_time_ns(1), Some(200));
    assert_eq!(phy.container.transaction_time_ns(2), Some(100));
    let id = RelationshipId([0x41; 32]);
    let first = phy.relationship_revision(id, 1).unwrap();
    let second = phy.relationship_revision(id, 2).unwrap();
    assert_eq!((first.global_version, first.relationship_version), (2, 1));
    assert_eq!((second.global_version, second.relationship_version), (1, 2));
    assert_eq!(
        phy.container.transaction_time_ns(first.global_version),
        Some(100)
    );
    assert_eq!(
        phy.container.transaction_time_ns(second.global_version),
        Some(200)
    );
    assert_eq!(first.kind, "  observes  ");
    assert_eq!(first.participants[0].entity.owner_id, "  external-entity  ");
    assert_eq!(first.participants[0].entity.entity_id, EntityId([0x11; 32]));
    assert_eq!(
        first.participants[0].role.as_deref(),
        Some("  source role  ")
    );
    assert_eq!(first.participants[1].entity.owner_id, "remote-entity");
    assert_eq!(first.participants[1].entity.entity_id, EntityId([0x22; 32]));
    assert_eq!(first.participants[1].role, None);
    assert_eq!(first.evidence[0].owner_id, "  external-memory  ");
    assert_eq!(first.evidence[0].memory_id, MemoryId([0x33; 32]));
    assert_eq!(first.evidence[1].owner_id, "remote-memory");
    assert_eq!(first.evidence[1].memory_id, MemoryId([0x44; 32]));
    assert_eq!(first.summary, "  prior summary  ");
    assert_eq!(first.mutation_id, "  mutation-one  ");
    assert_eq!(second.summary, "  later summary  ");
    assert_eq!(second.mutation_id, "  mutation-two  ");
    assert_eq!((first.created_at_ns, first.updated_at_ns), (10, 30));
    assert_eq!((second.created_at_ns, second.updated_at_ns), (10, 11));
    drop(phy);
    assert_eq!(
        fs::read(&path).unwrap(),
        before,
        "read-only V1 open must not rewrite bytes"
    );
}
