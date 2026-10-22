use super::*;
use crate::{FreshnessEventProof, freshness::FreshnessRecord};
use std::collections::{BTreeMap, BTreeSet};

fn id(byte: u8) -> MemoryId {
    MemoryId([byte; 32])
}

fn record_at(turn: u64) -> FreshnessRecord {
    let mut record = FreshnessRecord::created();
    record.admit(turn).unwrap();
    record
}

fn fixture() -> SemanticImportFreshnessSettlement {
    let first = id(2);
    let second = id(3);
    let mut first_record = record_at(10);
    first_record
        .settle_with_policy(10, &FreshnessPolicy::default_v1())
        .unwrap();
    let mut second_record = record_at(10);
    second_record
        .settle_with_policy(10, &FreshnessPolicy::default_v1())
        .unwrap();
    let event = AcceptedImportEventEvidence {
        event_id: "semantic-event-1".into(),
        payload_digest: [9; 32],
        accepted_turn: 12,
        affected_recipients: BTreeSet::from([first, second]),
    };
    SemanticImportFreshnessSettlement {
        scope: ImportSemanticScope {
            owner: [1; 16],
            import_id: [4; 32],
            start_evidence_digest: [5; 32],
            born_memories: BTreeSet::from([first, second]),
            affected_recipients: BTreeSet::from([first, second]),
            source_nodes: BTreeSet::from([
                ("convo01".into(), "node01".into()),
                ("convo02".into(), "node02".into()),
            ]),
            base_cut: 10,
            settlement_cut: 12,
            accepted_events: BTreeMap::from([
                (event.event_id.clone(), event.clone()),
                (
                    "semantic-event-2".into(),
                    AcceptedImportEventEvidence {
                        event_id: "semantic-event-2".into(),
                        payload_digest: [10; 32],
                        accepted_turn: 12,
                        affected_recipients: BTreeSet::from([first, second]),
                    },
                ),
            ]),
            superseded: BTreeSet::from(["semantic-event-1".into()]),
        },
        corpus_digest: [6; 32],
        manifest_digest: [7; 32],
        input_digest: [8; 32],
        global_version: 14,
        archive_version: 13,
        graph_version: 4,
        memory_graph_version: 4,
        cohort: BTreeSet::from([first, second]),
        records: BTreeMap::from([(first, first_record), (second, second_record)]),
    }
}

fn empty_zero_activity() -> SemanticImportFreshnessSettlement {
    SemanticImportFreshnessSettlement {
        scope: ImportSemanticScope {
            owner: [1; 16],
            import_id: [4; 32],
            start_evidence_digest: [5; 32],
            born_memories: BTreeSet::new(),
            affected_recipients: BTreeSet::new(),
            source_nodes: BTreeSet::new(),
            base_cut: 0,
            settlement_cut: 0,
            accepted_events: BTreeMap::new(),
            superseded: BTreeSet::new(),
        },
        corpus_digest: [6; 32],
        manifest_digest: [7; 32],
        input_digest: [8; 32],
        global_version: 0,
        archive_version: 0,
        graph_version: 0,
        memory_graph_version: 0,
        cohort: BTreeSet::new(),
        records: BTreeMap::new(),
    }
}

fn resign(bytes: &mut Vec<u8>) {
    bytes.truncate(bytes.len() - 32);
    let checksum = Sha256::digest(&*bytes);
    bytes.extend(checksum);
}

#[derive(Default)]
struct EncodedSections {
    predecessor_tag: usize,
    born_count: usize,
    born_ids: Vec<std::ops::Range<usize>>,
    source_pairs: Vec<std::ops::Range<usize>>,
    event_entries: Vec<std::ops::Range<usize>>,
    record_entries: Vec<std::ops::Range<usize>>,
    first_source_text_length: usize,
}

fn encoded_sections(payload: &[u8]) -> EncodedSections {
    let end = payload.len() - 32;
    let mut r = super::super::Reader {
        bytes: &payload[..end],
        at: 8,
    };
    r.take(16 + 7 * 32 + 6 * 8).unwrap();
    let mut sections = EncodedSections {
        predecessor_tag: r.at,
        ..Default::default()
    };
    match r.u8().unwrap() {
        0 => {}
        1 => {
            r.take(32).unwrap();
        }
        _ => panic!("fixture has invalid predecessor tag"),
    }
    sections.born_count = r.at;
    for _ in 0..r.u32().unwrap() {
        let start = r.at;
        r.take(32).unwrap();
        sections.born_ids.push(start..r.at);
    }
    for _ in 0..r.u32().unwrap() {
        r.take(32).unwrap();
    }
    for index in 0..r.u32().unwrap() {
        let start = r.at;
        if index == 0 {
            sections.first_source_text_length = r.at;
        }
        r.bytes().unwrap();
        r.bytes().unwrap();
        sections.source_pairs.push(start..r.at);
    }
    for _ in 0..r.u32().unwrap() {
        let start = r.at;
        r.bytes().unwrap();
        r.take(32 + 8).unwrap();
        for _ in 0..r.u32().unwrap() {
            r.take(32).unwrap();
        }
        sections.event_entries.push(start..r.at);
    }
    for _ in 0..r.u32().unwrap() {
        r.bytes().unwrap();
    }
    for _ in 0..r.u32().unwrap() {
        r.take(32).unwrap();
    }
    for _ in 0..r.u32().unwrap() {
        let start = r.at;
        r.take(32).unwrap();
        r.i16().unwrap();
        r.optional_turn().unwrap();
        r.optional_turn().unwrap();
        sections.record_entries.push(start..r.at);
    }
    assert_eq!(r.at, end);
    sections
}

fn swap_ranges(bytes: &mut [u8], first: &std::ops::Range<usize>, second: &std::ops::Range<usize>) {
    assert_eq!(first.len(), second.len());
    let first_bytes = bytes[first.clone()].to_vec();
    let second_bytes = bytes[second.clone()].to_vec();
    bytes[first.clone()].copy_from_slice(&second_bytes);
    bytes[second.clone()].copy_from_slice(&first_bytes);
}

fn assert_decode_error(bytes: &[u8], expected: &str) {
    let error = SemanticImportFreshnessSettlement::decode(bytes).unwrap_err();
    assert!(
        error.contains(expected),
        "expected {expected:?}, got {error:?}"
    );
}

#[test]
fn standalone_sparse_settlement_roundtrips() {
    {
        let settlement = fixture();
        let bytes = settlement.encode().unwrap();
        assert!(bytes.starts_with(SEMANTIC_SETTLEMENT_MAGIC));
        assert_eq!(
            SemanticImportFreshnessSettlement::decode(&bytes).unwrap(),
            settlement
        );
    }
}

#[test]
fn empty_zero_activity_settlement_roundtrips() {
    let settlement = empty_zero_activity();
    assert_eq!(
        SemanticImportFreshnessSettlement::decode(&settlement.encode().unwrap()).unwrap(),
        settlement
    );
}

#[test]
fn checksum_truncation_and_trailing_bytes_are_rejected() {
    let bytes = fixture().encode().unwrap();
    let mut bad_checksum = bytes.clone();
    bad_checksum[20] ^= 1;
    assert!(SemanticImportFreshnessSettlement::decode(&bad_checksum).is_err());

    for length in [0, 7, 39, bytes.len() - 1] {
        assert!(SemanticImportFreshnessSettlement::decode(&bytes[..length]).is_err());
    }

    let mut trailing = bytes;
    trailing.splice(trailing.len() - 32..trailing.len() - 32, [0xAA]);
    resign(&mut trailing);
    assert!(SemanticImportFreshnessSettlement::decode(&trailing).is_err());
}

#[test]
fn authenticated_unsorted_and_duplicate_ids_are_rejected() {
    let bytes = fixture().encode().unwrap();
    let sections = encoded_sections(&bytes);

    let mut unsorted = bytes.clone();
    swap_ranges(&mut unsorted, &sections.born_ids[0], &sections.born_ids[1]);
    resign(&mut unsorted);
    assert_decode_error(&unsorted, "unordered or duplicate semantic import identity");

    let mut duplicate = bytes;
    let first = duplicate[sections.born_ids[0].clone()].to_vec();
    duplicate[sections.born_ids[1].clone()].copy_from_slice(&first);
    resign(&mut duplicate);
    assert_decode_error(
        &duplicate,
        "unordered or duplicate semantic import identity",
    );
}

#[test]
fn authenticated_event_source_and_record_order_are_rejected() {
    let bytes = fixture().encode().unwrap();
    let sections = encoded_sections(&bytes);

    let mut events = bytes.clone();
    swap_ranges(
        &mut events,
        &sections.event_entries[0],
        &sections.event_entries[1],
    );
    resign(&mut events);
    assert_decode_error(&events, "unordered or duplicate accepted import event");

    let mut sources = bytes.clone();
    swap_ranges(
        &mut sources,
        &sections.source_pairs[0],
        &sections.source_pairs[1],
    );
    resign(&mut sources);
    assert_decode_error(&sources, "unordered or duplicate semantic import identity");

    let mut records = bytes;
    swap_ranges(
        &mut records,
        &sections.record_entries[0],
        &sections.record_entries[1],
    );
    resign(&mut records);
    assert_decode_error(&records, "unordered or duplicate semantic import record");
}

#[test]
fn authenticated_invalid_predecessor_policy_and_bounded_lengths_are_rejected() {
    let bytes = fixture().encode().unwrap();
    let sections = encoded_sections(&bytes);

    let mut predecessor = bytes.clone();
    predecessor[sections.predecessor_tag] = 2;
    resign(&mut predecessor);
    assert_decode_error(
        &predecessor,
        "invalid semantic import predecessor reserved tag",
    );

    let mut policy = bytes.clone();
    let historical_policy_digest = 8 + 16 + 5 * 32;
    policy[historical_policy_digest] ^= 1;
    resign(&mut policy);
    assert_decode_error(&policy, "unsupported semantic import policy");

    let mut huge_count = bytes.clone();
    huge_count[sections.born_count..sections.born_count + 4]
        .copy_from_slice(&u32::MAX.to_le_bytes());
    resign(&mut huge_count);
    assert_decode_error(&huge_count, "semantic import count exceeds remaining bytes");

    let mut truncated_extent = bytes;
    truncated_extent[sections.first_source_text_length..sections.first_source_text_length + 4]
        .copy_from_slice(&u32::MAX.to_le_bytes());
    resign(&mut truncated_extent);
    assert_decode_error(&truncated_extent, "truncated Freshness record");
}

#[test]
fn inconsistent_settlement_identity_cuts_cohort_membership_and_records_are_rejected() {
    let mut cases = Vec::new();

    let mut value = fixture();
    value.scope.owner = [0; 16];
    cases.push(value);
    let mut value = fixture();
    value.scope.import_id = [0; 32];
    cases.push(value);
    let mut value = fixture();
    value.scope.base_cut = 13;
    cases.push(value);
    let mut value = fixture();
    value.archive_version = value.global_version + 1;
    cases.push(value);
    let mut value = fixture();
    value.graph_version = value.global_version + 1;
    cases.push(value);
    let mut value = fixture();
    value.memory_graph_version = value.graph_version + 1;
    cases.push(value);
    let mut value = fixture();
    value.cohort.remove(&id(3));
    cases.push(value);
    let mut value = fixture();
    value.scope.superseded.insert("unknown-event".into());
    cases.push(value);
    let mut value = fixture();
    value.records.get_mut(&id(2)).unwrap().score = -9999;
    cases.push(value);

    for invalid in cases {
        assert!(invalid.encode().is_err());
    }
}

fn semantic_event() -> super::super::JournalEntry {
    let owner = [1; 16];
    let recipient = id(2);
    super::super::JournalEntry::Event {
        event_id: "event-1".into(),
        turn: 12,
        provenance: super::super::FreshnessEventProvenance {
            producer_kind: super::super::SEMANTIC_EVENT_PRODUCER_KIND,
            source_id: "event-1".into(),
            origin_owner: owner,
            accepted_turn: 12,
            policy_version: 1,
        },
        proof: FreshnessEventProof {
            graph_version: 3,
            community_generation: Some(4),
            community_graph_version: Some(3),
        },
        effects: BTreeMap::from([(recipient, 8)]),
        records: BTreeMap::from([(recipient, record_at(12))]),
        semantic: Some(super::super::SemanticEventEvidence {
            owner_uuid: owner,
            fragment: None,
            conversation_id: "conversation".into(),
            node_id: "node".into(),
            accepted_turn: 12,
            base_policy_digest: [5; 32],
            semantic_policy_version: 1,
            semantic_policy_digest: [6; 32],
            compatibility_profile_id: [7; 32],
            source_evidence_digest: [8; 32],
            target_evidence_digest: [9; 32],
        }),
    }
}

#[test]
fn accepted_event_digest_binds_immutable_inputs_but_not_derived_records() {
    let baseline = AcceptedImportEventEvidence::from_event([1; 16], &semantic_event()).unwrap();

    let mut changed = semantic_event();
    if let super::super::JournalEntry::Event { effects, .. } = &mut changed {
        effects.insert(id(2), 9);
    }
    assert_ne!(
        baseline.payload_digest,
        AcceptedImportEventEvidence::from_event([1; 16], &changed)
            .unwrap()
            .payload_digest
    );

    let mut changed = semantic_event();
    if let super::super::JournalEntry::Event { provenance, .. } = &mut changed {
        provenance.source_id = "different-source".into();
    }
    assert_ne!(
        baseline.payload_digest,
        AcceptedImportEventEvidence::from_event([1; 16], &changed)
            .unwrap()
            .payload_digest
    );

    let mut changed = semantic_event();
    if let super::super::JournalEntry::Event { proof, .. } = &mut changed {
        proof.graph_version += 1;
        proof.community_graph_version = Some(proof.graph_version);
    }
    assert_ne!(
        baseline.payload_digest,
        AcceptedImportEventEvidence::from_event([1; 16], &changed)
            .unwrap()
            .payload_digest
    );

    let mut changed = semantic_event();
    if let super::super::JournalEntry::Event { semantic, .. } = &mut changed {
        semantic.as_mut().unwrap().source_evidence_digest[0] ^= 1;
    }
    assert_ne!(
        baseline.payload_digest,
        AcceptedImportEventEvidence::from_event([1; 16], &changed)
            .unwrap()
            .payload_digest
    );

    let mut changed = semantic_event();
    if let super::super::JournalEntry::Event { records, .. } = &mut changed {
        records.get_mut(&id(2)).unwrap().score -= 1;
    }
    assert_eq!(
        baseline.payload_digest,
        AcceptedImportEventEvidence::from_event([1; 16], &changed)
            .unwrap()
            .payload_digest
    );
}

#[test]
fn accepted_event_digest_rejects_non_events_and_semantic_kind_mismatches() {
    let non_event = super::super::JournalEntry::Initialize {
        memory_id: id(2),
        turn: 12,
        record: record_at(12),
    };
    assert!(AcceptedImportEventEvidence::from_event([1; 16], &non_event).is_err());

    let mut missing_evidence = semantic_event();
    if let super::super::JournalEntry::Event {
        provenance,
        semantic,
        ..
    } = &mut missing_evidence
    {
        provenance.producer_kind = super::super::SEMANTIC_EVENT_PRODUCER_KIND;
        *semantic = None;
    }
    assert!(AcceptedImportEventEvidence::from_event([1; 16], &missing_evidence).is_err());

    let mut unexpected_evidence = semantic_event();
    if let super::super::JournalEntry::Event { provenance, .. } = &mut unexpected_evidence {
        provenance.producer_kind = 1;
    }
    assert!(AcceptedImportEventEvidence::from_event([1; 16], &unexpected_evidence).is_err());
}
