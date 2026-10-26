use super::RelationshipStore;
use crate::{EntityId, EntityRef, RelationshipError, RelationshipId, RelationshipMemoryRef};
use reliquary_perception_store::relationship_codec::RelationshipTombstone;
use reliquary_perception_store::relationship_model::{RelationshipParticipant, RelationshipRecord};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoreSnapshot {
    records: Vec<RelationshipRecord>,
    pruned_revisions: BTreeSet<(RelationshipId, u64)>,
    current: HashMap<RelationshipId, usize>,
    by_mutation: HashMap<String, usize>,
    by_participant: HashMap<EntityRef, BTreeSet<RelationshipId>>,
    revisions: HashMap<RelationshipId, u64>,
    tombstones: HashMap<RelationshipId, crate::RelationshipTombstoneCoordinates>,
    next_relationship_version: u64,
}

fn snapshot(store: &RelationshipStore) -> StoreSnapshot {
    StoreSnapshot {
        records: store.records.clone(),
        pruned_revisions: store.pruned_revisions.clone(),
        current: store.current.clone(),
        by_mutation: store.by_mutation.clone(),
        by_participant: store.by_participant.clone(),
        revisions: store.revisions.clone(),
        tombstones: store.tombstones.clone(),
        next_relationship_version: store.next_relationship_version,
    }
}

fn record(id: u8, revision: u64, global: u64, local: u64, mutation: &str) -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([id; 32]),
        revision,
        kind: "supports".into(),
        participants: vec![RelationshipParticipant {
            entity: EntityRef {
                owner_id: format!("entity-owner-{id}"),
                entity_id: EntityId([id.wrapping_add(1); 32]),
            },
            role: Some("source".into()),
        }],
        evidence: vec![RelationshipMemoryRef {
            owner_id: "memory-owner".into(),
            memory_id: [id.wrapping_add(2); 32],
        }],
        summary: format!("summary-{id}-{revision}"),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 11 + revision as i64,
        global_version: global,
        relationship_version: local,
    }
}

fn active_store(ids: &[u8]) -> (RelationshipStore, Vec<RelationshipRecord>) {
    let mut store = RelationshipStore::empty();
    let mut records = Vec::new();
    for (index, id) in ids.iter().copied().enumerate() {
        let accepted = record(
            id,
            1,
            index as u64 + 1,
            index as u64 + 1,
            &format!("create-{id}"),
        );
        store.insert_rebuilt(accepted.clone()).unwrap();
        records.push(accepted);
    }
    (store, records)
}

#[test]
fn exhausted_relationship_revision_rebuild_preserves_every_index_and_record() {
    let (mut store, records) = active_store(&[1, 2]);
    store.next_relationship_version = u64::MAX;
    let before = snapshot(&store);

    let mut candidate = records[0].clone();
    candidate.revision = 2;
    candidate.global_version = 90;
    candidate.relationship_version = u64::MAX;
    candidate.mutation_id = "revision-at-local-overflow".into();
    candidate.kind = "changed-kind-at-overflow".into();
    candidate.participants[0].entity.owner_id = "changed-participant-index".into();

    assert!(matches!(
        store.insert_rebuilt(candidate),
        Err(RelationshipError::VersionExhausted)
    ));
    assert_eq!(snapshot(&store), before);
}

#[test]
fn exhausted_relationship_terminal_rebuild_preserves_indexes_and_active_record() {
    let (mut store, records) = active_store(&[3]);
    store.next_relationship_version = u64::MAX;
    let before = snapshot(&store);

    assert!(matches!(
        store.insert_tombstone_rebuilt(
            RelationshipTombstone {
                id: records[0].id,
                revision: 2,
            },
            90,
            u64::MAX,
        ),
        Err(RelationshipError::VersionExhausted)
    ));
    assert_eq!(snapshot(&store), before);
}
