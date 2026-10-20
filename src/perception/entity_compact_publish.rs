//! Entity-owned persistence of a resolved compact revision.
use super::{EntityStore, resolve, validate_record};
use crate::entity_codec::{EntityVersion, encode_version};
use crate::entity_compact_codec::{encode_creation, encode_delta};
use crate::entity_compact_contract::{EntityCreation, EntityDelta};
use crate::entity_model::EntityRecord;
use crate::{Container, Entity, EntityError};

impl EntityStore {
    /// Callers resolve drafts and retries; this seam validates admission before
    /// writing and owns the carrier/marker pair for both ordinary and replay writes.
    pub(super) fn persist_compact_revision(
        &mut self,
        container: &mut Container,
        mut record: EntityRecord,
    ) -> Result<Entity, EntityError> {
        container.require_storage_format_writable()?;
        if self.tombstones.contains_key(&record.id) {
            return Err(EntityError::TerminalTombstone);
        }
        if self.by_mutation.contains_key(&record.mutation_id) {
            return Err(EntityError::MutationConflict);
        }
        validate_record(&record)?;
        let previous = self
            .current
            .get(&record.id)
            .map(|index| &self.records[*index]);
        let expected_revision = self
            .revisions
            .get(&record.id)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;
        if record.revision != expected_revision
            || previous.is_some_and(|prior| prior.created_at_ns != record.created_at_ns)
        {
            return Err(EntityError::RevisionConflict);
        }
        record.entity_version = self.next_entity_version;
        record
            .entity_version
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;
        record.global_version = container.next_version_candidate();
        record
            .global_version
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;
        let payload = match previous {
            Some(prior) => encode_delta(&EntityDelta::between(prior, &record)?)?,
            None => encode_creation(&EntityCreation::new(record.clone())?)?,
        };
        let carrier = container.append(&payload)?;
        let global_version = container.allocate_version()?;
        if global_version != record.global_version {
            return Err(EntityError::InvalidVersion);
        }
        container.append(&encode_version(EntityVersion {
            global_version,
            entity_version: record.entity_version,
            record: carrier,
        }))?;
        self.insert_rebuilt(record.clone())?;
        Ok(resolve(&record))
    }
}
