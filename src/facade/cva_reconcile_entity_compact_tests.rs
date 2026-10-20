//! Tail reconciliation of compact Entity publications and terminal state.
use super::{read_entity_tail, replay_entity_tail};
use crate::entity_codec::{
    DirectEntityTombstone, EntityTombstone, EntityVersion, encode_direct_tombstone,
    encode_tombstone, encode_version,
};
use crate::entity_compact_codec::{encode_creation, encode_delta};
use crate::entity_compact_contract::{EntityCreation, EntityDelta};
use crate::entity_model::EntityRecord;
use crate::owner_lifecycle::{HistoricalPayload, LifecycleState, TombstoneCoordinates};
use crate::{Container, Cva, EntityError, EntityGlobalId, EntityId};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "cva-reconcile-entity-compact-{}",
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

fn entity(id: u8, name: &str, mutation_id: &str) -> EntityRecord {
    EntityRecord {
        id: EntityId([id; 32]),
        global_id: None,
        revision: 1,
        canonical_name: name.into(),
        aliases: vec![format!("{name} alias")],
        kind: "project".into(),
        summary: format!("Details about {name}"),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: 20,
        global_version: 0,
        entity_version: 0,
    }
}

fn allocate(container: &mut Container, time_ns: i64) -> u64 {
    container
        .allocate_version_with_transaction_time(Some(time_ns))
        .unwrap()
}

fn append_version(container: &mut Container, carrier: crate::ObjectRef, global: u64, local: u64) {
    container
        .append(&encode_version(EntityVersion {
            global_version: global,
            entity_version: local,
            record: carrier,
        }))
        .unwrap();
}

// The test fixture uses the source Entity owner's dense local clock, including
// terminal publications. Its global ticket is allocated immediately before
// each physical carrier so both coordinates are known to the compact frame.
fn append_creation_at_local(
    container: &mut Container,
    mut record: EntityRecord,
    time_ns: i64,
    entity_version: u64,
) -> EntityRecord {
    record.global_version = allocate(container, time_ns);
    record.entity_version = entity_version;
    let carrier = container
        .append(&encode_creation(&EntityCreation::new(record.clone()).unwrap()).unwrap())
        .unwrap();
    append_version(container, carrier, record.global_version, entity_version);
    record
}

fn append_delta_at_local(
    container: &mut Container,
    previous: &EntityRecord,
    mut next: EntityRecord,
    time_ns: i64,
    entity_version: u64,
) -> EntityRecord {
    next.global_version = allocate(container, time_ns);
    next.entity_version = entity_version;
    let delta = EntityDelta::between(previous, &next).unwrap();
    let carrier = container.append(&encode_delta(&delta).unwrap()).unwrap();
    append_version(container, carrier, next.global_version, entity_version);
    next
}

fn create_raw_owner(path: &Path) -> Container {
    drop(Cva::create(path).unwrap());
    Container::open(path).unwrap()
}

#[test]
fn compact_tail_reconciliation_preserves_option_identity_history_terminals_and_times() {
    let scratch = Scratch::new();
    let source_path = scratch.file("source.rel");
    let mut container = create_raw_owner(&source_path);

    let mut first = entity(1, "Entity A", "a-create");
    first = append_creation_at_local(&mut container, first, 10_001, 1);

    let mut second = first.clone();
    second.revision = 2;
    second.global_id = Some(EntityGlobalId([0xA1; 16]));
    second.mutation_id = "a-global-id-a".into();
    second.canonical_name = "Entity A revised".into();
    second.updated_at_ns = 19;
    second = append_delta_at_local(&mut container, &first, second, 10_002, 2);

    let mut third = second.clone();
    third.revision = 3;
    third.global_id = Some(EntityGlobalId([0xB2; 16]));
    third.mutation_id = "a-global-id-b".into();
    third.aliases = vec!["Alternate A".into()];
    third = append_delta_at_local(&mut container, &second, third, 10_003, 3);

    let mut fourth = third.clone();
    fourth.revision = 4;
    fourth.global_id = None;
    fourth.mutation_id = "a-global-id-cleared".into();
    fourth.summary = "Latest A summary".into();
    fourth = append_delta_at_local(&mut container, &third, fourth, 10_004, 4);

    let direct = append_creation_at_local(
        &mut container,
        entity(2, "Direct terminal", "direct-create"),
        10_005,
        5,
    );
    let direct_global = allocate(&mut container, 10_006);
    let direct_tombstone = container
        .append(&encode_direct_tombstone(DirectEntityTombstone {
            id: direct.id,
            revision: 2,
        }))
        .unwrap();
    append_version(&mut container, direct_tombstone, direct_global, 6);

    let merged = append_creation_at_local(
        &mut container,
        entity(3, "Merged entity", "merge-create"),
        10_007,
        7,
    );
    let survivor = append_creation_at_local(
        &mut container,
        entity(4, "Merge survivor", "survivor-create"),
        10_008,
        8,
    );
    let merge_global = allocate(&mut container, 10_009);
    let merge_tombstone = container
        .append(&encode_tombstone(EntityTombstone {
            id: merged.id,
            revision: 2,
            replacement_id: survivor.id,
        }))
        .unwrap();
    append_version(&mut container, merge_tombstone, merge_global, 9);
    drop(container);

    let mut source = Cva::open(&source_path).unwrap();
    let expected_records = vec![
        first.clone(),
        second.clone(),
        third.clone(),
        fourth.clone(),
        direct.clone(),
        merged.clone(),
        survivor.clone(),
    ];
    assert_eq!(source.entities.records(), expected_records);

    let tail = read_entity_tail(&mut source, 0).unwrap();
    let destination_path = scratch.file("destination.rel");
    let mut destination = Cva::create(&destination_path).unwrap();
    let replayed = replay_entity_tail(&mut destination, tail).unwrap();
    assert_eq!(replayed.revisions, 9);
    assert_eq!(replayed.duplicate_revisions, 0);
    assert_eq!(destination.entities.records(), expected_records);
    for expected in &expected_records {
        assert_eq!(
            source.transaction_time_ns(expected.global_version),
            destination.transaction_time_ns(expected.global_version),
            "transaction time for {:?} revision {}",
            expected.id,
            expected.revision,
        );
    }

    assert_eq!(
        destination.entity_revision(first.id, 1).unwrap().global_id,
        None
    );
    assert_eq!(
        destination.entity_revision(first.id, 2).unwrap().global_id,
        Some(EntityGlobalId([0xA1; 16]))
    );
    assert_eq!(
        destination.entity_revision(first.id, 3).unwrap().global_id,
        Some(EntityGlobalId([0xB2; 16]))
    );
    assert_eq!(
        destination.entity_revision(first.id, 4).unwrap().global_id,
        None
    );
    assert!(matches!(
        destination.entity_lifecycle_state(direct.id).unwrap(),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(6),
            domain_version: Some(6),
        })
    ));
    assert_eq!(destination.retired_entity_replacement(direct.id), None);
    assert_eq!(
        destination.retired_entity_replacement(merged.id),
        Some(survivor.id)
    );
    let descriptor = destination.entity_retention_descriptor(merged.id).unwrap();
    assert_eq!(descriptor.tombstone_revision, 2);
    assert_eq!(descriptor.tombstone_global_version, 9);
    assert_eq!(descriptor.tombstone_entity_version, 9);
}

#[test]
fn compact_reconciliation_rejects_pruned_payload_without_fabricating_placeholder() {
    let scratch = Scratch::new();
    let source_path = scratch.file("pruned-source.rel");
    let pruned_path = scratch.file("pruned-authorized.rel");
    let mut source = Cva::create(&source_path).unwrap();
    let full = source
        .publish_entity(
            None,
            0,
            crate::EntityDraft {
                canonical_name: "Original name".into(),
                aliases: vec!["Original alias".into()],
                kind: "project".into(),
                // Exercise unavailable payload replay after satisfying native net-shrink admission.
                summary: "Original private description".repeat(128),
                mutation_id: "pruned-source".into(),
                created_at_ns: 10,
                updated_at_ns: 20,
            },
        )
        .unwrap()
        .0;
    source.tombstone_entity(full.id, full.revision).unwrap();
    source
        .prune_tombstoned_history(
            &pruned_path,
            crate::HistoryPruneRequest {
                targets: vec![crate::owner_lifecycle::LifecycleTarget::Entity(full.id)],
            },
        )
        .unwrap();
    drop(source);

    let mut source = Cva::open(&pruned_path).unwrap();
    assert!(matches!(
        source.historical_entity_revision(full.id, 1).unwrap(),
        HistoricalPayload::Pruned { .. }
    ));
    assert!(matches!(
        read_entity_tail(&mut source, 0),
        Err(crate::CvaReconcileError::Entities(
            EntityError::PayloadUnavailable(crate::owner_lifecycle::PayloadUnavailable::Pruned)
        ))
    ));

    // Identical visible placeholder text without prune evidence is authored
    // semantic content and remains an available payload on replay.
    let authored_path = scratch.file("authored-placeholder.rel");
    let mut container = create_raw_owner(&authored_path);
    let mut authored = entity(6, "[pruned]", "authored-placeholder");
    authored.aliases.clear();
    authored.summary.clear();
    authored.global_version = allocate(&mut container, 20_002);
    authored.entity_version = 1;
    let carrier = container
        .append(&encode_creation(&EntityCreation::new(authored.clone()).unwrap()).unwrap())
        .unwrap();
    append_version(&mut container, carrier, authored.global_version, 1);
    drop(container);

    let mut source = Cva::open(&authored_path).unwrap();
    let tail = read_entity_tail(&mut source, 0).unwrap();
    let destination_path = scratch.file("authored-destination.rel");
    let mut destination = Cva::create(&destination_path).unwrap();
    replay_entity_tail(&mut destination, tail).unwrap();
    assert_eq!(
        destination
            .entity_revision(authored.id, 1)
            .unwrap()
            .canonical_name,
        "[pruned]"
    );
    assert!(matches!(
        destination
            .historical_entity_revision(authored.id, 1)
            .unwrap(),
        HistoricalPayload::Available(_)
    ));
}

#[test]
fn direct_and_merge_terminal_retries_are_exact_and_cross_form_retries_fail() {
    use crate::entity_store::EntityReplayPublication;

    let scratch = Scratch::new();
    let direct_path = scratch.file("direct-retry.rel");
    let mut direct = Cva::create(&direct_path).unwrap();
    let deleted = entity(7, "Direct retry", "direct-retry-create");
    assert!(
        direct
            .entities
            .replay_publication(
                &mut direct.container,
                EntityReplayPublication::Record(deleted.clone()),
            )
            .unwrap()
    );
    let direct_terminal = EntityReplayPublication::Tombstone {
        id: deleted.id,
        revision: 2,
        replacement_id: None,
    };
    assert!(
        direct
            .entities
            .replay_publication(&mut direct.container, direct_terminal.clone())
            .unwrap()
    );
    assert!(
        !direct
            .entities
            .replay_publication(&mut direct.container, direct_terminal)
            .unwrap()
    );
    assert!(matches!(
        direct.entities.replay_publication(
            &mut direct.container,
            EntityReplayPublication::Tombstone {
                id: deleted.id,
                revision: 2,
                replacement_id: Some(EntityId([8; 32])),
            },
        ),
        Err(EntityError::TerminalTombstone)
    ));

    let merge_path = scratch.file("merge-retry.rel");
    let mut merge = Cva::create(&merge_path).unwrap();
    let retired = entity(9, "Merge retry", "merge-retry-create");
    let survivor = entity(10, "Merge survivor", "merge-survivor-create");
    for record in [&retired, &survivor] {
        assert!(
            merge
                .entities
                .replay_publication(
                    &mut merge.container,
                    EntityReplayPublication::Record(record.clone()),
                )
                .unwrap()
        );
    }
    let merge_terminal = EntityReplayPublication::Tombstone {
        id: retired.id,
        revision: 2,
        replacement_id: Some(survivor.id),
    };
    assert!(
        merge
            .entities
            .replay_publication(&mut merge.container, merge_terminal.clone())
            .unwrap()
    );
    assert!(
        !merge
            .entities
            .replay_publication(&mut merge.container, merge_terminal)
            .unwrap()
    );
    assert!(matches!(
        merge.entities.replay_publication(
            &mut merge.container,
            EntityReplayPublication::Tombstone {
                id: retired.id,
                revision: 2,
                replacement_id: None,
            },
        ),
        Err(EntityError::TerminalTombstone)
    ));
}
