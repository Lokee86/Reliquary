use crate::{
    Cva, EntityCandidateConfig, EntityResolutionBatchOutcome, EntityResolutionEngine,
    EntityResolverError, GeneralEndpoint, MemoryEntityMentionKey, Phylactery,
};

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub fn resolve_entity_mentions_with_engine_parallel<E: GeneralEndpoint>(
                &mut self,
                engine: &EntityResolutionEngine<E>,
                keys: &[MemoryEntityMentionKey],
                config: EntityCandidateConfig,
                now_ns: i64,
                workers: usize,
            ) -> Result<EntityResolutionBatchOutcome, EntityResolverError> {
                crate::perception_entity_resolution_runtime::resolve_rel_parallel(
                    self,
                    &engine.core,
                    keys,
                    config,
                    now_ns,
                    workers,
                )
            }
        }
    };
}

impl_owner!(Cva);
impl Phylactery {
    pub fn resolve_entity_mentions_with_engine_parallel<E: GeneralEndpoint>(
        &mut self,
        engine: &EntityResolutionEngine<E>,
        keys: &[MemoryEntityMentionKey],
        config: EntityCandidateConfig,
        now_ns: i64,
        workers: usize,
    ) -> Result<EntityResolutionBatchOutcome, EntityResolverError> {
        crate::perception_entity_resolution_runtime::resolve_phy_parallel(
            self,
            &engine.core,
            keys,
            config,
            now_ns,
            workers,
        )
    }
}
