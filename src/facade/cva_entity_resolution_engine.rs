use crate::entity_resolution_engine::EntityResolutionEngineCore;
use crate::{DecisionEndpoint, GeneralEndpoint};
use std::sync::Arc;

/// Public engine binding. Selection and resolution policy remain in the Perception core.
pub struct EntityResolutionEngine<E> {
    pub(crate) core: EntityResolutionEngineCore<E>,
}

impl<E: GeneralEndpoint> EntityResolutionEngine<E> {
    pub fn new(fallback: E) -> Self {
        Self {
            core: EntityResolutionEngineCore::new(fallback),
        }
    }

    pub fn with_decision_endpoint(mut self, decision: Option<Arc<dyn DecisionEndpoint>>) -> Self {
        self.core = self.core.with_decision_endpoint(decision);
        self
    }

    pub fn decision_model(&self) -> Option<&str> {
        self.core.decision_model()
    }

    pub fn fallback_model(&self) -> &str {
        self.core.fallback_model()
    }
}
