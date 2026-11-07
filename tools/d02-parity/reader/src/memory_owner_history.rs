use crate::{
    CandidateKind,
    cut::{fields, observe_cut},
    evidence_encoding::{hex, sha256},
};
use reliquary_memory_core::{MemoryRecord, MemorySourceRef, MemoryTemporalInference, PayloadState};
use reliquary_owner_session::{MemoryOwnerHistoryObservation, MemoryPublicationOrigin};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

use crate::memory_history::temporal_kind;

pub fn observe_memory_owner_history(kind: CandidateKind, path: &Path) -> Value {
    let report = json!({
        "schema_id": "reliquary-d02-memory-owner-history-v1",
        "status": "INCOMPLETE",
        "observation_status": "INCOMPLETE",
        "owner": null,
        "candidate_sha256_before": null,
        "candidate_sha256_after": null,
        "candidate_unchanged": null,
        "memory_head": null,
        "rows": null,
        "reason": "",
    });
    observe_cut(kind, path, report, |observer| {
        let history = observer.memory_owner_history()?;
        let head = history.memory_head;
        Ok(fields(json!({
            "observation_status": "OBSERVED",
            "memory_head": head.to_string(),
            "rows": retained_rows(history)?,
            "reason": "revision_body_current_terminal_only",
        })))
    })
}

fn retained_rows(mut observation: MemoryOwnerHistoryObservation) -> Result<Vec<Value>, String> {
    let mut rows = Vec::new();
    let mut bodies = BTreeMap::new();
    for (id, history) in observation.histories {
        for revision in &history.revisions {
            let record = &revision.record;
            let origin = origin(
                &mut observation.publication_origins,
                record.memory_version,
                id,
            )?;
            rows.push(commitment(
                "revision",
                &record.memory_version.to_be_bytes(),
                revision_value(record, &origin),
                "retained",
            )?);
            let PayloadState::Available(body) = &revision.payload else {
                return Err("memory_owner_body_availability_not_represented".into());
            };
            if let Some(previous) = bodies.insert(record.body_id, body.clone()) {
                if previous != *body {
                    return Err("memory_owner_body_identity_conflicts".into());
                }
            }
        }
        if let Some(tombstone) = history.tombstone {
            let origin = origin(
                &mut observation.publication_origins,
                tombstone.memory_version,
                id,
            )?;
            rows.push(commitment("terminal", &id.0, json!({
                "id": hex(&id.0), "revision": tombstone.revision,
                "global_version": origin.global_version, "memory_version": tombstone.memory_version,
                "transaction_time_ns": origin.transaction_time_ns,
            }), "tombstoned")?);
        } else {
            let latest = history
                .revisions
                .last()
                .ok_or("empty_retained_memory_history")?;
            rows.push(commitment(
                "current",
                &id.0,
                json!(latest.record.revision),
                "retained",
            )?);
        }
    }
    if !observation.publication_origins.is_empty() {
        return Err("memory_owner_orphan_publication_origin".into());
    }
    for (id, body) in bodies {
        rows.push(commitment(
            "body",
            &id.0,
            json!({
                "title": body.title, "content": body.content,
            }),
            "retained",
        )?);
    }
    Ok(rows)
}

fn origin(
    origins: &mut BTreeMap<u64, (reliquary_memory_core::MemoryId, MemoryPublicationOrigin)>,
    version: u64,
    id: reliquary_memory_core::MemoryId,
) -> Result<MemoryPublicationOrigin, String> {
    match origins.remove(&version) {
        Some((published_id, origin)) if published_id == id => Ok(origin),
        _ => Err(format!(
            "memory_owner_origin_unavailable_at_local_version:{version}"
        )),
    }
}

fn commitment(
    family: &str,
    key: &[u8],
    mut value: Value,
    availability: &str,
) -> Result<Value, String> {
    let coordinate: BTreeMap<_, _> = ["global_version", "memory_version", "transaction_time_ns"]
        .into_iter()
        .filter_map(|name| {
            value
                .get(name)?
                .as_number()
                .map(|number| (name, number.to_string()))
        })
        .collect();
    value.sort_all_objects();
    let canonical = serde_json::to_vec(&value).map_err(|_| "memory_owner_row_encoding_failed")?;
    Ok(json!({
        "projection_domain": "memory",
        "family": family,
        "stable_key_sha256": hex(&sha256(key)),
        "canonical_value_sha256": hex(&sha256(&canonical)),
        "canonical_byte_length": canonical.len().to_string(),
        "origin_coordinate": coordinate,
        "availability": availability,
    }))
}

fn revision_value(record: &MemoryRecord, origin: &MemoryPublicationOrigin) -> Value {
    let MemoryRecord {
        id,
        revision,
        body_id,
        category,
        memory_type,
        authority_kind,
        temporal_status,
        scope,
        lifecycle_state,
        archived,
        parent_id,
        superseded_by,
        source_node_id,
        content_source_conversation_id,
        content_source_node_id,
        grounding_source_conversation_id,
        grounding_source_node_id,
        source_episode_id,
        source_ref,
        source_time_ns,
        temporal_inference,
        mutation_id,
        created_at_ns,
        updated_at_ns,
        memory_version,
    } = record;
    json!({
        "id": hex(&id.0), "revision": revision, "body_id": hex(&body_id.0),
        "category": category, "memory_type": memory_type, "authority_kind": authority_kind,
        "temporal_status": temporal_status, "scope": scope, "lifecycle_state": lifecycle_state,
        "archived": archived, "parent_id": parent_id.map(|id| hex(&id.0)),
        "superseded_by": superseded_by.map(|id| hex(&id.0)), "source_node_id": source_node_id,
        "content_source_conversation_id": content_source_conversation_id,
        "content_source_node_id": content_source_node_id,
        "grounding_source_conversation_id": grounding_source_conversation_id,
        "grounding_source_node_id": grounding_source_node_id,
        "source_episode_id": source_episode_id.map(|id| hex(&id)), "source_time_ns": source_time_ns,
        "source_ref": source_ref.as_ref().map(source_value),
        "temporal_inference": temporal_inference.as_ref().map(temporal_value),
        "mutation_id": mutation_id, "created_at_ns": created_at_ns, "updated_at_ns": updated_at_ns,
        "memory_version": memory_version, "global_version": origin.global_version,
        "transaction_time_ns": origin.transaction_time_ns,
    })
}

fn source_value(source: &MemorySourceRef) -> Value {
    json!({
        "owner_id": source.owner_id, "principal_id": source.principal_id,
        "source_episode_id": hex(&source.source_episode_id), "source_node_id": source.source_node_id,
        "content_source_conversation_id": source.content_source_conversation_id,
        "content_source_node_id": source.content_source_node_id,
        "grounding_source_conversation_id": source.grounding_source_conversation_id,
        "grounding_source_node_id": source.grounding_source_node_id,
    })
}

fn temporal_value(temporal: &MemoryTemporalInference) -> Value {
    json!({
        "body_id": hex(&temporal.body_id.0), "source_time_ns": temporal.source_time_ns,
        "model": temporal.inference.model, "contract_version": temporal.inference.contract_version,
        "resolutions": temporal.inference.resolutions.iter().map(|resolution| json!({
            "start": resolution.start_byte, "end": resolution.end_byte,
            "kind": temporal_kind(resolution.kind), "evidence": resolution.evidence,
            "canonical_expression": resolution.canonical_expression,
        })).collect::<Vec<_>>(),
    })
}
