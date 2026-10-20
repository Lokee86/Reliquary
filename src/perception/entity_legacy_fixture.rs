//! Temporary V1 source construction for tests only.
//!
//! Keep this writer isolated from ordinary publication so tests can build
//! genuine legacy carriers after the production writer moves to compact data.
use super::*;
use crate::entity_codec::encode_record;

impl EntityStore {
    #[cfg(test)]
    pub(crate) fn publish_legacy_fixture(
        &mut self,
        container: &mut Container,
        id: Option<EntityId>,
        expected_revision: u64,
        mut draft: EntityDraft,
        requested_global_id: Option<EntityGlobalId>,
    ) -> Result<(Entity, bool), EntityError> {
        container.require_storage_format_writable()?;
        normalize_draft(&mut draft)?;
        let id = if draft.kind == crate::entity_principal::PRINCIPAL_ENTITY_KIND {
            let principal_id = crate::entity_principal::principal_entity_id(&draft.canonical_name);
            if id.is_some_and(|id| id != principal_id) {
                return Err(EntityError::InvalidField("principal Entity ID"));
            }
            principal_id
        } else {
            id.unwrap_or_else(|| entity_id(&draft.mutation_id))
        };
        if self.tombstones.contains_key(&id) {
            return Err(EntityError::TerminalTombstone);
        }
        if let Some(index) = self.by_mutation.get(&draft.mutation_id).copied() {
            let existing = resolve(&self.records[index]);
            return if existing.id == id
                && self.current.contains_key(&existing.id)
                && same_draft(&existing, &draft)
            {
                Ok((existing, false))
            } else {
                Err(EntityError::MutationConflict)
            };
        }
        let current = self.current.get(&id).map(|index| &self.records[*index]);
        let current_revision = self.revisions.get(&id).copied().unwrap_or(0);
        if current_revision != expected_revision {
            return Err(EntityError::RevisionConflict);
        }
        if current.is_some_and(|record| record.created_at_ns != draft.created_at_ns) {
            return Err(EntityError::InvalidField("Entity created_at"));
        }
        let global_id = match current.and_then(|record| record.global_id) {
            Some(existing) => {
                if requested_global_id.is_some_and(|requested| requested != existing) {
                    return Err(EntityError::InvalidField("Entity global UUID"));
                }
                existing
            }
            None => requested_global_id.unwrap_or_else(|| {
                if draft.kind == crate::entity_principal::PRINCIPAL_ENTITY_KIND {
                    crate::entity_principal::principal_entity_global_id(&draft.canonical_name)
                } else {
                    EntityGlobalId(*uuid::Uuid::new_v4().as_bytes())
                }
            }),
        };

        let revision = expected_revision
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;
        let entity_version = self.next_entity_version;
        entity_version
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;

        let mut record = EntityRecord {
            id,
            global_id: Some(global_id),
            revision,
            canonical_name: draft.canonical_name,
            aliases: draft.aliases,
            kind: draft.kind,
            summary: draft.summary,
            mutation_id: draft.mutation_id,
            created_at_ns: draft.created_at_ns,
            updated_at_ns: draft.updated_at_ns,
            global_version: 0,
            entity_version,
        };
        let carrier = container.append(&encode_record(&record)?)?;
        let global_version = container.allocate_version()?;
        container.append(&encode_version(EntityVersion {
            global_version,
            entity_version,
            record: carrier,
        }))?;
        record.global_version = global_version;
        self.insert_rebuilt(record.clone())?;
        Ok((resolve(&record), true))
    }
}
