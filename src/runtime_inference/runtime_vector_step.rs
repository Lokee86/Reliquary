use crate::{CompatibilityProfile, CompatibilityProfileId, MemoryBodyId, MemoryVectorError};

pub(crate) struct RuntimeMemoryVectorBatch {
    pub(crate) profile: CompatibilityProfile,
    pub(crate) body_ids: Vec<MemoryBodyId>,
    pub(crate) texts: Vec<String>,
}

pub(crate) fn select_body_ids(
    profile_id: CompatibilityProfileId,
    current: &[MemoryBodyId],
    missing: impl Fn(CompatibilityProfileId, &[MemoryBodyId]) -> Vec<MemoryBodyId>,
) -> Vec<MemoryBodyId> {
    if current.is_empty() {
        Vec::new()
    } else {
        missing(profile_id, current)
    }
}

pub(crate) fn pending_rows(
    profile_id: CompatibilityProfileId,
    body_ids: Vec<MemoryBodyId>,
    vectors: Vec<Vec<f32>>,
    mut has_location: impl FnMut(CompatibilityProfileId, MemoryBodyId) -> bool,
) -> (Vec<MemoryBodyId>, Vec<Vec<f32>>) {
    let mut pending_ids = Vec::with_capacity(body_ids.len());
    let mut pending_vectors = Vec::with_capacity(vectors.len());
    for (body_id, vector) in body_ids.into_iter().zip(vectors) {
        if !has_location(profile_id, body_id) {
            pending_ids.push(body_id);
            pending_vectors.push(vector);
        }
    }
    (pending_ids, pending_vectors)
}

pub(crate) fn validate_body_presence(
    body_ids: &[MemoryBodyId],
    mut contains: impl FnMut(MemoryBodyId) -> bool,
) -> Result<(), MemoryVectorError> {
    if body_ids.iter().any(|id| !contains(*id)) {
        Err(MemoryVectorError::MissingMemoryBody)
    } else {
        Ok(())
    }
}
