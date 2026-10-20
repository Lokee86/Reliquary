//! Contract checks for the existing effects-only CVAFRS02 event payload.
use super::*;
use std::collections::BTreeMap;

fn fixture_store() -> (FreshnessStore, [u8; 16], MemoryId, MemoryId) {
    let owner = [0x17; 16];
    let source = MemoryId([0x21; 32]);
    let target = MemoryId([0x32; 32]);
    let mut store = FreshnessStore::new(Some(owner));
    store
        .ingest(&encode_policy(owner, FreshnessPolicy::default_v1()))
        .unwrap();
    for id in [source, target] {
        let record = FreshnessRecord::created();
        store
            .ingest(&store.encode_initialize(owner, id, 0, record))
            .unwrap();
    }
    (store, owner, source, target)
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn expected_semantic_frame(
    owner: [u8; 16],
    event_id: &str,
    turn: u64,
    source: &str,
    evidence: &SemanticEventEvidence,
    recipient: MemoryId,
    effect: i16,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"CVAFRS02");
    out.extend_from_slice(&owner);
    out.push(2); // Event
    push_bytes(&mut out, event_id.as_bytes());
    out.extend_from_slice(&turn.to_le_bytes());
    out.push(SEMANTIC_EVENT_PRODUCER_KIND);
    push_bytes(&mut out, source.as_bytes());
    out.extend_from_slice(&owner); // origin owner
    out.extend_from_slice(&turn.to_le_bytes()); // provenance accepted turn
    out.extend_from_slice(&(crate::freshness::FRESHNESS_POLICY_VERSION as u64).to_le_bytes());
    out.extend_from_slice(&evidence.owner_uuid);
    push_bytes(&mut out, evidence.conversation_id.as_bytes());
    push_bytes(&mut out, evidence.node_id.as_bytes());
    out.extend_from_slice(&evidence.accepted_turn.to_le_bytes());
    out.extend_from_slice(&evidence.base_policy_digest);
    out.extend_from_slice(&evidence.semantic_policy_version.to_le_bytes());
    out.extend_from_slice(&evidence.semantic_policy_digest);
    out.extend_from_slice(&evidence.compatibility_profile_id);
    out.extend_from_slice(&evidence.source_evidence_digest);
    out.extend_from_slice(&evidence.target_evidence_digest);
    out.extend_from_slice(&0_u64.to_le_bytes()); // graph version
    out.push(0); // no Community generation
    out.push(0); // no Community graph version
    out.extend_from_slice(&1_u32.to_le_bytes()); // one recipient effect
    out.extend_from_slice(&recipient.0);
    out.extend_from_slice(&effect.to_le_bytes());
    out
}

fn semantic_evidence(owner: [u8; 16], turn: u64) -> SemanticEventEvidence {
    SemanticEventEvidence {
        owner_uuid: owner,
        fragment: None,
        conversation_id: "conversation-wire".into(),
        node_id: "node-wire".into(),
        accepted_turn: turn,
        base_policy_digest: [0x41; 32],
        semantic_policy_version: 3,
        semantic_policy_digest: [0x42; 32],
        compatibility_profile_id: [0x43; 32],
        source_evidence_digest: [0x44; 32],
        target_evidence_digest: [0x45; 32],
    }
}

#[test]
fn literal_event_wire_preserves_accepted_fields_and_omits_all_postimages() {
    let (store, owner, _source, target) = fixture_store();
    let turn = 9;
    let evidence = semantic_evidence(owner, turn);
    let event_id = evidence.event_id();
    let effect = store.policy.linkage_principal.min(7);
    let effects = BTreeMap::from([(target, effect)]);
    let payload = store
        .encode_semantic_event(owner, evidence.clone(), effects.clone())
        .unwrap();
    let expected =
        expected_semantic_frame(owner, &event_id, turn, &event_id, &evidence, target, effect);
    assert_eq!(
        payload, expected,
        "wire fixture covers every persisted event field"
    );

    let (_, decoded) = decode(&payload).unwrap();
    let JournalEntry::Event {
        event_id: decoded_id,
        turn: decoded_turn,
        provenance,
        proof,
        effects: decoded_effects,
        records,
        semantic,
    } = decoded
    else {
        panic!("decoded payload was not an Event");
    };
    assert_eq!(decoded_id, event_id);
    assert_eq!(decoded_turn, turn);
    assert_eq!(
        provenance,
        FreshnessEventProvenance {
            producer_kind: SEMANTIC_EVENT_PRODUCER_KIND,
            source_id: event_id.clone(),
            origin_owner: owner,
            accepted_turn: turn,
            policy_version: crate::freshness::FRESHNESS_POLICY_VERSION as u64,
        }
    );
    assert_eq!(
        proof,
        FreshnessEventProof {
            graph_version: 0,
            community_generation: None,
            community_graph_version: None,
        }
    );
    assert_eq!(decoded_effects, effects);
    assert_eq!(semantic, Some(evidence));
    assert!(
        records.is_empty(),
        "decoded event carries no derived postimages"
    );

    // Even a different in-memory postimage map cannot alter the wire payload.
    let alternate = JournalEntry::Event {
        event_id: decoded_id,
        turn: decoded_turn,
        provenance,
        proof,
        effects: decoded_effects,
        records: BTreeMap::from([(
            target,
            FreshnessRecord {
                score: -91,
                admitted_at_turn: Some(0),
                accounted_turn: Some(0),
            },
        )]),
        semantic,
    };
    assert_eq!(encode(owner, &alternate), payload);
}

#[test]
fn effect_only_replay_recomputes_exact_projection_and_preappend_rejects_forged_postimage() {
    let (store, owner, source, target) = fixture_store();
    let id = format!("dream-initial:{}:{}", hex_bytes(&owner), hex_id(&source));
    let turn = 25;
    let proof = FreshnessEventProof {
        graph_version: 0,
        community_generation: None,
        community_graph_version: None,
    };
    let effects = BTreeMap::from([(target, store.policy.linkage_principal)]);
    let expected = store
        .prepare_event_postimages_for(&id, turn, &effects)
        .unwrap();
    let provenance = FreshnessEventProvenance {
        producer_kind: 1,
        source_id: id.clone(),
        origin_owner: owner,
        accepted_turn: turn,
        policy_version: crate::freshness::FRESHNESS_POLICY_VERSION as u64,
    };
    let payload = store
        .encode_event_with_provenance(
            owner,
            id.clone(),
            turn,
            proof,
            effects.clone(),
            expected.clone(),
            Some(provenance.clone()),
        )
        .unwrap();
    let mut replay = store.clone();
    replay.ingest(&payload).unwrap();
    assert_eq!(replay.records[&target], expected[&target]);
    assert_eq!(replay.records[&source], store.records[&source]);
    let decoded = decode(&payload).unwrap().1;
    let JournalEntry::Event { records, .. } = decoded else {
        unreachable!()
    };
    assert!(records.is_empty());

    let mut forged = expected.clone();
    forged.get_mut(&target).unwrap().score -= 1;
    assert!(store.encode_event_with_provenance(
        owner, id, turn, proof, effects, forged, Some(provenance),
    ).is_err(), "preappend must reject a false supplied postimage before it is omitted");
}

#[test]
fn truncated_trailing_and_malformed_event_fields_fail_closed() {
    let (store, owner, _source, target) = fixture_store();
    let turn = 9;
    let evidence = semantic_evidence(owner, turn);
    let id = evidence.event_id();
    let payload = store
        .encode_semantic_event(
            owner,
            evidence.clone(),
            BTreeMap::from([(target, store.policy.linkage_principal.min(7))]),
        )
        .unwrap();

    for end in 0..payload.len() {
        assert!(decode(&payload[..end]).is_err(), "truncation at byte {end}");
    }
    let mut trailing = payload.clone();
    trailing.push(0);
    assert!(decode(&trailing).unwrap_err().contains("trailing bytes"));

    let mut bad_kind = payload.clone();
    let id_len = u32::from_le_bytes(bad_kind[25..29].try_into().unwrap()) as usize;
    let producer_kind = 8 + 16 + 1 + 4 + id_len + 8;
    bad_kind[producer_kind] = 0;
    assert!(decode(&bad_kind).is_err(), "unknown producer kind");

    // The proof's Community option tags follow the variable event/provenance/
    // semantic fields. Invalid optional tags are structural wire errors.
    let mut cursor = 8 + 16 + 1;
    let event_id_len = u32::from_le_bytes(payload[cursor..cursor + 4].try_into().unwrap()) as usize;
    cursor += 4 + event_id_len + 8;
    cursor += 1;
    let source_len = u32::from_le_bytes(payload[cursor..cursor + 4].try_into().unwrap()) as usize;
    cursor += 4 + source_len + 16 + 8 + 8;
    cursor += 16;
    let conversation_len =
        u32::from_le_bytes(payload[cursor..cursor + 4].try_into().unwrap()) as usize;
    cursor += 4 + conversation_len;
    let node_len = u32::from_le_bytes(payload[cursor..cursor + 4].try_into().unwrap()) as usize;
    cursor += 4 + node_len + 8 + 32 + 2 + 4 * 32;
    cursor += 8;
    let mut bad_optional = payload.clone();
    bad_optional[cursor] = 2;
    assert!(decode(&bad_optional).is_err(), "invalid proof option tag");

    // Accepted semantic evidence is also revalidated at owner ingest, even
    // where its UTF-8/layout remains structurally decodable.
    let mut wrong_semantic_owner = payload;
    // Semantic owner UUID starts after fixed event and provenance plus the
    // producer's semantic-evidence prefix (no fragment for producer kind 3).
    let mut pos = 8 + 16 + 1;
    let n = u32::from_le_bytes(wrong_semantic_owner[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4 + n + 8 + 1;
    let n = u32::from_le_bytes(wrong_semantic_owner[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4 + n + 16 + 8 + 8;
    wrong_semantic_owner[pos] ^= 0xff;
    let mut replay = store;
    assert!(replay.ingest(&wrong_semantic_owner).is_err());
    assert_eq!(id, evidence.event_id());
}

#[test]
fn unsupported_effect_event_version_is_rejected_at_owner_boundary() {
    let (mut store, owner, _, target) = fixture_store();
    let evidence = semantic_evidence(owner, 9);
    let mut payload = store
        .encode_semantic_event(owner, evidence, BTreeMap::from([(target, 7)]))
        .unwrap();
    payload[..8].copy_from_slice(b"CVAFRS99");
    assert!(store.ingest(&payload).unwrap_err().contains("unsupported"));
}
