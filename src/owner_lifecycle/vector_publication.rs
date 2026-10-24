use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::memory_vector_store::{MemoryVectorPublicationPlan, MemoryVectorStore};
use crate::packed_vector_store::PackedVectorStore;
use crate::{CompatibilityProfile, CompatibilityProfileError, Container, MemoryVectorError};

pub(crate) fn accept_profile(
    container: &mut Container,
    profiles: &mut CompatibilityProfileStore,
    candidate: CompatibilityProfile,
) -> Result<CompatibilityProfile, CompatibilityProfileError> {
    if let Some(existing) = profiles.compatible_with(&candidate) {
        return Ok(existing.clone());
    }
    profiles.put(container, candidate.clone())?;
    Ok(candidate)
}

pub(crate) fn publish_memory_vectors(
    container: &mut Container,
    profiles: &CompatibilityProfileStore,
    packed_vectors: &mut PackedVectorStore,
    memory_vectors: &mut MemoryVectorStore,
    plan: MemoryVectorPublicationPlan,
) -> Result<usize, MemoryVectorError> {
    let count = plan.body_ids.len();
    let packed_id = packed_vectors.put(container, plan.packed_vectors)?;
    memory_vectors.put(
        container,
        profiles,
        packed_vectors,
        plan.profile_id,
        packed_id,
        plan.body_ids,
    )?;
    Ok(count)
}
