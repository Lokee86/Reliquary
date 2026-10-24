use crate::{
    Cva, EntityCandidateConfig, EntityResolutionEngine, EntityResolutionOutcome,
    EntityResolutionPreparation, EntityResolver, EntityResolverError, GeneralEndpoint,
    MemoryEntityMentionKey, Phylactery,
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
                if let Some(outcome) = self.resolve_principal_entity_mention(key, now_ns)? {
                    return Ok(outcome);
                }
                match self.prepare_entity_resolution(key, config)? {
                    EntityResolutionPreparation::Complete(outcome) => Ok(outcome),
                    EntityResolutionPreparation::Ready(prepared) => {
                        let evaluation = resolver.core.evaluate_prepared(&prepared)?;
                        self.commit_entity_resolution(prepared, evaluation, now_ns)?
                            .ok_or_else(crate::entity_resolution_processor::stale_snapshot)
                    }
                }
            }

            pub fn resolve_entity_mention_with_engine<E: GeneralEndpoint>(
                &mut self,
                engine: &EntityResolutionEngine<E>,
                key: MemoryEntityMentionKey,
                config: EntityCandidateConfig,
                now_ns: i64,
            ) -> Result<EntityResolutionOutcome, EntityResolverError> {
                if let Some(outcome) = self.resolve_principal_entity_mention(key, now_ns)? {
                    return Ok(outcome);
                }
                match self.prepare_entity_resolution(key, config)? {
                    EntityResolutionPreparation::Complete(outcome) => Ok(outcome),
                    EntityResolutionPreparation::Ready(prepared) => {
                        let evaluation = engine.core.evaluate_prepared(&prepared)?;
                        self.commit_entity_resolution(prepared, evaluation, now_ns)?
                            .ok_or_else(crate::entity_resolution_processor::stale_snapshot)
                    }
                }
            }
        }
    };
}

impl_owner!(Cva);
impl_owner!(Phylactery);
