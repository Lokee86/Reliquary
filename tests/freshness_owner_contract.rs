#[test]
fn freshness_survives_public_copy_repack_and_reclamation() {
    let fixture = Fixture::new();
    let source_path = fixture.path("physical-source.rel");
    let mut source = Cva::create(&source_path).unwrap();
    let id = publish(&mut source, "physical-memory", "knowledge");
    turns(&mut source, 0, 2000);
    for (name, turn) in [("physical-early", 1000), ("physical-late", 2000)] {
        source
            .record_accepted_memory_use(
                AcceptedMemoryUseReceipt {
                    owner_uuid: source.owner_uuid().unwrap(),
                    accepted_turn: turn,
                    use_id: name.into(),
                    memories: vec![id],
                },
                2,
            )
            .unwrap();
    }
    let expected = source.freshness_record(id);
    let historical =
        [0u64, 999, 1000, 1507, 2000].map(|turn| source.freshness_records_at(turn).unwrap());
    let original = fs::read(&source_path).unwrap();
    let copies = [
        fixture.path("strict.rel"),
        fixture.path("repacked.rel"),
        fixture.path("reclaimed.rel"),
    ];
    Cva::reconcile(&source_path, &source_path, &copies[0]).unwrap();
    source
        .repack_missing_user_principal(&copies[1], "phy-00000000-0000-0000-0000-000000000123")
        .unwrap();
    source.reclaim_storage(&copies[2]).unwrap();
    for path in copies {
        let mut copied = Cva::open(&path).unwrap();
        assert_eq!(copied.owner_uuid(), source.owner_uuid());
        assert_eq!(copied.freshness_policy(), source.freshness_policy());
        assert_eq!(copied.freshness_record(id), expected);
        for (index, turn) in [0u64, 999, 1000, 1507, 2000].into_iter().enumerate() {
            assert_eq!(
                copied.freshness_records_at(turn).unwrap(),
                historical[index]
            );
        }
        let before = fs::read(&path).unwrap();
        for (name, turn) in [("physical-early", 1000), ("physical-late", 2000)] {
            assert_eq!(
                copied
                    .record_accepted_memory_use(
                        AcceptedMemoryUseReceipt {
                            owner_uuid: copied.owner_uuid().unwrap(),
                            accepted_turn: turn,
                            use_id: name.into(),
                            memories: vec![id]
                        },
                        8
                    )
                    .unwrap(),
                0
            );
        }
        assert_eq!(fs::read(&path).unwrap(), before);
        drop(copied);
        assert_eq!(Cva::open(path).unwrap().freshness_record(id), expected);
    }
    let migrated = fixture.path("current-migration.rel");
    assert!(reliquary_memory::migrate_file(&source_path, &migrated).is_err());
    assert!(
        !migrated.exists(),
        "current identified owners require no legacy migration"
    );
    assert_eq!(fs::read(source_path).unwrap(), original);
}

#[test]
fn invalid_historical_cuts_fail_without_mutating_owner() {
    use reliquary_memory::FreshnessEventCut;
    let fixture = Fixture::new();
    let path = fixture.path("invalid-cuts.rel");
    let mut rel = Cva::create(&path).unwrap();
    let id = publish(&mut rel, "cut-memory", "knowledge");
    let before = fs::read(&path).unwrap();
    assert!(rel.freshness_records_at(1).is_err());
    assert!(rel.freshness_record_at(id, 1).is_err());
    for cut in [
        FreshnessEventCut {
            rel_turn: 1,
            event_id_inclusive: None,
            graph_version: 0,
        },
        FreshnessEventCut {
            rel_turn: 0,
            event_id_inclusive: None,
            graph_version: 1,
        },
        FreshnessEventCut {
            rel_turn: 0,
            event_id_inclusive: Some(" ".into()),
            graph_version: 0,
        },
    ] {
        assert!(rel.freshness_record_at_cut(id, &cut).is_err());
    }
    assert_eq!(fs::read(path).unwrap(), before);
}

// Public owner-boundary contracts: logical events do not revise Memory bodies,
// and acknowledged use survives reopen and physical relocation.
use reliquary_memory::{
    AcceptedMemoryUseReceipt, Cva, FreshnessState, MemoryDraft, MemoryId, PackedVectors,
    ScalarType, VectorSchema,
};
use std::{fs, path::PathBuf};

#[test]
fn owner_policy_is_pinned_and_consumers_use_it_after_reopen() {
    let fixture = Fixture::new();
    let path = fixture.path("policy.rel");
    let mut rel = Cva::create(&path).unwrap();
    let policy = reliquary_memory::freshness::FreshnessPolicy {
        decay_turns_per_point: 5,
        access_principal: 30,
        linkage_principal: 60,
        local_hop_cost: 6,
        outside_hop_cost: 12,
    };
    rel.set_freshness_policy_before_enrollment(policy).unwrap();
    let id = publish(&mut rel, "configured-policy", "knowledge");
    turns(&mut rel, 0, 600);
    assert_eq!(rel.ego_memory_freshness(&[id], 600).unwrap()[0].score, -20);
    let accepted = receipt(&rel, id, "configured-use");
    assert_eq!(rel.record_accepted_memory_use(accepted, 2).unwrap(), 1);
    assert_eq!(rel.ego_memory_freshness(&[id], 600).unwrap()[0].score, 10);
    assert!(
        rel.set_freshness_policy_before_enrollment(
            reliquary_memory::freshness::FreshnessPolicy::default_v1()
        )
        .is_err()
    );
    rel.sync().unwrap();
    drop(rel);
    let rel = Cva::open(&path).unwrap();
    assert_eq!(rel.freshness_policy(), policy);
    assert_eq!(rel.ego_memory_freshness(&[id], 600).unwrap()[0].score, 10);
}

#[test]
fn ego_adapter_keeps_small_web_hard_keeps_and_graded_budget() {
    use reliquary_memory::EgoFreshnessCandidate;
    use std::collections::BTreeSet;
    let fixture = Fixture::new();
    let path = fixture.path("ego-budget.rel");
    let mut rel = Cva::create(&path).unwrap();
    let dormant = publish(&mut rel, "ego-dormant", "knowledge");
    turns(&mut rel, 0, 1000);
    let stale = publish(&mut rel, "ego-stale", "knowledge");
    turns(&mut rel, 1000, 2000);
    let fresh = publish(&mut rel, "ego-fresh", "knowledge");
    let candidates = [dormant, stale, fresh].map(|memory_id| EgoFreshnessCandidate {
        memory_id,
        context_cost: 10,
    });
    let before = fs::read(&path).unwrap();
    let all = rel
        .select_ego_routine_context(&candidates, 30, &BTreeSet::new(), 2000)
        .unwrap()
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(all, BTreeSet::from([dormant, stale, fresh]));
    assert_eq!(
        rel.select_ego_routine_context(&candidates, 10, &BTreeSet::new(), 2000)
            .unwrap(),
        vec![fresh]
    );
    assert_eq!(
        rel.select_ego_routine_context(&candidates, 5, &BTreeSet::from([dormant]), 2000)
            .unwrap(),
        vec![dormant]
    );
    assert_eq!(
        rel.select_ego_routine_context(&candidates, 20, &BTreeSet::from([dormant]), 2000)
            .unwrap(),
        vec![dormant, fresh]
    );
    assert!(rel.memory(dormant).is_ok());
    assert_eq!(fs::read(path).unwrap(), before);
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("freshness-owner-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn draft(key: &str, lifecycle: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: key.into(),
        content: format!("Immutable content for {key}"),
        scope: "project".into(),
        lifecycle_state: lifecycle.into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(0),
        mutation_id: key.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}
fn publish(rel: &mut Cva, key: &str, lifecycle: &str) -> MemoryId {
    let (memory, changed) = rel.publish_memory(None, 0, draft(key, lifecycle)).unwrap();
    assert!(changed);
    memory.id
}
fn turns(rel: &mut Cva, from: usize, to: usize) {
    for n in from..to {
        rel.append_node(
            format!("turn-{n}"),
            "conversation".into(),
            None,
            if n % 2 == 0 { "user" } else { "assistant" }.into(),
            n as i64,
            "owner activity",
        )
        .unwrap();
    }
}
fn receipt(rel: &Cva, id: MemoryId, use_id: &str) -> AcceptedMemoryUseReceipt {
    AcceptedMemoryUseReceipt {
        owner_uuid: rel.owner_uuid().unwrap(),
        accepted_turn: rel.rel_turn_count(),
        use_id: use_id.into(),
        memories: vec![id, id],
    }
}

#[test]
fn publication_distinguishes_creation_and_active_admission_and_historical_birth() {
    let fixture = Fixture::new();
    let mut rel = Cva::create(fixture.path("source.rel")).unwrap();
    let extracted = publish(&mut rel, "extracted", "extracted");
    let active = publish(&mut rel, "active", "knowledge");
    let unadmitted = rel.freshness_record(extracted).unwrap();
    assert_eq!(unadmitted.score, 100);
    assert_eq!(unadmitted.admitted_at_turn, None);
    assert_eq!(
        rel.freshness_record(active).unwrap().admitted_at_turn,
        Some(0)
    );
    turns(&mut rel, 0, 19);
    assert_eq!(
        rel.freshness_record(extracted)
            .unwrap()
            .score_at(19)
            .unwrap(),
        100
    );
    assert_eq!(
        rel.freshness_record(active).unwrap().score_at(19).unwrap(),
        99
    );
    let later = publish(&mut rel, "later", "knowledge");
    assert!(!rel.freshness_records_at(0).unwrap().contains_key(&later));
    assert_eq!(
        rel.freshness_record(later).unwrap().admitted_at_turn,
        Some(19)
    );
    rel.sync().unwrap();
    let source_bytes = fs::read(fixture.path("source.rel")).unwrap();
    drop(rel);
    let rel = Cva::open(fixture.path("source.rel")).unwrap();
    assert_eq!(rel.freshness_record(extracted).unwrap(), unadmitted);
    assert_eq!(
        fs::read(fixture.path("source.rel")).unwrap(),
        source_bytes,
        "ordinary open must not silently enroll or reanchor"
    );
}

#[test]
fn accepted_use_is_once_per_memory_and_survives_reopen_without_semantic_revisions() {
    let fixture = Fixture::new();
    let path = fixture.path("source.rel");
    let mut rel = Cva::create(&path).unwrap();
    let id = publish(&mut rel, "dormant", "knowledge");
    turns(&mut rel, 0, 2100);
    let before_memory = rel.memory(id).unwrap();
    let before_version = rel.memory_version();
    let before_global = rel.latest_global_version();
    assert_eq!(
        rel.freshness_record(id).unwrap().score_at(2100).unwrap(),
        -100
    );
    let use_receipt = receipt(&rel, id, "accepted-delivery-1");
    assert_eq!(
        rel.record_accepted_memory_use(use_receipt.clone(), 4)
            .unwrap(),
        1
    );
    let after = rel.freshness_record(id).unwrap();
    assert_eq!(after.score_at(2100).unwrap(), -75);
    assert_eq!(after.state_at(2100).unwrap(), FreshnessState::Dormant);
    assert_eq!(
        rel.record_accepted_memory_use(use_receipt.clone(), 1)
            .unwrap(),
        0
    );
    assert_eq!(rel.memory(id).unwrap(), before_memory);
    assert_eq!(rel.memory_version(), before_version);
    assert_eq!(rel.latest_global_version(), before_global);
    rel.sync().unwrap();
    drop(rel);
    let mut rel = Cva::open(&path).unwrap();
    assert_eq!(rel.freshness_record(id).unwrap(), after);
    assert_eq!(rel.record_accepted_memory_use(use_receipt, 8).unwrap(), 0);
    assert_eq!(rel.freshness_record(id).unwrap(), after);

    let schema = VectorSchema::new(2, ScalarType::F32).unwrap();
    rel.put_packed_vectors(PackedVectors::from_bytes(schema, vec![0; 8]).unwrap())
        .unwrap();
    let reclaimed = fixture.path("reclaimed.rel");
    rel.reclaim_storage(&reclaimed).unwrap();
    let mut copy = Cva::open(&reclaimed).unwrap();
    assert_eq!(copy.owner_uuid(), rel.owner_uuid());
    assert_eq!(copy.freshness_record(id), rel.freshness_record(id));
    let replay = receipt(&copy, id, "accepted-delivery-1");
    assert_eq!(copy.record_accepted_memory_use(replay, 2).unwrap(), 0);
}

#[test]
fn querying_dormant_memory_and_search_preparation_are_passive() {
    let fixture = Fixture::new();
    let path = fixture.path("source.rel");
    let mut rel = Cva::create(&path).unwrap();
    let id = publish(&mut rel, "cold-memory", "knowledge");
    turns(&mut rel, 0, 1600);
    let before = rel.freshness_record(id).unwrap();
    let bytes = fs::read(&path).unwrap();
    let _ = rel.memory(id).unwrap();
    let grades = rel.ego_memory_freshness(&[id, id], 1600).unwrap();
    assert_eq!(grades.len(), 1);
    assert_eq!(grades[0].state, FreshnessState::Dormant);
    assert_eq!(grades[0].score, -60);
    assert_eq!(rel.freshness_record(id).unwrap(), before);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let mut wrong = receipt(&rel, id, "wrong-owner");
    wrong.owner_uuid[0] ^= 1;
    assert!(rel.record_accepted_memory_use(wrong, 1).is_err());
    assert_eq!(rel.freshness_record(id).unwrap(), before);
}

#[test]
fn rejected_publications_leave_no_durable_freshness_intent() {
    let fixture = Fixture::new();
    let path = fixture.path("preflight.rel");
    let mut rel = Cva::create(&path).unwrap();
    let baseline = fs::read(&path).unwrap();
    let mut invalid = draft("invalid-authority", "knowledge");
    invalid.authority_kind = "inferred".into();
    assert!(rel.publish_memory(None, 0, invalid).is_err());
    assert_eq!(fs::read(&path).unwrap(), baseline);
    assert!(
        rel.publish_memory(None, 1, draft("bad-revision", "knowledge"))
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), baseline);
    let mut provenance = draft("bad-provenance", "knowledge");
    provenance.content_source_conversation_id = Some("missing".into());
    provenance.content_source_node_id = Some("missing".into());
    assert!(rel.publish_memory(None, 0, provenance).is_err());
    assert_eq!(fs::read(&path).unwrap(), baseline);
    let id = publish(&mut rel, "immutable", "knowledge");
    let accepted = fs::read(&path).unwrap();
    let mut changed_body = draft("metadata-change", "knowledge");
    changed_body.title = "immutable".into();
    assert!(rel.publish_memory(Some(id), 1, changed_body).is_err());
    assert_eq!(fs::read(&path).unwrap(), accepted);
    drop(rel);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened.freshness_record(id).unwrap().admitted_at_turn,
        Some(0)
    );
    assert_eq!(fs::read(&path).unwrap(), accepted);
}

#[test]
fn mutation_retry_uses_accepted_memory_identity_without_a_new_birth_intent() {
    let fixture = Fixture::new();
    let path = fixture.path("mutation-retry.rel");
    let mut rel = Cva::create(&path).unwrap();
    let id = publish(&mut rel, "original", "knowledge");
    let bytes = fs::read(&path).unwrap();
    let supplied_id = MemoryId([0xfa; 32]);
    let (retried, changed) = rel
        .publish_memory(Some(supplied_id), 0, draft("original", "knowledge"))
        .unwrap();
    assert!(!changed);
    assert_eq!(retried.id, id);
    assert!(!rel.freshness_has_pending_publication(supplied_id));
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn accepted_use_retry_resolves_original_effects_before_graph_traversal() {
    use reliquary_memory::{GraphRelationKind, GraphRelationOrigin};
    let fixture = Fixture::new();
    let path = fixture.path("use-retry.rel");
    let mut rel = Cva::create(&path).unwrap();
    let source = publish(&mut rel, "used-source", "knowledge");
    let target = publish(&mut rel, "later-neighbor", "knowledge");
    turns(&mut rel, 0, 2000);
    let accepted = receipt(&rel, source, "stable-delivery");
    assert_eq!(
        rel.record_accepted_memory_use(accepted.clone(), 2).unwrap(),
        1
    );
    let source_record = rel.freshness_record(source).unwrap();
    let target_record = rel.freshness_record(target).unwrap();
    rel.set_memory_relation_with_origin(
        source,
        target,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::Dream,
        rel.graph_version(),
    )
    .unwrap();
    let bytes = fs::read(&path).unwrap();
    assert_eq!(
        rel.record_accepted_memory_use(accepted.clone(), 4).unwrap(),
        0
    );
    assert_eq!(rel.freshness_record(source).unwrap(), source_record);
    assert_eq!(rel.freshness_record(target).unwrap(), target_record);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let mut changed_cut = accepted.clone();
    changed_cut.accepted_turn -= 1;
    assert!(rel.record_accepted_memory_use(changed_cut, 1).is_err());
    let mut changed_sources = accepted.clone();
    changed_sources.memories.push(target);
    assert!(rel.record_accepted_memory_use(changed_sources, 1).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    drop(rel);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.record_accepted_memory_use(accepted, 1).unwrap(), 0);
    assert_eq!(reopened.freshness_record(target).unwrap(), target_record);
}

#[test]
fn independent_sources_remain_additive_inside_one_atomic_delivery() {
    use reliquary_memory::{GraphRelationKind, GraphRelationOrigin};
    let fixture = Fixture::new();
    let path = fixture.path("independent-sources.rel");
    let mut rel = Cva::create(&path).unwrap();
    let a = publish(&mut rel, "source-a", "knowledge");
    let b = publish(&mut rel, "source-b", "knowledge");
    let target = publish(&mut rel, "shared-target", "knowledge");
    for source in [a, b] {
        rel.set_memory_relation_with_origin(
            source,
            target,
            GraphRelationKind::Topical,
            true,
            GraphRelationOrigin::Dream,
            rel.graph_version(),
        )
        .unwrap();
    }
    turns(&mut rel, 0, 2000);
    let accepted = AcceptedMemoryUseReceipt {
        owner_uuid: rel.owner_uuid().unwrap(),
        accepted_turn: 2000,
        use_id: "two-source-delivery".into(),
        memories: vec![b, a, a],
    };
    assert_eq!(
        rel.record_accepted_memory_use(accepted.clone(), 4).unwrap(),
        2
    );
    assert_eq!(
        rel.freshness_record(target)
            .unwrap()
            .score_at(2000)
            .unwrap(),
        -70
    );
    let bytes = fs::read(&path).unwrap();
    let mut fewer_sources = accepted.clone();
    fewer_sources.memories = vec![a];
    assert!(rel.record_accepted_memory_use(fewer_sources, 1).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let mut reordered = accepted;
    reordered.memories = vec![a, b];
    assert_eq!(rel.record_accepted_memory_use(reordered, 1).unwrap(), 0);
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn due_notifications_are_exact_bounded_and_rebuilt_after_open() {
    use reliquary_memory::FreshnessTransitionNotice;
    let fixture = Fixture::new();
    let path = fixture.path("due.rel");
    let mut rel = Cva::create(&path).unwrap();
    let id = publish(&mut rel, "due-memory", "knowledge");

    // A one-item read leaves the second crossed boundary queued.
    turns(&mut rel, 0, 1510);
    assert_eq!(
        rel.drain_freshness_notifications(1),
        vec![FreshnessTransitionNotice::Crossing(
            reliquary_memory::FreshnessDue {
                boundary_turn: 1000,
                memory: id,
                from_state: FreshnessState::Fresh,
                to_state: FreshnessState::Stale,
            }
        )]
    );
    assert_eq!(
        rel.drain_freshness_notifications(1),
        vec![FreshnessTransitionNotice::Crossing(
            reliquary_memory::FreshnessDue {
                boundary_turn: 1510,
                memory: id,
                from_state: FreshnessState::Stale,
                to_state: FreshnessState::Dormant,
            }
        )]
    );
    rel.sync().unwrap();
    drop(rel);

    // The schedule is derived and rebuilds from durable anchors. An unacknowledged/
    // already delivered crossing may be repeated after reopen (at-least-once contract).
    let mut rel = Cva::open(&path).unwrap();
    turns(&mut rel, 1510, 1511);
    assert_eq!(
        rel.drain_freshness_notifications(1),
        vec![FreshnessTransitionNotice::Crossing(
            reliquary_memory::FreshnessDue {
                boundary_turn: 1000,
                memory: id,
                from_state: FreshnessState::Fresh,
                to_state: FreshnessState::Stale,
            }
        )]
    );
}

#[test]
fn reinforcement_emits_a_reverse_crossing_notice() {
    use reliquary_memory::FreshnessTransitionNotice;
    let fixture = Fixture::new();
    let mut rel = Cva::create(fixture.path("reverse.rel")).unwrap();
    let id = publish(&mut rel, "reverse-memory", "knowledge");
    turns(&mut rel, 0, 1510);
    let _ = rel.drain_freshness_notifications(8);

    let accepted = receipt(&rel, id, "reverse-delivery");
    assert_eq!(rel.record_accepted_memory_use(accepted, 1).unwrap(), 1);
    assert_eq!(
        rel.drain_freshness_notifications(1),
        vec![FreshnessTransitionNotice::Reversal(
            reliquary_memory::FreshnessReversal {
                memory: id,
                at_turn: 1510,
                from_state: FreshnessState::Dormant,
                to_state: FreshnessState::Stale,
            }
        )]
    );
}
// Append these three tests to tests/freshness_owner_contract.rs. Existing Fixture,
// publish, turns and receipt helpers are in scope in that integration-test module.

#[test]
fn accepted_uses_replay_in_canonical_cut_order_after_reopen() {
    let fixture = Fixture::new();
    let base = fixture.path("canonical-base.rel");
    let mut seed = Cva::create(&base).unwrap();
    let id = publish(&mut seed, "canonical-memory", "knowledge");
    turns(&mut seed, 0, 2000);
    seed.sync().unwrap();
    drop(seed);

    let forward_path = fixture.path("forward.rel");
    let reverse_path = fixture.path("reverse.rel");
    fs::copy(&base, &forward_path).unwrap();
    fs::copy(&base, &reverse_path).unwrap();

    let apply = |rel: &mut Cva, use_id: &str, accepted_turn| {
        rel.record_accepted_memory_use(
            AcceptedMemoryUseReceipt {
                owner_uuid: rel.owner_uuid().unwrap(),
                accepted_turn,
                use_id: use_id.into(),
                memories: vec![id],
            },
            1,
        )
        .unwrap()
    };
    let mut forward = Cva::open(&forward_path).unwrap();
    let mut reverse = Cva::open(&reverse_path).unwrap();
    assert_eq!(forward.owner_uuid(), reverse.owner_uuid());
    assert_eq!(apply(&mut forward, "use-at-1000", 1000), 1);
    assert_eq!(apply(&mut forward, "use-at-2000", 2000), 1);
    assert_eq!(apply(&mut reverse, "use-at-2000", 2000), 1);
    // A late durable receipt still belongs at its original accepted cut.
    assert_eq!(apply(&mut reverse, "use-at-1000", 1000), 1);
    forward.sync().unwrap();
    reverse.sync().unwrap();
    drop(forward);
    drop(reverse);

    let forward = Cva::open(&forward_path).unwrap();
    let reverse = Cva::open(&reverse_path).unwrap();
    assert_eq!(forward.freshness_record(id), reverse.freshness_record(id));
    assert_eq!(
        forward.freshness_records_at(1000).unwrap(),
        reverse.freshness_records_at(1000).unwrap()
    );
    assert_eq!(
        forward.freshness_records_at(2000).unwrap(),
        reverse.freshness_records_at(2000).unwrap()
    );
    // The 1000-turn effect decays before the later effect, so this exercises replay
    // order rather than just commutative addition at one cut.
    assert_ne!(
        forward
            .freshness_record(id)
            .unwrap()
            .score_at(2000)
            .unwrap(),
        100
    );
}

#[test]
fn old_accepted_use_remains_deduplicated_after_recent_receipt_cache_turnover() {
    let fixture = Fixture::new();
    let path = fixture.path("receipt-cache.rel");
    let mut rel = Cva::create(&path).unwrap();
    let id = publish(&mut rel, "receipt-cache-memory", "knowledge");
    let first = receipt(&rel, id, "cache-use-0000");
    assert_eq!(rel.record_accepted_memory_use(first.clone(), 1).unwrap(), 1);
    for n in 0..300 {
        let use_id = format!("cache-use-{:04}", n + 1);
        assert_eq!(
            rel.record_accepted_memory_use(receipt(&rel, id, &use_id), 1)
                .unwrap(),
            1
        );
    }
    let before = rel.freshness_record(id).unwrap();
    assert_eq!(before.score_at(0).unwrap(), 100);
    let bytes = fs::read(&path).unwrap();
    // Must resolve the exact durable receipt after more entries than the bounded
    // recent cache, without re-running propagation or appending another event.
    assert_eq!(rel.record_accepted_memory_use(first, 1).unwrap(), 0);
    assert_eq!(rel.freshness_record(id).unwrap(), before);
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn accepted_use_before_memory_birth_is_rejected_without_durable_change() {
    let fixture = Fixture::new();
    let path = fixture.path("pre-birth-use.rel");
    let mut rel = Cva::create(&path).unwrap();
    turns(&mut rel, 0, 20);
    let id = publish(&mut rel, "born-at-twenty", "knowledge");
    assert_eq!(rel.freshness_record(id).unwrap().admitted_at_turn, Some(20));
    let bytes = fs::read(&path).unwrap();
    let before = rel.freshness_record(id).unwrap();
    let invalid = AcceptedMemoryUseReceipt {
        owner_uuid: rel.owner_uuid().unwrap(),
        accepted_turn: 19,
        use_id: "before-birth".into(),
        memories: vec![id],
    };
    assert!(rel.record_accepted_memory_use(invalid, 1).is_err());
    assert_eq!(rel.freshness_record(id).unwrap(), before);
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

// Append to tests/freshness_owner_contract.rs after `FreshnessEventCut` and
// `freshness_record_at_cut` are added. The current accepted-use API still returns
// usize, so reconstruct each stable event ID with the codec's canonical formatter.

fn accepted_use_event_id(owner: [u8; 16], use_id: &str, source: MemoryId) -> String {
    let owner = owner
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let use_id = use_id
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let source = source
        .0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("accepted-use:{owner}:{use_id}:{source}")
}

#[test]
fn same_turn_event_cuts_are_order_independent_and_bound_by_memory_graph_version() {
    use reliquary_memory::{FreshnessEventCut, GraphRelationKind, GraphRelationOrigin};

    let fixture = Fixture::new();
    let base = fixture.path("same-turn-base.rel");
    let mut seed = Cva::create(&base).unwrap();
    let source = publish(&mut seed, "same-turn-source", "knowledge");
    let neighbor = publish(&mut seed, "same-turn-neighbor", "knowledge");
    seed.set_memory_relation_with_origin(
        source,
        neighbor,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::Dream,
        seed.memory_graph_version(),
    )
    .unwrap();
    turns(&mut seed, 0, 1000);
    let accepted_turn = seed.rel_turn_count();
    let accepted_graph_version = seed.memory_graph_version();
    let owner = seed.owner_uuid().unwrap();
    assert!(accepted_graph_version > 0);
    seed.sync().unwrap();
    drop(seed);

    let forward_path = fixture.path("same-turn-forward.rel");
    let reverse_path = fixture.path("same-turn-reverse.rel");
    fs::copy(&base, &forward_path).unwrap();
    fs::copy(&base, &reverse_path).unwrap();

    let receipt_at_cut = |rel: &Cva, use_id: &str| AcceptedMemoryUseReceipt {
        owner_uuid: rel.owner_uuid().unwrap(),
        accepted_turn,
        use_id: use_id.into(),
        memories: vec![source],
    };
    let use_a = "same-cut-a";
    let use_b = "same-cut-b";
    let expected_a = accepted_use_event_id(owner, use_a, source);
    let expected_b = accepted_use_event_id(owner, use_b, source);
    assert_ne!(expected_a, expected_b);
    let (low_event_id, high_event_id) = if expected_a < expected_b {
        (expected_a.clone(), expected_b.clone())
    } else {
        (expected_b.clone(), expected_a.clone())
    };

    let mut forward = Cva::open(&forward_path).unwrap();
    let mut reverse = Cva::open(&reverse_path).unwrap();
    assert_eq!(forward.owner_uuid(), reverse.owner_uuid());
    assert_eq!(forward.owner_uuid(), Some(owner));
    let accepted_a = receipt_at_cut(&forward, use_a);
    let accepted_b = receipt_at_cut(&forward, use_b);
    assert_eq!(
        forward.record_accepted_memory_use(accepted_a, 1).unwrap(),
        1
    );
    assert_eq!(
        forward.record_accepted_memory_use(accepted_b, 1).unwrap(),
        1
    );
    let accepted_b = receipt_at_cut(&reverse, use_b);
    let accepted_a = receipt_at_cut(&reverse, use_a);
    assert_eq!(
        reverse.record_accepted_memory_use(accepted_b, 1).unwrap(),
        1
    );
    assert_eq!(
        reverse.record_accepted_memory_use(accepted_a, 1).unwrap(),
        1
    );
    forward.sync().unwrap();
    reverse.sync().unwrap();
    drop(forward);
    drop(reverse);

    let forward = Cva::open(&forward_path).unwrap();
    let reverse = Cva::open(&reverse_path).unwrap();
    for (event_id_inclusive, expected_score) in [(&low_event_id, 25), (&high_event_id, 50)] {
        let cut = FreshnessEventCut {
            rel_turn: accepted_turn,
            event_id_inclusive: Some(event_id_inclusive.clone()),
            graph_version: accepted_graph_version,
        };
        let a = forward
            .freshness_record_at_cut(source, &cut)
            .unwrap()
            .unwrap();
        let b = reverse
            .freshness_record_at_cut(source, &cut)
            .unwrap()
            .unwrap();
        assert_eq!(a, b);
        assert_eq!(a.score, expected_score);
    }

    // Both events pin memory-graph version N. At N-1, the same activity and event
    // cut must omit both effects. Turn 1000 decays the +100 birth anchor to zero.
    let before_graph = FreshnessEventCut {
        rel_turn: accepted_turn,
        event_id_inclusive: Some(high_event_id),
        graph_version: accepted_graph_version - 1,
    };
    let a = forward
        .freshness_record_at_cut(source, &before_graph)
        .unwrap()
        .unwrap();
    let b = reverse
        .freshness_record_at_cut(source, &before_graph)
        .unwrap()
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(a.score, 0);
}
