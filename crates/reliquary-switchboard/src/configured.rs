use super::runtime_config::{
    DREAM, EMBEDDING, ENTITY, ENTITY_DECISION, GENERAL, GeneralRoute, INSOMNIA, RETRIEVAL,
    RuntimeConfig, SHARED, invalid,
};
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
    insomnia: Option<Arc<dyn GeneralEndpoint>>,
    dream: Option<Arc<dyn GeneralEndpoint>>,
    entity_resolution: Option<Arc<dyn GeneralEndpoint>>,
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
        let credentials = config
            .credentials(|id| resolver.resolve(id).map_err(|_| "credential lookup failed"))?;
        Self::bind(config, credentials)
    }

    /// Reads the selected routes and resolves their protected credentials offline.
    pub fn from_protected_config(
        path: &Path,
        key_path: &Path,
    ) -> Result<Self, ConfiguredRuntimeError> {
        let (config, credentials) = super::protected_config::load(path, key_path)?;
        Self::bind(config, credentials)
    }

    /// Loads protected routes and explicitly uses the dedicated Insomnia route for retrieval.
    pub fn from_protected_config_with_retrieval_from_insomnia(
        path: &Path,
        key_path: &Path,
    ) -> Result<Self, ConfiguredRuntimeError> {
        let mut routes = Self::from_protected_config(path, key_path)?;
        let insomnia = routes
            .insomnia
            .clone()
            .ok_or_else(|| invalid(INSOMNIA, "required for the retrieval override"))?;
        routes.retrieval = SelectedDecision {
            source: Some(INSOMNIA),
            endpoint: Some(Arc::new(GeneralDecisionEndpoint::new(insomnia))),
        };
        Ok(routes)
    }

    fn bind(
        config: RuntimeConfig,
        credentials: CredentialsConfig,
    ) -> Result<Self, ConfiguredRuntimeError> {
        let bind_general = |name, route: Option<&GeneralRoute>| {
            route
                .map(|route| {
                    match route.reasoning_effort {
                        Some(effort) => {
                            ConfiguredGeneralEndpoint::from_route_with_reasoning_effort(
                                &route.endpoint,
                                &credentials,
                                Some(effort),
                            )
                        }
                        None => {
                            ConfiguredGeneralEndpoint::from_route(&route.endpoint, &credentials)
                        }
                    }
                    .map(|endpoint| Arc::new(endpoint) as Arc<dyn GeneralEndpoint>)
                })
                .transpose()
                .map_err(|_| ConfiguredRuntimeError::Setup { route: name })
        };
        let general = bind_general(GENERAL, config.general.as_ref())?;
        let insomnia = bind_general(INSOMNIA, config.insomnia.as_ref())?;
        let dream = bind_general(DREAM, config.dream.as_ref())?;
        let entity_resolution = bind_general(ENTITY, config.entity_resolution.as_ref())?;
        let routes = ModelSwitchboardConfig {
            general: config.general.map(|route| route.endpoint),
            general_decision: config.general_decision,
            entity_resolution_decision: config.entity_resolution_decision,
            memory_retrieval_decision: config.memory_retrieval_decision,
        };
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
                let source = match select_decision_route(&routes, consumer) {
                    DecisionRouteSelection::Dedicated(_) => Some(name),
                    DecisionRouteSelection::Shared(_) => Some(SHARED),
                    DecisionRouteSelection::General(_) => Some(GENERAL),
                    DecisionRouteSelection::Unavailable => None,
                };
                let endpoint = resolve_decision_endpoint(
                    &routes,
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
            ENTITY_DECISION,
            bind_direct(ENTITY_DECISION, routes.entity_resolution_decision.as_ref())?,
        )?;
        let retrieval = select(
            DecisionConsumer::MemoryRetrieval,
            RETRIEVAL,
            bind_direct(RETRIEVAL, routes.memory_retrieval_decision.as_ref())?,
        )?;
        Ok(Self {
            general,
            insomnia,
            dream,
            entity_resolution,
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

    pub fn insomnia_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.insomnia.clone().or_else(|| self.general_endpoint())
    }

    pub fn dream_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.dream.clone().or_else(|| self.general_endpoint())
    }

    pub fn entity_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.entity_resolution
            .clone()
            .or_else(|| self.general_endpoint())
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
        let general_source = |name, endpoint: &Option<Arc<dyn GeneralEndpoint>>| {
            if endpoint.is_some() {
                name
            } else if self.general.is_some() {
                GENERAL
            } else {
                "unavailable"
            }
        };
        format!(
            "configured routes: general={}, decision={}, embedding={}; decision selection: chronos={}, entity_resolution={}, memory_retrieval={}; general selection: insomnia={}, dream={}, entity_resolution={}",
            self.general.is_some(),
            self.chronos.endpoint.is_some(),
            self.embedding.is_some(),
            self.chronos.label(),
            self.entity.label(),
            self.retrieval.label(),
            general_source(INSOMNIA, &self.insomnia),
            general_source(DREAM, &self.dream),
            general_source(ENTITY, &self.entity_resolution),
        )
    }

    pub fn online_probe(&self) -> Result<String, ConfiguredRuntimeError> {
        let error = |route, failure| ConfiguredRuntimeError::Probe { route, failure };
        let mut probed = 0;
        for (route, endpoint) in [
            (GENERAL, &self.general),
            (INSOMNIA, &self.insomnia),
            (DREAM, &self.dream),
            (ENTITY, &self.entity_resolution),
        ] {
            if let Some(endpoint) = endpoint {
                probe_general(endpoint.as_ref()).map_err(|failure| error(route, failure))?;
                probed += 1;
            }
        }
        for (route, selection) in [
            (SHARED, &self.chronos),
            (ENTITY_DECISION, &self.entity),
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
