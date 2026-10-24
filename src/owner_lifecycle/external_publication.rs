//! External-source orchestration of one Memory-owned atomic publication.
use crate::memory_external_publication::{Publication, PublicationBody, body_spans, encode};
use crate::memory_model::ExternalMemoryBinding;
use crate::{Container, Memory, MemoryError, ObjectRef};

pub(crate) fn publish(
    container: &mut Container,
    memories: &mut crate::memory_store::MemoryStore,
    window: [u8; 32],
    prepared: &[crate::external_source::processing::PreparedExternalMemory],
    birth_turn: Option<u64>,
) -> Result<Vec<Memory>, MemoryError> {
    if let Some(existing) = memories.external_window(container, window)? {
        return Ok(existing);
    }
    let drafts = prepared
        .iter()
        .map(|m| (m.draft.clone(), m.temporal.clone(), m.routing.clone()))
        .collect();
    let batch = memories.stage_grouped_publication(container, drafts)?;
    // All accepted external work has an owner-local atomic receipt; loose mutations cannot impersonate it.
    if !batch.existing.is_empty() {
        return Err(MemoryError::MutationConflict);
    }
    let value = Publication {
        window,
        global_start: batch.global_version_start,
        transaction_ns: Container::transaction_time_now_ns()?,
        birth_turn,
        bindings: prepared
            .iter()
            .zip(&batch.records)
            .map(|(m, r)| ExternalMemoryBinding {
                memory: r.id.0,
                body: r.body_id.0,
                evidence: m.evidence.clone(),
            })
            .collect(),
        records: batch.records,
        bodies: batch
            .bodies
            .into_iter()
            .map(|(id, bytes)| PublicationBody { id, bytes })
            .collect(),
        routing: batch.routing_metadata,
    };
    let bytes = encode(&value)?;
    let object: ObjectRef = container.append(&bytes)?;
    container.commit_embedded_version_range_at(
        value.global_start,
        value.records.len(),
        Some(value.transaction_ns),
    )?;
    container.sync()?;
    let body_spans = body_spans(&bytes)?.ok_or(MemoryError::CorruptBody)?;
    memories.apply_grouped_spans(object, &bytes, &body_spans)?;
    let results = memories.apply_grouped_records(container, &value.records)?;
    memories.apply_grouped_routing_metadata(&value.routing)?;
    memories.apply_external_bindings(value.window, &value.bindings)?;
    Ok(results)
}
