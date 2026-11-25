use crate::vector_workflow::VectorWorkflowError;
use reliquary_owner_session::model::{CompatibilityProfile, EmbeddingMode};
use reliquary_owner_session::observation::{
    MAX_OBSERVATION_CLAIM_VECTOR_ROWS, ObservationCitation, ObservationClaimVector,
};
use reliquary_vectors::{
    EmbeddingEndpoint, compare_profiles, embed_batch_bounded, validate_embedding_batch,
};

pub const MAX_CLAIM_VECTOR_BATCH_BYTES: usize = 1 << 20;

/// Select the same accepted embedding space as an ordinary configured Memory query.
pub fn select_claim_vector_profile<'a>(
    profiles: &'a [CompatibilityProfile],
    observed: &CompatibilityProfile,
) -> Option<&'a CompatibilityProfile> {
    if let Some(exact) = profiles.iter().find(|profile| profile.id == observed.id) {
        return Some(exact);
    }
    let mut compatible = profiles
        .iter()
        .filter(|profile| compare_profiles(profile, observed).compatible);
    let profile = compatible.next()?;
    compatible.next().is_none().then_some(profile)
}

/// Embed propositions captured from current verified revisions outside the owner lease.
pub fn evaluate_claim_vectors(
    profile: &CompatibilityProfile,
    sources: Vec<(ObservationCitation, String)>,
    endpoint: &impl EmbeddingEndpoint,
) -> Result<Vec<ObservationClaimVector>, VectorWorkflowError> {
    let (citations, inputs): (Vec<_>, Vec<_>) = sources.into_iter().unzip();
    if inputs.is_empty() {
        return Err(VectorWorkflowError::EmptyBatch);
    }
    if inputs.len() > MAX_OBSERVATION_CLAIM_VECTOR_ROWS
        || inputs
            .len()
            .checked_mul(profile.dimensions as usize)
            .and_then(|values| values.checked_mul(std::mem::size_of::<f32>()))
            .is_none_or(|bytes| bytes > MAX_CLAIM_VECTOR_BATCH_BYTES)
    {
        return Err(VectorWorkflowError::BatchLimit);
    }
    let vectors = embed_batch_bounded(
        endpoint,
        EmbeddingMode::Document,
        &inputs,
        MAX_CLAIM_VECTOR_BATCH_BYTES,
    )
    .map_err(|error| VectorWorkflowError::Probe(error.to_string()))?;
    validate_embedding_batch(profile, &vectors, inputs.len())
        .map_err(|error| VectorWorkflowError::Probe(error.to_string()))?;
    Ok(citations
        .into_iter()
        .zip(vectors)
        .map(|(citation, vector)| ObservationClaimVector { citation, vector })
        .collect())
}
