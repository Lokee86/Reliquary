use crate::entity_candidates::{self, EntityCandidateContext};
use crate::{
    Cva, EntityCandidateConfig, EntityCandidateError, EntityCandidateSet, MemoryEntityMentionKey,
    Phylactery,
};

macro_rules! impl_candidate_owner {
    ($owner:ty) => {
        impl $owner {
            pub fn entity_candidates_for_mention(
                &mut self,
                key: MemoryEntityMentionKey,
                config: EntityCandidateConfig,
            ) -> Result<EntityCandidateSet, EntityCandidateError> {
                let mut context = EntityCandidateContext {
                    memories: &self.memories,
                    lexical_index: &mut self.lexical_index,
                    container: &mut self.container,
                    entities: &self.entities,
                    graph: &self.graph,
                };
                entity_candidates::entity_candidates_for_mention(&mut context, key, config)
            }
        }
    };
}

impl_candidate_owner!(Cva);
impl_candidate_owner!(Phylactery);
