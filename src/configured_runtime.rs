use reliquary_application::{VectorWorkflowError, prepare_compatibility_profile};
use reliquary_runtime_gateway::InsomniaWorkerConfig;
use reliquary_switchboard::{
    ConfiguredRoutes, DecisionConsumer, DecisionEndpoint, EmbeddingEndpoint, GeneralEndpoint,
};
use reliquary_vectors::SwitchboardEmbeddingEndpoint;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

pub use reliquary_switchboard::{
    ConfiguredRuntimeError, CredentialError, CredentialId, CredentialMaterial, CredentialResolver,
    EnvironmentCredentialResolver, RuntimeEndpointFailure,
};
pub use reliquary_vectors::{
    CompatibilityProbeReference, CompatibilityProfile, EmbeddingMode, VectorNormalization,
};

/// Offline-validated provider bundle for Root workflows.
pub struct ConfiguredRuntime {
    routes: ConfiguredRoutes,
}

impl fmt::Debug for ConfiguredRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfiguredRuntime")
            .field("readiness", &self.readiness())
            .finish()
    }
}

impl ConfiguredRuntime {
    /// Loads route definitions and resolves their credential references offline.
    pub fn from_path(
        path: impl AsRef<Path>,
        resolver: &dyn CredentialResolver,
    ) -> Result<Self, ConfiguredRuntimeError> {
        let routes = ConfiguredRoutes::from_path(path.as_ref(), resolver)?;
        Ok(Self { routes })
    }

    /// Loads selected routes and encrypted credentials from an existing protected config.
    pub fn from_protected_config(
        path: impl AsRef<Path>,
        key_path: impl AsRef<Path>,
    ) -> Result<Self, ConfiguredRuntimeError> {
        let routes = ConfiguredRoutes::from_protected_config(path.as_ref(), key_path.as_ref())?;
        Ok(Self { routes })
    }

    /// Loads protected routes and explicitly uses the dedicated Insomnia route for retrieval.
    ///
    /// Returns an error if the Insomnia route is absent or cannot be loaded.
    pub fn from_protected_config_with_retrieval_from_insomnia(
        path: impl AsRef<Path>,
        key_path: impl AsRef<Path>,
    ) -> Result<Self, ConfiguredRuntimeError> {
        let routes = ConfiguredRoutes::from_protected_config_with_retrieval_from_insomnia(
            path.as_ref(),
            key_path.as_ref(),
        )?;
        Ok(Self { routes })
    }

    /// Reports route availability and selection without contacting providers.
    pub fn readiness(&self) -> String {
        self.routes.readiness()
    }

    /// Explicitly probes each selected transport with synthetic input.
    pub fn online_probe(&self) -> Result<String, ConfiguredRuntimeError> {
        self.routes.online_probe()
    }

    /// Lease duration for configured worker claims, in nanoseconds.
    pub fn worker_lease_duration_ns(&self) -> i64 {
        self.routes
            .worker_lease_duration_ms()
            .map(|millis| i64::from(millis) * 1_000_000)
            .unwrap_or_else(|| InsomniaWorkerConfig::default().lease_duration_ns)
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
}
