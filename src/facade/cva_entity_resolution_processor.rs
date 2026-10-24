use crate::{
    Cva, EntityCandidateConfig, EntityResolutionEngine, EntityResolutionOutcome, EntityResolver,
    EntityResolverError, GeneralEndpoint, MemoryEntityMentionKey, Phylactery,
};

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub fn resolve_entity_mention<E: GeneralEndpoint>(
                &mut self,
                resolver: &EntityResolver<E>,
                key: MemoryEntityMentionKey,
                config: EntityCandidateConfig,
                now_ns: i64,
            ) -> Result<EntityResolutionOutcome, EntityResolverError> {
                crate::perception_entity_resolution_runtime::resolve_rel(
                    self,
                    &resolver.core,
                    key,
                    config,
                    now_ns,
                )
            }

            pub fn resolve_entity_mention_with_engine<E: GeneralEndpoint>(
                &mut self,
                engine: &EntityResolutionEngine<E>,
                key: MemoryEntityMentionKey,
                config: EntityCandidateConfig,
                now_ns: i64,
            ) -> Result<EntityResolutionOutcome, EntityResolverError> {
                crate::perception_entity_resolution_runtime::resolve_rel_with_engine(
                    self,
                    &engine.core,
                    key,
                    config,
                    now_ns,
                )
            }
        }
    };
}

impl_owner!(Cva);
impl Phylactery {
    pub fn resolve_entity_mention<E: GeneralEndpoint>(
        &mut self,
        resolver: &EntityResolver<E>,
        key: MemoryEntityMentionKey,
        config: EntityCandidateConfig,
        now_ns: i64,
    ) -> Result<EntityResolutionOutcome, EntityResolverError> {
        crate::perception_entity_resolution_runtime::resolve_phy(
            self,
            &resolver.core,
            key,
            config,
            now_ns,
        )
    }

    pub fn resolve_entity_mention_with_engine<E: GeneralEndpoint>(
        &mut self,
        engine: &EntityResolutionEngine<E>,
        key: MemoryEntityMentionKey,
        config: EntityCandidateConfig,
        now_ns: i64,
    ) -> Result<EntityResolutionOutcome, EntityResolverError> {
        crate::perception_entity_resolution_runtime::resolve_phy_with_engine(
            self,
            &engine.core,
            key,
            config,
            now_ns,
        )
    }
}
