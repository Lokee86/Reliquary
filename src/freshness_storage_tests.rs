use super::*;

#[test]
fn accepted_prefix_ignores_partial_tail_and_rejects_unsupported_records() {
    use std::io::Write;
    let path = std::env::temp_dir().join(format!("freshness-prefix-{}.rel", uuid::Uuid::new_v4()));
    let mut container = crate::Container::create(&path).unwrap();
    let store = enrolled();
    let proof = FreshnessEventProof {
        graph_version: 0,
        community_generation: None,
        community_graph_version: None,
    };
    let source = MemoryId([1; 32]);
    let payload = store
        .prepare_accepted_use_batch(
            [7; 16],
            "prefix-use".into(),
            0,
            vec![source],
            proof,
            vec![BTreeMap::from([(source, 25)])],
        )
        .unwrap();
    container.append(&payload).unwrap();
    container.sync().unwrap();
    let accepted_end = std::fs::metadata(&path).unwrap().len();
    let mut tail = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    tail.write_all(&u64::MAX.to_le_bytes()).unwrap();
    tail.write_all(b"partial").unwrap();
    drop(tail);
    let before = std::fs::read(&path).unwrap();
    let mut visits = 0;
    stream::scan_entries(&path, accepted_end, |_, _| {
        visits += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(visits, 1);
    assert!(stream::scan_entries(&path, accepted_end - 1, |_, _| Ok(())).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    drop(container);
    std::fs::remove_file(&path).unwrap();
    let mut container = crate::Container::create(&path).unwrap();
    container.append(b"CVAFRS99unsupported").unwrap();
    container.sync().unwrap();
    assert!(
        stream::scan_entries(
            &path,
            std::fs::metadata(&path).unwrap().len(),
            |_, _| Ok(())
        )
        .is_err()
    );
    drop(container);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn mixed_event_permutations_match_independent_saturating_history() {
    let owner = [7; 16];
    let target = MemoryId([1; 32]);
    let source = MemoryId([2; 32]);
    let proof = FreshnessEventProof {
        graph_version: 0,
        community_generation: None,
        community_graph_version: None,
    };
    let base = enrolled();
    // Enrollment is fixed; receipts deliberately mix Dream with consumed uses.
    let mut events = Vec::new();
    for (n, turn) in [1000u64, 1000, 1107, 1510, 1999].into_iter().enumerate() {
        let use_id = format!("permutation-{n}");
        events.push(JournalEntry::Event {
            event_id: accepted_event_id(owner, &use_id, target),
            turn,
            provenance: FreshnessEventProvenance {
                producer_kind: 2,
                source_id: use_id,
                origin_owner: owner,
                accepted_turn: turn,
                policy_version: 1,
            },
            proof,
            effects: BTreeMap::from([(target, 25)]),
            records: BTreeMap::new(),
        });
    }
    let dream_id = format!("dream-initial:{}:{}", hex_bytes(&owner), hex_id(&source));
    events.push(JournalEntry::Event {
        event_id: dream_id.clone(),
        turn: 1107,
        provenance: FreshnessEventProvenance {
            producer_kind: 1,
            source_id: dream_id,
            origin_owner: owner,
            accepted_turn: 1107,
            policy_version: 1,
        },
        proof,
        effects: BTreeMap::from([(target, 50)]),
        records: BTreeMap::new(),
    });
    let mut logical = events.clone();
    logical.sort_by_key(event_sort_key);
    let permutations = [
        vec![0, 1, 2, 3, 4, 5],
        vec![5, 4, 3, 2, 1, 0],
        vec![1, 0, 5, 2, 4, 3],
        vec![3, 1, 5, 4, 0, 2],
    ];
    for order in permutations {
        let mut store = base.clone();
        for index in order {
            store.ingest(&encode(owner, &events[index])).unwrap();
        }
        for cut in [999u64, 1000, 1107, 1510, 1999, 2011] {
            let mut score = 100i16;
            let mut accounted = 0u64;
            for event in &logical {
                let JournalEntry::Event { turn, effects, .. } = event else {
                    unreachable!()
                };
                if *turn > cut {
                    continue;
                }
                let decay = (*turn - accounted) / 10;
                score = (i64::from(score) - decay as i64).max(-100) as i16;
                accounted += decay * 10;
                score = (score + effects[&target]).min(100);
            }
            score = (i64::from(score) - ((cut - accounted) / 10) as i64).max(-100) as i16;
            let record = store.records_at(cut).unwrap()[&target];
            assert_eq!(record.score, score, "cut {cut}");
            let sorted_records = store
                .records_at_cut(&crate::FreshnessEventCut {
                    rel_turn: 1107,
                    event_id_inclusive: Some(event_sort_key(&logical[2]).1),
                    graph_version: 0,
                })
                .unwrap();
            assert_eq!(
                sorted_records,
                base.canonical_projection(
                    1107,
                    Some((&event_sort_key(&logical[2]).1, true)),
                    0,
                    &events
                )
                .unwrap()
                .into_iter()
                .map(|(id, mut r)| {
                    r.settle_with_policy(1107, &base.policy).unwrap();
                    (id, r)
                })
                .collect()
            );
        }
    }
}

#[test]
fn canonical_history_scaling_measurement() {
    use std::time::Instant;
    for count in [128usize, 512, 4096, 20000] {
        let path =
            std::env::temp_dir().join(format!("freshness-history-{}.rel", uuid::Uuid::new_v4()));
        let mut container = crate::Container::create(&path).unwrap();
        let mut store = enrolled();
        let owner = [7; 16];
        let source = MemoryId([1; 32]);
        let proof = FreshnessEventProof {
            graph_version: 0,
            community_generation: None,
            community_graph_version: None,
        };
        let mut cache = enrolled();
        for n in (0..count).rev() {
            let use_id = format!("long-{n:08}");
            let payload = store
                .prepare_accepted_use_batch(
                    owner,
                    use_id,
                    (n * 17) as u64,
                    vec![source],
                    proof,
                    vec![BTreeMap::from([(source, 25)])],
                )
                .unwrap();
            container.append(&payload).unwrap();
            cache.remember(decode(&payload).unwrap().1).unwrap();
            let (resident, bytes) = cache.recent_receipt_stats();
            assert!(resident <= RECENT_LIMIT && bytes <= RECENT_BYTES);
        }
        container.sync().unwrap();
        store.bind_history_path(&path, std::fs::metadata(&path).unwrap().len());
        let start = Instant::now();
        let result = store.records_at((count * 17) as u64).unwrap();
        let elapsed = start.elapsed();
        let mut expected = FreshnessRecord::created();
        expected.admit(0).unwrap();
        for n in 0..count {
            expected
                .reinforce_with_policy(25, (n * 17) as u64, &store.policy)
                .unwrap();
        }
        expected
            .settle_with_policy((count * 17) as u64, &store.policy)
            .unwrap();
        assert_eq!(result[&source], expected);
        let mut rebuilt_cache = enrolled();
        stream::scan_entries(
            &path,
            std::fs::metadata(&path).unwrap().len(),
            |_, entry| {
                rebuilt_cache.remember(entry).unwrap();
                let (resident, bytes) = rebuilt_cache.recent_receipt_stats();
                assert!(resident <= RECENT_LIMIT && bytes <= RECENT_BYTES);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            rebuilt_cache.recent_receipt_stats(),
            cache.recent_receipt_stats()
        );
        rebuilt_cache.bind_history_path(&path, std::fs::metadata(&path).unwrap().len());
        let evicted = format!("long-{:08}", count - 1);
        assert!(
            rebuilt_cache
                .resolve_accepted_use(&evicted, ((count - 1) * 17) as u64, &[source])
                .unwrap()
                .is_some()
        );
        assert!(
            rebuilt_cache
                .resolve_accepted_use(&evicted, 0, &[source])
                .is_err()
        );
        let retry = Instant::now();
        assert!(
            store
                .resolve_accepted_use("long-00000000", 0, &[source])
                .unwrap()
                .is_some()
        );
        eprintln!(
            "history={count} replay_ms={} oldest_retry_ms={}",
            elapsed.as_millis(),
            retry.elapsed().as_millis()
        );
        drop(container);
        std::fs::remove_file(path).unwrap();
    }
}

fn enrolled() -> FreshnessStore {
    let owner = [7; 16];
    let mut store = FreshnessStore::new(Some(owner));
    store
        .ingest(&encode_policy(owner, FreshnessPolicy::default_v1()))
        .unwrap();
    for byte in [1, 2] {
        let mut record = FreshnessRecord::created();
        record.admit(0).unwrap();
        store
            .ingest(&store.encode_initialize(owner, MemoryId([byte; 32]), 0, record))
            .unwrap();
    }
    store
}

#[test]
fn source_provenance_and_postimages_are_validated_before_projection() {
    let store = enrolled();
    let owner = [7; 16];
    let source = MemoryId([1; 32]);
    let target = MemoryId([2; 32]);
    let event_id = format!("dream-initial:{}:{}", hex_bytes(&owner), hex_id(&source));
    let turn = 10;
    let proof = FreshnessEventProof {
        graph_version: 0,
        community_generation: None,
        community_graph_version: None,
    };
    let effects = BTreeMap::from([(target, store.policy.linkage_principal)]);
    let records = store
        .prepare_event_postimages_for(&event_id, turn, &effects)
        .unwrap();
    let provenance = FreshnessEventProvenance {
        producer_kind: 1,
        source_id: event_id.clone(),
        origin_owner: owner,
        accepted_turn: turn,
        policy_version: crate::freshness::FRESHNESS_POLICY_VERSION as u64,
    };
    let valid = JournalEntry::Event {
        event_id: event_id.clone(),
        turn,
        provenance,
        proof,
        effects,
        records,
    };

    let rejected = |entry: JournalEntry| {
        let mut attempt = store.clone();
        let before = attempt.records.clone();
        let bytes = encode(owner, &entry);
        assert!(attempt.ingest(&bytes).is_err());
        assert_eq!(attempt.records, before);
        assert_eq!(attempt.births, store.births);
        assert_eq!(attempt.admissions, store.admissions);
        assert_eq!(attempt.recent, store.recent);
    };
    let mut bad = valid.clone();
    if let JournalEntry::Event { provenance, .. } = &mut bad {
        provenance.source_id.push_str("-forged");
    }
    rejected(bad);
    // Standalone source provenance may have a different original activity cut
    // after reconciliation. Invalid policy is always an error instead.
    let mut bad = valid.clone();
    if let JournalEntry::Event { provenance, .. } = &mut bad {
        provenance.policy_version += 1;
    }
    rejected(bad);
    let mut bad = valid.clone();
    if let JournalEntry::Event { proof, .. } = &mut bad {
        proof.graph_version = 3;
        proof.community_generation = Some(1);
        proof.community_graph_version = Some(2);
    }
    rejected(bad);
    // Compact durable events omit derived postimages. A malformed supplied
    // postimage must be rejected by the pre-append encoding boundary instead
    // of pretending it is serialized and recoverable from durable bytes.
    let mut bad = valid.clone();
    if let JournalEntry::Event { records, .. } = &mut bad {
        records.get_mut(&target).unwrap().score = -22;
    }
    let JournalEntry::Event {
        event_id,
        turn,
        provenance,
        proof,
        effects,
        records,
    } = bad
    else {
        unreachable!()
    };
    assert!(
        store
            .encode_event_with_provenance(
                owner,
                event_id,
                turn,
                proof,
                effects,
                records,
                Some(provenance),
            )
            .is_err()
    );
    assert_eq!(store.records, enrolled().records);
    let mut bad = valid.clone();
    if let JournalEntry::Event { effects, .. } = &mut bad {
        effects.insert(target, store.policy.linkage_principal - 1);
    }
    rejected(bad);
    let mut bad = valid.clone();
    if let JournalEntry::Event { provenance, .. } = &mut bad {
        provenance.origin_owner = [8; 16];
    }
    rejected(bad);

    let mut bad = valid.clone();
    if let JournalEntry::Event {
        event_id,
        provenance,
        ..
    } = &mut bad
    {
        *event_id = format!(
            "dream-initial:{}:{}",
            hex_bytes(&owner),
            hex_id(&MemoryId([9; 32]))
        );
        provenance.source_id = event_id.clone();
    }
    rejected(bad);

    // A reconciled destination turn need not be numerically greater than
    // the original accepted producer cut. Rebase verification belongs to
    // the transformation owner, while this codec preserves the origin cut.
    let mut rebased = valid.clone();
    if let JournalEntry::Event { provenance, .. } = &mut rebased {
        provenance.accepted_turn = turn + 5;
    }
    store.validate_event_entry(owner, &rebased).unwrap();

    let mut accepted = store.clone();
    let payload = encode(owner, &valid);
    accepted.ingest(&payload).unwrap();
    let accepted_records = accepted.records.clone();
    let accepted_recent = accepted.recent.clone();
    assert_eq!(accepted.ingest(&payload), Ok(()));
    assert_eq!(accepted.records, accepted_records);
    assert_eq!(accepted.recent, accepted_recent);
    let mut bad = valid.clone();
    if let JournalEntry::Event { proof, .. } = &mut bad {
        proof.graph_version = 1;
    }
    assert!(accepted.ingest(&encode(owner, &bad)).is_err());
    assert_eq!(accepted.records, accepted_records);
    assert_eq!(accepted.recent, accepted_recent);
}

#[test]
fn accepted_use_source_identity_must_match_principal_event() {
    let store = enrolled();
    let owner = [7; 16];
    let source = MemoryId([1; 32]);
    let target = MemoryId([2; 32]);
    let use_id = "r2-consumption";
    let id = accepted_event_id(owner, use_id, source);
    let valid = JournalEntry::Event {
        event_id: id,
        turn: 5,
        provenance: FreshnessEventProvenance {
            producer_kind: 2,
            source_id: use_id.into(),
            origin_owner: owner,
            accepted_turn: 5,
            policy_version: crate::freshness::FRESHNESS_POLICY_VERSION as u64,
        },
        proof: FreshnessEventProof {
            graph_version: 0,
            community_generation: None,
            community_graph_version: None,
        },
        effects: BTreeMap::from([(source, 25), (target, 20)]),
        records: BTreeMap::new(),
    };
    store.validate_event_entry(owner, &valid).unwrap();
    let mut bad = valid;
    if let JournalEntry::Event { effects, .. } = &mut bad {
        effects.remove(&source);
    }
    assert!(store.validate_event_entry(owner, &bad).is_err());
}

#[test]
fn rejected_second_source_does_not_apply_first_source() {
    let store = enrolled();
    let before = store.records.clone();
    let sources = vec![MemoryId([1; 32]), MemoryId([2; 32])];
    let proof = FreshnessEventProof {
        graph_version: 0,
        community_generation: None,
        community_graph_version: None,
    };
    let effects = vec![
        BTreeMap::from([(sources[0], 25)]),
        BTreeMap::from([(sources[1], 25), (MemoryId([99; 32]), 15)]),
    ];
    assert!(
        store
            .prepare_accepted_use_batch(
                [7; 16],
                "invalid-batch".into(),
                1000,
                sources,
                proof,
                effects
            )
            .is_err()
    );
    assert_eq!(store.records, before);
    assert!(store.recent.is_empty());
}

#[test]
fn truncated_and_trailing_batches_leave_projection_unchanged() {
    let store = enrolled();
    let sources = vec![MemoryId([1; 32]), MemoryId([2; 32])];
    let proof = FreshnessEventProof {
        graph_version: 0,
        community_generation: None,
        community_graph_version: None,
    };
    let effects = sources
        .iter()
        .map(|id| BTreeMap::from([(*id, 25)]))
        .collect();
    let payload = store
        .prepare_accepted_use_batch(
            [7; 16],
            "complete-batch".into(),
            1000,
            sources,
            proof,
            effects,
        )
        .unwrap();
    for end in 24..payload.len() {
        let mut attempt = store.clone();
        assert!(attempt.ingest(&payload[..end]).is_err(), "prefix {end}");
        assert_eq!(attempt.records, store.records);
        assert_eq!(attempt.recent, store.recent);
        assert_eq!(attempt.births, store.births);
    }
    let mut trailing = payload;
    trailing.push(0);
    let mut attempt = store.clone();
    assert!(attempt.ingest(&trailing).is_err());
    assert_eq!(attempt.records, store.records);
    assert_eq!(attempt.recent, store.recent);
}

fn draft(key: &str, lifecycle: &str, content: &str) -> crate::MemoryDraft {
    crate::MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: key.into(),
        content: content.into(),
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

fn publish(rel: &mut crate::Cva, key: &str, lifecycle: &str, content: &str) -> MemoryId {
    let (memory, changed) = rel
        .publish_memory(None, 0, draft(key, lifecycle, content))
        .unwrap();
    assert!(changed);
    memory.id
}

fn turns(rel: &mut crate::Cva, from: usize, to: usize) {
    for n in from..to {
        rel.append_node(
            format!("freshness-turn-{n}"),
            "conversation".into(),
            None,
            if n % 2 == 0 { "user" } else { "assistant" }.into(),
            n as i64,
            "accepted owner activity",
        )
        .unwrap();
    }
}

#[test]
fn long_owner_reopen_keeps_finite_receipts_and_exact_evicted_retry() {
    use std::time::Instant;
    let path = std::env::temp_dir().join(format!("freshness-cold-{}.rel", uuid::Uuid::new_v4()));
    let mut rel = crate::Cva::create(&path).unwrap();
    let source = publish(&mut rel, "cold-source", "knowledge", "Source.");
    turns(&mut rel, 0, 1000);
    let owner = rel.owner_uuid().unwrap();
    let proof = FreshnessEventProof {
        graph_version: rel.memory_graph_version(),
        community_generation: None,
        community_graph_version: None,
    };
    // Construct prevalidated receipts without benchmarking repeated public writes.
    let mut frozen = rel.freshness.clone();
    frozen.history_path = None;
    frozen.recent.clear();
    frozen.recent_bytes = 0;
    for n in (0..20000).rev() {
        let payload = frozen
            .prepare_accepted_use_batch(
                owner,
                format!("cold-{n:08}"),
                1000,
                vec![source],
                proof,
                vec![BTreeMap::from([(source, 25)])],
            )
            .unwrap();
        rel.container.append(&payload).unwrap();
    }
    rel.sync().unwrap();
    drop(rel);
    let original = std::fs::read(&path).unwrap();
    for reopen in 0..2 {
        let start = Instant::now();
        let mut rel = crate::Cva::open(&path).unwrap();
        let open_ms = start.elapsed().as_millis();
        assert_eq!(rel.freshness_record(source).unwrap().score, 100);
        assert!(rel.freshness.rebuild_receipts.is_none());
        let (count, bytes) = rel.freshness.recent_receipt_stats();
        assert!(count <= RECENT_LIMIT && bytes <= RECENT_BYTES);
        let retry = Instant::now();
        assert_eq!(
            rel.record_accepted_memory_use(
                crate::AcceptedMemoryUseReceipt {
                    owner_uuid: owner,
                    accepted_turn: 1000,
                    use_id: "cold-00019999".into(),
                    memories: vec![source]
                },
                4
            )
            .unwrap(),
            0
        );
        let retry_ms = retry.elapsed().as_millis();
        assert!(
            rel.record_accepted_memory_use(
                crate::AcceptedMemoryUseReceipt {
                    owner_uuid: owner,
                    accepted_turn: 999,
                    use_id: "cold-00019999".into(),
                    memories: vec![source]
                },
                1
            )
            .is_err()
        );
        let cut = Instant::now();
        assert_eq!(
            rel.freshness_record_at(source, 999).unwrap().unwrap().score,
            1
        );
        let cut_ms = cut.elapsed().as_millis();
        eprintln!(
            "owner_history=20000 reopen={reopen} open_ms={open_ms} evicted_retry_ms={retry_ms} historical_ms={cut_ms} resident_count={count} resident_bytes={bytes}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
    std::fs::remove_file(path).unwrap();
}
