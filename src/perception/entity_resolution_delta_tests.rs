use super::*;
use crate::entity_resolution_store::EntityResolutionStore;
use std::{fs, path::PathBuf};

fn key() -> MemoryEntityMentionKey {
    MemoryEntityMentionKey {
        memory_id: MemoryId([7; 32]),
        field: MemoryTextField::Content,
        start_byte: 0,
        end_byte: 2,
    }
}
fn pending() -> MemoryEntityResolutionStatus {
    MemoryEntityResolutionStatus::Pending(EntityResolutionPending {
        candidate_entity_ids: vec![EntityId([1; 32]), EntityId([2; 32])],
        reason: EntityResolutionReason::Ambiguous,
        candidate_set_fingerprint: [3; 32],
        context_fingerprint: [4; 32],
        first_seen_at_ns: 10,
        last_attempt_at_ns: 10,
        attempt_count: 1,
    })
}
fn change(revision: u32, status: Option<MemoryEntityResolutionStatus>) -> DecodedEntityResolution {
    DecodedEntityResolution {
        key: key(),
        revision,
        updated_at_ns: 10 + revision as i64,
        status,
    }
}
fn scratch() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("resolution-delta-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn entity_resolution_delta_initial_and_attempt_vectors_are_exact() {
    let initial = change(1, Some(pending()));
    let bytes = delta::encode(&initial, None).unwrap();
    assert_eq!(&bytes[..8], b"CVAERS02");
    assert_eq!(bytes.len(), 211);
    assert_eq!(bytes[61], 3);
    assert_eq!(bytes[62], 127);
    assert_eq!(
        &bytes[63..128],
        &[vec![2], vec![1; 32], vec![2; 32]].concat()
    );
    assert_eq!(
        delta::decode(&bytes, |_| None).unwrap(),
        Some(initial.clone())
    );
    let mut next = pending();
    if let MemoryEntityResolutionStatus::Pending(v) = &mut next {
        v.last_attempt_at_ns = 20;
        v.attempt_count = 2;
    }
    let second = change(2, Some(next));
    let bytes = delta::encode(&second, initial.status.as_ref()).unwrap();
    assert_eq!(bytes.len(), 73);
    assert_eq!(bytes[62], 96);
    assert_eq!(&bytes[63..71], &20i64.to_le_bytes());
    assert_eq!(&bytes[71..], &2u16.to_le_bytes());
    assert_eq!(
        delta::decode(&bytes, |_| initial.status.as_ref()).unwrap(),
        Some(second)
    );
    assert!(delta::decode(&bytes, |_| None).is_err());
}

#[test]
fn entity_resolution_delta_malformed_and_conflicts_leave_projection_unchanged() {
    let initial = change(1, Some(pending()));
    let bytes = delta::encode(&initial, None).unwrap();
    for end in 6..bytes.len() {
        assert!(delta::decode(&bytes[..end], |_| None).is_err());
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(delta::decode(&extra, |_| None).is_err());
    for (offset, value) in [(40, 255), (61, 9), (63, 9), (128, 0)] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(delta::decode(&bad, |_| None).is_err());
    }
    assert!(delta::decode(b"CVAERS03", |_| None).is_err());
    let mut store = EntityResolutionStore::default();
    store.ingest(&bytes).unwrap();
    let before = store.get(key()).cloned();
    assert!(store.ingest(&bytes).is_err());
    assert_eq!(store.get(key()).cloned(), before);
    let mut invalid = pending();
    if let MemoryEntityResolutionStatus::Pending(v) = &mut invalid {
        v.attempt_count = 0;
    }
    let bad = delta::encode(&change(2, Some(invalid)), initial.status.as_ref()).unwrap();
    assert!(store.ingest(&bad).is_err());
    assert_eq!(store.get(key()).cloned(), before);
}

#[test]
fn entity_resolution_delta_dormant_wake_retarget_terminal_reset_preserve_full_postimages() {
    let root = scratch();
    let mut container = crate::Container::create(root.join("raw.rel")).unwrap();
    let mut writer = EntityResolutionStore::default();
    let mut legacy = EntityResolutionStore::default();
    let pending = pending();
    let mut dormant = match pending.clone() {
        MemoryEntityResolutionStatus::Pending(v) => {
            MemoryEntityResolutionStatus::Dormant(EntityResolutionDormant {
                candidate_entity_ids: v.candidate_entity_ids,
                reason: v.reason,
                candidate_set_fingerprint: v.candidate_set_fingerprint,
                context_fingerprint: v.context_fingerprint,
                first_seen_at_ns: v.first_seen_at_ns,
                last_attempt_at_ns: v.last_attempt_at_ns,
                attempt_count: v.attempt_count,
                dormant_at_ns: 30,
            })
        }
        _ => unreachable!(),
    };
    if let MemoryEntityResolutionStatus::Dormant(v) = &mut dormant {
        v.candidate_entity_ids = vec![EntityId([2; 32])];
        v.candidate_set_fingerprint = [0; 32];
    }
    let changes = [
        change(1, Some(pending.clone())),
        change(2, Some(dormant)),
        change(3, Some(pending)),
        change(
            4,
            Some(MemoryEntityResolutionStatus::Resolved {
                entity_id: EntityId([2; 32]),
                reason: EntityResolutionReason::ContextMatch,
            }),
        ),
        change(5, None),
        change(
            1,
            Some(MemoryEntityResolutionStatus::Rejected {
                reason: EntityResolutionReason::GenericRole,
            }),
        ),
    ];
    for change in changes {
        legacy
            .ingest(&comparison_carrier(&change).unwrap())
            .unwrap();
        writer.persist_change(&mut container, change).unwrap();
        assert_eq!(writer.records(), legacy.records());
    }
    let expected = writer.records();
    let materialized = EntityResolutionStore::materialized_changes(&mut container).unwrap();
    assert_eq!(materialized.len(), 6);
    let mut reopened = EntityResolutionStore::default();
    container
        .visit_payloads::<MemoryError>(|_, p| {
            assert!(delta::is_current(p));
            reopened.ingest(p)
        })
        .unwrap();
    assert_eq!(reopened.records(), expected);
    let before = fs::read(container.path()).unwrap();
    assert!(
        writer
            .persist_change(&mut container, change(9, None))
            .is_err()
    );
    assert_eq!(fs::read(container.path()).unwrap(), before);
    drop(container);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn entity_resolution_delta_all_evidence_fields_change_literally() {
    let initial = pending();
    let mut next = initial.clone();
    if let MemoryEntityResolutionStatus::Pending(v) = &mut next {
        v.candidate_entity_ids = vec![EntityId([9; 32])];
        v.reason = EntityResolutionReason::RecurrenceRequired;
        v.candidate_set_fingerprint = [8; 32];
        v.context_fingerprint = [9; 32];
        v.first_seen_at_ns = 1;
        v.last_attempt_at_ns = 20;
        v.attempt_count = 9;
    }
    let decoded = change(2, Some(next));
    let bytes = delta::encode(&decoded, Some(&initial)).unwrap();
    assert_eq!(bytes[62], 127);
    assert_eq!(
        delta::decode(&bytes, |_| Some(&initial)).unwrap(),
        Some(decoded)
    );
}

#[test]
fn entity_resolution_delta_rel_history_upgrade_and_reclaim_match_every_postimage() {
    use crate::storage_upgrade::{
        StorageUpgradeRequest, plan_storage_upgrade, stage_graph_envelope_storage_upgrade,
    };
    let root = scratch();
    let (mut rel, _, keys, source) =
        crate::entity_resolution_test_support::rel_with_three_mentions();
    let key = keys[0];
    let mut expected = Vec::new();
    for (revision, fp, time) in [(0, 1, 10), (1, 2, 20), (2, 3, 30)] {
        rel.put_entity_unresolved(
            key,
            revision,
            vec![EntityId([1; 32]), EntityId([2; 32])],
            EntityResolutionReason::Ambiguous,
            [fp; 32],
            [9; 32],
            time,
        )
        .unwrap();
        expected.push(rel.entity_resolution(key).unwrap().clone());
    }
    rel.put_entity_resolved(
        key,
        3,
        EntityId([2; 32]),
        EntityResolutionReason::ContextMatch,
        40,
    )
    .unwrap();
    expected.push(rel.entity_resolution(key).unwrap().clone());
    rel.sync().unwrap();
    let before = fs::read(&source).unwrap();
    let changes = EntityResolutionStore::materialized_changes(&mut rel.container).unwrap();
    assert_eq!(
        changes
            .values()
            .map(|v| MemoryEntityResolution {
                key: v.key,
                revision: v.revision,
                updated_at_ns: v.updated_at_ns,
                status: v.status.clone().unwrap()
            })
            .collect::<Vec<_>>(),
        expected
    );
    let legacy = root.join("legacy.rel");
    crate::insomnia::completion_migration_tests::copy_with_legacy_archive_publications(
        &mut rel, &legacy,
    );
    let mut bytes = fs::read(&legacy).unwrap();
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    fs::write(&legacy, &bytes).unwrap();
    let plan = plan_storage_upgrade(StorageUpgradeRequest {
        source: legacy.clone(),
        destination: root.join("stage.rel"),
    })
    .unwrap();
    let stage = stage_graph_envelope_storage_upgrade(&plan).unwrap();
    let mut upgraded = crate::Cva::open(stage.test_path()).unwrap();
    let converted = EntityResolutionStore::materialized_changes(&mut upgraded.container).unwrap();
    assert_eq!(
        changes.values().collect::<Vec<_>>(),
        converted.values().collect::<Vec<_>>()
    );
    assert_eq!(upgraded.entity_resolution(key), expected.last());
    assert_eq!(fs::read(&legacy).unwrap(), bytes);
    assert_eq!(fs::read(&source).unwrap(), before);
    let reclaimed = root.join("reclaimed.rel");
    rel.reclaim_storage(&reclaimed).unwrap();
    let reopened = crate::Cva::open(&reclaimed).unwrap();
    assert_eq!(reopened.entity_resolution(key), expected.last());
    drop(reopened);
    drop(upgraded);
    drop(stage);
    drop(rel);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn entity_resolution_delta_raw_repeated_attempts_measure_all_fields_and_frames() {
    let mut previous = None;
    let mut full_bytes = 0;
    let mut compact_bytes = 0;
    for revision in 1..=64 {
        let mut status = pending();
        if let MemoryEntityResolutionStatus::Pending(v) = &mut status {
            v.last_attempt_at_ns = 10 + revision as i64;
            v.attempt_count = revision as u16;
            v.context_fingerprint = [revision as u8; 32];
        }
        let change = change(revision, Some(status.clone()));
        full_bytes += comparison_carrier(&change).unwrap().len() + 8;
        compact_bytes += delta::encode(&change, previous.as_ref()).unwrap().len() + 8;
        previous = Some(status);
    }
    assert_eq!((full_bytes, compact_bytes), (13952, 7338));
}

#[test]
fn entity_resolution_delta_initial_dormant_and_missing_inheritance_reject() {
    let status = match pending() {
        MemoryEntityResolutionStatus::Pending(v) => {
            MemoryEntityResolutionStatus::Dormant(EntityResolutionDormant {
                candidate_entity_ids: v.candidate_entity_ids,
                reason: v.reason,
                candidate_set_fingerprint: v.candidate_set_fingerprint,
                context_fingerprint: v.context_fingerprint,
                first_seen_at_ns: v.first_seen_at_ns,
                last_attempt_at_ns: v.last_attempt_at_ns,
                attempt_count: v.attempt_count,
                dormant_at_ns: 30,
            })
        }
        _ => unreachable!(),
    };
    let initial = change(1, Some(status.clone()));
    let bytes = delta::encode(&initial, None).unwrap();
    assert_eq!(bytes[62], 255);
    assert_eq!(delta::decode(&bytes, |_| None).unwrap(), Some(initial));
    let next = change(2, Some(status.clone()));
    let retry = delta::encode(&next, Some(&status)).unwrap();
    assert_eq!(retry[62], 0);
    assert!(delta::decode(&retry, |_| None).is_err());
    let terminal = MemoryEntityResolutionStatus::Rejected {
        reason: EntityResolutionReason::GenericRole,
    };
    assert!(delta::decode(&retry, |_| Some(&terminal)).is_err());
    let mut store = EntityResolutionStore::default();
    store.ingest(&bytes).unwrap();
    store
        .ingest(&delta::encode(&change(2, None), Some(&status)).unwrap())
        .unwrap();
    assert!(store.ingest(&retry).is_err());
    assert!(store.records().is_empty());
}

#[test]
fn entity_resolution_delta_explicit_v1_owner_rejection_is_nonmutating() {
    let root = scratch();
    let payload = delta::encode(
        &change(
            1,
            Some(MemoryEntityResolutionStatus::Rejected {
                reason: EntityResolutionReason::GenericRole,
            }),
        ),
        None,
    )
    .unwrap();
    for phy in [false, true] {
        let path = root.join(if phy { "input.phy" } else { "input.rel" });
        if phy {
            let mut owner = crate::Phylactery::create(&path).unwrap();
            owner.container.append(&payload).unwrap();
            owner.sync().unwrap();
        } else {
            let mut owner = crate::Cva::create(&path).unwrap();
            owner.container.append(&payload).unwrap();
            owner.sync().unwrap();
        }
        let mut bytes = fs::read(&path).unwrap();
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        fs::write(&path, &bytes).unwrap();
        if phy {
            assert!(matches!(
                crate::Phylactery::open_v1_read_only(&path),
                Err(crate::PhylacteryError::Memories(
                    MemoryError::CorruptRecord("compact Entity resolution in V1 input")
                ))
            ));
        } else {
            assert!(matches!(
                crate::Cva::open_v1_read_only(&path),
                Err(crate::CvaError::Memories(MemoryError::CorruptRecord(
                    "compact Entity resolution in V1 input"
                )))
            ));
        }
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::remove_dir_all(root).unwrap();
}
