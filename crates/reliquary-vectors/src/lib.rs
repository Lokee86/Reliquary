//! Pure embedding compatibility inference and deterministic batch validation.
//! Durable profile and vector writes remain with their owner stores.
mod compatibility_vector;
mod probe_error;
mod profile_probe;
pub use compatibility_vector::{cosine, validate_vectors};
pub use probe_error::{
    EmbeddingEndpoint, EmbeddingEndpointError, ProbeError, SwitchboardEmbeddingEndpoint,
};
pub use profile_probe::{
    DOCUMENT_PROBES, QUERY_PROBES, compare_profiles, compatibility_profile_id, embed_batch_bounded,
    profile_from_endpoint, validate_embedding_batch, verify_endpoint,
};
pub use reliquary_vector_store::{
    CompatibilityProbeReference, CompatibilityProfile, CompatibilityProfileId,
    CompatibilityProfileStats, EmbeddingMode, VectorNormalization,
};
pub const COMPATIBILITY_PROBE_SUITE_VERSION: u32 = 1;
pub const COMPATIBILITY_POLICY_VERSION: u32 = 3;
pub const COMPATIBILITY_MIN_COSINE: f64 = 0.99;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompatibilityReport {
    pub compatible: bool,
    pub minimum_cosine: Option<f64>,
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod contract_tests;
