use crate::entity_candidates::EntityCandidateContext;
use crate::entity_resolution_prepare::{self, EntityResolutionContext};
use crate::{
    Cva, EntityCandidateConfig, EntityResolutionPreparation, EntityResolverError,
    MemoryEntityMentionKey, Phylactery,
};

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn prepare_entity_resolution(
                &mut self,
                key: MemoryEntityMentionKey,
                config: EntityCandidateConfig,
            ) -> Result<EntityResolutionPreparation, EntityResolverError> {
                let owner_id = self.owner_id();
                let mut context = EntityResolutionContext {
                    candidates: EntityCandidateContext {
                        memories: &self.memories,
                        lexical_index: &mut self.lexical_index,
                        container: &mut self.container,
                        entities: &self.entities,
                        graph: &self.graph,
                    },
                    resolutions: &self.entity_resolutions,
                };
                entity_resolution_prepare::prepare_entity_resolution(
                    &mut context,
                    key,
                    config,
                    owner_id,
                )
            }
        }
    };
}

impl_owner!(Cva);
impl_owner!(Phylactery);
