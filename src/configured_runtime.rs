use reliquary_application::{VectorWorkflowError, prepare_compatibility_profile};
use reliquary_runtime_gateway::InsomniaWorkerConfig;
use reliquary_switchboard::{
    ConfiguredRoutes, DecisionConsumer, DecisionEndpoint, EmbeddingEndpoint, GeneralEndpoint,
};
use reliquary_vectors::SwitchboardEmbeddingEndpoint;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use crate::{ConfiguredRuntimeError, ReliquaryConfig, RetrievalConfig};
pub use reliquary_switchboard::RuntimeEndpointFailure;
pub use reliquary_vectors::{
    CompatibilityProbeReference, CompatibilityProfile, EmbeddingMode, VectorNormalization,
};

/// Offline-validated provider bundle for Root workflows.
pub struct ConfiguredRuntime {
    routes: ConfiguredRoutes,
    pub(crate) retrieval: RetrievalConfig,
}

impl fmt::Debug for ConfiguredRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfiguredRuntime")
            .field("readiness", &self.readiness())
            .finish()
    }
}

impl ConfiguredRuntime {
    pub(crate) fn new(routes: ConfiguredRoutes, retrieval: RetrievalConfig) -> Self {
        Self { routes, retrieval }
    }

    /// Opens CVCFG and resolves its OS-backed key and route bindings offline.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ConfiguredRuntimeError> {
        ReliquaryConfig::open(path)?.configured_runtime()
    }

    /// Reports route availability and selection without contacting providers.
    pub fn readiness(&self) -> String {
        self.routes.readiness()
    }

    /// Explicitly probes each selected transport with synthetic input.
    pub fn online_probe(&self) -> Result<String, ConfiguredRuntimeError> {
        self.routes.online_probe()
    }

    /// Probe a single configured route with synthetic input.
    pub fn online_probe_route(&self, route: &str) -> Result<String, ConfiguredRuntimeError> {
        self.routes.online_probe_route(route)
    }

    /// Lease duration for configured worker claims, in nanoseconds.
    pub fn worker_lease_duration_ns(&self) -> i64 {
        InsomniaWorkerConfig::default().lease_duration_ns
    }

    /// Explicitly computes the configured embedding endpoint's compatibility
    /// profile with the existing bounded query and document probes.
    pub fn probe_embedding_profile(
        &self,
    ) -> Result<Option<CompatibilityProfile>, ConfiguredRuntimeError> {
        self.embedding_endpoint()
            .map(|endpoint| {
                prepare_compatibility_profile(&SwitchboardEmbeddingEndpoint::new(endpoint))
            })
            .transpose()
            .map_err(|error| ConfiguredRuntimeError::Probe {
                route: "embedding",
                failure: match error {
                    VectorWorkflowError::IncompatibleEndpoint => {
                        RuntimeEndpointFailure::InvalidConfiguration
                    }
                    _ => RuntimeEndpointFailure::RequestFailed,
                },
            })
    }

    pub(crate) fn general_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.routes.general_endpoint()
    }

    pub(crate) fn insomnia_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.routes.insomnia_endpoint()
    }

    pub(crate) fn insomnia_ownership_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.routes.insomnia_ownership_endpoint()
    }

    pub(crate) fn dream_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.routes.dream_endpoint()
    }

    pub(crate) fn entity_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.routes.entity_endpoint()
    }

    pub(crate) fn decision_endpoint(&self) -> Option<Arc<dyn DecisionEndpoint>> {
        self.routes.decision_endpoint(DecisionConsumer::Chronos)
    }

    pub(crate) fn decision_endpoint_for(
        &self,
        consumer: DecisionConsumer,
    ) -> Option<Arc<dyn DecisionEndpoint>> {
        self.routes.decision_endpoint(consumer)
    }

    pub(crate) fn embedding_endpoint(&self) -> Option<Arc<dyn EmbeddingEndpoint>> {
        self.routes.embedding_endpoint()
    }

    pub(crate) fn processing_route_identity(
        &self,
    ) -> reliquary_switchboard::ProcessingRouteIdentity {
        self.routes.processing_route_identity()
    }

    pub(crate) fn embedding_endpoint_for(&self, name: &str) -> Option<Arc<dyn EmbeddingEndpoint>> {
        (name == "embedding")
            .then(|| self.embedding_endpoint())
            .flatten()
    }
}
