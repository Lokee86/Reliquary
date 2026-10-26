#[path = "entity_resolution_store_index.rs"]
mod index;
#[path = "entity_resolution_store_lifecycle.rs"]
mod lifecycle;
#[path = "entity_resolution_store_write.rs"]
mod write;

use index::{deindex_pending, index_pending};

use crate::entity_resolution_codec::{DecodedEntityResolution, delta};
use crate::entity_store::EntityStore;
use crate::memory_store::MemoryStore;
use crate::{
    EntityId, MAX_ENTITY_RESOLUTION_CANDIDATES, MemoryEntityMentionKey, MemoryEntityResolution,
    MemoryEntityResolutionStatus, MemoryError, MemoryId,
};
use reliquary_container::Container;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Default)]
pub(crate) struct EntityResolutionStore {
    current: HashMap<MemoryEntityMentionKey, MemoryEntityResolution>,
    revisions: HashMap<MemoryEntityMentionKey, u32>,
    maintenance: HashSet<MemoryEntityMentionKey>,
    pending_by_entity: HashMap<EntityId, HashSet<MemoryEntityMentionKey>>,
}

impl EntityResolutionStore {
    pub(crate) fn materialized_changes(
        container: &mut Container,
    ) -> Result<
        std::collections::BTreeMap<reliquary_container::ObjectRef, DecodedEntityResolution>,
        MemoryError,
    > {
        let mut projection = Self::default();
        let mut changes = std::collections::BTreeMap::new();
        (|| -> Result<(), MemoryError> {
            let mut physical_cursor = container.physical_cursor()?;
            while let Some(physical_frame) = physical_cursor.next_frame()? {
                let object = physical_frame.object;
                let payload = physical_frame.payload.as_slice();
                (|| -> Result<(), MemoryError> {
                    if let Some(change) = projection.decode(payload)? {
                        projection.apply(change.clone())?;
                        changes.insert(object, change);
                    }
                    Ok(())
                })()?;
            }
            Ok(())
        })()?;
        Ok(changes)
    }
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), MemoryError> {
        let Some(decoded) = self.decode(payload)? else {
            return Ok(());
        };
        self.apply(decoded)
    }

    pub(crate) fn decode(
        &self,
        payload: &[u8],
    ) -> Result<Option<DecodedEntityResolution>, MemoryError> {
        // Literal retained evidence validates without becoming active decisions.
        if crate::entity_resolution_codec::retained::decode(payload)?.is_some() {
            return Ok(None);
        }

        delta::decode(payload, |key| self.current.get(&key).map(|v| &v.status))
    }

    pub(crate) fn persist_change(
        &mut self,
        container: &mut Container,
        decoded: DecodedEntityResolution,
    ) -> Result<(), MemoryError> {
        let bytes = delta::encode(&decoded, self.current.get(&decoded.key).map(|v| &v.status))?;
        self.validate_change(&decoded)?;
        container.append(&bytes)?;
        self.apply_validated(decoded);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn from_fixture_changes(
        changes: impl IntoIterator<Item = DecodedEntityResolution>,
    ) -> Result<Self, MemoryError> {
        let mut store = Self::default();
        for change in changes {
            store.apply(change)?;
        }
        Ok(store)
    }
    fn apply(&mut self, decoded: DecodedEntityResolution) -> Result<(), MemoryError> {
        self.validate_change(&decoded)?;
        self.apply_validated(decoded);
        Ok(())
    }

    fn validate_change(&self, decoded: &DecodedEntityResolution) -> Result<(), MemoryError> {
        let expected = self
            .revisions
            .get(&decoded.key)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        if decoded.revision != expected {
            return Err(MemoryError::CorruptRecord(
                "Entity resolution revision sequence",
            ));
        }
        if let Some(status) = &decoded.status {
            validate_status(status)?;
        }
        Ok(())
    }

    fn apply_validated(&mut self, decoded: DecodedEntityResolution) {
        match decoded.status {
            Some(status) => {
                let value = MemoryEntityResolution {
                    key: decoded.key,
                    revision: decoded.revision,
                    updated_at_ns: decoded.updated_at_ns,
                    status,
                };
                if let Some(previous) = self.current.get(&value.key) {
                    deindex_pending(&mut self.pending_by_entity, value.key, &previous.status);
                }
                self.revisions.insert(value.key, value.revision);
                self.track_maintenance(value.key, &value.status);
                index_pending(&mut self.pending_by_entity, value.key, &value.status);
                self.current.insert(value.key, value);
            }
            None => {
                if let Some(previous) = self.current.remove(&decoded.key) {
                    deindex_pending(&mut self.pending_by_entity, decoded.key, &previous.status);
                }
                self.revisions.remove(&decoded.key);
                self.maintenance.remove(&decoded.key);
            }
        }
    }

    pub(crate) fn validate(
        &self,
        memories: &MemoryStore,
        entities: &EntityStore,
    ) -> Result<(), MemoryError> {
        for value in self.current.values() {
            memories.validate_historical_entity_mention_key(value.key)?;
            validate_status(&value.status)?;
            validate_historical_entity_refs(&value.status, entities)?;
        }
        Ok(())
    }

    pub(crate) fn get(&self, key: MemoryEntityMentionKey) -> Option<&MemoryEntityResolution> {
        self.current.get(&key)
    }

    pub(crate) fn for_memory(&self, id: MemoryId) -> Vec<MemoryEntityResolution> {
        let mut values: Vec<_> = self
            .current
            .values()
            .filter(|value| value.key.memory_id == id)
            .cloned()
            .collect();
        values.sort_by_key(|value| mention_sort_key(value.key));
        values
    }

    pub(crate) fn records(&self) -> Vec<MemoryEntityResolution> {
        let mut values: Vec<_> = self.current.values().cloned().collect();
        values.sort_by_key(|value| mention_sort_key(value.key));
        values
    }

    pub(crate) fn retry_needed(
        &self,
        key: MemoryEntityMentionKey,
        candidate_fingerprint: [u8; 32],
        context_fingerprint: [u8; 32],
    ) -> bool {
        match self.current.get(&key).map(|value| &value.status) {
            None => true,
            Some(MemoryEntityResolutionStatus::Pending(value)) => {
                value.candidate_set_fingerprint != candidate_fingerprint
                    || value.context_fingerprint != context_fingerprint
            }
            Some(MemoryEntityResolutionStatus::Dormant(value)) => {
                value.candidate_set_fingerprint != candidate_fingerprint
                    || value.context_fingerprint != context_fingerprint
            }
            Some(
                MemoryEntityResolutionStatus::Resolved { .. }
                | MemoryEntityResolutionStatus::Rejected { .. },
            ) => false,
        }
    }

    pub(super) fn append(
        &mut self,
        container: &mut Container,
        key: MemoryEntityMentionKey,
        updated_at_ns: i64,
        status: MemoryEntityResolutionStatus,
    ) -> Result<bool, MemoryError> {
        validate_status(&status)?;
        let revision = self
            .current
            .get(&key)
            .map(|value| value.revision)
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        self.persist_change(
            container,
            DecodedEntityResolution {
                key,
                revision,
                updated_at_ns,
                status: Some(status),
            },
        )?;
        Ok(true)
    }

    pub(super) fn tombstone(
        &mut self,
        container: &mut Container,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<bool, MemoryError> {
        let Some(current) = self.current.get(&key) else {
            return Ok(false);
        };
        let revision = current
            .revision
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        self.persist_change(
            container,
            DecodedEntityResolution {
                key,
                revision,
                updated_at_ns: now_ns,
                status: None,
            },
        )?;
        Ok(true)
    }

    pub(crate) fn import_current(
        &mut self,
        container: &mut Container,
        memories: &MemoryStore,
        entities: &EntityStore,
        mut value: MemoryEntityResolution,
    ) -> Result<(), MemoryError> {
        memories.validate_entity_mention_key(value.key)?;
        validate_status(&value.status)?;
        validate_live_entity_refs(&value.status, entities)?;
        if let Some(existing) = self.current.get(&value.key) {
            return if existing.status == value.status {
                Ok(())
            } else {
                Err(MemoryError::RoutingMetadataConflict)
            };
        }
        value.revision = 1;
        self.persist_change(
            container,
            DecodedEntityResolution {
                key: value.key,
                revision: 1,
                updated_at_ns: value.updated_at_ns,
                status: Some(value.status),
            },
        )?;
        Ok(())
    }

    fn track_maintenance(
        &mut self,
        key: MemoryEntityMentionKey,
        status: &MemoryEntityResolutionStatus,
    ) {
        if matches!(status, MemoryEntityResolutionStatus::Resolved { .. }) {
            self.maintenance.remove(&key);
        } else {
            self.maintenance.insert(key);
        }
    }
}

fn validate_live_entity_refs(
    status: &MemoryEntityResolutionStatus,
    entities: &EntityStore,
) -> Result<(), MemoryError> {
    validate_entity_refs_with(status, |id| entities.contains(id))
}

fn validate_historical_entity_refs(
    status: &MemoryEntityResolutionStatus,
    entities: &EntityStore,
) -> Result<(), MemoryError> {
    validate_entity_refs_with(status, |id| entities.contains_historical(id))
}

fn validate_entity_refs_with(
    status: &MemoryEntityResolutionStatus,
    mut contains: impl FnMut(EntityId) -> bool,
) -> Result<(), MemoryError> {
    let valid = match status {
        MemoryEntityResolutionStatus::Resolved { entity_id, .. } => contains(*entity_id),
        MemoryEntityResolutionStatus::Pending(value) => value
            .candidate_entity_ids
            .iter()
            .copied()
            .all(&mut contains),
        MemoryEntityResolutionStatus::Dormant(value) => value
            .candidate_entity_ids
            .iter()
            .copied()
            .all(&mut contains),
        MemoryEntityResolutionStatus::Rejected { .. } => true,
    };
    if valid {
        Ok(())
    } else {
        Err(MemoryError::InvalidField(
            "Entity resolution Entity reference",
        ))
    }
}

fn mention_sort_key(key: MemoryEntityMentionKey) -> ([u8; 32], u8, u32, u32) {
    (
        key.memory_id.0,
        key.field.tag(),
        key.start_byte,
        key.end_byte,
    )
}

pub(super) fn validate_status(status: &MemoryEntityResolutionStatus) -> Result<(), MemoryError> {
    let (candidates, first, last, attempt, dormant) = match status {
        MemoryEntityResolutionStatus::Pending(value) => (
            &value.candidate_entity_ids,
            Some(value.first_seen_at_ns),
            Some(value.last_attempt_at_ns),
            Some(value.attempt_count),
            None,
        ),
        MemoryEntityResolutionStatus::Dormant(value) => (
            &value.candidate_entity_ids,
            Some(value.first_seen_at_ns),
            Some(value.last_attempt_at_ns),
            Some(value.attempt_count),
            Some(value.dormant_at_ns),
        ),
        _ => return Ok(()),
    };
    if candidates.len() > MAX_ENTITY_RESOLUTION_CANDIDATES
        || candidates.windows(2).any(|pair| pair[0] >= pair[1])
        || attempt == Some(0)
        || first > last
        || dormant.is_some_and(|value| Some(value) < last)
    {
        return Err(MemoryError::InvalidField("Entity resolution state"));
    }
    Ok(())
}
