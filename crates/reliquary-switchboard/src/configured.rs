use super::runtime_config::{EMBEDDING, ENTITY, GENERAL, RETRIEVAL, RuntimeConfig, SHARED};
use super::runtime_probe::{probe_decision, probe_embedding, probe_general};
use super::*;
use std::io::ErrorKind;
use std::path::Path;

#[derive(Debug, Eq, PartialEq)]
pub enum ConfiguredRuntimeError {
    Io(ErrorKind),
    Invalid {
        route: &'static str,
        reason: &'static str,
    },
    Credential {
        route: &'static str,
        reason: &'static str,
    },
    Setup {
        route: &'static str,
    },
    Probe {
        route: &'static str,
        failure: RuntimeEndpointFailure,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum RuntimeEndpointFailure {
    InvalidConfiguration,
    RequestFailed,
    InvalidResponse,
    Backpressure { retry_after: Option<Duration> },
}

impl fmt::Display for ConfiguredRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(kind) => write!(f, "runtime configuration could not be read ({kind})"),
            Self::Invalid { route, reason } => {
                write!(f, "invalid {route} configuration ({reason})")
            }
            Self::Credential { route, reason } => write!(f, "{route} credential error ({reason})"),
            Self::Setup { route } => write!(f, "{route} endpoint setup failed"),
            Self::Probe { route, failure } => {
                write!(f, "{route} provider probe failed ({failure:?})")
            }
        }
    }
}

impl std::error::Error for ConfiguredRuntimeError {}

/// The selected Switchboard transports for an offline-validated runtime.
pub struct ConfiguredRoutes {
    general: Option<Arc<dyn GeneralEndpoint>>,
    embedding: Option<Arc<dyn EmbeddingEndpoint>>,
    chronos: SelectedDecision,
    entity: SelectedDecision,
    retrieval: SelectedDecision,
    worker_lease_duration_ms: Option<u32>,
}

impl ConfiguredRoutes {
    pub fn from_path(
        path: &Path,
        resolver: &dyn CredentialResolver,
    ) -> Result<Self, ConfiguredRuntimeError> {
        let config = RuntimeConfig::load(path)?;
        let credentials = config.credentials(resolver)?;
        let routes = &config.routes;
        let general: Option<Arc<dyn GeneralEndpoint>> = routes
            .general
            .as_ref()
            .map(|route| {
                ConfiguredGeneralEndpoint::from_route(route, &credentials)
                    .map(|endpoint| Arc::new(endpoint) as Arc<dyn GeneralEndpoint>)
            })
            .transpose()
            .map_err(|_| ConfiguredRuntimeError::Setup { route: GENERAL })?;
        let embedding = config
            .embedding
            .as_ref()
            .map(|route| {
                ConfiguredEmbeddingEndpoint::from_route(route, &credentials)
                    .map(|endpoint| Arc::new(endpoint) as Arc<dyn EmbeddingEndpoint>)
            })
            .transpose()
            .map_err(|_| ConfiguredRuntimeError::Setup { route: EMBEDDING })?;
        let bind_direct = |name, route: Option<&DecisionModelEndpoint>| {
            route
                .map(|route| {
                    ConfiguredDecisionEndpoint::from_route(route, &credentials)
                        .map(|endpoint| Arc::new(endpoint) as Arc<dyn DecisionEndpoint>)
                })
                .transpose()
                .map_err(|_| ConfiguredRuntimeError::Setup { route: name })
        };
        let shared = bind_direct(SHARED, routes.general_decision.as_ref())?;
        let select =
            |consumer, name, dedicated| -> Result<SelectedDecision, ConfiguredRuntimeError> {
                let source = match select_decision_route(routes, consumer) {
                    DecisionRouteSelection::Dedicated(_) => Some(name),
                    DecisionRouteSelection::Shared(_) => Some(SHARED),
                    DecisionRouteSelection::General(_) => Some(GENERAL),
                    DecisionRouteSelection::Unavailable => None,
                };
                let endpoint = resolve_decision_endpoint(
                    routes,
                    consumer,
                    dedicated,
                    shared.clone(),
                    general.clone(),
                )
                .map_err(|_| ConfiguredRuntimeError::Setup {
                    route: source.unwrap_or(name),
                })?;
                Ok(SelectedDecision { source, endpoint })
            };
        let chronos = select(DecisionConsumer::Chronos, SHARED, None)?;
        let entity = select(
            DecisionConsumer::EntityResolution,
            ENTITY,
            bind_direct(ENTITY, routes.entity_resolution_decision.as_ref())?,
        )?;
        let retrieval = select(
            DecisionConsumer::MemoryRetrieval,
            RETRIEVAL,
            bind_direct(RETRIEVAL, routes.memory_retrieval_decision.as_ref())?,
        )?;
        Ok(Self {
            general,
            embedding,
            chronos,
            entity,
            retrieval,
            worker_lease_duration_ms: config.worker_lease_duration_ms,
        })
    }

    pub fn general_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.general.clone()
    }

    pub fn decision_endpoint(
        &self,
        consumer: DecisionConsumer,
    ) -> Option<Arc<dyn DecisionEndpoint>> {
        match consumer {
            DecisionConsumer::Chronos => &self.chronos,
            DecisionConsumer::EntityResolution => &self.entity,
            DecisionConsumer::MemoryRetrieval => &self.retrieval,
        }
        .endpoint
        .clone()
    }

    pub fn embedding_endpoint(&self) -> Option<Arc<dyn EmbeddingEndpoint>> {
        self.embedding.clone()
    }

    pub fn worker_lease_duration_ms(&self) -> Option<u32> {
        self.worker_lease_duration_ms
    }

    pub fn readiness(&self) -> String {
        format!(
            "configured routes: general={}, decision={}, embedding={}; decision selection: chronos={}, entity_resolution={}, memory_retrieval={}",
            self.general.is_some(),
            self.chronos.endpoint.is_some(),
            self.embedding.is_some(),
            self.chronos.label(),
            self.entity.label(),
            self.retrieval.label(),
        )
    }

    pub fn online_probe(&self) -> Result<String, ConfiguredRuntimeError> {
        let error = |route, failure| ConfiguredRuntimeError::Probe { route, failure };
        let mut probed = 0;
        if let Some(endpoint) = &self.general {
            probe_general(endpoint.as_ref()).map_err(|failure| error(GENERAL, failure))?;
            probed += 1;
        }
        for (route, selection) in [
            (SHARED, &self.chronos),
            (ENTITY, &self.entity),
            (RETRIEVAL, &self.retrieval),
        ] {
            if let Some(endpoint) = &selection.endpoint
                && selection.source == Some(route)
            {
                probe_decision(endpoint.as_ref()).map_err(|failure| error(route, failure))?;
                probed += 1;
            }
        }
        if let Some(endpoint) = &self.embedding {
            probe_embedding(endpoint.as_ref()).map_err(|failure| error(EMBEDDING, failure))?;
            probed += 1;
        }
        Ok(format!(
            "provider routes probed: {probed}; {}",
            self.readiness()
        ))
    }
}

struct SelectedDecision {
    source: Option<&'static str>,
    endpoint: Option<Arc<dyn DecisionEndpoint>>,
}

impl SelectedDecision {
    fn label(&self) -> &'static str {
        self.source.unwrap_or("unavailable")
    }
}
