use super::model_config::{
    DREAM, EMBEDDING, ENTITY, ENTITY_DECISION, ENTITY_EXTRACTION, GENERAL, INSOMNIA,
    INSOMNIA_METADATA, RETRIEVAL, RuntimeConfig, SHARED,
};
use super::runtime_probe::{probe_decision, probe_embedding, probe_general};
use super::*;
use super::{HostedEmbeddingSettings, HostedModelRoute, HostedSwitchboardConfig};
use serde_json::{Value, json};
use std::io::ErrorKind;
use std::path::Path;

const NATIVE_PERCEPTION_DECISION: &str = "native-perception-decision";

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
    observation: SelectedDecision,
    hosted_perception_identity: Option<[u8; 32]>,
    processing_route_identity: ProcessingRouteIdentity,
}

#[derive(Clone, Copy)]
pub struct ProcessingRouteIdentity {
    pub entity_identity: [u8; 32],
    pub import: [u8; 32],
    pub live_dream: [u8; 32],
    pub perception: [u8; 32],
    pub perception_embedding: [u8; 32],
}

/// Query and document input handling on a bound embedding route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedEmbeddingInputSemantics {
    /// Both inputs are sent unchanged to a shared-space endpoint.
    Shared,
    /// Switchboard sends search_query and search_document respectively.
    SearchQueryDocument,
}

impl HostedEmbeddingInputSemantics {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::SearchQueryDocument => "search_query_document",
        }
    }
}

/// Stable, non-secret identity of Hermes-selected hosted inference models.
///
/// Credential rotation, profile names, and host leases are intentionally absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostedInferenceIdentity {
    general_provider: String,
    general_model: String,
    embedding_provider: String,
    embedding_model: String,
    embedding_dimensions: u32,
    embedding_input_semantics: HostedEmbeddingInputSemantics,
    embedding_normalization: EmbeddingNormalization,
}

impl HostedInferenceIdentity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        general_provider: impl Into<String>,
        general_model: impl Into<String>,
        embedding_provider: impl Into<String>,
        embedding_model: impl Into<String>,
        embedding_dimensions: u32,
        embedding_input_semantics: HostedEmbeddingInputSemantics,
        embedding_normalization: EmbeddingNormalization,
    ) -> Result<Self, ConfiguredRuntimeError> {
        let general_provider = general_provider.into();
        let general_model = general_model.into();
        let embedding_provider = embedding_provider.into();
        let embedding_model = embedding_model.into();
        let values = [
            general_provider.as_str(),
            general_model.as_str(),
            embedding_provider.as_str(),
            embedding_model.as_str(),
        ];
        if values.iter().any(|value| {
            value.is_empty() || value.len() > 256 || value.chars().any(char::is_control)
        }) || embedding_dimensions == 0
        {
            return Err(ConfiguredRuntimeError::Invalid {
                route: "hosted inference identity",
                reason: "provider, model, or embedding dimensions are invalid",
            });
        }
        Ok(Self {
            general_provider,
            general_model,
            embedding_provider,
            embedding_model,
            embedding_dimensions,
            embedding_input_semantics,
            embedding_normalization,
        })
    }

    pub fn general_provider(&self) -> &str {
        &self.general_provider
    }

    pub fn general_model(&self) -> &str {
        &self.general_model
    }

    pub fn embedding_provider(&self) -> &str {
        &self.embedding_provider
    }

    pub fn embedding_model(&self) -> &str {
        &self.embedding_model
    }

    pub fn embedding_dimensions(&self) -> u32 {
        self.embedding_dimensions
    }

    pub fn embedding_input_semantics(&self) -> HostedEmbeddingInputSemantics {
        self.embedding_input_semantics
    }

    pub fn embedding_normalization(&self) -> EmbeddingNormalization {
        self.embedding_normalization
    }
}

impl ConfiguredRoutes {
    pub fn from_path(path: &Path) -> Result<Self, ConfiguredRuntimeError> {
        ConfigFile::open(path)?.bind()
    }

    /// Bind a single Hermes-hosted general and embedding transport without reading CVCFG.
    pub fn hosted(
        general: Arc<dyn GeneralEndpoint>,
        embedding: Arc<dyn EmbeddingEndpoint>,
        identity: HostedInferenceIdentity,
    ) -> Result<Self, ConfiguredRuntimeError> {
        if general.model() != identity.general_model() {
            return Err(ConfiguredRuntimeError::Invalid {
                route: "hosted general endpoint",
                reason: "endpoint model does not match the bound identity",
            });
        }
        if embedding.model() != Some(identity.embedding_model())
            || embedding.dimensions() != identity.embedding_dimensions()
            || embedding.normalization() != identity.embedding_normalization()
        {
            return Err(ConfiguredRuntimeError::Invalid {
                route: "hosted embedding endpoint",
                reason: "endpoint model, dimensions, or normalization do not match the bound identity",
            });
        }
        let decision: Arc<dyn DecisionEndpoint> =
            Arc::new(GeneralDecisionEndpoint::new(general.clone()));
        let processing_route_identity = identity::hosted(&identity);
        Ok(Self {
            general: Some(general.clone()),
            insomnia: None,
            insomnia_metadata: None,
            entity_extraction: None,
            dream: None,
            entity_resolution: Some(general),
            embedding: Some(embedding),
            chronos: SelectedDecision {
                source: Some(GENERAL),
                endpoint: Some(decision.clone()),
            },
            entity: SelectedDecision {
                source: Some(GENERAL),
                endpoint: Some(decision.clone()),
            },
            retrieval: SelectedDecision {
                source: Some(GENERAL),
                endpoint: Some(decision),
            },
            observation: SelectedDecision {
                source: None,
                endpoint: None,
            },
            hosted_perception_identity: Some(processing_route_identity.perception),
            processing_route_identity,
        })
    }

    /// Resolve Switchboard-selected host routes into host-owned endpoint adapters.
    /// Resolvers must be local and side-effect-free; provider calls belong to
    /// endpoint invocation or explicit runtime probes, never route binding.
    pub fn hosted_switchboard<C, E>(
        config: &HostedSwitchboardConfig,
        mut completion_resolver: C,
        mut embedding_resolver: E,
    ) -> Result<Self, ConfiguredRuntimeError>
    where
        C: FnMut(
            ModelSlot,
            &HostedModelRoute,
        ) -> Result<(String, Arc<dyn GeneralEndpoint>), ConfiguredRuntimeError>,
        E: FnMut(
            ModelSlot,
            &HostedModelRoute,
        ) -> Result<(String, Arc<dyn EmbeddingEndpoint>), ConfiguredRuntimeError>,
    {
        let general = resolve_hosted_general(config, ModelSlot::General, &mut completion_resolver)?;
        let insomnia =
            resolve_hosted_general(config, ModelSlot::Insomnia, &mut completion_resolver)?;
        let insomnia_metadata = resolve_hosted_general(
            config,
            ModelSlot::InsomniaMetadata,
            &mut completion_resolver,
        )?;
        let entity_extraction = resolve_hosted_general(
            config,
            ModelSlot::EntityExtraction,
            &mut completion_resolver,
        )?;
        let dream = resolve_hosted_general(config, ModelSlot::Dream, &mut completion_resolver)?;
        let entity_resolution = resolve_hosted_general(
            config,
            ModelSlot::EntityResolution,
            &mut completion_resolver,
        )?;
        let general_decision =
            resolve_hosted_general(config, ModelSlot::GeneralDecision, &mut completion_resolver)?;
        let entity_decision = resolve_hosted_general(
            config,
            ModelSlot::EntityResolutionDecision,
            &mut completion_resolver,
        )?;
        let retrieval_decision = resolve_hosted_general(
            config,
            ModelSlot::MemoryRetrievalDecision,
            &mut completion_resolver,
        )?;
        let embedding = resolve_hosted_embedding(config, &mut embedding_resolver)?;

        let shared_adapter = general_decision.as_ref().map(|route| {
            Arc::new(GeneralDecisionEndpoint::new(route.endpoint.clone()))
                as Arc<dyn DecisionEndpoint>
        });
        let entity_adapter = entity_decision.as_ref().map(|route| {
            Arc::new(GeneralDecisionEndpoint::new(route.endpoint.clone()))
                as Arc<dyn DecisionEndpoint>
        });
        let retrieval_adapter = retrieval_decision.as_ref().map(|route| {
            Arc::new(GeneralDecisionEndpoint::new(route.endpoint.clone()))
                as Arc<dyn DecisionEndpoint>
        });
        let decision_selection = |dedicated_source,
                                  dedicated_adapter: Option<Arc<dyn DecisionEndpoint>>|
         -> Result<SelectedDecision, ConfiguredRuntimeError> {
            let source_kind = select_decision_source(
                dedicated_adapter.is_some(),
                shared_adapter.is_some(),
                general.is_some(),
            );
            let (source, endpoint) = match source_kind {
                DecisionRouteSource::Dedicated => (dedicated_source, dedicated_adapter),
                DecisionRouteSource::Shared => (Some(SHARED), shared_adapter.clone()),
                DecisionRouteSource::General => (
                    Some(GENERAL),
                    general.as_ref().map(|route| {
                        Arc::new(GeneralDecisionEndpoint::new(route.endpoint.clone()))
                            as Arc<dyn DecisionEndpoint>
                    }),
                ),
                DecisionRouteSource::Unavailable => (None, None),
            };
            if source_kind != DecisionRouteSource::Unavailable && endpoint.is_none() {
                return Err(ConfiguredRuntimeError::Setup {
                    route: source.unwrap_or(SHARED),
                });
            }
            Ok(SelectedDecision { source, endpoint })
        };
        let chronos = decision_selection(None, None)?;
        let entity = decision_selection(Some(ENTITY_DECISION), entity_adapter)?;
        let retrieval = decision_selection(Some(RETRIEVAL), retrieval_adapter)?;

        let legacy_compatible = hosted_legacy_identity(
            general.as_ref(),
            insomnia.as_ref(),
            insomnia_metadata.as_ref(),
            dream.as_ref(),
            entity_resolution.as_ref(),
            entity_extraction.as_ref(),
            embedding.as_ref(),
        );
        let processing_route_identity = if let Some(identity) = legacy_compatible {
            identity::hosted(&identity)
        } else {
            let effective_insomnia = insomnia.as_ref().or(general.as_ref());
            let effective_ownership = insomnia_metadata.as_ref().or(effective_insomnia);
            let effective_dream = dream.as_ref().or(general.as_ref());
            let entity_identity = hosted_route_identity(entity_resolution.as_ref());
            let embedding_identity = embedding
                .as_ref()
                .map(|route| {
                    let settings = &route.settings;
                    json!([
                        route.endpoint_ref,
                        route.provider,
                        route.model,
                        "shared",
                        normalization_name(settings.normalization),
                        settings.dimensions
                    ])
                })
                .unwrap_or(Value::Null);
            identity::hosted_switchboard(
                hosted_route_identity(general.as_ref()),
                entity_identity,
                hosted_route_identity(effective_insomnia),
                hosted_route_identity(effective_ownership),
                hosted_route_identity(effective_dream),
                hosted_route_identity(entity_extraction.as_ref()),
                embedding_identity,
            )
        };

        Ok(Self {
            general: general.map(|route| route.endpoint),
            insomnia: insomnia.map(|route| route.endpoint),
            insomnia_metadata: insomnia_metadata.map(|route| route.endpoint),
            entity_extraction: entity_extraction.map(|route| route.endpoint),
            dream: dream.map(|route| route.endpoint),
            entity_resolution: entity_resolution.map(|route| route.endpoint),
            embedding: embedding.map(|route| route.endpoint),
            chronos,
            entity,
            retrieval,
            observation: SelectedDecision {
                source: None,
                endpoint: None,
            },
            hosted_perception_identity: Some(processing_route_identity.perception),
            processing_route_identity,
        })
    }

    /// Bind one native endpoint for hosted Observation Contribution and Verify.
    pub fn with_native_perception_decision(
        mut self,
        endpoint: Arc<dyn DecisionEndpoint>,
        route_identity: &str,
    ) -> Result<Self, ConfiguredRuntimeError> {
        let invalid = |reason| ConfiguredRuntimeError::Invalid {
            route: NATIVE_PERCEPTION_DECISION,
            reason,
        };
        let base_identity = self
            .hosted_perception_identity
            .ok_or_else(|| invalid("runtime is not hosted"))?;
        if endpoint.model() != OBSERVATION_DECISION_MODEL {
            return Err(invalid("Observation requires typesafe/jev-1.13"));
        }
        if route_identity.trim().is_empty()
            || route_identity.len() > 256
            || route_identity.chars().any(char::is_control)
        {
            return Err(invalid("invalid host route identity"));
        }
        self.processing_route_identity.perception =
            identity::hosted_native_perception(base_identity, route_identity);
        self.observation = SelectedDecision {
            source: Some(NATIVE_PERCEPTION_DECISION),
            endpoint: Some(endpoint),
        };
        Ok(self)
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
        let observation = select(DecisionConsumer::ObservationContribution, SHARED, None)?;
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
            observation,
            hosted_perception_identity: None,
            processing_route_identity,
        })
    }

    pub fn general_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.general.clone()
    }

    pub fn insomnia_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.insomnia.clone().or_else(|| self.general_endpoint())
    }

    pub fn entity_extraction_endpoint(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.entity_extraction.clone()
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
            DecisionConsumer::ObservationContribution
            | DecisionConsumer::ObservationVerification => &self.observation,
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
            "configured routes: general={}, decision={}, embedding={}; decision selection: chronos={}, entity_resolution={}, memory_retrieval={}, observation_contribution={}, observation_verification={}; general selection: insomnia={}, dream={}, entity_resolution={}, insomnia_ownership={}",
            self.general.is_some(),
            self.chronos.endpoint.is_some(),
            self.embedding.is_some(),
            self.chronos.label(),
            self.entity.label(),
            self.retrieval.label(),
            self.observation.label(),
            self.observation.label(),
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
            (NATIVE_PERCEPTION_DECISION, &self.observation),
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

struct HostedGeneral {
    endpoint: Arc<dyn GeneralEndpoint>,
    endpoint_ref: String,
    provider: String,
    model: String,
    reasoning_effort: Option<String>,
}

struct HostedEmbedding {
    endpoint: Arc<dyn EmbeddingEndpoint>,
    endpoint_ref: String,
    provider: String,
    model: String,
    settings: HostedEmbeddingSettings,
}

fn resolve_hosted_general<F>(
    config: &HostedSwitchboardConfig,
    slot: ModelSlot,
    resolver: &mut F,
) -> Result<Option<HostedGeneral>, ConfiguredRuntimeError>
where
    F: FnMut(
        ModelSlot,
        &HostedModelRoute,
    ) -> Result<(String, Arc<dyn GeneralEndpoint>), ConfiguredRuntimeError>,
{
    let Some(route) = config.model(slot) else {
        return Ok(None);
    };
    let (provider, endpoint) = resolver(slot, route)?;
    let model = endpoint.model().to_owned();
    validate_hosted_identity(slot, &provider, &model)?;
    if route
        .model
        .as_ref()
        .is_some_and(|selected| selected != &model)
    {
        return Err(hosted_route_error(
            slot,
            "host endpoint resolved a different model",
        ));
    }
    Ok(Some(HostedGeneral {
        endpoint,
        endpoint_ref: route.endpoint.clone(),
        provider,
        model,
        reasoning_effort: route.reasoning_effort.clone(),
    }))
}

fn resolve_hosted_embedding<F>(
    config: &HostedSwitchboardConfig,
    resolver: &mut F,
) -> Result<Option<HostedEmbedding>, ConfiguredRuntimeError>
where
    F: FnMut(
        ModelSlot,
        &HostedModelRoute,
    ) -> Result<(String, Arc<dyn EmbeddingEndpoint>), ConfiguredRuntimeError>,
{
    let Some(route) = config.model(ModelSlot::Embedding) else {
        return Ok(None);
    };
    let settings = route
        .embedding
        .clone()
        .ok_or(ConfiguredRuntimeError::Invalid {
            route: EMBEDDING,
            reason: "embedding route has no compatibility settings",
        })?;
    let (provider, endpoint) = resolver(ModelSlot::Embedding, route)?;
    let model = endpoint
        .model()
        .ok_or(ConfiguredRuntimeError::Invalid {
            route: EMBEDDING,
            reason: "host embedding endpoint has no resolved model",
        })?
        .to_owned();
    validate_hosted_identity(ModelSlot::Embedding, &provider, &model)?;
    if route
        .model
        .as_ref()
        .is_some_and(|selected| selected != &model)
        || endpoint.dimensions() != settings.dimensions
        || endpoint.normalization() != settings.normalization
    {
        return Err(hosted_route_error(
            ModelSlot::Embedding,
            "host embedding endpoint does not match the selected route",
        ));
    }
    Ok(Some(HostedEmbedding {
        endpoint,
        endpoint_ref: route.endpoint.clone(),
        provider,
        model,
        settings,
    }))
}

fn validate_hosted_identity(
    slot: ModelSlot,
    provider: &str,
    model: &str,
) -> Result<(), ConfiguredRuntimeError> {
    if provider.is_empty()
        || provider.len() > 256
        || provider.chars().any(char::is_control)
        || model.is_empty()
        || model.len() > 256
        || model.chars().any(char::is_control)
    {
        return Err(hosted_route_error(
            slot,
            "host endpoint returned invalid route identity",
        ));
    }
    Ok(())
}

fn hosted_route_error(slot: ModelSlot, reason: &'static str) -> ConfiguredRuntimeError {
    ConfiguredRuntimeError::Invalid {
        route: slot.label(),
        reason,
    }
}

fn hosted_route_identity(route: Option<&HostedGeneral>) -> Value {
    route
        .map(|route| {
            json!([
                route.endpoint_ref.as_str(),
                route.provider.as_str(),
                route.model.as_str(),
                route.reasoning_effort.as_deref()
            ])
        })
        .unwrap_or(Value::Null)
}

fn normalization_name(normalization: EmbeddingNormalization) -> &'static str {
    match normalization {
        EmbeddingNormalization::None => "none",
        EmbeddingNormalization::L2 => "l2",
    }
}

fn hosted_legacy_identity(
    general: Option<&HostedGeneral>,
    insomnia: Option<&HostedGeneral>,
    metadata: Option<&HostedGeneral>,
    dream: Option<&HostedGeneral>,
    entity: Option<&HostedGeneral>,
    extraction: Option<&HostedGeneral>,
    embedding: Option<&HostedEmbedding>,
) -> Option<HostedInferenceIdentity> {
    let general = general?;
    let embedding = embedding?;
    if general.endpoint_ref != "chat"
        || general.reasoning_effort.is_some()
        || insomnia.is_some()
        || metadata.is_some()
        || dream.is_some()
        || extraction.is_some()
        || !entity.is_some_and(|route| {
            route.endpoint_ref == "chat"
                && route.endpoint_ref == general.endpoint_ref
                && route.provider == general.provider
                && route.model == general.model
                && route.reasoning_effort.is_none()
        })
    {
        return None;
    }
    if embedding.settings.normalization != EmbeddingNormalization::None {
        return None;
    }
    HostedInferenceIdentity::new(
        general.provider.clone(),
        general.model.clone(),
        embedding.provider.clone(),
        embedding.model.clone(),
        embedding.settings.dimensions,
        HostedEmbeddingInputSemantics::Shared,
        embedding.settings.normalization,
    )
    .ok()
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
