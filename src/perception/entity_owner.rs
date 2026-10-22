use crate::{
    Cva, Entity, EntityDraft, EntityError, EntityGlobalId, EntityId, EntityStats, Phylactery,
};

macro_rules! impl_owner {
    ($owner:ty, $reject_principal:expr) => {
        impl $owner {
            pub fn publish_entity(
                &mut self,
                id: Option<EntityId>,
                expected_revision: u64,
                draft: EntityDraft,
            ) -> Result<(Entity, bool), EntityError> {
                if $reject_principal
                    && draft.kind.trim() == crate::entity_principal::PRINCIPAL_ENTITY_KIND
                {
                    return Err(EntityError::InvalidField("Phylactery principal Entity"));
                }
                self.entities
                    .publish(&mut self.container, id, expected_revision, draft)
            }

            pub(crate) fn replay_entity_revision(
                &mut self,
                id: EntityId,
                expected_revision: u64,
                draft: EntityDraft,
                global_id: Option<EntityGlobalId>,
            ) -> Result<(Entity, bool), EntityError> {
                self.entities.publish_with_global_id(
                    &mut self.container,
                    Some(id),
                    expected_revision,
                    draft,
                    global_id,
                )
            }

            pub fn backfill_entity_global_ids(&mut self) -> Result<usize, EntityError> {
                let missing: Vec<_> = self
                    .entities()
                    .into_iter()
                    .filter(|entity| entity.global_id.is_none())
                    .collect();
                for entity in &missing {
                    self.publish_entity(
                        Some(entity.id),
                        entity.revision,
                        EntityDraft {
                            canonical_name: entity.canonical_name.clone(),
                            aliases: entity.aliases.clone(),
                            kind: entity.kind.clone(),
                            summary: entity.summary.clone(),
                            mutation_id: format!(
                                "entity-global-uuid-backfill:{}:{}",
                                entity
                                    .id
                                    .0
                                    .iter()
                                    .map(|byte| format!("{byte:02x}"))
                                    .collect::<String>(),
                                entity.revision + 1
                            ),
                            created_at_ns: entity.created_at_ns,
                            updated_at_ns: entity.updated_at_ns,
                        },
                    )?;
                }
                Ok(missing.len())
            }

            pub fn entity(&self, id: EntityId) -> Result<Entity, EntityError> {
                self.entities.entity(id)
            }

            pub fn tombstone_entity(
                &mut self,
                id: EntityId,
                expected_revision: u64,
            ) -> Result<crate::owner_lifecycle::TombstoneOutcome, EntityError> {
                let result =
                    self.entities
                        .tombstone_direct(&mut self.container, id, expected_revision)?;
                self.graph
                    .exclude_from_current(crate::SemanticNodeRef::entity(id))
                    .map_err(|_| EntityError::InvalidField("Entity graph projection"))?;
                Ok(result)
            }

            pub fn entity_retention_descriptor(
                &self,
                id: EntityId,
            ) -> Result<crate::EntityRetentionDescriptor, EntityError> {
                self.entities.retention_descriptor(id)
            }

            pub fn historical_entity_revision(
                &self,
                id: EntityId,
                revision: u64,
            ) -> Result<
                crate::owner_lifecycle::HistoricalPayload<Entity, crate::EntityRetainedRevision>,
                EntityError,
            > {
                let value = match self.entity_revision(id, revision) {
                    Ok(value) => value,
                    Err(EntityError::PayloadUnavailable(reason)) => {
                        let structure = self
                            .entity_retention_descriptor(id)?
                            .revisions
                            .into_iter()
                            .find(|r| r.revision == revision)
                            .ok_or(EntityError::MissingEntity)?;
                        return Ok(crate::owner_lifecycle::HistoricalPayload::Pruned {
                            structural_history: structure,
                            reason,
                        });
                    }
                    Err(error) => return Err(error),
                };
                Ok(match self.entity_lifecycle_state(id)? {
                    crate::owner_lifecycle::LifecycleState::Active => {
                        crate::owner_lifecycle::HistoricalPayload::Available(value)
                    }
                    crate::owner_lifecycle::LifecycleState::Tombstoned(_) => {
                        crate::owner_lifecycle::HistoricalPayload::Tombstoned(value)
                    }
                })
            }

            pub fn entity_lifecycle_state(
                &self,
                id: EntityId,
            ) -> Result<crate::owner_lifecycle::LifecycleState, EntityError> {
                self.entities.lifecycle_state(id)
            }

            pub fn entity_revision(
                &self,
                id: EntityId,
                revision: u64,
            ) -> Result<Entity, EntityError> {
                self.entities.entity_revision(id, revision)
            }

            pub fn entities(&self) -> Vec<Entity> {
                self.entities.current()
            }

            pub fn entity_candidates_for_surface(
                &self,
                surface: &str,
                limit: usize,
            ) -> Vec<Entity> {
                self.entities.candidates_for_surface(surface, limit)
            }

            pub fn entity_candidates_for_normalized_surface(
                &self,
                surface: &str,
                limit: usize,
            ) -> Vec<Entity> {
                self.entities
                    .candidates_for_normalized_surface(surface, limit)
            }

            pub fn entity_candidates_for_alias_surface(
                &self,
                surface: &str,
                limit: usize,
            ) -> Vec<Entity> {
                self.entities.candidates_for_alias_surface(surface, limit)
            }

            pub(crate) fn entity_creation_conflicts(
                &self,
                surface: &str,
                kind: &str,
                limit: usize,
            ) -> Vec<Entity> {
                self.entities.creation_conflicts(surface, kind, limit)
            }

            pub fn retired_entity_replacement(&self, id: EntityId) -> Option<EntityId> {
                self.entities.retired_replacement(id)
            }

            pub fn entity_version(&self) -> u64 {
                self.entities.entity_version()
            }

            pub fn entity_stats(&self) -> EntityStats {
                self.entities.stats()
            }
        }
    };
}

impl_owner!(Cva, false);
impl_owner!(Phylactery, true);
