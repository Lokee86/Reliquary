//! Entity-owned persistence of a resolved compact revision.
use super::{EntityStore, resolve, validate_record};
use crate::entity_compact_codec::publication::{self, EntityPublication};
use crate::entity_compact_contract::{EntityCreation, EntityDelta};
use crate::entity_model::EntityRecord;
use crate::{Container, Entity, EntityError};

impl EntityStore {
    /// Callers resolve drafts and retries; this seam validates admission before
    /// writing and owns one envelope for both ordinary and replay writes.
    pub(super) fn persist_compact_revision(
        &mut self,
        container: &mut Container,
        mut record: EntityRecord,
    ) -> Result<Entity, EntityError> {
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
            Some(prior) => publication::encode(&EntityPublication::Delta(EntityDelta::between(
                prior, &record,
            )?))?,
            None => publication::encode(&EntityPublication::Creation(EntityCreation::new(
                record.clone(),
            )?))?,
        };
        let (global_version, _) = container.append_publication_envelope(&payload)?;
        if global_version != record.global_version {
            return Err(EntityError::InvalidVersion);
        }
        self.insert_rebuilt(record.clone())?;
        Ok(resolve(&record))
    }
}
