//! Relationship-owned persistence of a resolved compact revision.
use super::{RelationshipStore, resolve, validate_record};
use crate::relationship_codec::{RelationshipVersion, encode_version};
use crate::relationship_compact_codec::{encode_creation, encode_delta};
use crate::relationship_compact_contract::{RelationshipCreation, RelationshipDelta};
use crate::relationship_model::RelationshipRecord;
use crate::{Container, Relationship, RelationshipError};

impl RelationshipStore {
    /// Callers resolve drafts and retries; this seam validates admission before
    /// writing and owns the carrier/marker pair for both ordinary and replay writes.
    pub(super) fn persist_compact_revision(
        &mut self,
        container: &mut Container,
        mut record: RelationshipRecord,
    ) -> Result<Relationship, RelationshipError> {
        container.require_storage_format_writable()?;
        if self.tombstones.contains_key(&record.id) {
            return Err(RelationshipError::TerminalTombstone);
        }
        if self.by_mutation.contains_key(&record.mutation_id) {
            return Err(RelationshipError::MutationConflict);
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
            .ok_or(RelationshipError::VersionExhausted)?;
        if record.revision != expected_revision
            || previous.is_some_and(|prior| prior.created_at_ns != record.created_at_ns)
        {
            return Err(RelationshipError::RevisionConflict);
        }
        record.relationship_version = self.next_relationship_version;
        record
            .relationship_version
            .checked_add(1)
            .ok_or(RelationshipError::VersionExhausted)?;
        record.global_version = container.next_version_candidate();
        record
            .global_version
            .checked_add(1)
            .ok_or(RelationshipError::VersionExhausted)?;
        let payload = match previous {
            Some(prior) => encode_delta(&RelationshipDelta::between(prior, &record)?)?,
            None => encode_creation(&RelationshipCreation::new(record.clone())?)?,
        };
        let carrier = container.append(&payload)?;
        let global_version = container.allocate_version()?;
        if global_version != record.global_version {
            return Err(RelationshipError::InvalidVersion);
        }
        container.append(&encode_version(RelationshipVersion {
            global_version,
            relationship_version: record.relationship_version,
            record: carrier,
        }))?;
        self.insert_rebuilt(record.clone())?;
        Ok(resolve(&record))
    }
}
