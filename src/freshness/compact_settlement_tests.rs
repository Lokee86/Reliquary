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

/// Temporary C3 proof over hash-pinned before-images. This never opens a writer
/// on a frozen file and never promotes a development corpus.
#[test]
#[ignore = "explicit Phase 3 frozen causal codec measurement"]
fn frozen_compact_settlements_match_exact_original_scope_and_records() {
    use crate::freshness::historical_contract::HistoricalScoreEvent;
    use serde_json::{Value, json};
    use std::{fs, path::Path, time::Instant};
    fn bytes<const N: usize>(text: &str) -> [u8; N] {
        assert_eq!(text.len(), N * 2);
        let mut out = [0; N];
        for (i, value) in out.iter_mut().enumerate() {
            *value = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap();
        }
        out
    }
    fn record(v: &Value) -> FreshnessRecord {
        FreshnessRecord {
            score: v["score"].as_i64().unwrap().try_into().unwrap(),
            admitted_at_turn: v["admitted_at_turn"].as_u64(),
            accounted_turn: v["accounted_turn"].as_u64(),
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let artifact = fs::read(
        root.join(".verification-tmp/phase3-s3-regeneration/phase3-s3-regeneration-audit.json"),
    )
    .unwrap();
    let audit: Value = serde_json::from_slice(&artifact).unwrap();
    let mut outcomes = Vec::new();
    let expected_sizes = [310786usize, 693922, 113404];
    for (index, f) in audit["fixtures"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            f["original_event_leaves_validated_against_prefix_journal"],
            true
        );
        let name = f["fixture"].as_str().unwrap();
        let path = root.join(".verification-tmp/phase3-before").join(name);
        let source = fs::read(&path).unwrap();
        assert_eq!(
            Sha256::digest(&source).as_slice(),
            bytes::<32>(f["source_sha256"].as_str().unwrap())
        );
        let offset = f["settlement_frame_offset"].as_u64().unwrap() as usize;
        let length = u64::from_le_bytes(source[offset..offset + 8].try_into().unwrap()) as usize;
        let original = &source[offset + 8..offset + 8 + length];
        assert_eq!(
            Sha256::digest(original).as_slice(),
            bytes::<32>(f["original_settlement_sha256"].as_str().unwrap())
        );
        let value =
            super::super::semantic_settlement::SemanticImportFreshnessSettlement::decode(original)
                .unwrap();
        let initial = f["causal_basis"]["initial_records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    MemoryId(bytes(r["memory_id"].as_str().unwrap())),
                    record(&r["value"]),
                )
            })
            .collect();
        let events: Vec<_> = f["causal_basis"]["ordered_scorer_rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| HistoricalScoreEvent {
                recipient: MemoryId(bytes(r["recipient"].as_str().unwrap())),
                position: r["position"].as_u64().unwrap(),
                event_id: bytes(r["event_id"].as_str().unwrap()),
                amount: r["amount"].as_i64().unwrap().try_into().unwrap(),
            })
            .collect();
        let ranked =
            HistoricalCausalBasis::from_events(&initial, &events, value.scope.settlement_cut)
                .unwrap();
        let corrections = f["residual_records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                let old = record(&r["original"]);
                let new = record(&r["regenerated"]);
                assert_eq!(
                    (old.admitted_at_turn, old.accounted_turn),
                    (new.admitted_at_turn, new.accounted_turn)
                );
                (
                    MemoryId(bytes(r["memory_id"].as_str().unwrap())),
                    old.score - new.score,
                )
            })
            .collect();
        let basis = HistoricalCausalBasis::from_canonical(
            initial,
            ranked.contributions().to_vec(),
            corrections,
            value.scope.settlement_cut,
        )
        .unwrap();
        let start = Instant::now();
        let payload = encode(&value, &basis).unwrap();
        let encode_ns = start.elapsed().as_nanos();
        assert_eq!(payload.len(), expected_sizes[index]);
        let start = Instant::now();
        let (decoded, decoded_basis) = decode(&payload)
            .unwrap()
            .resolve(&value.scope.accepted_events)
            .unwrap();
        let decode_ns = start.elapsed().as_nanos();
        assert_eq!(decoded, value);
        assert_eq!(decoded_basis, basis);
        assert_eq!(decoded.encode().unwrap(), original);
        assert_eq!(
            Sha256::digest(fs::read(&path).unwrap()).as_slice(),
            bytes::<32>(f["source_sha256"].as_str().unwrap())
        );
        outcomes.push(json!({"fixture":name,"original_payload_bytes":original.len(),"compact_payload_bytes":payload.len(),"encode_ns":encode_ns,"decode_ns":decode_ns,"exact_original_payload_roundtrip":true,"exact_basis_roundtrip":true,"new_payload_sha256":format!("{:x}",Sha256::digest(&payload))}));
    }
    let report = json!({"status":"rust_codec_prototype_not_runtime_or_conversion_acceptance","basis_audit_sha256":format!("{:x}",Sha256::digest(&artifact)),"fixtures":outcomes});
    fs::write(
        root.join(".verification-tmp/phase3-c3-rust-compact-measurement.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}
