use super::EntityStore;
use crate::entity_codec::{DirectEntityTombstone, EntityTombstone};
use crate::entity_model::EntityRecord;
use crate::{EntityError, EntityGlobalId, EntityId};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoreSnapshot {
    records: Vec<EntityRecord>,
    pruned_revisions: BTreeSet<(EntityId, u64)>,
    current: HashMap<EntityId, usize>,
    by_mutation: HashMap<String, usize>,
    by_surface: HashMap<String, BTreeSet<EntityId>>,
    by_normalized_surface: HashMap<String, BTreeSet<EntityId>>,
    by_alias_surface: HashMap<String, BTreeSet<EntityId>>,
    revisions: HashMap<EntityId, u64>,
    retired: HashMap<EntityId, EntityId>,
    tombstones: HashMap<EntityId, crate::EntityTerminalCoordinates>,
    next_entity_version: u64,
}

fn snapshot(store: &EntityStore) -> StoreSnapshot {
    StoreSnapshot {
        records: store.records.clone(),
        pruned_revisions: store.pruned_revisions.clone(),
        current: store.current.clone(),
        by_mutation: store.by_mutation.clone(),
        by_surface: store.by_surface.clone(),
        by_normalized_surface: store.by_normalized_surface.clone(),
        by_alias_surface: store.by_alias_surface.clone(),
        revisions: store.revisions.clone(),
        retired: store.retired.clone(),
        tombstones: store.tombstones.clone(),
        next_entity_version: store.next_entity_version,
    }
}

fn record(id: u8, local: u64) -> EntityRecord {
    EntityRecord {
        id: EntityId([id; 32]),
        global_id: Some(EntityGlobalId([id.wrapping_add(40); 16])),
        revision: 1,
        canonical_name: format!("Entity {id}"),
        aliases: vec![format!("Alias {id}")],
        kind: "project".into(),
        summary: format!("Summary {id}"),
        mutation_id: format!("create-{id}"),
        created_at_ns: 10,
        updated_at_ns: 11,
        global_version: u64::from(local),
        entity_version: local,
    }
}

fn active_store(ids: &[u8]) -> (EntityStore, Vec<EntityRecord>) {
    let mut store = EntityStore::empty();
    let mut records = Vec::new();
    for (index, id) in ids.iter().copied().enumerate() {
        let accepted = record(id, index as u64 + 1);
        store.insert_rebuilt(accepted.clone()).unwrap();
        records.push(accepted);
    }
    (store, records)
}

#[test]
fn exhausted_record_rebuild_does_not_mutate_any_entity_index_or_history() {
    let (mut store, records) = active_store(&[1, 2]);
    store.next_entity_version = u64::MAX;
    let before = snapshot(&store);

    let mut candidate = records[0].clone();
    candidate.revision = 2;
    candidate.global_version = 90;
    candidate.entity_version = u64::MAX;
    candidate.mutation_id = "revision-at-local-overflow".into();
    candidate.canonical_name = "Changed surface at local overflow".into();
    candidate.aliases = vec!["Changed alias at local overflow".into()];

    assert!(matches!(
        store.insert_rebuilt(candidate),
        Err(EntityError::VersionExhausted)
    ));
    assert_eq!(snapshot(&store), before);
}

#[test]
fn exhausted_direct_tombstone_rebuild_does_not_remove_entity_or_surfaces() {
    let (mut store, records) = active_store(&[3]);
    store.next_entity_version = u64::MAX;
    let before = snapshot(&store);

    assert!(matches!(
        store.insert_direct_tombstone_rebuilt(
            DirectEntityTombstone {
                id: records[0].id,
                revision: 2,
            },
            90,
            u64::MAX,
        ),
        Err(EntityError::VersionExhausted)
    ));
    assert_eq!(snapshot(&store), before);
}

#[test]
fn exhausted_merge_tombstone_rebuild_does_not_retire_entity_or_change_indexes() {
    let (mut store, records) = active_store(&[4, 5]);
    store.next_entity_version = u64::MAX;
    let before = snapshot(&store);

    assert!(matches!(
        store.insert_tombstone_rebuilt(
            EntityTombstone {
                id: records[0].id,
                revision: 2,
                replacement_id: records[1].id,
            },
            90,
            u64::MAX,
        ),
        Err(EntityError::VersionExhausted)
    ));
    assert_eq!(snapshot(&store), before);
}
