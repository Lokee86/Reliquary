use super::*;

fn source(id: u8, time: i64) -> InsomniaEpisodeSource {
    InsomniaEpisodeSource {
        episode_id: EpisodeId([id; 32]),
        priority: InsomniaPriority::Live,
        source_through_ns: time,
        conversation_id: "accepted-conversation".into(),
        start_node_id: format!("node-{id}"),
        finalized_at_ns: time,
    }
}

#[test]
fn standalone_work_store_schedules_accepted_coordinates_and_applies_only_local_receipt_state() {
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("insomnia-owner-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut container = Container::create(directory.join("owner.rel")).unwrap();
    let mut store = InsomniaStore::empty();
    store.initialize(&mut container).unwrap();
    store.queue(&mut container, &source(2, 20), 20).unwrap();
    store.queue(&mut container, &source(1, 10), 10).unwrap();
    let claim = store
        .claim_next(&mut container, "worker", 30, 100)
        .unwrap()
        .unwrap();
    assert_eq!(claim.episode_id, EpisodeId([1; 32]));
    let mut receipt = InsomniaAttempt {
        episode_id: claim.episode_id,
        attempt: claim.attempt_count,
        state: InsomniaWorkState::Pending,
        started_at_ns: 30,
        completed_at_ns: 40,
        extractor_model: "accepted-model".into(),
        extractor_version: "accepted-contract".into(),
        memory_ids: vec![crate::MemoryId([3; 32])],
        external_memory_refs: Vec::new(),
        rejected_count: 0,
        error: None,
    };
    let before = store.stats();
    assert!(matches!(
        store.apply_completion(&receipt),
        Err(InsomniaError::InvalidTransition)
    ));
    assert_eq!(store.stats(), before);
    receipt.state = InsomniaWorkState::Complete;
    store.apply_completion(&receipt).unwrap();
    assert_eq!(
        store.work(claim.episode_id).unwrap().state,
        InsomniaWorkState::Complete
    );
    assert_eq!(
        store.completed_memory_ids().collect::<Vec<_>>(),
        receipt.memory_ids
    );
    assert_eq!(
        store
            .claim_next(&mut container, "worker", 40, 100)
            .unwrap()
            .unwrap()
            .episode_id,
        EpisodeId([2; 32])
    );
}
