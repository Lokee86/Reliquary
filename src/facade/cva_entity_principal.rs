use crate::entity_principal::PrincipalContext;
use crate::{
    Cva, EntityResolutionOutcome, EntityResolverError, MemoryEntityMentionKey, Phylactery,
};
impl Cva {
    pub(crate) fn resolve_principal_entity_mention(
        &mut self,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
        PrincipalContext {
            container: &mut self.container,
            memories: &self.memories,
            entities: &mut self.entities,
            entity_resolutions: &mut self.entity_resolutions,
            graph: &mut self.graph,
            archive: Some(&self.archive),
        }
        .resolve_principal_entity_mention(key, now_ns)
    }
    pub(crate) fn sync_principal_profile(
        &mut self,
        principal_id: &str,
        profile: &crate::PhylacteryProfile,
        now_ns: i64,
    ) -> Result<bool, crate::EntityError> {
        PrincipalContext {
            container: &mut self.container,
            memories: &self.memories,
            entities: &mut self.entities,
            entity_resolutions: &mut self.entity_resolutions,
            graph: &mut self.graph,
            archive: Some(&self.archive),
        }
        .sync_principal_profile(principal_id, profile, now_ns)
    }
}
impl Phylactery {
    pub(crate) fn resolve_principal_entity_mention(
        &mut self,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
        PrincipalContext {
            container: &mut self.container,
            memories: &self.memories,
            entities: &mut self.entities,
            entity_resolutions: &mut self.entity_resolutions,
            graph: &mut self.graph,
            archive: None,
        }
        .reject_phy_principal_entity_mention(key, now_ns)
    }
}
