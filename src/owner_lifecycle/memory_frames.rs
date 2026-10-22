//! Existing grouped publication dispatch, above the independent Memory owner.
use crate::memory_rebuild::MemoryOpenState;
use crate::{MemoryError, ObjectRef};

pub(crate) fn ingest(
    state: &mut MemoryOpenState,
    chunk: ObjectRef,
    payload: &[u8],
    latest_global: u64,
) -> Result<(), MemoryError> {
    if let Some(publication) = crate::memory_external_publication::decode(payload)? {
        let spans = crate::memory_external_publication::body_spans(payload)?.ok_or(
            MemoryError::CorruptRecord("missing external Memory body spans"),
        )?;
        state.ingest_grouped(
            chunk,
            payload,
            &publication.records,
            &spans,
            &publication.routing,
            latest_global,
        )?;
        state.ingest_external_bindings(publication.window, &publication.bindings)?;
        return Ok(());
    }
    if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
        .map_err(MemoryError::CorruptRecord)?
    {
        let spans = crate::insomnia::completion::decode_completion_body_spans(payload)
            .map_err(MemoryError::CorruptRecord)?
            .ok_or(MemoryError::CorruptRecord(
                "missing completion Memory body spans",
            ))?;
        return state.ingest_grouped(
            chunk,
            payload,
            &completion.records,
            &spans,
            &completion.routing_metadata,
            latest_global,
        );
    }
    state.ingest(chunk, payload, latest_global)
}
