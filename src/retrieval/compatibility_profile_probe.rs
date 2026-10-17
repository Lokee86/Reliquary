use crate::compatibility_vector::{cosine, validate_vectors};
use crate::{
    COMPATIBILITY_MIN_COSINE, COMPATIBILITY_MIN_STORED_POLICY_VERSION,
    COMPATIBILITY_POLICY_VERSION, COMPATIBILITY_PROBE_SUITE_VERSION, CompatibilityProbeReference,
    CompatibilityProfile, CompatibilityProfileError, CompatibilityProfileId, CompatibilityReport,
    EmbeddingEndpoint, EmbeddingMode,
};
use sha2::{Digest, Sha256};

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

pub(crate) fn validate_profile(
    profile: &CompatibilityProfile,
) -> Result<(), CompatibilityProfileError> {
    if profile.dimensions == 0
        || profile.probe_suite_version != COMPATIBILITY_PROBE_SUITE_VERSION
        || !(COMPATIBILITY_MIN_STORED_POLICY_VERSION..=COMPATIBILITY_POLICY_VERSION)
            .contains(&profile.compatibility_policy_version)
        || profile.references.len() != QUERY_PROBES.len() + DOCUMENT_PROBES.len()
    {
        return Err(CompatibilityProfileError::InvalidProfile(
            "profile contract",
        ));
    }
    let modes = [
        EmbeddingMode::Query,
        EmbeddingMode::Query,
        EmbeddingMode::Document,
        EmbeddingMode::Document,
    ];
    for (reference, expected_mode) in profile.references.iter().zip(modes) {
        if reference.mode != expected_mode {
            return Err(CompatibilityProfileError::InvalidProfile("probe order"));
        }
    }
    let vectors = profile
        .references
        .iter()
        .map(|reference| reference.vector.clone())
        .collect::<Vec<_>>();
    validate_vectors(
        profile.dimensions,
        profile.normalization,
        &vectors,
        vectors.len(),
    )
}

pub(crate) fn compatibility_profile_id(profile: &CompatibilityProfile) -> CompatibilityProfileId {
    let mut hash = Sha256::new();
    hash.update(b"CVA-COMPATIBILITY-PROFILE-V1\0");
    hash.update(profile.dimensions.to_le_bytes());
    hash.update([profile.normalization.tag()]);
    hash.update(profile.probe_suite_version.to_le_bytes());
    hash.update(profile.compatibility_policy_version.to_le_bytes());
    for reference in &profile.references {
        hash.update([reference.mode.tag()]);
        for value in &reference.vector {
            hash.update(value.to_le_bytes());
        }
    }
    CompatibilityProfileId(hash.finalize().into())
}
