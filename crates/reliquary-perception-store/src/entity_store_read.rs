use super::{EntityStore, alias_surface_keys, normalize_surface, normalized_surface, resolve};
use crate::entity_model::EntityRecord;
use crate::{Entity, EntityError, EntityId, EntityStats};

impl EntityStore {
    pub(crate) fn current_record_for_replay(
        &self,
        id: EntityId,
    ) -> Result<&EntityRecord, EntityError> {
        if self.tombstones.contains_key(&id) {
            return Err(EntityError::TerminalTombstone);
        }
        self.current
            .get(&id)
            .map(|index| &self.records[*index])
            .ok_or(EntityError::MissingEntity)
    }

    pub fn entity_revision(
        &self,
        id: EntityId,
        revision: u64,
    ) -> Result<Entity, EntityError> {
        if self.pruned_revisions.contains(&(id, revision)) {
            return Err(EntityError::PayloadUnavailable(
                crate::EntityPayloadUnavailable::Pruned,
            ));
        }
        self.records
            .iter()
            .find(|record| record.id == id && record.revision == revision)
            .map(resolve)
            .ok_or(EntityError::MissingEntity)
    }

    pub fn lifecycle_state(
        &self,
        id: EntityId,
    ) -> Result<crate::EntityLifecycleState, EntityError> {
        if let Some(&coordinates) = self.tombstones.get(&id) {
            return Ok(crate::EntityLifecycleState::Tombstoned(
                coordinates,
            ));
        }
        self.current
            .contains_key(&id)
            .then_some(crate::EntityLifecycleState::Active)
            .ok_or(EntityError::MissingEntity)
    }

    pub fn retention_descriptor(
        &self,
        id: EntityId,
    ) -> Result<crate::entity_model::EntityRetentionDescriptor, EntityError> {
        let coordinates = self.tombstones.get(&id).copied().ok_or_else(|| {
            if self.contains_historical(id) {
                EntityError::PayloadRequiredByLiveTarget
            } else {
                EntityError::MissingEntity
            }
        })?;
        let revisions = self
            .records
            .iter()
            .filter(|record| record.id == id)
            .map(|record| crate::entity_model::EntityRetainedRevision {
                revision: record.revision,
                global_version: record.global_version,
                entity_version: record.entity_version,
                global_id: record.global_id,
                kind: record.kind.clone(),
                created_at_ns: record.created_at_ns,
                updated_at_ns: record.updated_at_ns,
                mutation_id: record.mutation_id.clone(),
            })
            .collect();
        Ok(crate::entity_model::EntityRetentionDescriptor {
            id,
            tombstone_revision: self.revisions[&id],
            tombstone_global_version: coordinates.global_version,
            tombstone_entity_version: coordinates.entity_version,
            replacement_id: self.retired.get(&id).copied(),
            revisions,
        })
    }
    pub fn entity(&self, id: EntityId) -> Result<Entity, EntityError> {
        self.current
            .get(&id)
            .map(|index| resolve(&self.records[*index]))
            .ok_or(EntityError::MissingEntity)
    }

    pub fn contains_historical(&self, id: EntityId) -> bool {
        self.revisions.contains_key(&id)
    }

    pub fn contains(&self, id: EntityId) -> bool {
        self.current.contains_key(&id)
    }

    pub fn current(&self) -> Vec<Entity> {
        let mut values: Vec<_> = self
            .current
            .values()
            .map(|index| resolve(&self.records[*index]))
            .collect();
        values.sort_by_key(|value| value.id);
        values
    }

    pub fn candidates_for_surface(&self, surface: &str, limit: usize) -> Vec<Entity> {
        let needle = normalize_surface(surface);
        self.by_surface
            .get(&needle)
            .into_iter()
            .flat_map(|ids| ids.iter())
            .take(limit)
            .filter_map(|id| self.current.get(id))
            .map(|index| resolve(&self.records[*index]))
            .collect()
    }

    pub fn candidates_for_normalized_surface(
        &self,
        surface: &str,
        limit: usize,
    ) -> Vec<Entity> {
        let needle = normalized_surface(surface);
        if needle.is_empty() {
            return Vec::new();
        }
        self.by_normalized_surface
            .get(&needle)
            .into_iter()
            .flat_map(|ids| ids.iter())
            .take(limit)
            .filter_map(|id| self.current.get(id))
            .map(|index| resolve(&self.records[*index]))
            .collect()
    }

    pub fn candidates_for_alias_surface(&self, surface: &str, limit: usize) -> Vec<Entity> {
        let mut ids = std::collections::BTreeSet::new();
        for key in alias_surface_keys(surface) {
            if let Some(values) = self.by_alias_surface.get(&key) {
                for id in values {
                    ids.insert(*id);
                    if ids.len() >= limit {
                        break;
                    }
                }
            }
            if ids.len() >= limit {
                break;
            }
        }
        ids.into_iter()
            .filter_map(|id| self.current.get(&id))
            .map(|index| resolve(&self.records[*index]))
            .collect()
    }

    pub fn records(&self) -> &[EntityRecord] {
        &self.records
    }

    pub fn tombstone_global_versions(&self) -> impl Iterator<Item = u64> + '_ {
        self.tombstones
            .values()
            .map(|coordinates| coordinates.global_version)
    }

    pub fn entity_version(&self) -> u64 {
        self.next_entity_version.saturating_sub(1)
    }

    pub fn stats(&self) -> EntityStats {
        EntityStats {
            entities: self.current.len(),
            revisions: self.records.len(),
            entity_version: self.entity_version(),
        }
    }
}

impl EntityStore {
    pub fn is_revision_pruned(&self, id: EntityId, revision: u64) -> bool {
        self.pruned_revisions.contains(&(id, revision))
    }
}
