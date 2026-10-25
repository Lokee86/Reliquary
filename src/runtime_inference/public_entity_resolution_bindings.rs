//! Gateway-owned constructors and configuration calls for facade-owned public bindings.
//! The Perception cores continue to own inference and admission policy.

use crate::entity_resolution_engine::EntityResolutionEngineCore;
use crate::entity_resolver::EntityResolverCore;
use crate::{DecisionEndpoint, GeneralEndpoint};
use std::sync::Arc;

pub(crate) fn gateway_entity_resolution_engine_new<E: GeneralEndpoint>(
    fallback: E,
) -> EntityResolutionEngineCore<E> {
    EntityResolutionEngineCore::new(fallback)
}

pub(crate) fn gateway_entity_resolution_engine_with_decision_endpoint<E: GeneralEndpoint>(
    core: EntityResolutionEngineCore<E>,
    decision: Option<Arc<dyn DecisionEndpoint>>,
) -> EntityResolutionEngineCore<E> {
    core.with_decision_endpoint(decision)
}

pub(crate) fn gateway_entity_resolution_engine_decision_model<E: GeneralEndpoint>(
    core: &EntityResolutionEngineCore<E>,
) -> Option<&str> {
    core.decision_model()
}

pub(crate) fn gateway_entity_resolution_engine_fallback_model<E: GeneralEndpoint>(
    core: &EntityResolutionEngineCore<E>,
) -> &str {
    core.fallback_model()
}

pub(crate) fn gateway_entity_resolver_new<E: GeneralEndpoint>(
    endpoint: E,
) -> EntityResolverCore<E> {
    EntityResolverCore::new(endpoint)
}

pub(crate) fn gateway_entity_resolver_with_system_prompt<E: GeneralEndpoint>(
    endpoint: E,
    system_prompt: impl Into<String>,
) -> EntityResolverCore<E> {
    EntityResolverCore::with_system_prompt(endpoint, system_prompt)
}

pub(crate) fn gateway_entity_resolver_model<E: GeneralEndpoint>(
    core: &EntityResolverCore<E>,
) -> &str {
    core.model()
}
