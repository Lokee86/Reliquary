use crate::compatibility_vector::{cosine, validate_vectors};
use crate::{
    COMPATIBILITY_MIN_COSINE, COMPATIBILITY_POLICY_VERSION, COMPATIBILITY_PROBE_SUITE_VERSION,
    CompatibilityProbeReference, CompatibilityProfile, CompatibilityProfileId, CompatibilityReport,
    EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, ProbeError,
};
pub use reliquary_vector_store::profile::compatibility_profile_id;
pub const QUERY_PROBES: [&str; 2] = [
    "continuity compatibility query: cedar river glass",
    "continuity compatibility query: violet engine station",
];
pub const DOCUMENT_PROBES: [&str; 2] = [
    "continuity compatibility document: quiet harbor satellite",
    "continuity compatibility document: copper lantern winter",
];
pub fn profile_from_endpoint(
    endpoint: &impl EmbeddingEndpoint,
) -> Result<CompatibilityProfile, ProbeError> {
    let dimensions = endpoint.dimensions();
    if dimensions == 0 {
        return Err(ProbeError::InvalidProfile("zero dimensions"));
    }
    let normalization = endpoint.normalization();
    let mut references = Vec::with_capacity(4);
    for (mode, probes) in [
        (EmbeddingMode::Query, QUERY_PROBES.as_slice()),
        (EmbeddingMode::Document, DOCUMENT_PROBES.as_slice()),
    ] {
        let inputs = probes.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>();
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
pub fn verify_endpoint(
    profile: &CompatibilityProfile,
    endpoint: &impl EmbeddingEndpoint,
) -> Result<CompatibilityReport, ProbeError> {
    Ok(compare_profiles(profile, &profile_from_endpoint(endpoint)?))
}
pub fn compare_profiles(
    expected: &CompatibilityProfile,
    observed: &CompatibilityProfile,
) -> CompatibilityReport {
    if expected.dimensions != observed.dimensions
        || expected.normalization != observed.normalization
        || expected.probe_suite_version != observed.probe_suite_version
        || expected.references.len() != observed.references.len()
    {
        return CompatibilityReport {
            compatible: false,
            minimum_cosine: None,
        };
    }
    let mut min = 1.0_f64;
    for (a, b) in expected.references.iter().zip(&observed.references) {
        if a.mode != b.mode {
            return CompatibilityReport {
                compatible: false,
                minimum_cosine: None,
            };
        }
        let Some(similarity) = cosine(&a.vector, &b.vector) else {
            return CompatibilityReport {
                compatible: false,
                minimum_cosine: None,
            };
        };
        min = min.min(similarity)
    }
    CompatibilityReport {
        compatible: min + 1e-12 >= COMPATIBILITY_MIN_COSINE,
        minimum_cosine: Some(min),
    }
}
pub fn validate_embedding_batch(
    profile: &CompatibilityProfile,
    vectors: &[Vec<f32>],
    expected: usize,
) -> Result<(), ProbeError> {
    validate_vectors(profile.dimensions, profile.normalization, vectors, expected)
}

/// Request one embedding batch with a hard response-byte ceiling, then verify
/// count, width, finiteness, and the endpoint's declared normalization.
pub fn embed_batch_bounded(
    endpoint: &impl EmbeddingEndpoint,
    mode: EmbeddingMode,
    inputs: &[String],
    max_response_bytes: usize,
) -> Result<Vec<Vec<f32>>, ProbeError> {
    if !endpoint.supports_bounded_embedding() {
        return Err(
            EmbeddingEndpointError::InvalidConfiguration("bounded embedding unsupported").into(),
        );
    }
    let dimensions =
        usize::try_from(endpoint.dimensions()).map_err(|_| ProbeError::SizeOverflow)?;
    let expected_bytes = inputs
        .len()
        .checked_mul(dimensions)
        .and_then(|values| values.checked_mul(std::mem::size_of::<f32>()))
        .ok_or(ProbeError::SizeOverflow)?;
    if expected_bytes > max_response_bytes {
        return Err(EmbeddingEndpointError::InvalidResponse("bounded output exceeded").into());
    }
    let vectors = endpoint.embed_bounded(mode, inputs, max_response_bytes)?;
    validate_vectors(
        endpoint.dimensions(),
        endpoint.normalization(),
        &vectors,
        inputs.len(),
    )?;
    Ok(vectors)
}
