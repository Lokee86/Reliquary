use super::shell::stage_relationship_storage_upgrade;
use super::{StorageUpgradeError, StorageUpgradeRequest, plan_storage_upgrade};
use crate::relationship_codec::{
    RelationshipTombstone, RelationshipVersion, decode_tombstone, decode_version, encode_record,
    encode_tombstone, encode_version,
};
use crate::relationship_model::RelationshipRecord;
use crate::{
    Container, ContainerError, Cva, EntityId, EntityRef, FileKind, MemoryId, MemoryRef, Phylactery,
    RelationshipId, RelationshipParticipant,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("relationship-upgrade-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn record(revision: u64, global: u64, local: u64, mutation: &str) -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([0x45; 32]),
        revision,
        kind: "authored relation kind".into(),
        participants: vec![
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "rel-owner".into(),
                    entity_id: EntityId([0x11; 32]),
                },
                role: Some("source".into()),
            },
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "phy-owner".into(),
                    entity_id: EntityId([0x22; 32]),
                },
                role: None,
            },
        ],
        evidence: vec![MemoryRef {
            owner_id: "evidence-owner".into(),
            memory_id: MemoryId([0x33; 32]),
        }],
        summary: "retained description".into(),
        mutation_id: mutation.into(),
        created_at_ns: 100,
        updated_at_ns: 100 + revision as i64,
        global_version: global,
        relationship_version: local,
    }
}

fn downgrade_header_to_v1(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn append_record(container: &mut Container, record: &RelationshipRecord) {
    let carrier = container.append(&encode_record(record).unwrap()).unwrap();
    container
        .append(&encode_version(RelationshipVersion {
            global_version: record.global_version,
            relationship_version: record.relationship_version,
            record: carrier,
        }))
        .unwrap();
}

fn append_history(container: &mut Container) -> (RelationshipRecord, RelationshipRecord) {
    for (expected, time) in [(1, 40_101), (2, 40_202), (3, 40_303)] {
        assert_eq!(
            container
                .allocate_version_with_transaction_time(Some(time))
                .unwrap(),
            expected
        );
    }
    let mut first = record(1, 1, 1, "legacy-create");
    first.participants.sort();
    append_record(container, &first);
    let mut second = record(2, 2, 2, "legacy-revise");
    second.participants[0].role = Some("reviewer".into());
    second.evidence.push(MemoryRef {
        owner_id: "second-evidence-owner".into(),
        memory_id: MemoryId([0x34; 32]),
    });
    second.summary = "revised retained description".into();
    second.updated_at_ns = 150;
    second.participants.sort();
    append_record(container, &second);
    let terminal = container
        .append(&encode_tombstone(RelationshipTombstone {
            id: first.id,
            revision: 3,
        }))
        .unwrap();
    container
        .append(&encode_version(RelationshipVersion {
            global_version: 3,
            relationship_version: 3,
            record: terminal,
        }))
        .unwrap();
    (first, second)
}

fn create_v1_fixture(path: &Path, kind: FileKind) -> (RelationshipRecord, RelationshipRecord) {
    let (first, second) = match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::create(path).unwrap();
            let records = append_history(&mut owner.container);
            owner.sync().unwrap();
            records
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::create(path).unwrap();
            let records = append_history(&mut owner.container);
            owner.sync().unwrap();
            records
        }
    };
    downgrade_header_to_v1(path);
    (first, second)
}

fn relationship_carrier_bytes(path: &Path, v1: bool) -> (u64, Vec<Vec<u8>>) {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open_compact(path).unwrap()
    };
    let mut refs = Vec::new();
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            if let Some(version) = decode_version(payload).unwrap() {
                refs.push(version.record);
            }
            Ok(())
        })
        .unwrap();
    let carriers = refs
        .into_iter()
        .map(|reference| container.read(reference).unwrap())
        .collect::<Vec<_>>();
    let bytes = carriers.iter().map(|payload| payload.len() as u64).sum();
    (bytes, carriers)
}

#[test]
fn relationship_upgrade_preserves_v1_history_for_rel_and_phy() {
    let mut measurements = Vec::new();
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        let fixture = Fixture::new();
        let source = fixture.0.join(if kind == FileKind::Reliquary {
            "source.rel"
        } else {
            "source.phy"
        });
        let (first, second) = create_v1_fixture(&source, kind);
        let source_bytes = fs::read(&source).unwrap();
        let source_digest = Sha256::digest(&source_bytes);
        let request = StorageUpgradeRequest {
            source: source.clone(),
            destination: fixture.0.join("destination"),
        };
        let plan = plan_storage_upgrade(request.clone()).unwrap();
        let staged = stage_relationship_storage_upgrade(&plan).unwrap();

        let (before, old_carriers) = relationship_carrier_bytes(&source, true);
        let (after, new_carriers) = relationship_carrier_bytes(staged.test_path(), false);
        assert_eq!(old_carriers.len(), 3);
        assert!(new_carriers[0].starts_with(b"CVARNC01"));
        assert!(new_carriers[1].starts_with(b"CVARND01"));
        assert!(decode_tombstone(&new_carriers[2]).unwrap().is_some());

        let staged_path = staged.test_path();
        match kind {
            FileKind::Reliquary => {
                let owner = Cva::open(staged_path).unwrap();
                assert_eq!(
                    owner.relationships.records(),
                    &[first.clone(), second.clone()]
                );
                assert!(matches!(
                    owner.relationship_lifecycle_state(first.id).unwrap(),
                    crate::owner_lifecycle::LifecycleState::Tombstoned(_)
                ));
                for global in 1..=3 {
                    assert_eq!(
                        owner.transaction_time_ns(global),
                        Some(40_000 + global as i64 * 101)
                    );
                }
            }
            FileKind::Phylactery => {
                let owner = Phylactery::open(staged_path).unwrap();
                assert_eq!(
                    owner.relationships.records(),
                    &[first.clone(), second.clone()]
                );
                assert!(matches!(
                    owner.relationship_lifecycle_state(first.id).unwrap(),
                    crate::owner_lifecycle::LifecycleState::Tombstoned(_)
                ));
                for global in 1..=3 {
                    assert_eq!(
                        owner.transaction_time_ns(global),
                        Some(40_000 + global as i64 * 101)
                    );
                }
            }
        }

        assert_eq!(fs::read(&source).unwrap(), source_bytes);
        assert_eq!(Sha256::digest(fs::read(&source).unwrap()), source_digest);
        assert!(!request.destination.exists());
        measurements.push(format!(
            "{:?}: legacy carrier bytes={before}, compact carrier bytes={after}, reduction={} bytes",
            kind,
            i128::from(before) - i128::from(after)
        ));
        drop(staged);
        assert!(!request.destination.exists());
    }

    let artifact = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join("relationship-upgrade-size.txt");
    fs::create_dir_all(artifact.parent().unwrap()).unwrap();
    fs::write(
        artifact,
        format!(
            "Raw accepted Relationship carrier byte measurement (no compression)\n{}\n",
            measurements.join("\n")
        ),
    )
    .unwrap();
}

#[test]
fn malformed_relationship_upgrade_rejects_without_source_or_destination_change() {
    let fixture = Fixture::new();
    let source = fixture.0.join("malformed.rel");
    let mut owner = Cva::create(&source).unwrap();
    owner.container.allocate_version().unwrap();
    let skipped = record(2, 1, 1, "revision-gap");
    append_record(&mut owner.container, &skipped);
    owner.sync().unwrap();
    downgrade_header_to_v1(&source);

    let source_bytes = fs::read(&source).unwrap();
    let destination = fixture.0.join("must-not-be-promoted");
    let plan = plan_storage_upgrade(StorageUpgradeRequest {
        source: source.clone(),
        destination: destination.clone(),
    })
    .unwrap();
    assert!(matches!(
        stage_relationship_storage_upgrade(&plan),
        Err(StorageUpgradeError::Relationship(_))
            | Err(StorageUpgradeError::Migration(_))
            | Err(StorageUpgradeError::Owner(_))
    ));
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert!(!destination.exists());
    assert!(fs::read_dir(&fixture.0).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".storage-upgrade-")
    }));
}
