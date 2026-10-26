//! Product admission for foreign Memory evidence. The compiled store owns persistence.
use crate::entity_store::EntityStore;
use crate::memory_store::MemoryStore;
use crate::{
    EntityId, EntityResolutionCompaction, EntityResolutionReason, MemoryEntityMentionKey,
    MemoryEntityResolution, MemoryEntityResolutionStatus, MemoryError, MemoryId,
};
use reliquary_container::Container;
use reliquary_perception_store::entity_resolution_model as wire;
use reliquary_perception_store::{ResolutionError, entity_resolution_store::EntityResolutionStore};

pub(crate) fn memory_error(e: ResolutionError) -> MemoryError {
    match e {
        ResolutionError::Container(e) => MemoryError::Container(e),
        ResolutionError::CorruptRecord(e) => MemoryError::CorruptRecord(e),
        ResolutionError::InvalidField(e) => MemoryError::InvalidField(e),
        ResolutionError::FieldTooLarge => MemoryError::FieldTooLarge,
        ResolutionError::InvalidVersion => MemoryError::InvalidVersion,
        ResolutionError::VersionExhausted => MemoryError::VersionExhausted,
        ResolutionError::RevisionConflict => MemoryError::RevisionConflict,
        ResolutionError::RoutingMetadataConflict => MemoryError::RoutingMetadataConflict,
    }
}
pub(crate) fn store_key(key: MemoryEntityMentionKey) -> wire::MemoryEntityMentionKey {
    wire::MemoryEntityMentionKey {
        memory_id: key.memory_id.0,
        field: wire::MemoryTextField::from_tag(key.field.tag()).unwrap(),
        start_byte: key.start_byte,
        end_byte: key.end_byte,
    }
}
pub(crate) fn product_key(key: wire::MemoryEntityMentionKey) -> MemoryEntityMentionKey {
    MemoryEntityMentionKey {
        memory_id: MemoryId(key.memory_id),
        field: crate::MemoryTextField::from_tag(key.field.tag()).unwrap(),
        start_byte: key.start_byte,
        end_byte: key.end_byte,
    }
}
fn product_value(v: wire::MemoryEntityResolution) -> MemoryEntityResolution {
    MemoryEntityResolution {
        key: product_key(v.key),
        revision: v.revision,
        updated_at_ns: v.updated_at_ns,
        status: v.status,
    }
}
fn validate_refs(
    status: &MemoryEntityResolutionStatus,
    entities: &EntityStore,
    historical: bool,
) -> Result<(), MemoryError> {
    let contains = |id| {
        if historical {
            entities.contains_historical(id)
        } else {
            entities.contains(id)
        }
    };
    let valid = match status {
        MemoryEntityResolutionStatus::Resolved { entity_id, .. } => contains(*entity_id),
        MemoryEntityResolutionStatus::Pending(v) => {
            v.candidate_entity_ids.iter().copied().all(contains)
        }
        MemoryEntityResolutionStatus::Dormant(v) => {
            v.candidate_entity_ids.iter().copied().all(contains)
        }
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
fn validate_write(
    store: &EntityResolutionStore,
    memories: &MemoryStore,
    key: MemoryEntityMentionKey,
    expected: u32,
) -> Result<(), MemoryError> {
    memories.validate_entity_mention_key(key)?;
    if store.get(store_key(key)).map(|v| v.revision).unwrap_or(0) != expected {
        return Err(MemoryError::RevisionConflict);
    }
    Ok(())
}

pub(crate) trait ResolutionAdmission {
    fn owner_get(&self, key: MemoryEntityMentionKey) -> Option<MemoryEntityResolution>;
    fn owner_records(&self) -> Vec<MemoryEntityResolution>;
    fn owner_for_memory(&self, id: MemoryId) -> Vec<MemoryEntityResolution>;
    fn owner_pending_for_entity(&self, id: EntityId) -> Vec<MemoryEntityMentionKey>;
    fn owner_retry_needed(&self, key: MemoryEntityMentionKey, c: [u8; 32], x: [u8; 32]) -> bool;
    fn owner_validate(&self, m: &MemoryStore, e: &EntityStore) -> Result<(), MemoryError>;
    fn owner_put_resolved(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
        k: MemoryEntityMentionKey,
        r: u32,
        id: EntityId,
        reason: EntityResolutionReason,
        now: i64,
    ) -> Result<bool, MemoryError>;
    fn owner_put_rejected(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        k: MemoryEntityMentionKey,
        r: u32,
        reason: EntityResolutionReason,
        now: i64,
    ) -> Result<bool, MemoryError>;
    fn owner_put_unresolved(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
        k: MemoryEntityMentionKey,
        r: u32,
        ids: Vec<EntityId>,
        reason: EntityResolutionReason,
        cf: [u8; 32],
        xf: [u8; 32],
        now: i64,
    ) -> Result<bool, MemoryError>;
    fn owner_tombstone(
        &mut self,
        c: &mut Container,
        k: MemoryEntityMentionKey,
        now: i64,
    ) -> Result<bool, MemoryError>;
    fn owner_compact(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        now: i64,
    ) -> Result<EntityResolutionCompaction, MemoryError>;
    fn owner_import_current(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
        v: MemoryEntityResolution,
    ) -> Result<(), MemoryError>;
    fn owner_retarget_resolved(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
        k: MemoryEntityMentionKey,
        from: EntityId,
        to: EntityId,
        now: i64,
    ) -> Result<bool, MemoryError>;
    fn owner_retarget_entity_references(
        &mut self,
        c: &mut Container,
        e: &EntityStore,
        from: EntityId,
        to: EntityId,
        now: i64,
    ) -> Result<usize, MemoryError>;
    fn owner_repair_retired_entity_references(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
    ) -> Result<usize, MemoryError>;
}
impl ResolutionAdmission for EntityResolutionStore {
    fn owner_get(&self, key: MemoryEntityMentionKey) -> Option<MemoryEntityResolution> {
        self.get(store_key(key)).cloned().map(product_value)
    }
    fn owner_records(&self) -> Vec<MemoryEntityResolution> {
        self.records().into_iter().map(product_value).collect()
    }
    fn owner_for_memory(&self, id: MemoryId) -> Vec<MemoryEntityResolution> {
        self.for_memory(id.0)
            .into_iter()
            .map(product_value)
            .collect()
    }
    fn owner_pending_for_entity(&self, id: EntityId) -> Vec<MemoryEntityMentionKey> {
        self.pending_for_entity(id)
            .into_iter()
            .map(product_key)
            .collect()
    }
    fn owner_retry_needed(&self, key: MemoryEntityMentionKey, c: [u8; 32], x: [u8; 32]) -> bool {
        self.retry_needed(store_key(key), c, x)
    }
    fn owner_validate(&self, m: &MemoryStore, e: &EntityStore) -> Result<(), MemoryError> {
        for v in self.records() {
            m.validate_historical_entity_mention_key(product_key(v.key))?;
            reliquary_perception_store::entity_resolution_store::validate_status(&v.status)
                .map_err(memory_error)?;
            validate_refs(&v.status, e, true)?;
        }
        Ok(())
    }
    fn owner_put_resolved(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
        k: MemoryEntityMentionKey,
        r: u32,
        id: EntityId,
        reason: EntityResolutionReason,
        now: i64,
    ) -> Result<bool, MemoryError> {
        validate_write(self, m, k, r)?;
        if !e.contains(id) {
            return Err(MemoryError::InvalidField(
                "Entity resolution Entity reference",
            ));
        }
        self.put_resolved(c, store_key(k), r, id, reason, now)
            .map_err(memory_error)
    }
    fn owner_put_rejected(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        k: MemoryEntityMentionKey,
        r: u32,
        reason: EntityResolutionReason,
        now: i64,
    ) -> Result<bool, MemoryError> {
        validate_write(self, m, k, r)?;
        self.put_rejected(c, store_key(k), r, reason, now)
            .map_err(memory_error)
    }
    fn owner_put_unresolved(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
        k: MemoryEntityMentionKey,
        r: u32,
        mut ids: Vec<EntityId>,
        reason: EntityResolutionReason,
        cf: [u8; 32],
        xf: [u8; 32],
        now: i64,
    ) -> Result<bool, MemoryError> {
        validate_write(self, m, k, r)?;
        ids.sort();
        ids.dedup();
        if ids.len() > crate::MAX_ENTITY_RESOLUTION_CANDIDATES {
            return Err(MemoryError::FieldTooLarge);
        }
        if ids.iter().any(|id| !e.contains(*id)) {
            return Err(MemoryError::InvalidField(
                "Entity resolution Entity reference",
            ));
        }
        self.put_unresolved(c, store_key(k), r, ids, reason, cf, xf, now)
            .map_err(memory_error)
    }
    fn owner_tombstone(
        &mut self,
        c: &mut Container,
        k: MemoryEntityMentionKey,
        now: i64,
    ) -> Result<bool, MemoryError> {
        self.tombstone(c, store_key(k), now).map_err(memory_error)
    }
    fn owner_compact(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        now: i64,
    ) -> Result<EntityResolutionCompaction, MemoryError> {
        let inactive = self
            .maintenance_sources()
            .filter(|id| m.resolution_source_inactive(MemoryId(*id)))
            .collect();
        let v = self.compact(c, &inactive, now).map_err(memory_error)?;
        Ok(EntityResolutionCompaction {
            dormant: v.dormant,
            purged: v.purged,
            rejected_purged: v.rejected_purged,
        })
    }
    fn owner_import_current(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
        v: MemoryEntityResolution,
    ) -> Result<(), MemoryError> {
        m.validate_entity_mention_key(v.key)?;
        reliquary_perception_store::entity_resolution_store::validate_status(&v.status)
            .map_err(memory_error)?;
        validate_refs(&v.status, e, false)?;
        self.import_current(
            c,
            wire::MemoryEntityResolution {
                key: store_key(v.key),
                revision: v.revision,
                updated_at_ns: v.updated_at_ns,
                status: v.status,
            },
        )
        .map_err(memory_error)
    }
    fn owner_retarget_resolved(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
        k: MemoryEntityMentionKey,
        from: EntityId,
        to: EntityId,
        now: i64,
    ) -> Result<bool, MemoryError> {
        m.validate_entity_mention_key(k)?;
        if !e.contains(to) {
            return Err(MemoryError::InvalidField(
                "Entity resolution Entity reference",
            ));
        }
        self.retarget_resolved(c, store_key(k), from, to, now)
            .map_err(memory_error)
    }
    fn owner_retarget_entity_references(
        &mut self,
        c: &mut Container,
        e: &EntityStore,
        from: EntityId,
        to: EntityId,
        now: i64,
    ) -> Result<usize, MemoryError> {
        if !e.contains(to) {
            return Err(MemoryError::InvalidField(
                "Entity resolution Entity reference",
            ));
        }
        self.retarget_entity_references(c, from, to, now)
            .map_err(memory_error)
    }
    fn owner_repair_retired_entity_references(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
    ) -> Result<usize, MemoryError> {
        let mut changes = Vec::new();
        for v in self.records() {
            m.validate_historical_entity_mention_key(product_key(v.key))?;
            reliquary_perception_store::entity_resolution_store::validate_status(&v.status)
                .map_err(memory_error)?;
            let mut status = v.status.clone();
            let canonical = |id| {
                e.canonical_active_id(id)
                    .or_else(|| e.contains_historical(id).then_some(id))
                    .ok_or(MemoryError::InvalidField(
                        "Entity resolution Entity reference",
                    ))
            };
            match &mut status {
                MemoryEntityResolutionStatus::Resolved { entity_id, .. } => {
                    *entity_id = canonical(*entity_id)?
                }
                MemoryEntityResolutionStatus::Pending(p) => {
                    let old = p.candidate_entity_ids.clone();
                    p.candidate_entity_ids = old
                        .iter()
                        .copied()
                        .map(canonical)
                        .collect::<Result<Vec<_>, _>>()?;
                    p.candidate_entity_ids.sort();
                    p.candidate_entity_ids.dedup();
                    if old != p.candidate_entity_ids {
                        p.candidate_set_fingerprint = [0; 32];
                    }
                }
                MemoryEntityResolutionStatus::Dormant(p) => {
                    let old = p.candidate_entity_ids.clone();
                    p.candidate_entity_ids = old
                        .iter()
                        .copied()
                        .map(canonical)
                        .collect::<Result<Vec<_>, _>>()?;
                    p.candidate_entity_ids.sort();
                    p.candidate_entity_ids.dedup();
                    if old != p.candidate_entity_ids {
                        p.candidate_set_fingerprint = [0; 32];
                    }
                }
                _ => {}
            }
            if status != v.status {
                changes.push(
                    reliquary_perception_store::entity_resolution_codec::DecodedEntityResolution {
                        key: v.key,
                        revision: v
                            .revision
                            .checked_add(1)
                            .ok_or(MemoryError::VersionExhausted)?,
                        updated_at_ns: v.updated_at_ns,
                        status: Some(status),
                    },
                );
            }
        }
        let count = changes.len();
        for change in changes {
            self.persist_change(c, change).map_err(memory_error)?;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mention_boundary_preserves_every_literal_coordinate_and_field_tag() {
        for field in [
            crate::MemoryTextField::Title,
            crate::MemoryTextField::Content,
        ] {
            let key = MemoryEntityMentionKey {
                memory_id: MemoryId([0xab; 32]),
                field,
                start_byte: 3,
                end_byte: u32::MAX,
            };
            let wire = store_key(key);
            assert_eq!(wire.memory_id, key.memory_id.0);
            assert_eq!(wire.field.tag(), field.tag());
            assert_eq!(product_key(wire), key);
        }
    }
}
