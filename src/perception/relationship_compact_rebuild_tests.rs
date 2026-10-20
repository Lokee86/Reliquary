//! Accepted publication reconstruction and fail-closed framing regressions.
use crate::owner_lifecycle::{LifecycleState, TombstoneCoordinates};
use crate::relationship_codec::{
    RelationshipTombstone, RelationshipVersion, encode_tombstone, encode_version,
};
use crate::relationship_compact_codec::{encode_creation, encode_delta};
use crate::relationship_compact_contract::{
    RelationshipCreation, RelationshipDelta, RelationshipDescriptionProof,
};
use crate::relationship_model::RelationshipRecord;
use crate::relationship_rebuild::RelationshipOpenState;
use crate::relationship_store::RelationshipStore;
use crate::{
    Container, EntityId, EntityRef, FormatVersion, MemoryId, MemoryRef, RelationshipError,
    RelationshipId, RelationshipParticipant,
};
use std::fs;
use std::path::{Path, PathBuf};

fn test_path(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!(
            "relationship-compact-rebuild-{}",
            uuid::Uuid::new_v4()
        ));
    fs::create_dir_all(&directory).unwrap();
    directory.join(name)
}

fn initialize(path: &Path) -> Container {
    let mut container = Container::create(path).unwrap();
    RelationshipStore::empty()
        .initialize(&mut container)
        .unwrap();
    container
}

fn record(id: u8, revision: u64, global: u64, local: u64, mutation: &str) -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([id; 32]),
        revision,
        kind: "  observes  ".into(),
        participants: vec![
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "external-a".into(),
                    entity_id: EntityId([id.wrapping_add(1); 32]),
                },
                role: Some("  source role  ".into()),
            },
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "external-b".into(),
                    entity_id: EntityId([id.wrapping_add(2); 32]),
                },
                role: None,
            },
        ],
        evidence: vec![MemoryRef {
            owner_id: "external-memory".into(),
            memory_id: MemoryId([id.wrapping_add(3); 32]),
        }],
        summary: format!("  description-{id}  "),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 20,
        global_version: global,
        relationship_version: local,
    }
}

fn reserve_tickets(container: &mut Container, count: u64) {
    for expected in 1..=count {
        assert_eq!(
            container
                .allocate_version_with_transaction_time(Some(1_000 + expected as i64))
                .unwrap(),
            expected
        );
    }
}

fn append_creation(container: &mut Container, value: &RelationshipRecord) -> crate::ObjectRef {
    container
        .append(&encode_creation(&RelationshipCreation::new(value.clone()).unwrap()).unwrap())
        .unwrap()
}

fn append_delta(
    container: &mut Container,
    previous: &RelationshipRecord,
    next: &RelationshipRecord,
) -> (crate::ObjectRef, RelationshipDelta) {
    let delta = RelationshipDelta::between(previous, next).unwrap();
    let carrier = container.append(&encode_delta(&delta).unwrap()).unwrap();
    (carrier, delta)
}

fn append_version(container: &mut Container, carrier: crate::ObjectRef, global: u64, local: u64) {
    container
        .append(&encode_version(RelationshipVersion {
            global_version: global,
            relationship_version: local,
            record: carrier,
        }))
        .unwrap();
}

fn replay(path: &Path) -> Result<(RelationshipStore, Container), RelationshipError> {
    let mut state = RelationshipOpenState::new();
    let container = Container::open_scanned(path, |chunk, payload, latest| {
        state.ingest(chunk, payload, latest)
    })?;
    Ok((state.finish(&container)?, container))
}

fn replay_capturing_errors(
    path: &Path,
) -> (
    Result<RelationshipStore, RelationshipError>,
    Vec<RelationshipError>,
) {
    let mut state = RelationshipOpenState::new();
    let mut errors = Vec::new();
    let container = Container::open_scanned(path, |chunk, payload, latest| {
        if let Err(error) = state.ingest(chunk, payload, latest) {
            errors.push(error);
        }
        Ok::<(), RelationshipError>(())
    })
    .unwrap();
    (state.finish(&container), errors)
}

#[test]
fn compact_creation_delta_interleaving_tombstone_and_late_tickets_replay_exactly() {
    let path = test_path("interleaved.rel");
    let mut container = initialize(&path);
    reserve_tickets(&mut container, 4);

    // Two Relationship IDs interleave local clocks while global tickets were
    // reserved 1..4 and linked in marker order 2,1,3,4.
    let a1 = record(0x41, 1, 2, 1, "a-create");
    let a1_ref = append_creation(&mut container, &a1);
    append_version(&mut container, a1_ref, 2, 1);
    let b1 = record(0x42, 1, 1, 2, "b-create");
    let b1_ref = append_creation(&mut container, &b1);
    append_version(&mut container, b1_ref, 1, 2);

    let mut a2 = a1.clone();
    a2.revision = 2;
    a2.global_version = 3;
    a2.relationship_version = 3;
    a2.mutation_id = "a-update".into();
    a2.summary = "  changed description  ".into();
    a2.updated_at_ns = 15;
    let (a2_ref, delta) = append_delta(&mut container, &a1, &a2);
    assert_eq!(delta.global_version, 3);
    append_version(&mut container, a2_ref, 3, 3);

    let tombstone_ref = container
        .append(&encode_tombstone(RelationshipTombstone {
            id: a1.id,
            revision: 3,
        }))
        .unwrap();
    append_version(&mut container, tombstone_ref, 4, 4);
    drop(container);

    let (store, container) = replay(&path).unwrap();
    assert_eq!(store.records(), &[a1.clone(), b1.clone(), a2.clone()]);
    assert_eq!(store.relationship_version(), 4);
    assert_eq!(container.transaction_time_ns(1), Some(1_001));
    assert_eq!(container.transaction_time_ns(2), Some(1_002));
    assert_eq!(container.transaction_time_ns(3), Some(1_003));
    assert_eq!(container.transaction_time_ns(4), Some(1_004));
    assert_eq!(
        store
            .relationship_revision(a1.id, 1)
            .unwrap()
            .global_version,
        2
    );
    assert_eq!(
        store
            .relationship_revision(a1.id, 2)
            .unwrap()
            .global_version,
        3
    );
    assert_eq!(
        store.relationship_revision(a1.id, 2).unwrap().updated_at_ns,
        15
    );
    assert_eq!(
        store
            .relationship_revision(b1.id, 1)
            .unwrap()
            .relationship_version,
        2
    );
    assert_eq!(
        store.lifecycle_state(a1.id).unwrap(),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(4),
            domain_version: Some(4)
        })
    );
    let retained = store.retention_descriptor(a1.id).unwrap();
    assert_eq!(retained.tombstone_revision, 3);
    assert_eq!(retained.revisions.len(), 2);
}

#[test]
fn unversioned_compact_creation_and_delta_remain_inert() {
    let path = test_path("orphan-inert.rel");
    let mut container = initialize(&path);
    reserve_tickets(&mut container, 1);
    let accepted = record(0x51, 1, 1, 1, "accepted");
    let accepted_ref = append_creation(&mut container, &accepted);
    append_version(&mut container, accepted_ref, 1, 1);

    let orphan = record(0x52, 1, 2, 2, "orphan-create");
    let orphan_ref = append_creation(&mut container, &orphan);
    let mut orphan_next = orphan.clone();
    orphan_next.revision = 2;
    orphan_next.global_version = 3;
    orphan_next.relationship_version = 3;
    orphan_next.mutation_id = "orphan-delta".into();
    orphan_next.summary = "unpublished".into();
    let (_delta_ref, _) = append_delta(&mut container, &orphan, &orphan_next);
    let _ = orphan_ref; // Both compact frames intentionally lack publication markers.
    drop(container);

    let (store, _) = replay(&path).unwrap();
    assert_eq!(store.records(), &[accepted]);
    assert_eq!(store.relationship_version(), 1);
}

#[test]
fn missing_base_coordinate_mismatch_redundant_delta_and_mutation_conflict_fail_atomically() {
    let base = record(0x61, 1, 1, 1, "base");
    let mut target = base.clone();
    target.revision = 2;
    target.global_version = 2;
    target.relationship_version = 2;
    target.mutation_id = "next".into();
    target.summary = "changed".into();

    let missing_path = test_path("missing-base.rel");
    let mut container = initialize(&missing_path);
    reserve_tickets(&mut container, 2);
    let (delta_ref, _) = append_delta(&mut container, &base, &target);
    append_version(&mut container, delta_ref, 2, 2);
    drop(container);
    let (result, errors) = replay_capturing_errors(&missing_path);
    assert!(matches!(
        errors.first(),
        Some(RelationshipError::MissingRelationship)
    ));
    assert_eq!(result.unwrap().records(), &[]);

    let mismatch_path = test_path("coordinate-mismatch.rel");
    let mut container = initialize(&mismatch_path);
    reserve_tickets(&mut container, 3);
    let base_ref = append_creation(&mut container, &base);
    append_version(&mut container, base_ref, 1, 1);
    let (delta_ref, _) = append_delta(&mut container, &base, &target);
    append_version(&mut container, delta_ref, 3, 2); // Carrier says global 2.
    drop(container);
    let (result, errors) = replay_capturing_errors(&mismatch_path);
    assert!(matches!(
        errors.first(),
        Some(RelationshipError::InvalidVersion)
    ));
    assert_eq!(result.unwrap().records(), &[base.clone()]);

    let redundant_path = test_path("redundant-delta.rel");
    let mut container = initialize(&redundant_path);
    reserve_tickets(&mut container, 2);
    let base_ref = append_creation(&mut container, &base);
    append_version(&mut container, base_ref, 1, 1);
    let mut redundant = RelationshipDelta::between(&base, &target).unwrap();
    redundant.fields.summary = crate::compact_delta::FieldChange::Set(base.summary.clone());
    let redundant_ref = container
        .append(&encode_delta(&redundant).unwrap())
        .unwrap();
    append_version(&mut container, redundant_ref, 2, 2);
    drop(container);
    let (result, errors) = replay_capturing_errors(&redundant_path);
    assert_eq!(errors.len(), 1);
    assert_eq!(result.unwrap().records(), &[base.clone()]);

    let mutation_path = test_path("mutation-conflict.rel");
    let mut container = initialize(&mutation_path);
    reserve_tickets(&mut container, 2);
    let base_ref = append_creation(&mut container, &base);
    append_version(&mut container, base_ref, 1, 1);
    let mut delta = RelationshipDelta::between(&base, &target).unwrap();
    delta.mutation_id = base.mutation_id.clone();
    let delta_ref = container.append(&encode_delta(&delta).unwrap()).unwrap();
    append_version(&mut container, delta_ref, 2, 2);
    drop(container);
    let (result, errors) = replay_capturing_errors(&mutation_path);
    assert_eq!(errors.len(), 1);
    assert_eq!(result.unwrap().records(), &[base]);
}

#[test]
fn terminal_tombstone_blocks_compact_resurrection_and_keeps_accepted_history() {
    let path = test_path("terminal-no-resurrection.rel");
    let mut container = initialize(&path);
    reserve_tickets(&mut container, 3);
    let base = record(0x71, 1, 1, 1, "base");
    let base_ref = append_creation(&mut container, &base);
    append_version(&mut container, base_ref, 1, 1);
    let tombstone_ref = container
        .append(&encode_tombstone(RelationshipTombstone {
            id: base.id,
            revision: 2,
        }))
        .unwrap();
    append_version(&mut container, tombstone_ref, 2, 2);
    let mut resurrection = base.clone();
    resurrection.revision = 2;
    resurrection.global_version = 3;
    resurrection.relationship_version = 3;
    resurrection.mutation_id = "resurrection".into();
    resurrection.summary = "must not be admitted".into();
    let delta_ref = container
        .append(&encode_delta(&RelationshipDelta::between(&base, &resurrection).unwrap()).unwrap())
        .unwrap();
    append_version(&mut container, delta_ref, 3, 3);
    drop(container);

    let (result, errors) = replay_capturing_errors(&path);
    assert!(matches!(
        errors.first(),
        Some(RelationshipError::TerminalTombstone)
    ));
    let store = result.unwrap();
    assert_eq!(store.records(), &[base.clone()]);
    assert_eq!(
        store.lifecycle_state(base.id).unwrap(),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(2),
            domain_version: Some(2)
        })
    );
}

fn downgrade_header_to_v1(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

#[test]
fn v1_rejects_published_compact_but_keeps_an_unversioned_compact_frame_inert() {
    let published_path = test_path("published-compact-v1.rel");
    let mut container = initialize(&published_path);
    reserve_tickets(&mut container, 1);
    let first = record(0x81, 1, 1, 1, "v1-compact");
    let carrier = append_creation(&mut container, &first);
    append_version(&mut container, carrier, 1, 1);
    drop(container);
    downgrade_header_to_v1(&published_path);
    assert!(matches!(
        replay(&published_path),
        Err(RelationshipError::ConflictingFormat)
    ));

    let orphan_path = test_path("orphan-compact-v1.rel");
    let mut container = initialize(&orphan_path);
    let orphan = record(0x82, 1, 1, 1, "orphan-v1");
    append_creation(&mut container, &orphan);
    drop(container);
    downgrade_header_to_v1(&orphan_path);
    let (store, container) = replay(&orphan_path).unwrap();
    assert_eq!(container.version(), FormatVersion { major: 1, minor: 0 });
    assert_eq!(store.records(), &[]);
}

#[test]
fn published_compact_prune_proof_is_rejected_until_file_admission_exists() {
    let path = test_path("pruned-compact-pending-admission.rel");
    let mut container = initialize(&path);
    reserve_tickets(&mut container, 1);
    let mut redacted = record(0x91, 1, 1, 1, "pruned");
    redacted.summary.clear();
    let proof = RelationshipDescriptionProof {
        original_carrier_digest: [0xA5; 32],
        manifest_digest: [0x5A; 32],
    };
    let creation = RelationshipCreation::new_pruned(redacted.clone(), proof).unwrap();
    let carrier = container
        .append(&encode_creation(&creation).unwrap())
        .unwrap();
    append_version(&mut container, carrier, 1, 1);
    drop(container);
    assert!(matches!(
        replay(&path),
        Err(RelationshipError::CorruptRecord(
            "compact Relationship proof admission unavailable"
        ))
    ));
}
