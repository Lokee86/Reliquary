use super::{EntityStore, validate_record};
use crate::entity_model::EntityRecord;
use crate::{EntityError, EntityId, EntityRetainedRevision};
use reliquary_container::Container;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityReplayPublication {
    Record(EntityRecord),
    PrunedRecord {
        id: EntityId,
        revision: EntityRetainedRevision,
    },
    Tombstone {
        id: EntityId,
        revision: u64,
        replacement_id: Option<EntityId>,
    },
}

impl EntityReplayPublication {
    pub fn normalized_import_evidence(&self) -> Result<Vec<u8>, EntityError> {
        let value = match self {
            Self::Record(record) => serde_json::to_vec(&(
                "entity-import-evidence-v1",
                "record",
                record.id.0,
                record.global_id.map(|id| id.0),
                record.revision,
                &record.canonical_name,
                &record.aliases,
                &record.kind,
                &record.summary,
                &record.mutation_id,
                record.created_at_ns,
                record.updated_at_ns,
            )),
            Self::PrunedRecord { id, revision } => serde_json::to_vec(&(
                "entity-import-evidence-v1",
                "pruned",
                id.0,
                revision.global_id.map(|id| id.0),
                revision.revision,
                &revision.kind,
                revision.created_at_ns,
                revision.updated_at_ns,
                &revision.mutation_id,
            )),
            Self::Tombstone {
                id,
                revision,
                replacement_id,
            } => serde_json::to_vec(&(
                "entity-import-evidence-v1",
                "tombstone",
                id.0,
                revision,
                replacement_id.map(|id| id.0),
            )),
        };
        value.map_err(|_| EntityError::CorruptRecord("Entity semantic evidence encoding"))
    }
}

impl EntityStore {
    pub fn publication_at(
        &self,
        global_version: u64,
        entity_version: u64,
    ) -> Result<Option<EntityReplayPublication>, EntityError> {
        match self.retained_publication_at(global_version, entity_version)? {
            Some(EntityReplayPublication::PrunedRecord { .. }) => Err(
                EntityError::PayloadUnavailable(crate::EntityPayloadUnavailable::Pruned),
            ),
            publication => Ok(publication),
        }
    }

    pub fn retained_publication_at(
        &self,
        global_version: u64,
        entity_version: u64,
    ) -> Result<Option<EntityReplayPublication>, EntityError> {
        if let Some(record) = self.records.iter().find(|record| {
            record.global_version == global_version && record.entity_version == entity_version
        }) {
            if self
                .pruned_revisions
                .contains(&(record.id, record.revision))
            {
                return Ok(Some(EntityReplayPublication::PrunedRecord {
                    id: record.id,
                    revision: EntityRetainedRevision {
                        revision: record.revision,
                        global_version: record.global_version,
                        entity_version: record.entity_version,
                        global_id: record.global_id,
                        kind: record.kind.clone(),
                        created_at_ns: record.created_at_ns,
                        updated_at_ns: record.updated_at_ns,
                        mutation_id: record.mutation_id.clone(),
                    },
                }));
            }
            return Ok(Some(EntityReplayPublication::Record(record.clone())));
        }

        for (id, coordinates) in &self.tombstones {
            if coordinates.global_version == global_version
                && coordinates.entity_version == entity_version
            {
                let revision = *self.revisions.get(id).ok_or(EntityError::InvalidVersion)?;
                return Ok(Some(EntityReplayPublication::Tombstone {
                    id: *id,
                    revision,
                    replacement_id: self.retired.get(id).copied(),
                }));
            }
        }
        Ok(None)
    }

    pub fn replay_publication(
        &mut self,
        container: &mut Container,
        publication: EntityReplayPublication,
    ) -> Result<bool, EntityError> {
        match publication {
            EntityReplayPublication::Record(record) => self.replay_record(container, record),
            EntityReplayPublication::PrunedRecord { .. } => Err(EntityError::PayloadUnavailable(
                crate::EntityPayloadUnavailable::Pruned,
            )),
            EntityReplayPublication::Tombstone {
                id,
                revision,
                replacement_id,
            } => {
                if self.tombstones.contains_key(&id) {
                    return if self.revisions.get(&id).copied() == Some(revision)
                        && self.retired.get(&id).copied() == replacement_id
                    {
                        Ok(false)
                    } else {
                        Err(EntityError::TerminalTombstone)
                    };
                }
                let expected_revision = revision
                    .checked_sub(1)
                    .ok_or(EntityError::RevisionConflict)?;
                match replacement_id {
                    Some(replacement_id) => {
                        self.tombstone(container, id, expected_revision, replacement_id)
                    }
                    None => match self.tombstone_direct(container, id, expected_revision)? {
                        crate::EntityTerminalOutcome::Published(_) => Ok(true),
                        crate::EntityTerminalOutcome::AlreadyTombstoned(_) => Ok(false),
                    },
                }
            }
        }
    }

    fn replay_record(
        &mut self,
        container: &mut Container,
        record: EntityRecord,
    ) -> Result<bool, EntityError> {
        validate_record(&record)?;
        if self.tombstones.contains_key(&record.id) {
            return Err(EntityError::TerminalTombstone);
        }
        if let Some(index) = self.by_mutation.get(&record.mutation_id).copied() {
            let existing = &self.records[index];
            return if existing.id == record.id
                && self.current.contains_key(&record.id)
                && same_replay_record(existing, &record)
            {
                Ok(false)
            } else {
                Err(EntityError::MutationConflict)
            };
        }
        self.persist_compact_revision(container, record)?;
        Ok(true)
    }
}

fn same_replay_record(left: &EntityRecord, right: &EntityRecord) -> bool {
    left.id == right.id
        && left.global_id == right.global_id
        && left.revision == right.revision
        && left.canonical_name == right.canonical_name
        && left.aliases == right.aliases
        && left.kind == right.kind
        && left.summary == right.summary
        && left.mutation_id == right.mutation_id
        && left.created_at_ns == right.created_at_ns
        && left.updated_at_ns == right.updated_at_ns
}

#[cfg(test)]
#[path = "entity_import_evidence_tests.rs"]
mod import_evidence_tests;
#[cfg(test)]
#[path = "entity_replay_tests.rs"]
mod tests;
