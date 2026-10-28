pub use reliquary_vector_store::{
    CompatibilityProbeReference, CompatibilityProfile, CompatibilityProfileId,
    CompatibilityProfileStats,
};

pub const COMPATIBILITY_PROBE_SUITE_VERSION: u32 = 1;
pub const COMPATIBILITY_POLICY_VERSION: u32 = 3;
pub const COMPATIBILITY_MIN_COSINE: f64 = 0.99;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompatibilityReport {
    pub compatible: bool,
    pub minimum_cosine: Option<f64>,
}
