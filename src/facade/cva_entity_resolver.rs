use crate::GeneralEndpoint;
use crate::entity_resolver::EntityResolverCore;

pub struct EntityResolver<E> {
    pub(crate) core: EntityResolverCore<E>,
}

impl<E: GeneralEndpoint> EntityResolver<E> {
    pub fn new(endpoint: E) -> Self {
        Self {
            core: crate::public_entity_resolution_bindings::gateway_entity_resolver_new(endpoint),
        }
    }

    pub fn with_system_prompt(endpoint: E, system_prompt: impl Into<String>) -> Self {
        Self {
            core:
                crate::public_entity_resolution_bindings::gateway_entity_resolver_with_system_prompt(
                    endpoint,
                    system_prompt,
                ),
        }
    }

    pub fn model(&self) -> &str {
        crate::public_entity_resolution_bindings::gateway_entity_resolver_model(&self.core)
    }
}
