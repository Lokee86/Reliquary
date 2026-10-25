use crate::entity_resolution_engine::EntityResolutionEngineCore;
use crate::{DecisionEndpoint, GeneralEndpoint};
use std::sync::Arc;

pub struct EntityResolutionEngine<E> {
    pub(crate) core: EntityResolutionEngineCore<E>,
}

impl<E: GeneralEndpoint> EntityResolutionEngine<E> {
    pub fn new(fallback: E) -> Self {
        Self {
            core: crate::public_entity_resolution_bindings::gateway_entity_resolution_engine_new(
                fallback,
            ),
        }
    }

    pub fn with_decision_endpoint(mut self, decision: Option<Arc<dyn DecisionEndpoint>>) -> Self {
        self.core = crate::public_entity_resolution_bindings::gateway_entity_resolution_engine_with_decision_endpoint(
            self.core,
            decision,
        );
        self
    }

    pub fn decision_model(&self) -> Option<&str> {
        crate::public_entity_resolution_bindings::gateway_entity_resolution_engine_decision_model(
            &self.core,
        )
    }

    pub fn fallback_model(&self) -> &str {
        crate::public_entity_resolution_bindings::gateway_entity_resolution_engine_fallback_model(
            &self.core,
        )
    }
}
