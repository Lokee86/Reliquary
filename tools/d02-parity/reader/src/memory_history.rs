use crate::{
    CandidateKind,
    cut::{fields, observe_cut},
    evidence_encoding::{hex, sha256},
};
use reliquary_memory_core::{
    MemoryId, MemoryRevision, MemorySourceRef, MemoryTemporalInference, PayloadState,
    TemporalIndicationKind,
};
use reliquary_owner_session::{MemoryHistoryCutObservation, MemoryPublicationOrigin};
use serde_json::{Value, json};
use std::path::Path;

pub fn observe_memory_history(kind: CandidateKind, path: &Path, id: MemoryId, cut: u64) -> Value {
    let report = json!({
        "schema_id": "reliquary-d02-memory-history-observation-v1",
        "status": "INCOMPLETE",
        "memory_observation_status": "INCOMPLETE",
        "requested_memory_version": cut.to_string(),
        "requested_memory_id": hex(&id.0),
        "owner": null,
        "candidate_sha256_before": null,
        "candidate_sha256_after": null,
        "candidate_unchanged": null,
        "memory_head": null,
        "projection": null,
        "projection_sha256": null,
        "reason": "",
    });
    observe_cut(kind, path, report, |observer| {
        let observation = observer.memory_history_cut(id, cut)?;
        let history = observation.history.as_ref().map(|history| {
            json!({
                "revisions": history.revisions.iter()
                    .map(|revision| revision_value(revision, &observation)).collect::<Vec<_>>(),
                "tombstone": history.tombstone.map(|tombstone| json!({
                    "revision": tombstone.revision.to_string(),
                    "memory_version": tombstone.memory_version.to_string(),
                    "standalone_origin": observation.standalone_publication_origins.get(&tombstone.memory_version).map(origin_value),
                })),
            })
        });
        let projection = json!({
            "memory_id": hex(&id.0),
            "memory_version": cut.to_string(),
            "presence": if history.is_some() { "retained_history" } else { "absent_at_cut" },
            "history": history,
        });
        let mut preimage = b"reliquary:d02:memory-history:v1\0".to_vec();
        preimage.extend(serde_json::to_vec(&projection).expect("Memory projection is JSON"));
        Ok(fields(json!({
            "memory_head": observation.memory_head.to_string(),
            "projection_sha256": hex(&sha256(&preimage)),
            "projection": projection,
            "memory_observation_status": "OBSERVED",
            "reason": "one_memory_history_cut_only; other families and admission incomplete",
        })))
    })
}

fn revision_value(revision: &MemoryRevision, observation: &MemoryHistoryCutObservation) -> Value {
    let record = &revision.record;
    let payload = match &revision.payload {
        PayloadState::Available(body) => json!({
            "availability": "present",
            "title_sha256": hex(&sha256(body.title.as_bytes())),
            "title_utf8_length": body.title.len().to_string(),
            "content_sha256": hex(&sha256(body.content.as_bytes())),
            "content_utf8_length": body.content.len().to_string(),
        }),
        PayloadState::Absent => match observation.pruned_body_manifests.get(&record.body_id) {
            Some(digest) => json!({
                "availability": "pruned_terminal",
                "prune_manifest_sha256": hex(digest),
            }),
            None => json!({"availability": "no_retained_local_body"}),
        },
    };
    json!({
        "memory_id": hex(&record.id.0),
        "revision": record.revision.to_string(),
        "body_id": hex(&record.body_id.0),
        "payload": payload,
        "category": record.category,
        "memory_type": record.memory_type,
        "authority_kind": record.authority_kind,
        "temporal_status": record.temporal_status,
        "scope": record.scope,
        "lifecycle_state": record.lifecycle_state,
        "archived": record.archived,
        "parent_id": record.parent_id.map(|id| hex(&id.0)),
        "superseded_by": record.superseded_by.map(|id| hex(&id.0)),
        "source_node_id": record.source_node_id,
        "content_source_conversation_id": record.content_source_conversation_id,
        "content_source_node_id": record.content_source_node_id,
        "grounding_source_conversation_id": record.grounding_source_conversation_id,
        "grounding_source_node_id": record.grounding_source_node_id,
        "source_episode_id": record.source_episode_id.map(|id| hex(&id)),
        "source_ref": record.source_ref.as_ref().map(source_value),
        "source_time_ns": record.source_time_ns.map(|time| time.to_string()),
        "temporal_inference": record.temporal_inference.as_ref().map(temporal_value),
        "mutation_id": record.mutation_id,
        "created_at_ns": record.created_at_ns.to_string(),
        "updated_at_ns": record.updated_at_ns.to_string(),
        "memory_version": record.memory_version.to_string(),
        "standalone_origin": observation.standalone_publication_origins.get(&record.memory_version).map(origin_value),
    })
}

fn origin_value(origin: &MemoryPublicationOrigin) -> Value {
    json!({
        "global_version": origin.global_version.to_string(),
        "transaction_time_ns": origin.transaction_time_ns.map(|time| time.to_string()),
        "bound_reservation": origin.bound_reservation,
    })
}

fn source_value(source: &MemorySourceRef) -> Value {
    json!({
        "owner_id": source.owner_id,
        "principal_id": source.principal_id,
        "source_episode_id": hex(&source.source_episode_id),
        "source_node_id": source.source_node_id,
        "content_source_conversation_id": source.content_source_conversation_id,
        "content_source_node_id": source.content_source_node_id,
        "grounding_source_conversation_id": source.grounding_source_conversation_id,
        "grounding_source_node_id": source.grounding_source_node_id,
    })
}

fn temporal_value(temporal: &MemoryTemporalInference) -> Value {
    json!({
        "body_id": hex(&temporal.body_id.0),
        "source_time_ns": temporal.source_time_ns.map(|time| time.to_string()),
        "model": temporal.inference.model,
        "contract_version": temporal.inference.contract_version,
        "resolutions": temporal.inference.resolutions.iter().map(|resolution| json!({
            "start_byte": resolution.start_byte.to_string(),
            "end_byte": resolution.end_byte.to_string(),
            "kind": match resolution.kind {
                TemporalIndicationKind::Explicit => "explicit",
                TemporalIndicationKind::Calendar => "calendar",
                TemporalIndicationKind::Relative => "relative",
                TemporalIndicationKind::Boundary => "boundary",
                TemporalIndicationKind::Recurrence => "recurrence",
                TemporalIndicationKind::Duration => "duration",
            },
            "evidence_sha256": hex(&sha256(resolution.evidence.as_bytes())),
            "canonical_expression": resolution.canonical_expression,
        })).collect::<Vec<_>>(),
    })
}
