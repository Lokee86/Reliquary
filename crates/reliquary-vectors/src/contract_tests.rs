use std::sync::Mutex;

use crate::compatibility_profile_probe::{
    DOCUMENT_PROBES, QUERY_PROBES, compare_profiles, profile_from_endpoint, verify_endpoint,
};
use crate::{
    COMPATIBILITY_MIN_COSINE, COMPATIBILITY_POLICY_VERSION, COMPATIBILITY_PROBE_SUITE_VERSION,
    CompatibilityProfile, EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, ProbeError,
    SimulatedEmbeddingEndpoint, VectorNormalization,
};

struct RecordingEndpoint {
    calls: Mutex<Vec<(EmbeddingMode, Vec<String>)>>,
    result: Result<Vec<Vec<f32>>, &'static str>,
}

impl RecordingEndpoint {
    fn recording() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            result: Ok(vec![vec![1.0, 0.0]; 2]),
        }
    }

    fn malformed(message: &'static str) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            result: Err(message),
        }
    }
}

impl EmbeddingEndpoint for RecordingEndpoint {
    fn dimensions(&self) -> u32 {
        2
    }

    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.calls.lock().unwrap().push((mode, inputs.to_vec()));
        match &self.result {
            Ok(vectors) => Ok(vectors.clone()),
            Err("wrong dimension") => Ok(vec![vec![1.0]; inputs.len()]),
            Err("non-finite") => Ok(vec![vec![f32::NAN, 0.0]; inputs.len()]),
            Err("wrong count") => Ok(Vec::new()),
            Err(_) => unreachable!(),
        }
    }
}

#[test]
fn compatibility_policy_versions_and_fixed_probe_order_are_stable() {
    assert_eq!(COMPATIBILITY_PROBE_SUITE_VERSION, 1);
    assert_eq!(COMPATIBILITY_POLICY_VERSION, 3);
    assert_eq!(COMPATIBILITY_MIN_COSINE, 0.99);

    let endpoint = RecordingEndpoint::recording();
    let profile = profile_from_endpoint(&endpoint).unwrap();
    let calls = endpoint.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, EmbeddingMode::Query);
    assert_eq!(calls[0].1, QUERY_PROBES.map(str::to_owned));
    assert_eq!(calls[1].0, EmbeddingMode::Document);
    assert_eq!(calls[1].1, DOCUMENT_PROBES.map(str::to_owned));
    assert_eq!(
        profile
            .references
            .iter()
            .map(|reference| reference.mode)
            .collect::<Vec<_>>(),
        [
            EmbeddingMode::Query,
            EmbeddingMode::Query,
            EmbeddingMode::Document,
            EmbeddingMode::Document
        ]
    );
}

#[test]
fn simulated_endpoint_profile_digest_matches_frozen_baseline() {
    let endpoint = SimulatedEmbeddingEndpoint::new(16, VectorNormalization::L2, 7);
    let profile = profile_from_endpoint(&endpoint).unwrap();
    let actual = profile
        .id
        .0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        actual,
        "6a639dc66fc6e0cac000a921241aa129e2a0ddacee7eee218699bccceeb45423"
    );
}

fn profile_with_references(vector: Vec<f32>) -> CompatibilityProfile {
    let endpoint = RecordingEndpoint::recording();
    let mut profile = profile_from_endpoint(&endpoint).unwrap();
    for reference in &mut profile.references {
        reference.vector = vector.clone();
    }
    profile
}

#[test]
fn compatibility_comparison_accepts_and_rejects_vectors_on_opposite_sides_of_floor() {
    let expected = profile_with_references(vec![1.0, 0.0]);
    let above = profile_with_references(vec![0.995, (1.0_f32 - 0.995_f32.powi(2)).sqrt()]);
    let below = profile_with_references(vec![0.985, (1.0_f32 - 0.985_f32.powi(2)).sqrt()]);

    let passing = compare_profiles(&expected, &above);
    assert!(passing.compatible);
    assert!(passing.minimum_cosine.unwrap() >= COMPATIBILITY_MIN_COSINE);
    let failing = compare_profiles(&expected, &below);
    assert!(!failing.compatible);
    assert!(failing.minimum_cosine.unwrap() < COMPATIBILITY_MIN_COSINE);
}

#[test]
fn endpoint_verification_preserves_threshold_behavior() {
    let baseline = SimulatedEmbeddingEndpoint::new(16, VectorNormalization::L2, 9);
    let profile = profile_from_endpoint(&baseline).unwrap();
    let above = baseline.clone().with_drift(0.01);
    let below = baseline.with_drift(0.3);

    assert!(verify_endpoint(&profile, &above).unwrap().compatible);
    assert!(!verify_endpoint(&profile, &below).unwrap().compatible);
}

#[test]
fn probe_rejects_wrong_count_dimension_and_non_finite_endpoint_output() {
    for (failure, expected) in [
        ("wrong count", "embedding count mismatch"),
        ("wrong dimension", "invalid embedding vector"),
        ("non-finite", "invalid embedding vector"),
    ] {
        assert!(matches!(
            profile_from_endpoint(&RecordingEndpoint::malformed(failure)),
            Err(ProbeError::InvalidProfile(message)) if message == expected
        ));
    }
}

#[test]
fn simulated_endpoint_preserves_bounded_mode_and_validation_contract() {
    let endpoint = SimulatedEmbeddingEndpoint::new(2, VectorNormalization::L2, 7);
    let inputs = vec!["example".to_string()];
    assert!(endpoint.supports_bounded_embedding());
    let query = endpoint
        .embed_bounded(EmbeddingMode::Query, &inputs, 8)
        .unwrap();
    assert_eq!(
        query,
        endpoint.embed(EmbeddingMode::Query, &inputs).unwrap()
    );
    assert_ne!(
        query,
        endpoint.embed(EmbeddingMode::Document, &inputs).unwrap()
    );
    assert!(matches!(
        endpoint.embed_bounded(EmbeddingMode::Query, &inputs, 7),
        Err(EmbeddingEndpointError::InvalidResponse(
            "bounded output exceeded"
        ))
    ));
    assert!(
        SimulatedEmbeddingEndpoint::new(0, VectorNormalization::L2, 7)
            .embed(EmbeddingMode::Query, &inputs)
            .is_err()
    );
}
