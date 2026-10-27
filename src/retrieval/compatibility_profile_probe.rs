use crate::compatibility_vector::{cosine, validate_vectors};
use crate::{
    COMPATIBILITY_MIN_COSINE, COMPATIBILITY_POLICY_VERSION, COMPATIBILITY_PROBE_SUITE_VERSION,
    CompatibilityProbeReference, CompatibilityProfile, CompatibilityProfileError,
    CompatibilityProfileId, CompatibilityReport, EmbeddingEndpoint, EmbeddingMode,
};
pub(crate) use reliquary_vector_store::profile::compatibility_profile_id;

pub(crate) const QUERY_PROBES: [&str; 2] = [
    "continuity compatibility query: cedar river glass",
    "continuity compatibility query: violet engine station",
];
pub(crate) const DOCUMENT_PROBES: [&str; 2] = [
    "continuity compatibility document: quiet harbor satellite",
    "continuity compatibility document: copper lantern winter",
];

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

pub(crate) fn compare_profiles(
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
    let mut minimum = 1.0_f64;
    for (left, right) in expected.references.iter().zip(&observed.references) {
        if left.mode != right.mode {
            return CompatibilityReport {
                compatible: false,
                minimum_cosine: None,
            };
        }
        let Some(similarity) = cosine(&left.vector, &right.vector) else {
            return CompatibilityReport {
                compatible: false,
                minimum_cosine: None,
            };
        };
        minimum = minimum.min(similarity);
    }
    CompatibilityReport {
        compatible: minimum + 1e-12 >= COMPATIBILITY_MIN_COSINE,
        minimum_cosine: Some(minimum),
    }
}

pub(crate) fn validate_embedding_batch(
    profile: &CompatibilityProfile,
    vectors: &[Vec<f32>],
    expected: usize,
) -> Result<(), CompatibilityProfileError> {
    validate_vectors(profile.dimensions, profile.normalization, vectors, expected)
}
