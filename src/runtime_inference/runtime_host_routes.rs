use crate::{DecisionEndpoint, EmbeddingEndpoint, GeneralEndpoint};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct ReliquaryRuntimeRoutes {
    general: Option<Arc<dyn GeneralEndpoint>>,
    insomnia: Option<Arc<dyn GeneralEndpoint>>,
    insomnia_metadata: Option<Arc<dyn GeneralEndpoint>>,
    entity_extraction: Option<Arc<dyn GeneralEndpoint>>,
    entity_resolution_decision: Option<Arc<dyn DecisionEndpoint>>,
    memory_retrieval_decision: Option<Arc<dyn DecisionEndpoint>>,
    entity_resolution: Option<Arc<dyn GeneralEndpoint>>,
    chronos: Option<Arc<dyn GeneralEndpoint>>,
    dream: Option<Arc<dyn GeneralEndpoint>>,
    embedding: Option<Arc<dyn EmbeddingEndpoint + Send + Sync>>,
}

impl ReliquaryRuntimeRoutes {
    pub fn new(
        general: Option<Arc<dyn GeneralEndpoint>>,
        insomnia: Option<Arc<dyn GeneralEndpoint>>,
        insomnia_metadata: Option<Arc<dyn GeneralEndpoint>>,
        dream: Option<Arc<dyn GeneralEndpoint>>,
        embedding: Option<Arc<dyn EmbeddingEndpoint + Send + Sync>>,
    ) -> Self {
        Self {
            general,
            insomnia,
            insomnia_metadata,
            entity_extraction: None,
            entity_resolution_decision: None,
            memory_retrieval_decision: None,
            entity_resolution: None,
            chronos: None,
            dream,
            embedding,
        }
    }

    pub(crate) fn insomnia(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.insomnia.clone().or_else(|| self.general.clone())
    }

    pub(crate) fn insomnia_metadata(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.insomnia_metadata.clone()
    }

    pub(crate) fn insomnia_ownership(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.insomnia_metadata().or_else(|| self.insomnia())
    }

    pub fn with_entity_routes(
        mut self,
        extraction: Option<Arc<dyn GeneralEndpoint>>,
        resolution: Option<Arc<dyn GeneralEndpoint>>,
    ) -> Self {
        self.entity_extraction = extraction;
        self.entity_resolution = resolution;
        self
    }

    pub(crate) fn entity_extraction(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.entity_extraction.clone()
    }

    pub fn with_entity_resolution_decision(
        mut self,
        endpoint: Option<Arc<dyn DecisionEndpoint>>,
    ) -> Self {
        self.entity_resolution_decision = endpoint;
        self
    }

    pub(crate) fn entity_resolution_decision(&self) -> Option<Arc<dyn DecisionEndpoint>> {
        self.entity_resolution_decision.clone()
    }

    pub fn with_memory_retrieval_decision(
        mut self,
        endpoint: Option<Arc<dyn DecisionEndpoint>>,
    ) -> Self {
        self.memory_retrieval_decision = endpoint;
        self
    }

    pub(crate) fn memory_retrieval_decision(&self) -> Option<Arc<dyn DecisionEndpoint>> {
        self.memory_retrieval_decision.clone()
    }

    pub(crate) fn entity_resolution(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.entity_resolution.clone()
    }

    pub fn with_chronos(mut self, endpoint: Option<Arc<dyn GeneralEndpoint>>) -> Self {
        self.chronos = endpoint;
        self
    }

    pub(crate) fn chronos(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.chronos.clone()
    }

    pub(crate) fn dream(&self) -> Option<Arc<dyn GeneralEndpoint>> {
        self.dream.clone().or_else(|| self.general.clone())
    }

    pub fn embedding(&self) -> Option<Arc<dyn EmbeddingEndpoint + Send + Sync>> {
        self.embedding.clone()
    }
}
