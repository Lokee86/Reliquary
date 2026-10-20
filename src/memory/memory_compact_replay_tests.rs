#[path = "memory_publication_replay_tests.rs"]
mod publication;

use crate::memory_codec::{
    MemoryTombstone, MemoryVersion, encode_body, encode_record, encode_tombstone, encode_version,
};
use crate::memory_compact_codec::{decode_delta, encode_creation, encode_delta};
use crate::memory_compact_contract::{MemoryCreation, MemoryDelta, MemoryFieldChange};
use crate::memory_model::{MemoryRecord, memory_body_bytes, memory_body_id};
use crate::memory_rebuild::MemoryOpenState;
use crate::memory_store::MemoryStore;
use crate::owner_lifecycle::{LifecycleState, TombstoneCoordinates};
use crate::{
    Container, ContainerError, FileKind, FormatVersion, MemoryBodyId, MemoryError, MemoryId,
    MemoryTemporalInference, TemporalIndicationKind, TemporalInference,
    TemporalInferenceResolution,
};
use std::fs;
use std::path::{Path, PathBuf};

fn test_path(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("memory-compact-replay-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    directory.join(name)
}

fn initial_record() -> MemoryRecord {
    MemoryRecord {
        id: MemoryId([41; 32]),
        revision: 1,
        body_id: MemoryBodyId([42; 32]),
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
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
        source_ref: None,
        temporal_inference: None,
        mutation_id: "creation".into(),
        created_at_ns: 100,
        updated_at_ns: 100,
        global_version: 1,
        memory_version: 1,
    }
}

fn successor(previous: &MemoryRecord) -> MemoryRecord {
    let mut next = previous.clone();
    next.revision = 2;
    next.global_version = 2;
    next.memory_version = 2;
    next.mutation_id = "delta-one".into();
    next.scope = "personal".into();
    next.updated_at_ns = 110;
    next
}

fn initialize_container(path: &Path) -> Container {
    let mut container = Container::create(path).unwrap();
    MemoryStore::empty().initialize(&mut container).unwrap();
    container
}

fn append_body(container: &mut Container, title: &str, content: &str) -> MemoryBodyId {
    let body = memory_body_bytes(title, content);
    let id = memory_body_id(title, content);
    container.append(&encode_body(id, &body).unwrap()).unwrap();
    id
}

fn append_ticket(container: &mut Container) -> u64 {
    let transaction_time_ns = 1_000 + container.latest_version() as i64;
    container
        .allocate_version_with_transaction_time(Some(transaction_time_ns))
        .unwrap()
}

fn append_version_marker(
    container: &mut Container,
    record: crate::ObjectRef,
    global_version: u64,
    memory_version: u64,
) {
    container
        .append(&encode_version(MemoryVersion {
            global_version,
            memory_version,
            record,
        }))
        .unwrap();
}

fn append_creation(container: &mut Container, record: &MemoryRecord) -> u64 {
    let chunk = container
        .append(&encode_creation(&MemoryCreation::new(record.clone()).unwrap()).unwrap())
        .unwrap();
    let global_version = append_ticket(container);
    append_version_marker(container, chunk, global_version, record.memory_version);
    global_version
}

fn replay(path: &Path) -> Result<MemoryStore, MemoryError> {
    let mut state = MemoryOpenState::new();
    let mut container = Container::open_scanned(path, |chunk, payload, latest| {
        state.ingest(chunk, payload, latest)
    })?;
    state.finish(&mut container)
}

fn replay_while_capturing_ingest_errors(path: &Path) -> (MemoryStore, Vec<MemoryError>) {
    let mut state = MemoryOpenState::new();
    let mut errors = Vec::new();
    let mut container = Container::open_scanned(path, |chunk, payload, latest| {
        if let Err(error) = state.ingest(chunk, payload, latest) {
            errors.push(error);
        }
        Ok::<(), MemoryError>(())
    })
    .unwrap();
    let store = state.finish(&mut container).unwrap();
    (store, errors)
}

fn append_compact_delta(
    container: &mut Container,
    previous: &MemoryRecord,
    next: &MemoryRecord,
) -> (crate::ObjectRef, MemoryDelta) {
    let delta = MemoryDelta::between(previous, next).unwrap();
    let chunk = container.append(&encode_delta(&delta).unwrap()).unwrap();
    (chunk, delta)
}

#[test]
fn compact_creation_delta_and_tombstone_reopen_with_exact_history_and_clocks() {
    let path = test_path("compact-history.rel");
    let mut container = initialize_container(&path);
    let mut first = initial_record();
    let body_id = append_body(&mut container, "Decision", "Keep the exact Memory history.");
    first.body_id = body_id;
    assert_eq!(append_creation(&mut container, &first), 1);

    let next = successor(&first);
    let (delta_chunk, delta) = append_compact_delta(&mut container, &first, &next);
    assert_eq!(delta.global_version, 2);
    assert_eq!(append_ticket(&mut container), 2);
    append_version_marker(&mut container, delta_chunk, 2, 2);

    let tombstone_chunk = container
        .append(&encode_tombstone(MemoryTombstone {
            id: first.id,
            revision: 3,
        }))
        .unwrap();
    assert_eq!(append_ticket(&mut container), 3);
    append_version_marker(&mut container, tombstone_chunk, 3, 3);
    drop(container);

    let store = replay(&path).unwrap();
    assert_eq!(store.records(), &[first.clone(), next.clone()]);
    assert_eq!(store.memory_version(), 3);
    assert!(!store.contains_memory(first.id));
    assert!(store.contains_historical_memory(first.id));
    assert_eq!(
        store.lifecycle_state(first.id).unwrap(),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(3),
            domain_version: Some(3),
        })
    );
    let retained = store.retention_descriptor(first.id).unwrap();
    assert_eq!(retained.tombstone_revision, 3);
    assert_eq!(retained.tombstone_global_version, 3);
    assert_eq!(retained.tombstone_memory_version, 3);
    assert_eq!(retained.revisions.len(), 2);
    assert_eq!(retained.revisions[0].mutation_id, "creation");
    assert_eq!(retained.revisions[1].mutation_id, "delta-one");
}

#[test]
fn unversioned_compact_delta_stays_inert_after_reopen() {
    let path = test_path("unversioned-delta.rel");
    let mut container = initialize_container(&path);
    let mut first = initial_record();
    first.body_id = append_body(&mut container, "Decision", "A versioned creation.");
    append_creation(&mut container, &first);

    let next = successor(&first);
    let (_orphan_chunk, delta) = append_compact_delta(&mut container, &first, &next);
    assert_eq!(delta.memory_version, 2);
    let mut orphan_creation = first.clone();
    orphan_creation.id = MemoryId([43; 32]);
    orphan_creation.mutation_id = "unversioned-creation".into();
    orphan_creation.global_version = 2;
    orphan_creation.memory_version = 2;
    container
        .append(&encode_creation(&MemoryCreation::new(orphan_creation).unwrap()).unwrap())
        .unwrap();
    drop(container);

    let store = replay(&path).unwrap();
    assert_eq!(store.records(), &[first]);
    assert_eq!(store.memory_version(), 1);
}

#[test]
fn linked_compact_delta_with_mismatched_global_coordinate_is_rejected_atomically() {
    let path = test_path("delta-coordinate-mismatch.rel");
    let mut container = initialize_container(&path);
    let mut first = initial_record();
    first.body_id = append_body(&mut container, "Decision", "A valid creation.");
    append_creation(&mut container, &first);

    // Reserve global version 2, then make the delta claim it while its CVAMEMV1
    // marker names version 3. Both coordinates are within the container head.
    assert_eq!(append_ticket(&mut container), 2);
    let mut next = successor(&first);
    next.global_version = 2;
    let (delta_chunk, _delta) = append_compact_delta(&mut container, &first, &next);
    assert_eq!(append_ticket(&mut container), 3);
    append_version_marker(&mut container, delta_chunk, 3, 2);
    drop(container);

    let (store, errors) = replay_while_capturing_ingest_errors(&path);
    assert!(matches!(errors.first(), Some(MemoryError::InvalidVersion)));
    assert_eq!(store.records(), &[first]);
    assert_eq!(store.memory_version(), 1);
}

#[test]
fn compact_creation_preserves_legacy_late_ticket_visibility() {
    let legacy_path = test_path("legacy-late-ticket.rel");
    let mut legacy_container = initialize_container(&legacy_path);
    let mut legacy_first = initial_record();
    legacy_first.body_id = append_body(&mut legacy_container, "First", "The first valid Memory.");
    let first_chunk = legacy_container
        .append(&encode_record(&legacy_first).unwrap())
        .unwrap();
    assert_eq!(append_ticket(&mut legacy_container), 1);
    append_version_marker(&mut legacy_container, first_chunk, 1, 1);
    assert_eq!(append_ticket(&mut legacy_container), 2);
    assert_eq!(append_ticket(&mut legacy_container), 3);
    let mut legacy_late = initial_record();
    legacy_late.id = MemoryId([43; 32]);
    legacy_late.mutation_id = "late-allocated-ticket".into();
    legacy_late.body_id = append_body(
        &mut legacy_container,
        "Late",
        "Physically later creation bound to ticket 2.",
    );
    legacy_late.memory_version = 2;
    let legacy_chunk = legacy_container
        .append(&encode_record(&legacy_late).unwrap())
        .unwrap();
    append_version_marker(&mut legacy_container, legacy_chunk, 2, 2);
    drop(legacy_container);
    let mut legacy_bytes = fs::read(&legacy_path).unwrap();
    legacy_bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    legacy_bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(&legacy_path, legacy_bytes).unwrap();
    let legacy_bytes_before_replay = fs::read(&legacy_path).unwrap();

    let compact_path = test_path("compact-late-ticket.rel");
    let mut compact_container = initialize_container(&compact_path);
    let mut compact_first = initial_record();
    compact_first.body_id = append_body(&mut compact_container, "First", "The first valid Memory.");
    assert_eq!(append_creation(&mut compact_container, &compact_first), 1);
    assert_eq!(append_ticket(&mut compact_container), 2);
    assert_eq!(append_ticket(&mut compact_container), 3);
    let mut compact_late = initial_record();
    compact_late.id = MemoryId([43; 32]);
    compact_late.mutation_id = "late-allocated-ticket".into();
    compact_late.body_id = append_body(
        &mut compact_container,
        "Late",
        "Physically later creation bound to ticket 2.",
    );
    compact_late.global_version = 2;
    compact_late.memory_version = 2;
    let compact_chunk = compact_container
        .append(&encode_creation(&MemoryCreation::new(compact_late).unwrap()).unwrap())
        .unwrap();
    append_version_marker(&mut compact_container, compact_chunk, 2, 2);
    drop(compact_container);

    let legacy_store = replay(&legacy_path).unwrap();
    assert_eq!(fs::read(&legacy_path).unwrap(), legacy_bytes_before_replay);
    let compact_store = replay(&compact_path).unwrap();
    assert_eq!(legacy_store.records(), compact_store.records());
    assert_eq!(
        legacy_store.memory_version(),
        compact_store.memory_version()
    );
    assert_eq!(legacy_store.records()[1].global_version, 2);
    assert_eq!(legacy_store.records()[1].memory_version, 2);
}

#[test]
fn redundant_delta_rejection_leaves_prior_memory_state_unchanged() {
    let path = test_path("redundant-delta-canonicality.rel");
    let mut container = initialize_container(&path);
    let mut first = initial_record();
    first.body_id = append_body(&mut container, "Decision", "A valid creation.");
    append_creation(&mut container, &first);
    let next = successor(&first);
    let mut delta = MemoryDelta::between(&first, &next).unwrap();
    delta.fields.category = MemoryFieldChange::Set(first.category.clone());
    let delta_chunk = container.append(&encode_delta(&delta).unwrap()).unwrap();
    assert_eq!(append_ticket(&mut container), 2);
    append_version_marker(&mut container, delta_chunk, 2, 2);
    drop(container);

    let (store, errors) = replay_while_capturing_ingest_errors(&path);
    assert!(matches!(
        errors.as_slice(),
        [MemoryError::CorruptRecord(
            "noncanonical Memory delta fields"
        )]
    ));
    assert_eq!(store.records(), &[first]);
    assert_eq!(store.memory_version(), 1);
}

#[test]
fn invalid_temporal_inference_binding_delta_does_not_replace_prior_revision() {
    let path = test_path("bad-inference-binding.rel");
    let mut container = initialize_container(&path);
    let mut first = initial_record();
    first.body_id = append_body(&mut container, "Decision", "A valid creation.");
    append_creation(&mut container, &first);

    let mut next = successor(&first);
    next.temporal_inference = Some(MemoryTemporalInference {
        body_id: first.body_id,
        source_time_ns: Some(7),
        inference: TemporalInference {
            model: "test-model".into(),
            contract_version: "v1".into(),
            resolutions: vec![TemporalInferenceResolution {
                start_byte: 0,
                end_byte: 3,
                kind: TemporalIndicationKind::Relative,
                evidence: "now".into(),
                canonical_expression: "P0D".into(),
            }],
        },
    });
    let delta = MemoryDelta::between(&first, &next).unwrap();
    let delta_chunk = container.append(&encode_delta(&delta).unwrap()).unwrap();
    assert_eq!(append_ticket(&mut container), 2);
    append_version_marker(&mut container, delta_chunk, 2, 2);
    drop(container);

    let (store, errors) = replay_while_capturing_ingest_errors(&path);
    assert!(matches!(
        errors.as_slice(),
        [MemoryError::InvalidField("temporal inference binding")]
    ));
    assert_eq!(store.records(), &[first]);
    assert_eq!(store.memory_version(), 1);
}

#[test]
fn v1_open_rejects_compact_memory_payload_without_modifying_file() {
    let path = test_path("v1-compact-rejected.rel");
    let mut container = initialize_container(&path);
    let mut first = initial_record();
    first.body_id = append_body(&mut container, "Decision", "Compact payload in V1.");
    container
        .append(&encode_creation(&MemoryCreation::new(first).unwrap()).unwrap())
        .unwrap();
    drop(container);

    let mut bytes = fs::read(&path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(&path, bytes).unwrap();
    let before = fs::read(&path).unwrap();

    let mut state = MemoryOpenState::new();
    let result = Container::open_scanned_version(
        &path,
        Some(FormatVersion { major: 1, minor: 0 }),
        |chunk, payload, latest| state.ingest(chunk, payload, latest),
    )
    .and_then(|mut container| state.finish(&mut container));
    assert!(matches!(result, Err(MemoryError::ConflictingFormat)));
    assert_eq!(fs::read(path).unwrap(), before);
}

fn append_v1_out_of_order_global_memory(container: &mut Container) -> Vec<MemoryRecord> {
    let mut first = initial_record();
    first.body_id = append_body(container, "Stable body", "Both revisions retain this body.");
    first.global_version = 2;
    first.memory_version = 1;

    let mut second = successor(&first);
    second.global_version = 1;
    second.memory_version = 2;

    // Reserve both tickets before binding either revision. V1 accepts late
    // markers, including a successor whose global ticket predates its parent.
    assert_eq!(append_ticket(container), 1);
    assert_eq!(append_ticket(container), 2);
    for record in [&first, &second] {
        let chunk = container.append(&encode_record(record).unwrap()).unwrap();
        append_version_marker(
            container,
            chunk,
            record.global_version,
            record.memory_version,
        );
    }
    vec![first, second]
}

fn downgrade_header_to_v1(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn capture_memory_history(
    path: &Path,
) -> (
    Vec<MemoryRecord>,
    Vec<Option<i64>>,
    Vec<crate::historical_fingerprint::HistoricalFingerprint>,
) {
    let mut state = MemoryOpenState::new();
    let mut container = Container::open_scanned(path, |chunk, payload, latest| {
        state.ingest(chunk, payload, latest)
    })
    .unwrap();
    let store = state.finish(&mut container).unwrap();
    let records = store.records().to_vec();
    let transaction_times = records
        .iter()
        .map(|record| container.transaction_time_ns(record.global_version))
        .collect();
    let history =
        crate::historical_memory_fingerprint::MemoryHistory::read(&mut container, &store).unwrap();
    let cuts = (0..=store.memory_version())
        .map(|cut| history.at(cut).unwrap().fingerprint)
        .collect();
    (records, transaction_times, cuts)
}

fn assert_v1_memory_upgrade_preserves_reversed_global_history(path: &Path, kind: FileKind) {
    let expected = {
        let mut owner = match kind {
            FileKind::Reliquary => crate::Cva::create(path).unwrap().container,
            FileKind::Phylactery => crate::Phylactery::create(path).unwrap().container,
        };
        append_v1_out_of_order_global_memory(&mut owner)
    };
    downgrade_header_to_v1(path);

    let source_history = capture_memory_history(path);
    assert_eq!(source_history.0, expected);
    assert_eq!(source_history.0[0].global_version, 2);
    assert_eq!(source_history.0[1].global_version, 1);
    assert_ne!(source_history.1[0], source_history.1[1]);

    let destination = path.with_extension("staged");
    let plan = crate::storage_upgrade::plan_storage_upgrade(
        crate::storage_upgrade::StorageUpgradeRequest {
            source: path.to_path_buf(),
            destination,
        },
    )
    .unwrap();
    let staged = crate::storage_upgrade::stage_memory_storage_upgrade(&plan).unwrap();
    let staged_history = capture_memory_history(staged.test_path());
    assert_eq!(staged_history.0, expected);
    assert_eq!(staged_history.1, source_history.1);
    assert_eq!(staged_history.2, source_history.2);

    let mut container = Container::open(staged.test_path()).unwrap();
    let mut creation_count = 0;
    let mut delta_count = 0;
    let mut reversed_delta_seen = false;
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            if payload.starts_with(b"CVAMC001") {
                creation_count += 1;
            } else if payload.starts_with(b"CVAMD001") {
                delta_count += 1;
                let delta = decode_delta(payload).unwrap();
                reversed_delta_seen |=
                    delta.revision == 2 && delta.global_version == 1 && delta.memory_version == 2;
            }
            assert!(!payload.starts_with(b"CVAMEMR"));
            Ok(())
        })
        .unwrap();
    assert_eq!(creation_count, 1);
    assert_eq!(delta_count, 1);
    assert!(reversed_delta_seen);
}

#[test]
fn v1_rel_and_phy_reopen_accept_successor_with_earlier_global_ticket() {
    let rel_path = test_path("v1-reversed-global.rel");
    let mut rel = crate::Cva::create(&rel_path).unwrap();
    let rel_expected = append_v1_out_of_order_global_memory(&mut rel.container);
    drop(rel);
    downgrade_header_to_v1(&rel_path);

    let rel = crate::Cva::open_v1_read_only(&rel_path).unwrap();
    assert_eq!(rel.memories.records(), rel_expected);
    assert_eq!(rel.memories.memory_version(), 2);
    assert_eq!(rel.memories.records()[0].global_version, 2);
    assert_eq!(rel.memories.records()[1].global_version, 1);
    assert_eq!(
        rel.memories.records()[0].body_id,
        rel.memories.records()[1].body_id
    );
    drop(rel);

    let phy_path = test_path("v1-reversed-global.phy");
    let mut phy = crate::Phylactery::create(&phy_path).unwrap();
    let phy_expected = append_v1_out_of_order_global_memory(&mut phy.container);
    drop(phy);
    downgrade_header_to_v1(&phy_path);

    let phy = crate::Phylactery::open_v1_read_only(&phy_path).unwrap();
    assert_eq!(phy.memories.records(), phy_expected);
    assert_eq!(phy.memories.memory_version(), 2);
    assert_eq!(phy.memories.records()[0].global_version, 2);
    assert_eq!(phy.memories.records()[1].global_version, 1);
    assert_eq!(
        phy.memories.records()[0].body_id,
        phy.memories.records()[1].body_id
    );
}

#[test]
fn memory_upgrade_preserves_v1_reversed_global_history_for_rel_and_phy() {
    let rel_path = test_path("v1-reversed-global-upgrade.rel");
    assert_v1_memory_upgrade_preserves_reversed_global_history(&rel_path, FileKind::Reliquary);

    let phy_path = test_path("v1-reversed-global-upgrade.phy");
    assert_v1_memory_upgrade_preserves_reversed_global_history(&phy_path, FileKind::Phylactery);
}
