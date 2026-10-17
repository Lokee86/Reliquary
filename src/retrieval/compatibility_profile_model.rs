use crate::{EmbeddingMode, VectorNormalization};

pub const COMPATIBILITY_PROBE_SUITE_VERSION: u32 = 1;
pub const COMPATIBILITY_POLICY_VERSION: u32 = 3;
pub const COMPATIBILITY_MIN_COSINE: f64 = 0.99;
pub(crate) const COMPATIBILITY_MIN_STORED_POLICY_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CompatibilityProfileId(pub [u8; 32]);

#[derive(Clone, Debug, PartialEq)]
pub struct CompatibilityProbeReference {
    pub mode: EmbeddingMode,
    pub vector: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompatibilityProfile {
    pub id: CompatibilityProfileId,
    pub dimensions: u32,
    pub normalization: VectorNormalization,
    pub probe_suite_version: u32,
    pub compatibility_policy_version: u32,
    pub references: Vec<CompatibilityProbeReference>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompatibilityReport {
    pub compatible: bool,
    pub minimum_cosine: Option<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompatibilityProfileStats {
    pub profiles: usize,
    pub active_profiles: usize,
    pub retired_profiles: usize,
}
