//! Compact Entity carrier replay and owner-state admission tests.
use crate::entity_codec::{
    DirectEntityTombstone, EntityTombstone, EntityVersion, encode_direct_tombstone,
    encode_tombstone, encode_version,
};
use crate::entity_compact_codec::{decode_delta, encode_creation, encode_delta};
use crate::entity_compact_contract::{EntityCreation, EntityDelta, EntityDescriptionProof};
use crate::entity_model::EntityRecord;
use crate::entity_rebuild::EntityOpenState;
use crate::entity_store::EntityStore;
use crate::{Container, EntityError, EntityGlobalId, EntityId, FormatVersion};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn test_path(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("entity-compact-rebuild-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    directory.join(name)
}

fn record(
    id: u8,
    name: &str,
    mutation_id: &str,
    revision: u64,
    global: u64,
    local: u64,
) -> EntityRecord {
    EntityRecord {
        id: EntityId([id; 32]),
        global_id: Some(EntityGlobalId([id.wrapping_add(80); 16])),
        revision,
        canonical_name: name.into(),
        aliases: vec![format!("{name} alias")],
        kind: "project".into(),
        summary: format!("Summary for {name}"),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: 20,
        global_version: global,
        entity_version: local,
    }
}

fn initialize(path: &Path) -> Container {
    let mut container = Container::create(path).unwrap();
    EntityStore::empty().initialize(&mut container).unwrap();
    container
}

fn reserve_tickets(container: &mut Container, count: u64) {
    for expected in 1..=count {
        assert_eq!(container.allocate_version().unwrap(), expected);
    }
}

fn append_version(
    container: &mut Container,
    carrier: crate::ObjectRef,
    global_version: u64,
    entity_version: u64,
) {
    container
        .append(&encode_version(EntityVersion {
            global_version,
            entity_version,
            record: carrier,
        }))
        .unwrap();
}

fn append_creation(container: &mut Container, record: &EntityRecord) -> crate::ObjectRef {
    container
        .append(&encode_creation(&EntityCreation::new(record.clone()).unwrap()).unwrap())
        .unwrap()
}

fn append_delta(
    container: &mut Container,
    previous: &EntityRecord,
    next: &EntityRecord,
) -> crate::ObjectRef {
    container
        .append(&encode_delta(&EntityDelta::between(previous, next).unwrap()).unwrap())
        .unwrap()
}

fn replay(path: &Path) -> Result<EntityStore, EntityError> {
    let mut state = EntityOpenState::new();
    let container = Container::open_scanned(path, |chunk, payload, latest| {
        state.ingest(chunk, payload, latest)
    })?;
    state.finish(&container)
}

fn replay_capturing_errors(path: &Path) -> (Result<EntityStore, EntityError>, Vec<EntityError>) {
    let mut state = EntityOpenState::new();
    let mut errors = Vec::new();
    let container = Container::open_scanned(path, |chunk, payload, latest| {
        if let Err(error) = state.ingest(chunk, payload, latest) {
            errors.push(error);
        }
        Ok::<(), EntityError>(())
    })
    .unwrap();
    (state.finish(&container), errors)
}

#[test]
fn compact_creation_and_deltas_replay_interleaved_entities_and_late_global_tickets() {
    let path = test_path("interleaved-history.rel");
    let mut container = initialize(&path);
    reserve_tickets(&mut container, 4);

    // Owner-local versions are dense while already-reserved global tickets are
    // late-bound in physical marker order 2, 1, 3, 4.
    let a1 = record(11, "Entity A", "a-create", 1, 2, 1);
    let a1_ref = append_creation(&mut container, &a1);
    append_version(&mut container, a1_ref, 2, 1);

    let b1 = record(22, "Entity B", "b-create", 1, 1, 2);
    let b1_ref = append_creation(&mut container, &b1);
    append_version(&mut container, b1_ref, 1, 2);

    let mut a2 = a1.clone();
    a2.revision = 2;
    a2.global_version = 3;
    a2.entity_version = 3;
    a2.mutation_id = "a-update".into();
    a2.canonical_name = "Entity A revised".into();
    let a2_ref = append_delta(&mut container, &a1, &a2);
    append_version(&mut container, a2_ref, 3, 3);

    let tombstone_ref = container
        .append(&encode_tombstone(EntityTombstone {
            id: a1.id,
            revision: 3,
            replacement_id: b1.id,
        }))
        .unwrap();
    append_version(&mut container, tombstone_ref, 4, 4);
    drop(container);

    let store = replay(&path).unwrap();
    assert_eq!(store.records(), &[a1.clone(), b1.clone(), a2.clone()]);
    assert_eq!(store.entity_version(), 4);
    assert_eq!(
        store.entity_revision(a1.id, 1).unwrap().canonical_name,
        a1.canonical_name
    );
    assert_eq!(
        store.entity_revision(a1.id, 2).unwrap().canonical_name,
        a2.canonical_name
    );
    assert_eq!(
        store.entity_revision(b1.id, 1).unwrap().mutation_id,
        b1.mutation_id
    );
    assert_eq!(
        store.lifecycle_state(a1.id).unwrap(),
        crate::owner_lifecycle::LifecycleState::Tombstoned(
            crate::owner_lifecycle::TombstoneCoordinates {
                global_version: Some(4),
                domain_version: Some(4),
            }
        )
    );
    let retained = store.retention_descriptor(a1.id).unwrap();
    assert_eq!(retained.tombstone_revision, 3);
    assert_eq!(retained.replacement_id, Some(b1.id));
    assert_eq!(retained.revisions.len(), 2);
    assert_eq!(retained.revisions[0].global_version, 2);
    assert_eq!(retained.revisions[1].global_version, 3);
}

#[test]
fn valid_unversioned_compact_creation_and_delta_remain_inert() {
    let path = test_path("unversioned-inert.rel");
    let mut container = initialize(&path);
    let first = record(31, "Seed", "seed", 1, 1, 1);
    let first_ref = append_creation(&mut container, &first);
    let first_ticket = container.allocate_version().unwrap();
    append_version(&mut container, first_ref, first_ticket, 1);

    let mut unversioned_creation = record(32, "Orphan", "orphan-create", 1, 2, 2);
    let creation_bytes =
        encode_creation(&EntityCreation::new(unversioned_creation.clone()).unwrap()).unwrap();
    container.append(&creation_bytes).unwrap();
    unversioned_creation.revision = 2;
    unversioned_creation.global_version = 3;
    unversioned_creation.entity_version = 3;
    unversioned_creation.mutation_id = "orphan-delta".into();
    let unversioned_delta = EntityDelta::between(
        &record(32, "Orphan", "orphan-create", 1, 2, 2),
        &unversioned_creation,
    )
    .unwrap();
    container
        .append(&encode_delta(&unversioned_delta).unwrap())
        .unwrap();
    drop(container);

    let store = replay(&path).unwrap();
    assert_eq!(store.records(), &[first]);
    assert_eq!(store.entity_version(), 1);
}

#[test]
fn marker_coordinate_mismatch_and_redundant_delta_leave_prior_history_unchanged() {
    let mismatch_path = test_path("mismatch.rel");
    let mut container = initialize(&mismatch_path);
    reserve_tickets(&mut container, 3);
    let first = record(41, "Base", "base", 1, 1, 1);
    let first_ref = append_creation(&mut container, &first);
    append_version(&mut container, first_ref, 1, 1);
    let mut next = first.clone();
    next.revision = 2;
    next.global_version = 2;
    next.entity_version = 2;
    next.mutation_id = "next".into();
    next.summary = "changed".into();
    let mismatch_ref = append_delta(&mut container, &first, &next);
    append_version(&mut container, mismatch_ref, 3, 2);
    drop(container);

    let (result, errors) = replay_capturing_errors(&mismatch_path);
    assert_eq!(errors.len(), 1);
    assert_eq!(result.unwrap().records(), &[first.clone()]);

    let redundant_path = test_path("redundant.rel");
    let mut container = initialize(&redundant_path);
    reserve_tickets(&mut container, 2);
    let first_ref = append_creation(&mut container, &first);
    append_version(&mut container, first_ref, 1, 1);
    let mut redundant = EntityDelta::between(&first, &next).unwrap();
    redundant.fields.summary = crate::compact_delta::FieldChange::Set(first.summary.clone());
    let redundant_ref = container
        .append(&encode_delta(&redundant).unwrap())
        .unwrap();
    append_version(&mut container, redundant_ref, 2, 2);
    drop(container);

    let (result, errors) = replay_capturing_errors(&redundant_path);
    assert_eq!(errors.len(), 1);
    assert_eq!(result.unwrap().records(), &[first]);
}

#[test]
fn missing_delta_base_mutation_conflict_and_tombstone_resurrection_are_rejected() {
    let missing_path = test_path("missing-base.rel");
    let mut container = initialize(&missing_path);
    reserve_tickets(&mut container, 2);
    let missing_prior = record(51, "Missing", "m0", 1, 1, 1);
    let mut missing_next = missing_prior.clone();
    missing_next.revision = 2;
    missing_next.entity_version = 2;
    missing_next.global_version = 2;
    missing_next.mutation_id = "m1".into();
    missing_next.summary = "changed".into();
    let missing_ref = append_delta(&mut container, &missing_prior, &missing_next);
    append_version(&mut container, missing_ref, 2, 2);
    drop(container);
    let (result, errors) = replay_capturing_errors(&missing_path);
    assert!(matches!(errors.as_slice(), [EntityError::MissingEntity]));
    assert!(result.unwrap().records().is_empty());

    let mutation_path = test_path("mutation-conflict.rel");
    let mut container = initialize(&mutation_path);
    reserve_tickets(&mut container, 2);
    let first = record(52, "Mutation", "same-mutation", 1, 1, 1);
    let first_ref = append_creation(&mut container, &first);
    append_version(&mut container, first_ref, 1, 1);
    let mut next = first.clone();
    next.revision = 2;
    next.entity_version = 2;
    next.global_version = 2;
    next.summary = "different content".into();
    next.mutation_id = "fresh-before-corruption".into();
    let mut delta = EntityDelta::between(&first, &next).unwrap();
    delta.mutation_id = first.mutation_id.clone();
    let delta_ref = container.append(&encode_delta(&delta).unwrap()).unwrap();
    append_version(&mut container, delta_ref, 2, 2);
    drop(container);
    let (result, errors) = replay_capturing_errors(&mutation_path);
    assert_eq!(errors.len(), 1);
    assert_eq!(result.unwrap().records(), &[first]);

    let resurrection_path = test_path("tombstone-resurrection.rel");
    let mut container = initialize(&resurrection_path);
    reserve_tickets(&mut container, 4);
    let entity = record(53, "Retired", "retired-create", 1, 1, 1);
    let replacement = record(54, "Replacement", "replacement-create", 1, 2, 2);
    let entity_ref = append_creation(&mut container, &entity);
    append_version(&mut container, entity_ref, 1, 1);
    let replacement_ref = append_creation(&mut container, &replacement);
    append_version(&mut container, replacement_ref, 2, 2);
    let tombstone_ref = container
        .append(&encode_direct_tombstone(DirectEntityTombstone {
            id: entity.id,
            revision: 2,
        }))
        .unwrap();
    append_version(&mut container, tombstone_ref, 3, 3);
    let mut after_tombstone = entity.clone();
    after_tombstone.revision = 2;
    after_tombstone.global_version = 4;
    after_tombstone.entity_version = 4;
    after_tombstone.mutation_id = "resurrection".into();
    after_tombstone.summary = "must not return".into();
    let resurrection_ref = append_delta(&mut container, &entity, &after_tombstone);
    append_version(&mut container, resurrection_ref, 4, 4);
    drop(container);
    let (result, errors) = replay_capturing_errors(&resurrection_path);
    assert_eq!(errors.len(), 1);
    let store = result.unwrap();
    assert_eq!(store.entity_version(), 3);
    assert_eq!(store.records().len(), 2);
    assert!(matches!(
        store.lifecycle_state(entity.id),
        Ok(crate::owner_lifecycle::LifecycleState::Tombstoned(_))
    ));
}

#[test]
fn unknown_compact_versions_pruned_proofs_and_v1_compact_frames_fail_closed() {
    let path = test_path("unknown-version.rel");
    let mut container = initialize(&path);
    let first = record(61, "Valid", "valid", 1, 1, 1);
    let first_ref = append_creation(&mut container, &first);
    let ticket = container.allocate_version().unwrap();
    append_version(&mut container, first_ref, ticket, 1);
    let mut unknown = b"CVAENC99".to_vec();
    unknown.extend_from_slice(&[0; 32]);
    let checksum = Sha256::digest(&unknown);
    unknown.extend_from_slice(&checksum);
    container.append(&unknown).unwrap();
    drop(container);
    let (result, errors) = replay_capturing_errors(&path);
    assert_eq!(errors.len(), 1);
    assert_eq!(result.unwrap().records(), &[first]);

    let pruned_path = test_path("pruned-proof.rel");
    let mut container = initialize(&pruned_path);
    let mut pruned_record = record(62, "[pruned]", "pruned", 1, 1, 1);
    pruned_record.aliases.clear();
    pruned_record.summary.clear();
    let proof = EntityDescriptionProof {
        original_carrier_digest: [0xA1; 32],
        manifest_digest: [0xB2; 32],
    };
    let creation = EntityCreation::new_pruned(pruned_record, proof).unwrap();
    container
        .append(&encode_creation(&creation).unwrap())
        .unwrap();
    drop(container);
    let (result, errors) = replay_capturing_errors(&pruned_path);
    assert!(matches!(
        errors.as_slice(),
        [EntityError::CorruptRecord(
            "compact Entity description proof admission pending"
        )]
    ));
    assert!(result.unwrap().records().is_empty());

    for published in [false, true] {
        let v1_path = test_path("v1-compact.rel");
        let mut container = initialize(&v1_path);
        let compact = record(63, "Compact", "compact", 1, 1, 1);
        let carrier = container
            .append(&encode_creation(&EntityCreation::new(compact).unwrap()).unwrap())
            .unwrap();
        if published {
            let ticket = container.allocate_version().unwrap();
            append_version(&mut container, carrier, ticket, 1);
        }
        drop(container);
        let mut bytes = fs::read(&v1_path).unwrap();
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
        fs::write(&v1_path, &bytes).unwrap();
        let before = fs::read(&v1_path).unwrap();
        let mut state = EntityOpenState::new();
        let container = Container::open_scanned_version(
            &v1_path,
            Some(FormatVersion { major: 1, minor: 0 }),
            |chunk, payload, latest| state.ingest(chunk, payload, latest),
        )
        .unwrap();
        if published {
            assert!(matches!(
                state.finish(&container),
                Err(EntityError::ConflictingFormat)
            ));
        } else {
            assert!(state.finish(&container).unwrap().records().is_empty());
        }
        assert_eq!(fs::read(v1_path).unwrap(), before);
    }
}

#[test]
fn malformed_compact_delta_mask_is_rejected_before_store_mutation() {
    let path = test_path("unknown-mask.rel");
    let mut container = initialize(&path);
    let first = record(71, "Mask", "mask-base", 1, 1, 1);
    let first_ref = append_creation(&mut container, &first);
    let first_ticket = container.allocate_version().unwrap();
    append_version(&mut container, first_ref, first_ticket, 1);
    let mut next = first.clone();
    next.revision = 2;
    next.global_version = 2;
    next.entity_version = 2;
    next.mutation_id = "mask-next".into();
    next.summary = "changed".into();
    let delta = EntityDelta::between(&first, &next).unwrap();
    let mut payload = encode_delta(&delta).unwrap();
    let mutation_start = 72;
    let mutation_len = u32::from_le_bytes(
        payload[mutation_start..mutation_start + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let mask = mutation_start + 4 + mutation_len;
    payload[mask..mask + 8].copy_from_slice(&(1_u64 << 6).to_le_bytes());
    let checksum_at = payload.len() - 32;
    let checksum = Sha256::digest(&payload[..checksum_at]);
    payload[checksum_at..].copy_from_slice(&checksum);
    let bad_ref = container.append(&payload).unwrap();
    let delta_ticket = container.allocate_version().unwrap();
    append_version(&mut container, bad_ref, delta_ticket, 2);
    drop(container);

    let (result, errors) = replay_capturing_errors(&path);
    assert!(matches!(
        errors.first(),
        Some(EntityError::CorruptRecord("unknown Entity delta field"))
    ));
    assert_eq!(result.unwrap().records(), &[first]);
    // Keep the codec import exercised independently of EntityOpenState routing.
    assert!(decode_delta(&payload).is_err());
}
