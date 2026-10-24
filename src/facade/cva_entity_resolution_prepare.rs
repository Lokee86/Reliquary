use crate::entity_candidates::EntityCandidateContext;
use crate::entity_resolution_prepare::{self, EntityResolutionContext};
use crate::lexical_search::MemoryLexicalReadView;
use crate::memory_store::MemoryReadView;
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
                let memories = MemoryReadView::new(&self.owner.memories, &self.owner.container);
                let mut context = EntityResolutionContext {
                    candidates: EntityCandidateContext {
                        memories: MemoryLexicalReadView::new(&mut self.lexical_index, memories),
                        entities: &self.owner.entities,
                        graph: &self.owner.graph,
                    },
                    resolutions: &self.owner.entity_resolutions,
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
