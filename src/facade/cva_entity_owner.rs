use crate::{
    Cva, Entity, EntityDraft, EntityError, EntityGlobalId, EntityId, EntityStats, Phylactery,
};

macro_rules! impl_owner {
    ($owner:ty, $publish:ident, $replay:ident, $backfill:ident, $entity:ident, $tombstone:ident, $retention:ident, $historical:ident, $lifecycle:ident, $revision:ident, $surface:ident, $normalized:ident, $alias:ident, $conflicts:ident, $replacement:ident, $version:ident, $entities:ident, $stats:ident) => {
        impl $owner {
            pub fn publish_entity(
                &mut self,
                id: Option<EntityId>,
                expected_revision: u64,
                draft: EntityDraft,
            ) -> Result<(Entity, bool), EntityError> {
                crate::perception_entity_merge_runtime::$publish(self, id, expected_revision, draft)
            }
            pub(crate) fn replay_entity_revision(
                &mut self,
                id: EntityId,
                expected_revision: u64,
                draft: EntityDraft,
                global_id: Option<EntityGlobalId>,
            ) -> Result<(Entity, bool), EntityError> {
                crate::perception_entity_merge_runtime::$replay(
                    self,
                    id,
                    expected_revision,
                    draft,
                    global_id,
                )
            }
            pub fn backfill_entity_global_ids(&mut self) -> Result<usize, EntityError> {
                crate::perception_entity_merge_runtime::$backfill(self)
            }
            pub fn entity(&self, id: EntityId) -> Result<Entity, EntityError> {
                crate::perception_entity_merge_runtime::$entity(self, id)
            }
            pub fn tombstone_entity(
                &mut self,
                id: EntityId,
                expected_revision: u64,
            ) -> Result<crate::owner_lifecycle::TombstoneOutcome, EntityError> {
                crate::perception_entity_merge_runtime::$tombstone(self, id, expected_revision)
            }
            pub fn entity_retention_descriptor(
                &self,
                id: EntityId,
            ) -> Result<crate::EntityRetentionDescriptor, EntityError> {
                crate::perception_entity_merge_runtime::$retention(self, id)
            }
            pub fn historical_entity_revision(
                &self,
                id: EntityId,
                revision: u64,
            ) -> Result<
                crate::owner_lifecycle::HistoricalPayload<Entity, crate::EntityRetainedRevision>,
                EntityError,
            > {
                crate::perception_entity_merge_runtime::$historical(self, id, revision)
            }
            pub fn entity_lifecycle_state(
                &self,
                id: EntityId,
            ) -> Result<crate::owner_lifecycle::LifecycleState, EntityError> {
                crate::perception_entity_merge_runtime::$lifecycle(self, id)
            }
            pub fn entity_revision(
                &self,
                id: EntityId,
                revision: u64,
            ) -> Result<Entity, EntityError> {
                crate::perception_entity_merge_runtime::$revision(self, id, revision)
            }
            pub fn entity_candidates_for_surface(
                &self,
                surface: &str,
                limit: usize,
            ) -> Vec<Entity> {
                crate::perception_entity_merge_runtime::$surface(self, surface, limit)
            }
            pub fn entity_candidates_for_normalized_surface(
                &self,
                surface: &str,
                limit: usize,
            ) -> Vec<Entity> {
                crate::perception_entity_merge_runtime::$normalized(self, surface, limit)
            }
            pub fn entity_candidates_for_alias_surface(
                &self,
                surface: &str,
                limit: usize,
            ) -> Vec<Entity> {
                crate::perception_entity_merge_runtime::$alias(self, surface, limit)
            }
            pub(crate) fn entity_creation_conflicts(
                &self,
                surface: &str,
                kind: &str,
                limit: usize,
            ) -> Vec<Entity> {
                crate::perception_entity_merge_runtime::$conflicts(self, surface, kind, limit)
            }
            pub fn retired_entity_replacement(&self, id: EntityId) -> Option<EntityId> {
                crate::perception_entity_merge_runtime::$replacement(self, id)
            }
            pub fn entity_version(&self) -> u64 {
                crate::perception_entity_merge_runtime::$version(self)
            }
            pub fn entities(&self) -> Vec<Entity> {
                crate::perception_entity_merge_runtime::$entities(self)
            }
            pub fn entity_stats(&self) -> EntityStats {
                crate::perception_entity_merge_runtime::$stats(self)
            }
        }
    };
}

impl_owner!(
    Cva,
    publish_entity_rel,
    replay_entity_revision_rel,
    backfill_entity_global_ids_rel,
    entity_rel,
    tombstone_entity_rel,
    entity_retention_rel,
    historical_entity_rel,
    entity_lifecycle_rel,
    entity_revision_rel,
    entity_candidates_rel,
    entity_candidates_normalized_rel,
    entity_candidates_alias_rel,
    entity_creation_conflicts_rel,
    entity_retired_replacement_rel,
    entity_version_rel,
    entities_rel,
    entity_stats_rel
);
impl_owner!(
    Phylactery,
    publish_entity_phy,
    replay_entity_revision_phy,
    backfill_entity_global_ids_phy,
    entity_phy,
    tombstone_entity_phy,
    entity_retention_phy,
    historical_entity_phy,
    entity_lifecycle_phy,
    entity_revision_phy,
    entity_candidates_phy,
    entity_candidates_normalized_phy,
    entity_candidates_alias_phy,
    entity_creation_conflicts_phy,
    entity_retired_replacement_phy,
    entity_version_phy,
    entities_phy,
    entity_stats_phy
);
