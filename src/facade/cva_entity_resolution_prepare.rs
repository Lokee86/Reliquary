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
                crate::perception_entity_resolution_runtime::prepare_rel(self, key, config)
            }
        }
    };
}

impl_owner!(Cva);
impl Phylactery {
    pub(crate) fn prepare_entity_resolution(
        &mut self,
        key: MemoryEntityMentionKey,
        config: EntityCandidateConfig,
    ) -> Result<EntityResolutionPreparation, EntityResolverError> {
        crate::perception_entity_resolution_runtime::prepare_phy(self, key, config)
    }
}
