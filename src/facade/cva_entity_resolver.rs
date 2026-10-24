use crate::GeneralEndpoint;
use crate::entity_resolver::EntityResolverCore;

/// Public resolver binding. Inference policy is implemented by the Perception core.
pub struct EntityResolver<E> {
    pub(crate) core: EntityResolverCore<E>,
}

impl<E: GeneralEndpoint> EntityResolver<E> {
    pub fn new(endpoint: E) -> Self {
        Self {
            core: EntityResolverCore::new(endpoint),
        }
    }

    pub fn with_system_prompt(endpoint: E, system_prompt: impl Into<String>) -> Self {
        Self {
            core: EntityResolverCore::with_system_prompt(endpoint, system_prompt),
        }
    }

    pub fn model(&self) -> &str {
        self.core.model()
    }
}
