use super::model_config::{
    DREAM, EMBEDDING, ENTITY, ENTITY_DECISION, ENTITY_EXTRACTION, GENERAL, INSOMNIA,
    INSOMNIA_METADATA, RETRIEVAL, RuntimeConfig, SHARED,
};
use super::runtime_probe::{probe_decision, probe_embedding, probe_general};
use super::*;
use std::io::ErrorKind;
use std::path::Path;

#[path = "configured_identity.rs"]
mod identity;

#[derive(Debug, Eq, PartialEq)]
pub enum ConfiguredRuntimeError {
    Io(ErrorKind),
    Write(ErrorKind),
    Authorization(DeviceAuthError),
    MasterKey(MasterKeyError),
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
    HttpStatus(u16),
    InvalidResponse,
    Backpressure { retry_after: Option<Duration> },
}

impl fmt::Display for ConfiguredRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(kind) => write!(f, "runtime configuration could not be read ({kind})"),
            Self::Write(kind) => write!(f, "configuration could not be saved ({kind})"),
            Self::Authorization(error) => error.fmt(f),
            Self::MasterKey(error) => error.fmt(f),
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

impl From<MasterKeyError> for ConfiguredRuntimeError {
    fn from(error: MasterKeyError) -> Self {
        Self::MasterKey(error)
    }
}

impl From<DeviceAuthError> for ConfiguredRuntimeError {
    fn from(error: DeviceAuthError) -> Self {
        Self::Authorization(error)
    }
}

/// The selected Switchboard transports for an offline-validated runtime.
pub struct ConfiguredRoutes {
    general: Option<Arc<dyn GeneralEndpoint>>,
    insomnia: Option<Arc<dyn GeneralEndpoint>>,
    insomnia_metadata: Option<Arc<dyn GeneralEndpoint>>,
    entity_extraction: Option<Arc<dyn GeneralEndpoint>>,
    dream: Option<Arc<dyn GeneralEndpoint>>,
    entity_resolution: Option<Arc<dyn GeneralEndpoint>>,
    embedding: Option<Arc<dyn EmbeddingEndpoint>>,
    chronos: SelectedDecision,
    entity: SelectedDecision,
    retrieval: SelectedDecision,
    processing_route_identity: ProcessingRouteIdentity,
}

#[derive(Clone, Copy)]
pub struct ProcessingRouteIdentity {
    pub import: [u8; 32],
    pub live_dream: [u8; 32],
}

impl ConfiguredRoutes {
    pub fn from_path(path: &Path) -> Result<Self, ConfiguredRuntimeError> {
        ConfigFile::open(path)?.bind()
    }

    pub(super) fn bind(
        config: RuntimeConfig,
        credentials: &CredentialsConfig,
    ) -> Result<Self, ConfiguredRuntimeError> {
        let processing_route_identity = identity::processing_routes(&config, credentials);
        let bind_general = |name, route: Option<&GeneralRoute>| {
            route
                .map(|route| {
                    ConfiguredGeneralEndpoint::from_route_with_reasoning_effort(
                        &route.endpoint,
                        credentials,
                        route.reasoning_effort,
                    )
                    .map(|endpoint| Arc::new(endpoint) as Arc<dyn GeneralEndpoint>)
                })
                .transpose()
                .map_err(|_| ConfiguredRuntimeError::Setup { route: name })
        };
        let general = bind_general(GENERAL, config.general.as_ref())?;
        let insomnia = bind_general(INSOMNIA, config.insomnia.as_ref())?;
        let insomnia_metadata = bind_general(INSOMNIA_METADATA, config.insomnia_metadata.as_ref())?;
        let entity_extraction = bind_general(ENTITY_EXTRACTION, config.entity_extraction.as_ref())?;
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
                ConfiguredEmbeddingEndpoint::from_route(route, credentials)
                    .map(|endpoint| Arc::new(endpoint) as Arc<dyn EmbeddingEndpoint>)
            })
            .transpose()
            .map_err(|_| ConfiguredRuntimeError::Setup { route: EMBEDDING })?;
        let bind_direct = |name, route: Option<&DecisionModelEndpoint>| {
            route
                .map(|route| {
                    ConfiguredDecisionEndpoint::from_route(route, credentials)
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
            insomnia_metadata,
            entity_extraction,
            dream,
            entity_resolution,
            embedding,
            chronos,
            entity,
            retrieval,
            processing_route_identity,
        })
    }

    pub fn general_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.general.clone()
    }

    pub fn insomnia_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.insomnia.clone().or_else(|| self.general_endpoint())
    }

    pub fn insomnia_ownership_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.insomnia_metadata
            .clone()
            .or_else(|| self.insomnia_endpoint())
    }

    pub fn dream_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.dream.clone().or_else(|| self.general_endpoint())
    }

    pub fn entity_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.entity_resolution.clone()
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

    pub fn processing_route_identity(&self) -> ProcessingRouteIdentity {
        self.processing_route_identity
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
            "configured routes: general={}, decision={}, embedding={}; decision selection: chronos={}, entity_resolution={}, memory_retrieval={}; general selection: insomnia={}, dream={}, entity_resolution={}, insomnia_ownership={}",
            self.general.is_some(),
            self.chronos.endpoint.is_some(),
            self.embedding.is_some(),
            self.chronos.label(),
            self.entity.label(),
            self.retrieval.label(),
            general_source(INSOMNIA, &self.insomnia),
            general_source(DREAM, &self.dream),
            if self.entity_resolution.is_some() {
                ENTITY
            } else {
                "unavailable"
            },
            if self.insomnia_metadata.is_some() {
                INSOMNIA_METADATA
            } else {
                general_source(INSOMNIA, &self.insomnia)
            },
        )
    }

    pub fn online_probe(&self) -> Result<String, ConfiguredRuntimeError> {
        let mut probed = 0;
        for (route, transport) in self.probes() {
            transport.check(route)?;
            probed += 1;
        }
        Ok(format!(
            "provider routes probed: {probed}; {}",
            self.readiness()
        ))
    }

    pub fn online_probe_route(&self, selected: &str) -> Result<String, ConfiguredRuntimeError> {
        let (route, transport) = self.probes().find(|(route, _)| *route == selected).ok_or(
            ConfiguredRuntimeError::Invalid {
                route: "provider probe",
                reason: "requested route is not configured",
            },
        )?;
        transport.check(route)?;
        Ok(format!("provider route probed: {route}"))
    }

    fn probes(&self) -> impl Iterator<Item = (&'static str, Probe<'_>)> {
        let general = [
            (GENERAL, &self.general),
            (INSOMNIA, &self.insomnia),
            (INSOMNIA_METADATA, &self.insomnia_metadata),
            (ENTITY_EXTRACTION, &self.entity_extraction),
            (DREAM, &self.dream),
            (ENTITY, &self.entity_resolution),
        ]
        .into_iter()
        .filter_map(|(route, endpoint)| {
            endpoint
                .as_deref()
                .map(|endpoint| (route, Probe::General(endpoint)))
        });
        let decisions = [
            (SHARED, &self.chronos),
            (ENTITY_DECISION, &self.entity),
            (RETRIEVAL, &self.retrieval),
        ]
        .into_iter()
        .filter_map(|(route, selection)| {
            let endpoint = selection.endpoint.as_deref()?;
            (selection.source == Some(route)).then_some((route, Probe::Decision(endpoint)))
        });
        general.chain(decisions).chain(
            self.embedding
                .as_deref()
                .map(|endpoint| (EMBEDDING, Probe::Embedding(endpoint))),
        )
    }
}

enum Probe<'a> {
    General(&'a dyn GeneralEndpoint),
    Decision(&'a dyn DecisionEndpoint),
    Embedding(&'a dyn EmbeddingEndpoint),
}

impl Probe<'_> {
    fn check(self, route: &'static str) -> Result<(), ConfiguredRuntimeError> {
        match self {
            Self::General(endpoint) => probe_general(endpoint),
            Self::Decision(endpoint) => probe_decision(endpoint),
            Self::Embedding(endpoint) => probe_embedding(endpoint),
        }
        .map_err(|failure| ConfiguredRuntimeError::Probe { route, failure })
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
