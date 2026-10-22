//! Pure semantic serializers for Freshness historical fingerprint rows.
use crate::freshness::{FreshnessPolicy, FreshnessRecord};
use crate::freshness_storage::{
    AcceptedImportEventEvidence, FreshnessEventProvenance, FreshnessReplayEntry,
    FreshnessRewriteBaseline, HistoricalImportFreshnessSettlement, ImportEventScope,
    ImportSemanticScope, SemanticEventEvidence, SemanticFragmentEvidence,
};
use crate::{FreshnessEventProof, MemoryId};
use serde_json::{Value, json};

pub(super) fn entry_value(entry: &FreshnessReplayEntry) -> (String, Vec<u8>, Value) {
    match entry {
        FreshnessReplayEntry::RewriteBaseline { baseline } => (
            "rewrite_baseline".into(),
            baseline.memory_id.0.to_vec(),
            baseline_value(baseline),
        ),
        FreshnessReplayEntry::Initialize {
            memory_id,
            source_turn,
            admitted_at_birth,
        } => (
            "initialize".into(),
            memory_id.0.to_vec(),
            json!({
                "memory_id": memory_id.0,
                "source_turn": source_turn,
                "admitted_at_birth": admitted_at_birth,
            }),
        ),
        FreshnessReplayEntry::Admit {
            memory_id,
            source_turn,
        } => (
            "admit".into(),
            memory_id.0.to_vec(),
            json!({"memory_id": memory_id.0, "source_turn": source_turn}),
        ),
        FreshnessReplayEntry::Event {
            event_id,
            source_turn,
            effects,
            proof,
            provenance,
            semantic,
        } => (
            "event".into(),
            event_id.as_bytes().to_vec(),
            json!({
                "event_id": event_id,
                "source_turn": source_turn,
                "effects": effects.iter().map(|(id, effect)| json!([id.0, effect])).collect::<Vec<_>>(),
                "proof": proof_value(proof),
                "provenance": provenance.as_ref().map(provenance_value),
                "semantic": semantic.as_ref().map(semantic_value),
            }),
        ),
        FreshnessReplayEntry::AcceptedUseReceipt {
            use_id,
            source_turn,
            sources,
            policy_version,
        } => (
            "use_receipt".into(),
            use_id.as_bytes().to_vec(),
            json!({
                "use_id": use_id,
                "source_turn": source_turn,
                "sources": sources.iter().map(|id| id.0).collect::<Vec<_>>(),
                "policy_version": policy_version,
            }),
        ),
    }
}

pub(super) fn settlement_value(settlement: &HistoricalImportFreshnessSettlement) -> Value {
    let HistoricalImportFreshnessSettlement {
        epoch,
        semantic,
        epoch_prefix_digest: _, // Physical frozen-prefix evidence is excluded by the semantic fingerprint schema.
        owner,
        import_id,
        start_evidence_digest,
        corpus_digest,
        manifest_digest,
        input_digest,
        cohort,
        base_cut,
        settlement_cut,
        global_version,
        input_bytes: _, // Physical input length is not semantic acceptance evidence.
        archive_version,
        graph_version,
        memory_graph_version,
        records,
        superseded,
    } = settlement;
    json!({
        "kind": "import_settlement",
        "epoch_scope": epoch.as_ref().map(epoch_scope_value),
        "semantic_scope": semantic.as_ref().map(semantic_scope_value),
        "owner": owner,
        "import_id": import_id,
        "start_evidence_digest": start_evidence_digest,
        "corpus_digest": corpus_digest,
        "manifest_digest": manifest_digest,
        "input_digest": input_digest,
        "cohort": memory_ids(cohort),
        "base_cut": base_cut,
        "settlement_cut": settlement_cut,
        "global_version": global_version,
        "archive_version": archive_version,
        "graph_version": graph_version,
        "memory_graph_version": memory_graph_version,
        "records": records.iter().map(|(id, record)| json!([id.0, freshness_record_value(record)])).collect::<Vec<_>>(),
        "superseded": superseded.iter().collect::<Vec<_>>(),
    })
}

pub(super) fn record_value(record: &FreshnessRecord) -> Value {
    freshness_record_value(record)
}

pub(super) fn policy_value(policy: &FreshnessPolicy) -> Value {
    let FreshnessPolicy {
        decay_turns_per_point,
        access_principal,
        linkage_principal,
        local_hop_cost,
        outside_hop_cost,
    } = *policy;
    json!({
        "policy_version": crate::freshness::FRESHNESS_POLICY_VERSION,
        "decay_turns_per_point": decay_turns_per_point,
        "access_principal": access_principal,
        "linkage_principal": linkage_principal,
        "local_hop_cost": local_hop_cost,
        "outside_hop_cost": outside_hop_cost,
    })
}

fn epoch_scope_value(scope: &ImportEventScope) -> Value {
    let ImportEventScope {
        owner,
        import_id,
        start_evidence_digest,
        born_memories,
        affected_recipients,
        source_nodes,
        start_prefix: _,
        freeze_prefix: _,
        base_cut,
        freeze_cut,
    } = scope;
    // Byte prefix bounds authenticate a physical legacy source interval and are
    // validated separately. They are not canonical owner state locations.
    json!({"owner": owner, "import_id": import_id, "start_evidence_digest": start_evidence_digest,
        "born_memories": memory_ids(born_memories), "affected_recipients": memory_ids(affected_recipients),
        "source_nodes": source_nodes.iter().map(|(conversation, node)| json!([conversation, node])).collect::<Vec<_>>(),
        "base_cut": base_cut, "freeze_cut": freeze_cut})
}

fn baseline_value(baseline: &FreshnessRewriteBaseline) -> Value {
    let FreshnessRewriteBaseline {
        owner_uuid,
        history_generation,
        memory_id,
        source_cut,
        source_initial_decay_carry,
        destination_cut,
        source_record,
        effective_record,
        destination_record,
        decay_remainder,
        policy_digest,
    } = baseline;
    json!({
        "owner_uuid": owner_uuid,
        "history_generation": history_generation.as_bytes(),
        "memory_id": memory_id.0,
        "source_cut": source_cut,
        "source_initial_decay_carry": source_initial_decay_carry,
        "destination_cut": destination_cut,
        "source_record": freshness_record_value(source_record),
        "effective_record": freshness_record_value(effective_record),
        "destination_record": freshness_record_value(destination_record),
        "decay_remainder": decay_remainder,
        "policy_digest": policy_digest,
    })
}

fn freshness_record_value(record: &FreshnessRecord) -> Value {
    let FreshnessRecord {
        score,
        admitted_at_turn,
        accounted_turn,
    } = *record;
    json!({
        "score": score,
        "admitted_at_turn": admitted_at_turn,
        "accounted_turn": accounted_turn,
    })
}

fn proof_value(proof: &FreshnessEventProof) -> Value {
    let FreshnessEventProof {
        graph_version,
        community_generation,
        community_graph_version,
    } = *proof;
    json!({
        "graph_version": graph_version,
        "community_generation": community_generation,
        "community_graph_version": community_graph_version,
    })
}

fn provenance_value(provenance: &FreshnessEventProvenance) -> Value {
    let FreshnessEventProvenance {
        producer_kind,
        source_id,
        origin_owner,
        accepted_turn,
        policy_version,
    } = provenance;
    json!({
        "producer_kind": producer_kind,
        "source_id": source_id,
        "origin_owner": origin_owner,
        "accepted_turn": accepted_turn,
        "policy_version": policy_version,
    })
}

fn semantic_value(semantic: &SemanticEventEvidence) -> Value {
    let SemanticEventEvidence {
        owner_uuid,
        fragment,
        conversation_id,
        node_id,
        accepted_turn,
        base_policy_digest,
        semantic_policy_version,
        semantic_policy_digest,
        compatibility_profile_id,
        source_evidence_digest,
        target_evidence_digest,
    } = semantic;
    json!({
        "owner_uuid": owner_uuid,
        "fragment": fragment.as_ref().map(fragment_value),
        "conversation_id": conversation_id,
        "node_id": node_id,
        "accepted_turn": accepted_turn,
        "base_policy_digest": base_policy_digest,
        "semantic_policy_version": semantic_policy_version,
        "semantic_policy_digest": semantic_policy_digest,
        "compatibility_profile_id": compatibility_profile_id,
        "source_evidence_digest": source_evidence_digest,
        "target_evidence_digest": target_evidence_digest,
    })
}

fn fragment_value(fragment: &SemanticFragmentEvidence) -> Value {
    let SemanticFragmentEvidence {
        fragment_id,
        sources,
    } = fragment;
    json!({
        "fragment_id": fragment_id.0,
        "sources": sources.iter().map(|(conversation, node)| json!([conversation, node])).collect::<Vec<_>>(),
    })
}

fn semantic_scope_value(scope: &ImportSemanticScope) -> Value {
    let ImportSemanticScope {
        owner,
        import_id,
        start_evidence_digest,
        born_memories,
        affected_recipients,
        source_nodes,
        base_cut,
        settlement_cut,
        accepted_events,
        superseded,
    } = scope;
    json!({
        "owner": owner,
        "import_id": import_id,
        "start_evidence_digest": start_evidence_digest,
        "born_memories": memory_ids(born_memories),
        "affected_recipients": memory_ids(affected_recipients),
        "source_nodes": source_nodes.iter().map(|(conversation, node)| json!([conversation, node])).collect::<Vec<_>>(),
        "base_cut": base_cut,
        "settlement_cut": settlement_cut,
        "accepted_events": accepted_events.iter().map(|(id, evidence)| json!([id, accepted_event_value(evidence)])).collect::<Vec<_>>(),
        "superseded": superseded.iter().collect::<Vec<_>>(),
        "predecessor_digest": Value::Null,
    })
}

fn accepted_event_value(evidence: &AcceptedImportEventEvidence) -> Value {
    let AcceptedImportEventEvidence {
        event_id,
        payload_digest,
        accepted_turn,
        affected_recipients,
    } = evidence;
    json!({
        "event_id": event_id,
        "payload_digest": payload_digest,
        "accepted_turn": accepted_turn,
        "affected_recipients": memory_ids(affected_recipients),
    })
}

fn memory_ids(ids: &std::collections::BTreeSet<MemoryId>) -> Vec<[u8; 32]> {
    ids.iter().map(|id| id.0).collect()
}
