use super::entity::prepare;
use super::{StorageUpgradeError, StorageUpgradeRequest, plan_storage_upgrade};
use crate::entity_model::EntityRecord;
use crate::{Container, ContainerError, Cva, EntityGlobalId, EntityId, FileKind, Phylactery};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("entity-upgrade-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn downgrade_header_to_v1(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn legacy_r1_record(record: &EntityRecord) -> Vec<u8> {
    let mut out = b"CVAENTR1".to_vec();
    out.extend_from_slice(&record.id.0);
    out.extend_from_slice(&record.revision.to_le_bytes());
    out.extend_from_slice(&record.created_at_ns.to_le_bytes());
    out.extend_from_slice(&record.updated_at_ns.to_le_bytes());
    write_string(&mut out, &record.canonical_name);
    out.extend_from_slice(&(record.aliases.len() as u16).to_le_bytes());
    for alias in &record.aliases {
        write_string(&mut out, alias);
    }
    write_string(&mut out, &record.kind);
    write_string(&mut out, &record.summary);
    write_string(&mut out, &record.mutation_id);
    out
}

fn write_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn append_record(
    container: &mut Container,
    id: EntityId,
    global_id: Option<EntityGlobalId>,
    revision: u64,
    global_version: u64,
    entity_version: u64,
    mutation_id: &str,
    updated_at_ns: i64,
) {
    let record = EntityRecord {
        id,
        global_id,
        revision,
        canonical_name: "Legacy project entity".into(),
        aliases: vec!["Legacy alias".into()],
        kind: "project".into(),
        summary: "Stable semantic evidence".into(),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns,
        global_version: 0,
        entity_version: 0,
    };
    let payload = if global_id.is_some() {
        crate::entity_codec::encode_record(&record).unwrap()
    } else {
        legacy_r1_record(&record)
    };
    let carrier = container.append(&payload).unwrap();
    container
        .append(&crate::entity_codec::encode_version(
            crate::entity_codec::EntityVersion {
                global_version,
                entity_version,
                record: carrier,
            },
        ))
        .unwrap();
}

fn create_v1_fixture(path: &Path, kind: FileKind) -> (EntityId, EntityId) {
    match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::create(path).unwrap();
            append_entity_history(&mut owner.container);
            owner.sync().unwrap();
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::create(path).unwrap();
            append_entity_history(&mut owner.container);
            owner.sync().unwrap();
        }
    }
    downgrade_header_to_v1(path);
    (EntityId([0x61; 32]), EntityId([0x62; 32]))
}

fn append_entity_history(container: &mut Container) {
    let id = EntityId([0x61; 32]);
    let replacement = EntityId([0x62; 32]);
    // Reserve tickets first to create a legal late binding: revision 1 is
    // physically marked after ticket 2, then revision 2 claims ticket 1.
    for expected in 1..=8 {
        assert_eq!(container.allocate_version().unwrap(), expected);
    }
    append_record(container, id, None, 1, 2, 1, "entity-r1", 10);
    container.append(b"interleaved-other-owner-frame").unwrap();
    append_record(
        container,
        id,
        Some(EntityGlobalId([0xA1; 16])),
        2,
        1,
        2,
        "entity-r2-promote",
        15,
    );
    append_record(
        container,
        id,
        Some(EntityGlobalId([0xB2; 16])),
        3,
        3,
        3,
        "entity-r3-change-id",
        11,
    );
    append_record(container, id, None, 4, 4, 4, "entity-r4-clear-id", 10);
    append_record(
        container,
        id,
        None,
        5,
        5,
        5,
        "entity-r5-empty-mutable-delta",
        10,
    );
    append_record(
        container,
        replacement,
        Some(EntityGlobalId([0xC3; 16])),
        1,
        6,
        6,
        "replacement-r1",
        10,
    );
    let terminal = container
        .append(&crate::entity_codec::encode_tombstone(
            crate::entity_codec::EntityTombstone {
                id,
                revision: 6,
                replacement_id: replacement,
            },
        ))
        .unwrap();
    container
        .append(&crate::entity_codec::encode_version(
            crate::entity_codec::EntityVersion {
                global_version: 7,
                entity_version: 7,
                record: terminal,
            },
        ))
        .unwrap();

    // This syntactically valid carrier has no accepted marker and must remain
    // inert: prepare returns replacements only for accepted publications.
    let unpublished = EntityRecord {
        id: EntityId([0x63; 32]),
        global_id: Some(EntityGlobalId([0xD4; 16])),
        revision: 1,
        canonical_name: "Unpublished entity".into(),
        aliases: Vec::new(),
        kind: "project".into(),
        summary: "Inert".into(),
        mutation_id: "unpublished-carrier".into(),
        created_at_ns: 10,
        updated_at_ns: 10,
        global_version: 0,
        entity_version: 0,
    };
    container
        .append(&crate::entity_codec::encode_record(&unpublished).unwrap())
        .unwrap();
}

#[test]
fn entity_upgrade_prepares_only_accepted_v1_history_without_changing_source() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        let fixture = Fixture::new();
        let path = fixture.0.join(match kind {
            FileKind::Reliquary => "source.rel",
            FileKind::Phylactery => "source.phy",
        });
        let (id, replacement) = create_v1_fixture(&path, kind);
        let source_bytes = fs::read(&path).unwrap();
        let source_digest = Sha256::digest(&source_bytes);

        match kind {
            FileKind::Reliquary => {
                let owner = Cva::open_v1_read_only(&path).unwrap();
                assert_eq!(owner.entity_stats().revisions, 6);
                assert_eq!(owner.entity_revision(id, 1).unwrap().global_id, None);
                assert_eq!(
                    owner.entity_revision(id, 2).unwrap().global_id,
                    Some(EntityGlobalId([0xA1; 16]))
                );
                assert_eq!(
                    owner.entity_revision(id, 3).unwrap().global_id,
                    Some(EntityGlobalId([0xB2; 16]))
                );
                assert_eq!(owner.entity_revision(id, 4).unwrap().global_id, None);
                assert_eq!(owner.entity_revision(id, 3).unwrap().updated_at_ns, 11);
                assert_eq!(owner.entity_revision(id, 4).unwrap().updated_at_ns, 10);
                assert!(matches!(
                    owner.entity_lifecycle_state(id).unwrap(),
                    crate::owner_lifecycle::LifecycleState::Tombstoned(_)
                ));
                assert!(owner.entity(replacement).is_ok());
            }
            FileKind::Phylactery => {
                let owner = Phylactery::open_v1_read_only(&path).unwrap();
                assert_eq!(owner.entity_stats().revisions, 6);
                assert_eq!(owner.entity_revision(id, 1).unwrap().global_id, None);
                assert_eq!(
                    owner.entity_revision(id, 2).unwrap().global_id,
                    Some(EntityGlobalId([0xA1; 16]))
                );
                assert_eq!(
                    owner.entity_revision(id, 3).unwrap().global_id,
                    Some(EntityGlobalId([0xB2; 16]))
                );
                assert_eq!(owner.entity_revision(id, 4).unwrap().global_id, None);
                assert_eq!(owner.entity_revision(id, 3).unwrap().updated_at_ns, 11);
                assert_eq!(owner.entity_revision(id, 4).unwrap().updated_at_ns, 10);
                assert!(matches!(
                    owner.entity_lifecycle_state(id).unwrap(),
                    crate::owner_lifecycle::LifecycleState::Tombstoned(_)
                ));
                assert!(owner.entity(replacement).is_ok());
            }
        }

        let mut source = Container::open_v1_read_only(&path).unwrap();
        let prepared = prepare(&source).unwrap();
        assert_eq!(
            prepared.len(),
            6,
            "five revisions plus replacement creation"
        );
        let mut original = std::collections::BTreeMap::new();
        source
            .visit_payloads::<ContainerError>(|object, payload| {
                original.insert(object, payload.to_vec());
                Ok(())
            })
            .unwrap();
        let mut previous = std::collections::HashMap::new();
        let mut reconstructed = std::collections::BTreeMap::new();
        for (object, payload) in &prepared {
            assert!(
                original.contains_key(object),
                "replacement uses a source carrier ref"
            );
            let record = if payload.starts_with(b"CVAENC01") {
                crate::entity_compact_codec::decode_creation(payload)
                    .unwrap()
                    .record
            } else {
                let delta = crate::entity_compact_codec::decode_delta(payload).unwrap();
                let base = previous
                    .get(&delta.id)
                    .expect("delta follows its accepted creation");
                crate::entity_compact_apply::apply_delta(base, &delta).unwrap()
            };
            assert!(
                payload.starts_with(b"CVAENC01") || payload.starts_with(b"CVAEND01"),
                "prepared payload is a compact creation or delta"
            );
            previous.insert(record.id, record.clone());
            reconstructed.insert((record.id, record.revision), record);
        }
        for revision in 1..=5 {
            let record = reconstructed.get(&(id, revision)).unwrap();
            assert_eq!(
                record.global_version,
                [2, 1, 3, 4, 5][(revision - 1) as usize]
            );
            assert_eq!(record.entity_version, revision);
            assert_eq!(
                record.global_id,
                match revision {
                    1 | 4 | 5 => None,
                    2 => Some(EntityGlobalId([0xA1; 16])),
                    _ => Some(EntityGlobalId([0xB2; 16])),
                }
            );
        }
        assert_eq!(reconstructed[&(replacement, 1)].global_version, 6);
        let unpublished_offset = original
            .iter()
            .find_map(|(object, payload)| {
                crate::entity_codec::decode_record(payload)
                    .ok()
                    .flatten()
                    .filter(|record| record.mutation_id == "unpublished-carrier")
                    .map(|_| *object)
            })
            .expect("source has an unversioned Entity carrier");
        assert!(!prepared.contains_key(&unpublished_offset));
        assert_eq!(fs::read(&path).unwrap(), source_bytes);
        assert_eq!(Sha256::digest(fs::read(&path).unwrap()), source_digest);

        let request = StorageUpgradeRequest {
            source: path.clone(),
            destination: fixture.0.join("must-not-be-published"),
        };
        let plan = plan_storage_upgrade(request.clone()).unwrap();
        assert!(matches!(
            super::shell::stage_with_check(&plan, || {
                Err(StorageUpgradeError::Migration("injected abort".into()))
            }),
            Err(StorageUpgradeError::Migration(_))
        ));
        assert!(!request.destination.exists());
        assert_eq!(fs::read(&path).unwrap(), source_bytes);
        assert!(fs::read_dir(&fixture.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".storage-upgrade-")
        }));
    }
}
