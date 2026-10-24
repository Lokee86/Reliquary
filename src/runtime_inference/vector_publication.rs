use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::memory_store::MemoryReadView;
use crate::memory_vector_store::MemoryVectorStore;
use crate::runtime_vector_step_policy::{self, RuntimeMemoryVectorBatch};
use crate::{CompatibilityProfileId, MemoryVectorError};

pub(crate) fn prepare_batch(
    memories: &MemoryReadView<'_>,
    profiles: &CompatibilityProfileStore,
    vectors: &MemoryVectorStore,
    profile_id: CompatibilityProfileId,
) -> Result<Option<RuntimeMemoryVectorBatch>, MemoryVectorError> {
    let profile = profiles
        .get_active(profile_id)
        .cloned()
        .ok_or(MemoryVectorError::MissingProfile)?;
    let current = memories.current_body_ids();
    let body_ids = if current.is_empty() {
        Vec::new()
    } else {
        vectors.missing_body_ids(profile_id, &current)
    };
    if body_ids.is_empty() {
        return Ok(None);
    }
    let mut texts = Vec::with_capacity(body_ids.len());
    for body_id in &body_ids {
        texts.push(memories.body_embedding_text(*body_id)?);
    }
    Ok(Some(RuntimeMemoryVectorBatch {
        profile,
        body_ids,
        texts,
    }))
}
