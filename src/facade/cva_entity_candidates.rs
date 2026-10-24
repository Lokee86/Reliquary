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
                crate::perception_entity_resolution_runtime::candidates_rel(self, key, config)
            }
        }
    };
}

impl_candidate_owner!(Cva);
impl Phylactery {
    pub fn entity_candidates_for_mention(
        &mut self,
        key: MemoryEntityMentionKey,
        config: EntityCandidateConfig,
    ) -> Result<EntityCandidateSet, EntityCandidateError> {
        crate::perception_entity_resolution_runtime::candidates_phy(self, key, config)
    }
}
