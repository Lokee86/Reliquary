use crate::entity_resolution_commit::{self, EntityResolutionCommitContext};
use crate::{
    Cva, EntityResolutionEvaluation, EntityResolutionOutcome, EntityResolutionPrepared,
    EntityResolverError, Phylactery,
};

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn commit_entity_resolution(
                &mut self,
                prepared: EntityResolutionPrepared,
                evaluation: EntityResolutionEvaluation,
                now_ns: i64,
            ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
                let mut context = EntityResolutionCommitContext {
                    container: &mut self.container,
                    memories: &self.memories,
                    entities: &mut self.entities,
                    graph: &mut self.graph,
                    resolutions: &mut self.entity_resolutions,
                };
                entity_resolution_commit::commit_entity_resolution(
                    &mut context,
                    prepared,
                    evaluation,
                    now_ns,
                )
            }
        }
    };
}
impl_owner!(Cva);
impl_owner!(Phylactery);
