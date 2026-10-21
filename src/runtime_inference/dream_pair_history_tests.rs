use super::*;
use std::{fs, path::PathBuf};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("dream-pair-compact-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(name: &str) -> crate::MemoryDraft {
    crate::MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: name.into(),
        content: format!("Pair fixture {name}"),
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
        mutation_id: format!("pair-{name}"),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn old_pair(left: MemoryId, right: MemoryId) -> Vec<u8> {
    let pair = canonical_pair(left, right);
    let mut bytes = b"CVADRP01".to_vec();
    bytes.extend_from_slice(&pair.0.0);
    bytes.extend_from_slice(&pair.1.0);
    bytes
}

#[test]
fn dream_pair_mapping_and_pair_have_independent_fixed_wire_vectors() {
    let id = MemoryId([7; 32]);
    let mut expected = b"CVADRM01".to_vec();
    expected.extend_from_slice(&[4, 3, 2, 1]);
    expected.extend_from_slice(&[7; 32]);
    assert_eq!(codec::encode_memory(0x01020304, id), expected);
    assert!(
        matches!(codec::decode(&expected).unwrap(), Some(codec::Record::Memory(0x01020304, value)) if value == id)
    );
    assert_eq!(codec::encode_pair(1, 2), b"CVADRP02\x01\0\0\0\x02\0\0\0");
    for good in [expected, codec::encode_pair(1, 2)] {
        for end in 8..good.len() {
            assert!(codec::decode(&good[..end]).is_err());
        }
        let mut trailing = good.clone();
        trailing.push(0);
        assert!(codec::decode(&trailing).is_err());
    }
    for bad in [
        b"CVADRP03".to_vec(),
        b"CVADRM02".to_vec(),
        codec::encode_memory(0, id),
        codec::encode_pair(0, 1),
        codec::encode_pair(1, 1),
    ] {
        assert!(codec::decode(&bad).is_err());
    }
}

#[test]
fn dream_pair_mapping_conflicts_and_forward_references_cannot_mutate_membership() {
    let a = MemoryId([1; 32]);
    let b = MemoryId([2; 32]);
    let mut store = DreamPairStore::default();
    store.ingest(&codec::encode_memory(1, a)).unwrap();
    store.ingest(&codec::encode_memory(1, a)).unwrap();
    for invalid in [
        codec::encode_memory(1, b),
        codec::encode_memory(2, a),
        codec::encode_memory(3, b),
        codec::encode_pair(1, 2),
    ] {
        assert!(store.ingest(&invalid).is_err());
        assert_eq!(store.memory_ids, [a]);
        assert!(store.records().is_empty());
    }
    store.ingest(&codec::encode_memory(2, b)).unwrap();
    assert!(store.ingest(&codec::encode_pair(2, 1)).is_err());
    assert!(store.records().is_empty());
    store.ingest(&codec::encode_pair(1, 2)).unwrap();
    store.ingest(&codec::encode_pair(1, 2)).unwrap();
    store.ingest(&old_pair(b, a)).unwrap();
    assert_eq!(store.records(), [(a, b)]);
}

#[test]
fn dream_pair_mapping_orphans_still_require_historical_memory_authority() {
    let scratch = Scratch::new();
    let path = scratch.0.join("missing.rel");
    let mut rel = Cva::create(&path).unwrap();
    rel.container
        .append(&codec::encode_memory(1, MemoryId([1; 32])))
        .unwrap();
    rel.sync().unwrap();
    drop(rel);
    let bytes = fs::read(&path).unwrap();
    assert!(Cva::open(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn dream_pair_writers_reopen_with_canonical_pairs_and_clock_neutral_exact_retries() {
    let scratch = Scratch::new();
    for phy in [false, true] {
        let path = scratch
            .0
            .join(if phy { "writer.phy" } else { "writer.rel" });
        let expected = if phy {
            let mut owner = Phylactery::create(&path).unwrap();
            let a = owner.publish_memory(None, 0, draft("a")).unwrap().0.id;
            let b = owner.publish_memory(None, 0, draft("b")).unwrap().0.id;
            let c = owner.publish_memory(None, 0, draft("c")).unwrap().0.id;
            let cut = owner.latest_global_version();
            assert!(owner.mark_dream_pair_evaluated(a, b).unwrap());
            assert!(owner.mark_dream_pair_evaluated(c, a).unwrap());
            let length = fs::metadata(&path).unwrap().len();
            assert!(!owner.mark_dream_pair_evaluated(b, a).unwrap());
            assert_eq!(fs::metadata(&path).unwrap().len(), length);
            assert_eq!(owner.latest_global_version(), cut);
            owner.sync().unwrap();
            owner.dream_pair_records()
        } else {
            let mut owner = Cva::create(&path).unwrap();
            let a = owner.publish_memory(None, 0, draft("a")).unwrap().0.id;
            let b = owner.publish_memory(None, 0, draft("b")).unwrap().0.id;
            let c = owner.publish_memory(None, 0, draft("c")).unwrap().0.id;
            let cut = owner.latest_global_version();
            assert!(owner.mark_dream_pair_evaluated(a, b).unwrap());
            assert!(owner.mark_dream_pair_evaluated(c, a).unwrap());
            let length = fs::metadata(&path).unwrap().len();
            assert!(!owner.mark_dream_pair_evaluated(b, a).unwrap());
            assert_eq!(fs::metadata(&path).unwrap().len(), length);
            assert_eq!(owner.latest_global_version(), cut);
            owner.sync().unwrap();
            owner.dream_pair_records()
        };
        let actual = if phy {
            Phylactery::open(&path).unwrap().dream_pair_records()
        } else {
            Cva::open(&path).unwrap().dream_pair_records()
        };
        assert_eq!(actual, expected);
        let mut scan = Container::open(&path).unwrap();
        let (mut rows, mut pairs) = (0, 0);
        scan.visit_payloads::<crate::ContainerError>(|_, bytes| {
            assert!(!bytes.starts_with(b"CVADRP01"));
            rows += usize::from(bytes.starts_with(codec::MEMORY_MAGIC));
            pairs += usize::from(bytes.starts_with(codec::PAIR_MAGIC));
            Ok(())
        })
        .unwrap();
        assert_eq!((rows, pairs), (3, 2));
    }
}

#[test]
fn dream_pair_v1_upgrade_preserves_rel_and_phy_sets_and_rejects_disguised_current_rows() {
    let scratch = Scratch::new();
    for phy in [false, true] {
        let path = scratch
            .0
            .join(if phy { "legacy.phy" } else { "legacy.rel" });
        let expected = if phy {
            let mut owner = Phylactery::create(&path).unwrap();
            let ids = ["x", "y", "z"].map(|name| {
                owner
                    .memories
                    .publish_legacy_fixture(&mut owner.container, None, 0, draft(name))
                    .unwrap()
                    .0
                    .id
            });
            for (a, b) in [(ids[0], ids[1]), (ids[1], ids[2])] {
                owner.container.append(&old_pair(a, b)).unwrap();
            }
            owner.sync().unwrap();
            vec![
                canonical_pair(ids[0], ids[1]),
                canonical_pair(ids[1], ids[2]),
            ]
        } else {
            let mut owner = Cva::create(&path).unwrap();
            let ids = ["x", "y", "z"].map(|name| {
                owner
                    .memories
                    .publish_legacy_fixture(&mut owner.container, None, 0, draft(name))
                    .unwrap()
                    .0
                    .id
            });
            for (a, b) in [(ids[0], ids[1]), (ids[1], ids[2])] {
                owner.container.append(&old_pair(a, b)).unwrap();
            }
            owner.sync().unwrap();
            vec![
                canonical_pair(ids[0], ids[1]),
                canonical_pair(ids[1], ids[2]),
            ]
        };
        let mut expected = expected;
        expected.sort();
        let mut bytes = fs::read(&path).unwrap();
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        fs::write(&path, &bytes).unwrap();
        let plan = crate::storage_upgrade::plan_storage_upgrade(
            crate::storage_upgrade::StorageUpgradeRequest {
                source: path.clone(),
                destination: scratch.0.join(if phy { "out.phy" } else { "out.rel" }),
            },
        )
        .unwrap();
        let stage = crate::storage_upgrade::stage_graph_envelope_storage_upgrade(&plan).unwrap();
        let actual = if phy {
            Phylactery::open(stage.test_path())
                .unwrap()
                .dream_pair_records()
        } else {
            Cva::open(stage.test_path()).unwrap().dream_pair_records()
        };
        assert_eq!(actual, expected);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        // Isolate the pair-family boundary using genuine legacy Memory frames,
        // so publication-envelope rejection cannot make this check vacuous.
        let mut fake_v1 = bytes.clone();
        let mapping = codec::encode_memory(1, expected[0].0);
        fake_v1.extend_from_slice(&(mapping.len() as u64).to_le_bytes());
        fake_v1.extend_from_slice(&mapping);
        let disguised = scratch.0.join(if phy {
            "disguised.phy"
        } else {
            "disguised.rel"
        });
        fs::write(&disguised, fake_v1).unwrap();
        let error = if phy {
            Phylactery::open_v1_read_only(&disguised)
                .err()
                .unwrap()
                .to_string()
        } else {
            Cva::open_v1_read_only(&disguised)
                .err()
                .unwrap()
                .to_string()
        };
        assert!(error.contains("compact Dream pair in V1 input"), "{error}");
    }
}

#[test]
fn dream_pair_raw_byte_measurement_includes_reference_table_cost() {
    for count in [2, 4, 16] {
        let scratch = Scratch::new();
        let path = scratch.0.join("measurement.rel");
        let mut rel = Cva::create(&path).unwrap();
        let ids = (0..count)
            .map(|n| {
                rel.publish_memory(None, 0, draft(&format!("measurement-{n}")))
                    .unwrap()
                    .0
                    .id
            })
            .collect::<Vec<_>>();
        for a in 0..count {
            for b in a + 1..count {
                rel.mark_dream_pair_evaluated(ids[a], ids[b]).unwrap();
            }
        }
        let pair_count = count * (count - 1) / 2;
        let expected = rel.dream_pair_records();
        rel.sync().unwrap();
        drop(rel);
        assert_eq!(Cva::open(&path).unwrap().dream_pair_records(), expected);
        let mut bytes = 0;
        let mut frames = 0;
        Container::open(&path)
            .unwrap()
            .visit_payloads::<crate::ContainerError>(|_, payload| {
                if DreamPairStore::is_compact_payload(payload) {
                    bytes += payload.len();
                    frames += 1;
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(bytes, 44 * count + 16 * pair_count);
        assert_eq!(frames, count + pair_count);
        println!(
            "Dream pairs memories={count} pairs={pair_count}: legacy_payload={} current_payload={bytes} legacy_framed={} current_framed={}",
            72 * pair_count,
            80 * pair_count,
            bytes + frames * 8
        );
    }
}

#[test]
fn dream_pair_mappings_survive_terminal_payload_pruning_and_reclamation() {
    let scratch = Scratch::new();
    let path = scratch.0.join("maintenance.rel");
    let mut owner = Cva::create(&path).unwrap();
    let mut large = draft("prunable");
    large.content = "historical pair endpoint ".repeat(2048);
    let a = owner.publish_memory(None, 0, large).unwrap().0.id;
    let b = owner
        .publish_memory(None, 0, draft("survivor"))
        .unwrap()
        .0
        .id;
    owner.mark_dream_pair_evaluated(a, b).unwrap();
    owner.tombstone_memory(a, 1).unwrap();
    owner.sync().unwrap();
    let expected = owner.dream_pair_records();
    let source_bytes = fs::read(&path).unwrap();
    let pruned = scratch.0.join("pruned.rel");
    owner
        .prune_tombstoned_history(
            &pruned,
            crate::HistoryPruneRequest {
                targets: vec![crate::owner_lifecycle::LifecycleTarget::Memory(a)],
            },
        )
        .unwrap();
    let mut pruned_owner = Cva::open(&pruned).unwrap();
    assert_eq!(pruned_owner.dream_pair_records(), expected);
    let reclaimed = scratch.0.join("reclaimed.rel");
    pruned_owner.reclaim_storage(&reclaimed).unwrap();
    assert_eq!(
        Cva::open(&reclaimed).unwrap().dream_pair_records(),
        expected
    );
    assert_eq!(fs::read(&path).unwrap(), source_bytes);
}

#[test]
fn dream_pair_retry_after_durable_mapping_prefix_emits_only_missing_rows_and_membership() {
    let scratch = Scratch::new();
    let path = scratch.0.join("mapping-prefix.rel");
    let mut rel = Cva::create(&path).unwrap();
    let a = rel.publish_memory(None, 0, draft("prefix-a")).unwrap().0.id;
    let b = rel.publish_memory(None, 0, draft("prefix-b")).unwrap().0.id;
    rel.mark_dream_pair_evaluated(a, b).unwrap();
    rel.sync().unwrap();
    let cut = rel.latest_global_version();
    let mut first_mapping_end = None;
    rel.container
        .visit_payloads::<crate::ContainerError>(|object, payload| {
            if payload.starts_with(codec::MEMORY_MAGIC) && first_mapping_end.is_none() {
                first_mapping_end = Some(object.offset() + 8 + payload.len() as u64);
            }
            Ok(())
        })
        .unwrap();
    drop(rel);
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(first_mapping_end.unwrap())
        .unwrap();
    let mut recovered = Cva::open(&path).unwrap();
    assert!(recovered.dream_pair_records().is_empty());
    assert!(recovered.mark_dream_pair_evaluated(b, a).unwrap());
    assert_eq!(recovered.latest_global_version(), cut);
    recovered.sync().unwrap();
    drop(recovered);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.dream_pair_records(), [canonical_pair(a, b)]);
    let mut mappings = 0;
    Container::open(&path)
        .unwrap()
        .visit_payloads::<crate::ContainerError>(|_, payload| {
            mappings += usize::from(payload.starts_with(codec::MEMORY_MAGIC));
            Ok(())
        })
        .unwrap();
    assert_eq!(mappings, 2);
}
