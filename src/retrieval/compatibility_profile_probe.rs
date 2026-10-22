//! Provider probe composition above durable profile storage.
use crate::CompatibilityProfileId;
use crate::compatibility_profile_validation::{
    DOCUMENT_PROBES, QUERY_PROBES, compare_profiles, compatibility_profile_id,
};
use crate::compatibility_vector::validate_vectors;
use crate::{
    COMPATIBILITY_POLICY_VERSION, COMPATIBILITY_PROBE_SUITE_VERSION, CompatibilityProbeReference,
    CompatibilityProfile, CompatibilityProfileError, CompatibilityReport, EmbeddingEndpoint,
    EmbeddingMode,
};

pub(crate) fn profile_from_endpoint(
    endpoint: &impl EmbeddingEndpoint,
) -> Result<CompatibilityProfile, CompatibilityProfileError> {
    let dimensions = endpoint.dimensions();
    if dimensions == 0 {
        return Err(CompatibilityProfileError::InvalidProfile("zero dimensions"));
    }
    let normalization = endpoint.normalization();
    let mut references = Vec::with_capacity(QUERY_PROBES.len() + DOCUMENT_PROBES.len());
    for (mode, probes) in [
        (EmbeddingMode::Query, QUERY_PROBES.as_slice()),
        (EmbeddingMode::Document, DOCUMENT_PROBES.as_slice()),
    ] {
        let inputs = probes
            .iter()
            .map(|probe| (*probe).to_string())
            .collect::<Vec<_>>();
        let vectors = endpoint.embed(mode, &inputs)?;
        validate_vectors(dimensions, normalization, &vectors, probes.len())?;
        references.extend(
            vectors
                .into_iter()
                .map(|vector| CompatibilityProbeReference { mode, vector }),
        );
    }
    let mut profile = CompatibilityProfile {
        id: CompatibilityProfileId([0; 32]),
        dimensions,
        normalization,
        probe_suite_version: COMPATIBILITY_PROBE_SUITE_VERSION,
        compatibility_policy_version: COMPATIBILITY_POLICY_VERSION,
        references,
    };
    profile.id = compatibility_profile_id(&profile);
    Ok(profile)
}

pub(crate) fn verify_endpoint(
    profile: &CompatibilityProfile,
    endpoint: &impl EmbeddingEndpoint,
) -> Result<CompatibilityReport, CompatibilityProfileError> {
    let observed = profile_from_endpoint(endpoint)?;
    Ok(compare_profiles(profile, &observed))
}

pub(crate) fn validate_embedding_batch(
    profile: &CompatibilityProfile,
    vectors: &[Vec<f32>],
    expected: usize,
) -> Result<(), CompatibilityProfileError> {
    validate_vectors(profile.dimensions, profile.normalization, vectors, expected)
}
