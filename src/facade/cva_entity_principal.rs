use crate::{
    Cva, EntityResolutionOutcome, EntityResolverError, MemoryEntityMentionKey, Phylactery,
};

impl Cva {
    pub(crate) fn resolve_principal_entity_mention(
        &mut self,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
        crate::perception_entity_resolution_runtime::resolve_principal_rel(self, key, now_ns)
    }

    pub(crate) fn sync_principal_profile(
        &mut self,
        principal_id: &str,
        profile: &crate::PhylacteryProfile,
        now_ns: i64,
    ) -> Result<bool, crate::EntityError> {
        crate::perception_entity_resolution_runtime::sync_principal_profile_rel(
            self,
            principal_id,
            profile,
            now_ns,
        )
    }
}

impl Phylactery {
    pub(crate) fn resolve_principal_entity_mention(
        &mut self,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
        crate::perception_entity_resolution_runtime::resolve_principal_phy(self, key, now_ns)
    }
}
