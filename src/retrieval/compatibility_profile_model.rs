pub use reliquary_vector_store::{
    CompatibilityProbeReference, CompatibilityProfile, CompatibilityProfileId,
    CompatibilityProfileStats, EmbeddingMode, VectorNormalization,
};

pub const COMPATIBILITY_PROBE_SUITE_VERSION: u32 = 1;
pub const COMPATIBILITY_POLICY_VERSION: u32 = 3;
pub const COMPATIBILITY_MIN_COSINE: f64 = 0.99;
pub(crate) const COMPATIBILITY_MIN_STORED_POLICY_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompatibilityReport {
    pub compatible: bool,
    pub minimum_cosine: Option<f64>,
}
