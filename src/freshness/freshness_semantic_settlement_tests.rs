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
    let event = |event_id: &str, digest: u8| AcceptedImportEventEvidence {
        event_id: event_id.into(),
        payload_digest: [digest; 32],
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
                ("semantic-event-1".into(), event("semantic-event-1", 9)),
                ("semantic-event-2".into(), event("semantic-event-2", 10)),
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
        records: BTreeMap::from([(first, record_at(10)), (second, record_at(10))]),
    }
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
    value.records.get_mut(&id(3)).unwrap().score = -9999;
    cases.push(value);

    for invalid in cases {
        assert!(invalid.validate().is_err());
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
