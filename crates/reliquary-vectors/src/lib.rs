mod embedding_endpoint;

pub use embedding_endpoint::{
    EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, SimulatedEmbeddingEndpoint,
    VectorNormalization,
};

mod compatibility_profile_model;
pub mod compatibility_profile_probe;
pub mod compatibility_vector;
mod probe_error;
pub use compatibility_profile_model::{
    COMPATIBILITY_MIN_COSINE, COMPATIBILITY_POLICY_VERSION, COMPATIBILITY_PROBE_SUITE_VERSION,
    CompatibilityProbeReference, CompatibilityProfile, CompatibilityProfileId,
    CompatibilityProfileStats, CompatibilityReport,
};
pub use probe_error::ProbeError;

mod batch_packing;
pub use batch_packing::{BatchPackingError, pack_embedding_batch};
#[cfg(test)]
mod contract_tests;
