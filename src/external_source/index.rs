use super::model::*;
use super::store::{IndexedChunk, IndexedMessage, SourceState};
use crate::{
    CompatibilityProfile, EmbeddingEndpoint, EmbeddingMode, PackedVectors, ScalarType, VectorSchema,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct PreparedIndex {
    pub expected_revision: Option<u64>,
    pub profile: CompatibilityProfile,
    pub state: SourceState,
    pub matrices: Vec<PackedVectors>,
}
fn verify(
    endpoint: &impl EmbeddingEndpoint,
    profile: &CompatibilityProfile,
) -> Result<(), ExternalSourceError> {
    if profile.dimensions == 0
        || profile.dimensions > 4096
        || !crate::compatibility_profile_probe::verify_endpoint(profile, endpoint)
            .map_err(|_| ExternalSourceError::Unavailable)?
            .compatible
    {
        return Err(ExternalSourceError::InvalidContract);
    }
    Ok(())
}
fn source_contract(
    source: &impl ExternalConversationSource,
) -> Result<String, ExternalSourceError> {
    let namespace = source.source_namespace().to_owned();
    identifier(&namespace)?;
    if source.contract_version() != EXTERNAL_SOURCE_CONTRACT {
        return Err(ExternalSourceError::InvalidContract);
    }
    Ok(namespace)
}
pub(crate) fn prepare_incremental(
    source: &impl ExternalConversationSource,
    principal: &str,
    endpoint: &impl EmbeddingEndpoint,
    profile: CompatibilityProfile,
    old: SourceState,
) -> Result<PreparedIndex, ExternalSourceError> {
    source.authorize(principal)?;
    let namespace = source_contract(source)?;
    if namespace != old.namespace || profile.id.0 != old.profile {
        return Err(ExternalSourceError::InvalidContract);
    }
    verify(endpoint, &profile)?;
    let batch = source.changes_after(principal, old.cursor, MAX_EXTERNAL_BATCH)?;
    if batch.changes.len() > MAX_EXTERNAL_BATCH {
        return Err(ExternalSourceError::Bounds);
    }
    if old
        .cursor
        .checked_add(1)
        .is_none_or(|next| next < batch.feed_floor)
    {
        return Err(ExternalSourceError::RebuildRequired);
    }
    if batch.feed_watermark < old.cursor
        || (batch.changes.is_empty() && batch.feed_watermark != old.cursor)
    {
        return Err(ExternalSourceError::InvalidContract);
    }
    let digest: [u8; 32] = Sha256::digest(
        serde_json::to_vec(&batch.changes).map_err(|_| ExternalSourceError::InvalidContract)?,
    )
    .into();
    let expected_revision = Some(old.revision);
    let base = old.cursor;
    let mut state = old;
    let mut sequence = base;
    let mut messages: BTreeMap<_, _> = state
        .messages
        .into_iter()
        .map(|m| {
            (
                (
                    m.descriptor.reference.conversation_id.clone(),
                    m.descriptor.reference.message_id.clone(),
                ),
                m,
            )
        })
        .collect();
    for change in &batch.changes {
        sequence = sequence.checked_add(1).ok_or(ExternalSourceError::Bounds)?;
        if change.sequence != sequence || sequence > batch.feed_watermark {
            return Err(ExternalSourceError::InvalidContract);
        }
        match &change.kind {
            ExternalChangeKind::Upsert(descriptor) => {
                descriptor.validate(&namespace)?;
                let key = (
                    descriptor.reference.conversation_id.clone(),
                    descriptor.reference.message_id.clone(),
                );
                let chunks = messages
                    .get(&key)
                    .filter(|m| m.descriptor == *descriptor)
                    .map(|m| m.chunks.clone())
                    .unwrap_or_default();
                messages.insert(
                    key,
                    IndexedMessage {
                        descriptor: descriptor.clone(),
                        chunks,
                    },
                );
            }
            ExternalChangeKind::Visibility {
                conversation_id,
                message_ids,
                active,
            } => {
                identifier(conversation_id)?;
                if message_ids.is_empty() || message_ids.len() > MAX_EXTERNAL_BATCH {
                    return Err(ExternalSourceError::Bounds);
                }
                let mut seen = BTreeSet::new();
                for id in message_ids {
                    identifier(id)?;
                    if !seen.insert(id) {
                        return Err(ExternalSourceError::InvalidContract);
                    }
                    let row = messages
                        .get_mut(&(conversation_id.clone(), id.clone()))
                        .ok_or(ExternalSourceError::InvalidContract)?;
                    row.descriptor.active = *active;
                    if !active {
                        row.chunks.clear();
                    }
                }
            }
            ExternalChangeKind::DeleteConversation { conversation_id } => {
                identifier(conversation_id)?;
                messages.retain(|(conversation, _), _| conversation != conversation_id);
            }
        }
    }
    state.messages = messages.into_values().collect();
    state.cursor = sequence;
    state.last_base = base;
    state.last_digest = digest;
    state.rebuild = false;
    if !batch.changes.is_empty() {
        state.revision = state
            .revision
            .checked_add(1)
            .ok_or(ExternalSourceError::Bounds)?;
    }
    let matrices = fill_missing(source, principal, endpoint, &mut state)?;
    source.authorize(principal)?;
    Ok(PreparedIndex {
        expected_revision,
        profile,
        state,
        matrices,
    })
}
pub(crate) fn prepare_rebuild(
    source: &impl ExternalConversationSource,
    principal: &str,
    endpoint: &impl EmbeddingEndpoint,
    profile: CompatibilityProfile,
    old: Option<&SourceState>,
) -> Result<PreparedIndex, ExternalSourceError> {
    source.authorize(principal)?;
    let namespace = source_contract(source)?;
    verify(endpoint, &profile)?;
    let manifest = source.snapshot_manifest(principal)?;
    if manifest.source_namespace != namespace
        || manifest.contract_version != EXTERNAL_SOURCE_CONTRACT
        || old.is_some_and(|s| manifest.watermark < s.cursor)
    {
        return Err(ExternalSourceError::InvalidContract);
    }
    if manifest.messages.len() > MAX_EXTERNAL_MANIFEST {
        return Err(ExternalSourceError::Bounds);
    }
    let mut unique = BTreeMap::new();
    for descriptor in manifest.messages {
        descriptor.validate(&namespace)?;
        let key = (
            descriptor.reference.conversation_id.clone(),
            descriptor.reference.message_id.clone(),
        );
        if unique
            .insert(
                key,
                IndexedMessage {
                    descriptor,
                    chunks: Vec::new(),
                },
            )
            .is_some()
        {
            return Err(ExternalSourceError::InvalidContract);
        }
    }
    let mut state = SourceState {
        namespace,
        contract: EXTERNAL_SOURCE_CONTRACT,
        profile: profile.id.0,
        dimensions: profile.dimensions,
        generation: old
            .map_or(Some(1), |s| s.generation.checked_add(1))
            .ok_or(ExternalSourceError::Bounds)?,
        revision: old
            .map_or(Some(1), |s| s.revision.checked_add(1))
            .ok_or(ExternalSourceError::Bounds)?,
        cursor: manifest.watermark,
        messages: unique.into_values().collect(),
        last_base: old.map_or(0, |s| s.cursor),
        last_digest: [0; 32],
        rebuild: true,
    };
    let matrices = fill_missing(source, principal, endpoint, &mut state)?;
    source.authorize(principal)?;
    Ok(PreparedIndex {
        expected_revision: old.map(|s| s.revision),
        profile,
        state,
        matrices,
    })
}
fn fill_missing(
    source: &impl ExternalConversationSource,
    principal: &str,
    endpoint: &impl EmbeddingEndpoint,
    state: &mut SourceState,
) -> Result<Vec<PackedVectors>, ExternalSourceError> {
    if state.messages.len() > MAX_EXTERNAL_MANIFEST {
        return Err(ExternalSourceError::Bounds);
    }
    let positions: Vec<_> = state
        .messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.descriptor.active && m.chunks.is_empty())
        .map(|(i, _)| i)
        .collect();
    let mut matrices = Vec::new();
    let mut matrix_bytes = 0usize;
    for batch in positions.chunks(MAX_EXTERNAL_BATCH) {
        let expected: Vec<_> = batch
            .iter()
            .map(|i| state.messages[*i].descriptor.clone())
            .collect();
        let hydrated = source.hydrate(principal, &expected)?;
        if hydrated.len() != expected.len() {
            return Err(ExternalSourceError::InvalidContract);
        }
        for ((position, expected), message) in batch.iter().zip(&expected).zip(hydrated) {
            message.validate(expected)?;
            let ranges = chunk_ranges(&message.content);
            let inputs: Vec<_> = ranges
                .iter()
                .map(|(a, b)| message.content[*a..*b].to_owned())
                .collect();
            if inputs.is_empty() {
                continue;
            }
            let vectors = endpoint
                .embed(EmbeddingMode::Document, &inputs)
                .map_err(|_| ExternalSourceError::Unavailable)?;
            if vectors.len() != inputs.len() {
                return Err(ExternalSourceError::InvalidContract);
            }
            let mut bytes = Vec::new();
            for vector in &vectors {
                validate_vector(vector, state.dimensions)?;
                for value in vector {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            matrix_bytes = matrix_bytes
                .checked_add(bytes.len())
                .ok_or(ExternalSourceError::Bounds)?;
            if matrix_bytes > 64 * 1024 * 1024 {
                return Err(ExternalSourceError::Bounds);
            }
            let schema = VectorSchema::new(state.dimensions, ScalarType::F32)
                .map_err(|_| ExternalSourceError::InvalidContract)?;
            let matrix = crate::packed_vector_store::packed_vector_id(schema, &bytes).0;
            let packed = PackedVectors::from_bytes(schema, bytes)
                .map_err(|_| ExternalSourceError::InvalidContract)?;
            state.messages[*position].chunks = ranges
                .iter()
                .enumerate()
                .map(|(row, (start, end))| IndexedChunk {
                    reference: ExternalChunkRef {
                        message: expected.reference.clone(),
                        start: *start as u64,
                        end: *end as u64,
                    },
                    matrix,
                    row: row as u32,
                    terms: term_hashes(&message.content[*start..*end]),
                })
                .collect();
            matrices.push(packed);
        }
    }
    super::store::encode(state)?;
    Ok(matrices)
}
fn chunk_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let mut end = (start + 4096).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        result.push((start, end));
        start = end;
    }
    result
}
pub(crate) fn term_hashes(text: &str) -> Vec<[u8; 32]> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| Sha256::digest(s.to_lowercase().as_bytes()).into())
        .collect::<BTreeSet<[u8; 32]>>()
        .into_iter()
        .take(1024)
        .collect()
}
pub(crate) fn validate_vector(vector: &[f32], dimensions: u32) -> Result<(), ExternalSourceError> {
    if vector.len() != dimensions as usize
        || vector.iter().any(|v| !v.is_finite())
        || vector.iter().all(|v| *v == 0.0)
    {
        return Err(ExternalSourceError::InvalidContract);
    }
    Ok(())
}
pub(crate) fn query_vector(
    endpoint: &impl EmbeddingEndpoint,
    profile: &CompatibilityProfile,
    query: &str,
) -> Result<Vec<f32>, ExternalSourceError> {
    if query.len() > MAX_EXTERNAL_MESSAGE_BYTES || query.trim().is_empty() {
        return Err(ExternalSourceError::Bounds);
    }
    verify(endpoint, profile)?;
    let vectors = endpoint
        .embed(EmbeddingMode::Query, &[query.to_owned()])
        .map_err(|_| ExternalSourceError::Unavailable)?;
    if vectors.len() != 1 {
        return Err(ExternalSourceError::InvalidContract);
    }
    let vector = vectors.into_iter().next().unwrap();
    validate_vector(&vector, profile.dimensions)?;
    Ok(vector)
}
