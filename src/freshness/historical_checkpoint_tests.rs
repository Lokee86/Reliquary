use super::*;

fn id(n: u8) -> MemoryId {
    MemoryId([n; 32])
}

fn checkpoint() -> HistoricalCheckpoint {
    let mut record = FreshnessRecord::created();
    record.admit(1).unwrap();
    HistoricalCheckpoint {
        import_id: [1; 32],
        manifest_digest: [2; 32],
        position: 10,
        records: [(id(1), record)].into_iter().collect(),
        applied_event_ids: BTreeSet::new(),
    }
}

#[test]
fn repeated_import_is_a_noop_and_conflicting_retry_fails_closed() {
    let candidate = checkpoint();
    let mut store = HistoricalCheckpointStore::default();
    assert_eq!(
        store.publish(candidate.clone()),
        Ok(HistoricalCheckpointCommit::Published)
    );
    assert_eq!(
        store.publish(candidate),
        Ok(HistoricalCheckpointCommit::AlreadyCommitted)
    );
    let mut conflict = checkpoint();
    conflict.manifest_digest = [3; 32];
    assert_eq!(
        store.publish(conflict),
        Err(HistoricalCheckpointError::ConflictingImport)
    );
}

#[test]
fn dropped_crash_prefix_does_not_replace_authoritative_projection() {
    let original = checkpoint();
    let mut store = HistoricalCheckpointStore::default();
    store.publish(original.clone()).unwrap();
    let mut incomplete = checkpoint();
    incomplete.records.clear();
    assert_eq!(
        store.recover_complete_candidate(incomplete),
        Err(HistoricalCheckpointError::InvalidCheckpoint)
    );
    assert_eq!(store.committed(), Some(&original));
}

#[test]
fn due_index_rebuild_and_post_checkpoint_event_replay_are_idempotent() {
    let mut store = HistoricalCheckpointStore::default();
    store.publish(checkpoint()).unwrap();
    let index = store.rebuild_due_index(10).unwrap();
    assert_eq!(index.len(), 1);
    assert_eq!(
        store.apply_post_checkpoint_event([9; 32], id(1), 25, 20, &FreshnessPolicy::default_v1(),),
        Ok(true)
    );
    assert_eq!(
        store.apply_post_checkpoint_event([9; 32], id(1), 25, 20, &FreshnessPolicy::default_v1(),),
        Ok(false)
    );
    assert_eq!(store.committed().unwrap().records[&id(1)].score, 100);
}
