use super::*;

fn record() -> MemoryRecord {
    MemoryRecord {
        id: MemoryId([1; 32]),
        revision: 1,
        body_id: MemoryBodyId([2; 32]),
        category: "c".into(),
        memory_type: "t".into(),
        authority_kind: "a".into(),
        temporal_status: "ts".into(),
        scope: "s".into(),
        lifecycle_state: "l".into(),
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
        mutation_id: "m".into(),
        created_at_ns: -7,
        updated_at_ns: 9,
        global_version: 31,
        memory_version: 2,
    }
}

#[test]
fn exhausted_rebuild_versions_do_not_partially_insert_or_remove_memory() {
    let mut store = MemoryStore::empty();
    let mut first = record();
    first.memory_version = 1;
    store.insert_rebuilt(first.clone()).unwrap();
    store.next_memory_version = u64::MAX;
    let records = store.records.clone();
    let current = store.current.clone();
    let revisions = store.revisions.clone();
    let mutations = store.by_mutation.clone();
    let mut next = first.clone();
    next.revision = 2;
    next.memory_version = u64::MAX;
    next.mutation_id = "overflow".into();
    assert!(matches!(
        store.insert_rebuilt(next),
        Err(MemoryError::VersionExhausted)
    ));
    assert_eq!(store.records, records);
    assert_eq!(store.current, current);
    assert_eq!(store.revisions, revisions);
    assert_eq!(store.by_mutation, mutations);
    assert!(matches!(
        store.insert_tombstone_rebuilt(
            MemoryTombstone {
                id: first.id,
                revision: 2
            },
            9,
            u64::MAX
        ),
        Err(MemoryError::VersionExhausted)
    ));
    assert_eq!(store.records, records);
    assert_eq!(store.current, current);
    assert_eq!(store.revisions, revisions);
    assert!(store.tombstones.is_empty());
    assert_eq!(store.next_memory_version, u64::MAX);
}
