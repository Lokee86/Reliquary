use crate::entity_admission::{is_bare_generic_surface, is_obvious_transient_occurrence};
use crate::entity_resolution_decision::try_decision_fast_path;
use crate::{
    DecisionEndpoint, EntityResolutionEvaluation, EntityResolutionPrepared, EntityResolver,
    EntityResolverError, GeneralEndpoint,
};
use std::sync::Arc;

pub struct EntityResolutionEngine<E> {
    fallback: EntityResolver<E>,
    decision: Option<Arc<dyn DecisionEndpoint>>,
}

impl<E: GeneralEndpoint> EntityResolutionEngine<E> {
    pub fn new(fallback: E) -> Self {
        Self {
            fallback: EntityResolver::new(fallback),
            decision: None,
        }
    }

    pub fn with_decision_endpoint(mut self, decision: Option<Arc<dyn DecisionEndpoint>>) -> Self {
        self.decision = decision;
        self
    }

    pub fn decision_model(&self) -> Option<&str> {
        self.decision.as_ref().map(|endpoint| endpoint.model())
    }

    pub fn fallback_model(&self) -> &str {
        self.fallback.model()
    }

    pub fn evaluate_prepared(
        &self,
        prepared: &EntityResolutionPrepared<'_>,
    ) -> Result<EntityResolutionEvaluation, EntityResolverError> {
        if is_bare_generic_surface(&prepared.candidates.mention.text) {
            return Ok(EntityResolutionEvaluation {
                output: crate::EntityResolverOutput {
                    decision: crate::EntityResolutionDecision::Reject,
                    reason: crate::EntityResolutionReason::GenericRole,
                },
                materialization: None,
            });
        }

        if is_obvious_transient_occurrence(&prepared.memory, &prepared.candidates) {
            return Ok(EntityResolutionEvaluation {
                output: crate::EntityResolverOutput {
                    decision: crate::EntityResolutionDecision::Reject,
                    reason: crate::EntityResolutionReason::TransientValue,
                },
                materialization: None,
            });
        }

        if let Some(decision) = &self.decision {
            if let Ok(Some(evaluation)) = try_decision_fast_path(decision.as_ref(), prepared) {
                return Ok(evaluation);
            }
        }
        self.fallback.evaluate_prepared(prepared)
    }
}
