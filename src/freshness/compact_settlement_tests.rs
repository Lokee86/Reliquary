use super::*;
use crate::freshness::FreshnessRecord;
fn id(n: u8) -> MemoryId {
    MemoryId([n; 32])
}
fn fixture() -> (SemanticImportFreshnessSettlement, HistoricalCausalBasis) {
    let mut record = FreshnessRecord::created();
    record.admit(10).unwrap();
    record.score = 90;
    let initial = [(id(1), record), (id(2), record)].into_iter().collect();
    let basis = HistoricalCausalBasis::from_canonical(
        initial,
        vec![
            CausalContribution {
                position: 12,
                effects: [(id(1), -100), (id(2), -30)].into_iter().collect(),
            },
            CausalContribution {
                position: 12,
                effects: [(id(1), 50)].into_iter().collect(),
            },
        ],
        [(id(2), 7)].into_iter().collect(),
        21,
    )
    .unwrap();
    let event = AcceptedImportEventEvidence {
        event_id: "event-1".into(),
        payload_digest: [9; 32],
        accepted_turn: 12,
        affected_recipients: [id(1), id(2)].into_iter().collect(),
    };
    let event2 = AcceptedImportEventEvidence {
        event_id: "event-2".into(),
        payload_digest: [10; 32],
        accepted_turn: 12,
        affected_recipients: [id(1)].into_iter().collect(),
    };
    let value = SemanticImportFreshnessSettlement {
        scope: ImportSemanticScope {
            owner: [1; 16],
            import_id: [2; 32],
            start_evidence_digest: [3; 32],
            born_memories: [id(1)].into_iter().collect(),
            affected_recipients: [id(1), id(2)].into_iter().collect(),
            source_nodes: [
                ("convo-α".into(), "node-1".into()),
                ("convo-β".into(), "node-2".into()),
            ]
            .into_iter()
            .collect(),
            base_cut: 10,
            settlement_cut: 21,
            accepted_events: [
                (event.event_id.clone(), event),
                (event2.event_id.clone(), event2),
            ]
            .into_iter()
            .collect(),
            superseded: ["event-1".into()].into_iter().collect(),
        },
        corpus_digest: [4; 32],
        manifest_digest: [5; 32],
        input_digest: [6; 32],
        global_version: 25,
        archive_version: 24,
        graph_version: 23,
        memory_graph_version: 22,
        cohort: [id(1), id(2)].into_iter().collect(),
        records: basis.reconstruct(21).unwrap(),
    };
    (value, basis)
}
fn resign(bytes: &mut Vec<u8>) {
    bytes.truncate(bytes.len() - 32);
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update(&bytes);
    bytes.extend(hash.finalize());
}
#[test]
fn compact_roundtrip_preserves_exact_scope_records_attribution_and_basis() {
    let (value, basis) = fixture();
    let payload = encode(&value, &basis).unwrap();
    assert!(payload.starts_with(MAGIC));
    let (decoded, decoded_basis) = decode(&payload)
        .unwrap()
        .resolve(&value.scope.accepted_events)
        .unwrap();
    assert_eq!(decoded, value);
    assert_eq!(decoded_basis, basis);
    assert_eq!(encode(&decoded, &decoded_basis).unwrap(), payload);
}
#[test]
fn missing_or_changed_independent_leaf_refuses_before_acceptance() {
    let (value, basis) = fixture();
    let payload = encode(&value, &basis).unwrap();
    let mut leaves = value.scope.accepted_events.clone();
    leaves.remove("event-1");
    assert!(decode(&payload).unwrap().resolve(&leaves).is_err());
    for field in 0..3 {
        let mut leaves = value.scope.accepted_events.clone();
        let leaf = leaves.get_mut("event-1").unwrap();
        match field {
            0 => leaf.payload_digest[0] ^= 1,
            1 => leaf.accepted_turn += 1,
            _ => leaf.event_id.push('x'),
        };
        assert!(decode(&payload).unwrap().resolve(&leaves).is_err());
    }
}
#[test]
fn checksum_truncation_reserved_owner_and_nonminimal_count_refuse() {
    let (value, basis) = fixture();
    let payload = encode(&value, &basis).unwrap();
    for index in [0, 8, 296, payload.len() / 2, payload.len() - 1] {
        let mut bytes = payload.clone();
        bytes[index] ^= 1;
        assert!(decode(&bytes).is_err());
    }
    for end in 0..payload.len() {
        assert!(decode(&payload[..end]).is_err());
    }
    let mut bytes = payload.clone();
    bytes[8..24].fill(0);
    resign(&mut bytes);
    assert!(decode(&bytes).is_err());
    let mut bytes = payload.clone();
    bytes[296] = 1;
    resign(&mut bytes);
    assert!(decode(&bytes).is_err());
    let mut bytes = payload.clone();
    bytes[297] |= 128;
    bytes.insert(298, 0);
    resign(&mut bytes);
    assert!(decode(&bytes).is_err());
    let mut bytes = payload.clone();
    let end = bytes.len() - 32;
    bytes.insert(end, 0);
    resign(&mut bytes);
    assert!(decode(&bytes).is_err());
}
#[test]
fn encoder_refuses_a_basis_that_changes_accepted_output_or_record_population() {
    let (mut value, basis) = fixture();
    value.records.get_mut(&id(1)).unwrap().score += 1;
    assert!(encode(&value, &basis).is_err());
    let (mut value, basis) = fixture();
    value.records.remove(&id(1));
    assert!(encode(&value, &basis).is_err());
}

#[test]
fn production_owner_refuses_previous_settlement_format_without_installing_records() {
    let mut store = super::super::FreshnessStore::default();
    assert!(
        store
            .ingest(b"CVAFRS06")
            .unwrap_err()
            .contains("explicit migration")
    );
    assert!(store.records().is_empty());
    assert!(store.import_settlements().is_empty());
}

#[test]
fn text_reference_expansion_budget_refuses_before_large_copy() {
    let mut budget = TextBudget { remaining: 7 };
    budget.charge(4).unwrap();
    assert!(
        budget
            .charge(4)
            .unwrap_err()
            .contains("expanded text limit")
    );
    let (value, basis) = fixture();
    let mut settlement = super::super::HistoricalImportFreshnessSettlement::from_semantic(value);
    assert!(settlement.try_encode().is_err());
    settlement.causal = Some(basis);
    settlement.records.get_mut(&id(1)).unwrap().score += 1;
    assert!(settlement.try_encode().is_err());
    assert!(settlement.validate().is_err());
}

#[test]
fn policy_preflight_precedes_missing_retained_leaf_and_installs_nothing() {
    let (value, basis) = fixture();
    let payload = encode(&value, &basis).unwrap();
    let mut store = super::super::FreshnessStore::default();
    store.owner_uuid = Some(value.scope.owner);
    // Missing retained leaves must not mask absent pinned-policy evidence.
    assert_eq!(
        store.ingest_import_settlement(&payload).unwrap_err(),
        "import settlement owner/base policy mismatch"
    );
    store.policy_persisted = true;
    store.policy.decay_turns_per_point = 11;
    assert_eq!(
        store.ingest_import_settlement(&payload).unwrap_err(),
        "import settlement owner/base policy mismatch"
    );
    assert!(store.records().is_empty());
    assert!(store.import_settlements().is_empty());
}

#[derive(Default)]
struct AdversarialOffsets {
    memory_ids: Vec<std::ops::Range<usize>>,
    text_prefixes: Vec<(usize, usize)>,
    accepted_event_indexes: Vec<usize>,
    superseded_bitmap: usize,
    contribution_positions: Vec<usize>,
    effect_ordinals: Vec<usize>,
    correction_amounts: Vec<usize>,
}

fn adversarial_offsets(payload: &[u8]) -> AdversarialOffsets {
    let mut r = CompactReader::new(&payload[..payload.len() - 32]);
    r.take(8 + 16 + 7 * 32 + 6 * 8 + 1).unwrap();
    let mut offsets = AdversarialOffsets::default();

    for _ in 0..r.count(32).unwrap() {
        let start = payload.len() - 32 - r.remaining();
        r.take(32).unwrap();
        offsets.memory_ids.push(start..start + 32);
    }

    let text_count = r.count(2).unwrap();
    for _ in 0..text_count {
        let prefix_offset = payload.len() - 32 - r.remaining();
        let prefix = usize::try_from(r.var().unwrap()).unwrap();
        offsets.text_prefixes.push((prefix_offset, prefix));
        let suffix = r.count(1).unwrap();
        r.take(suffix).unwrap();
    }

    let set_count = r.count(1).unwrap();
    for _ in 0..set_count {
        for _ in 0..r.count(1).unwrap() {
            r.var().unwrap();
        }
    }
    r.index(set_count).unwrap();
    r.index(set_count).unwrap();

    for _ in 0..r.count(2).unwrap() {
        r.index(text_count).unwrap();
        r.index(text_count).unwrap();
    }

    let accepted_count = r.count(41).unwrap();
    for _ in 0..accepted_count {
        offsets
            .accepted_event_indexes
            .push(payload.len() - 32 - r.remaining());
        r.index(text_count).unwrap();
        r.array::<32>().unwrap();
        r.u64().unwrap();
    }
    offsets.superseded_bitmap = payload.len() - 32 - r.remaining();
    r.take((accepted_count + 7) / 8).unwrap();

    r.index(set_count).unwrap();
    let record_keys_set = r.index(set_count).unwrap();
    let mut anchor_count = 0;
    for _ in 0..r.count(5).unwrap() {
        r.record().unwrap();
        anchor_count += 1;
    }
    assert_eq!(r.count(1).unwrap(), 2); // fixture has two record keys
    for _ in 0..2 {
        r.index(anchor_count).unwrap();
    }

    for _ in 0..r.count(4).unwrap() {
        offsets
            .contribution_positions
            .push(payload.len() - 32 - r.remaining());
        r.var().unwrap();
        for _ in 0..r.count(2).unwrap() {
            offsets
                .effect_ordinals
                .push(payload.len() - 32 - r.remaining());
            r.var().unwrap();
            r.signed().unwrap();
        }
    }
    for _ in 0..r.count(2).unwrap() {
        r.var().unwrap();
        offsets
            .correction_amounts
            .push(payload.len() - 32 - r.remaining());
        r.signed().unwrap();
    }
    r.finish().unwrap();
    let _ = record_keys_set;
    offsets
}

#[test]
fn checksum_valid_memory_dictionary_duplicates_and_unsorted_entries_refuse() {
    let (value, basis) = fixture();
    let payload = encode(&value, &basis).unwrap();
    let offsets = adversarial_offsets(&payload);
    assert_eq!(offsets.memory_ids.len(), 2);

    let mut duplicate = payload.clone();
    let first = duplicate[offsets.memory_ids[0].clone()].to_vec();
    duplicate[offsets.memory_ids[1].clone()].copy_from_slice(&first);
    resign(&mut duplicate);
    assert!(
        decode(&duplicate)
            .err()
            .unwrap()
            .contains("unordered or duplicate compact dictionary")
    );

    let mut unsorted = payload;
    let first = unsorted[offsets.memory_ids[0].clone()].to_vec();
    let second = unsorted[offsets.memory_ids[1].clone()].to_vec();
    unsorted[offsets.memory_ids[0].clone()].copy_from_slice(&second);
    unsorted[offsets.memory_ids[1].clone()].copy_from_slice(&first);
    resign(&mut unsorted);
    assert!(
        decode(&unsorted)
            .err()
            .unwrap()
            .contains("unordered or duplicate compact dictionary")
    );
}

#[test]
fn checksum_valid_nonmaximal_text_prefix_refuses() {
    let (value, basis) = fixture();
    let mut payload = encode(&value, &basis).unwrap();
    let offsets = adversarial_offsets(&payload);
    let index = offsets
        .text_prefixes
        .iter()
        .position(|(_, prefix)| *prefix > 0)
        .unwrap();
    let (offset, prefix) = offsets.text_prefixes[index];
    let texts: Vec<_> = value
        .scope
        .source_nodes
        .iter()
        .flat_map(|(c, n)| [c.as_str(), n.as_str()])
        .chain(value.scope.accepted_events.keys().map(String::as_str))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    assert!(prefix < 128 && payload[offset + 1] < 127);
    // Preserve the exact reconstructed text: move one predecessor byte into
    // the suffix. Only its nonmaximal representation is invalid, including
    // when the maximal byte prefix splits a UTF-8 code point.
    let byte = texts[index - 1].as_bytes()[prefix - 1];
    payload[offset] = u8::try_from(prefix - 1).unwrap();
    payload[offset + 1] += 1;
    payload.insert(offset + 2, byte);
    resign(&mut payload);
    assert!(
        decode(&payload)
            .err()
            .unwrap()
            .contains("nonmaximal compact text prefix")
    );
}

#[test]
fn checksum_valid_zero_or_duplicate_causal_recipient_ordinals_refuse() {
    let (value, basis) = fixture();
    let payload = encode(&value, &basis).unwrap();
    let offsets = adversarial_offsets(&payload);
    assert!(offsets.effect_ordinals.len() >= 2);

    let mut zero = payload.clone();
    zero[offsets.effect_ordinals[0]] = 0;
    resign(&mut zero);
    assert!(
        decode(&zero)
            .err()
            .unwrap()
            .contains("zero compact ordinal delta")
    );

    // A zero delta after the first recipient would repeat its dictionary index.
    let mut duplicate = payload;
    duplicate[offsets.effect_ordinals[1]] = 0;
    resign(&mut duplicate);
    assert!(
        decode(&duplicate)
            .err()
            .unwrap()
            .contains("zero compact ordinal delta")
    );
}

#[test]
fn checksum_valid_zero_source_position_and_dirty_bitmap_padding_refuse() {
    let (value, basis) = fixture();
    let payload = encode(&value, &basis).unwrap();
    let offsets = adversarial_offsets(&payload);
    assert!(!offsets.contribution_positions.is_empty());

    let mut zero_position = payload.clone();
    zero_position[offsets.contribution_positions[0]] = 0;
    resign(&mut zero_position);
    assert!(
        decode(&zero_position)
            .err()
            .unwrap()
            .contains("invalid causal contribution order/extent")
    );

    let mut dirty_padding = payload;
    dirty_padding[offsets.superseded_bitmap] |= 0x80;
    resign(&mut dirty_padding);
    assert!(
        decode(&dirty_padding)
            .err()
            .unwrap()
            .contains("nonzero compact bitmap padding")
    );
}

#[test]
fn checksum_valid_duplicate_accepted_event_identity_refuses() {
    let (value, basis) = fixture();
    let mut payload = encode(&value, &basis).unwrap();
    let offsets = adversarial_offsets(&payload);
    assert_eq!(offsets.accepted_event_indexes.len(), 2);
    let first = offsets.accepted_event_indexes[0];
    let second = offsets.accepted_event_indexes[1];
    assert!(payload[first] < 128 && payload[second] < 128);
    payload[second] = payload[first];
    resign(&mut payload);
    assert!(
        decode(&payload)
            .err()
            .unwrap()
            .contains("unordered compact accepted event identities")
    );
}

#[test]
fn checksum_valid_zero_causal_correction_refuses() {
    let (value, basis) = fixture();
    let mut payload = encode(&value, &basis).unwrap();
    let offsets = adversarial_offsets(&payload);
    assert_eq!(offsets.correction_amounts.len(), 1);
    payload[offsets.correction_amounts[0]] = 0;
    resign(&mut payload);
    assert!(
        decode(&payload)
            .err()
            .unwrap()
            .contains("invalid causal score correction")
    );
}
