use super::{RelationshipStore, validate_record};
use crate::relationship_model::RelationshipRecord;
use crate::{RelationshipError, RelationshipId, RelationshipRetainedRevision};
use reliquary_container::Container;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationshipReplayPublication {
    Record(RelationshipRecord),
    PrunedRecord {
        id: RelationshipId,
        revision: RelationshipRetainedRevision,
    },
    Tombstone {
        id: RelationshipId,
        revision: u64,
    },
}

impl RelationshipReplayPublication {
    pub fn normalized_import_evidence(&self) -> Result<Vec<u8>, RelationshipError> {
        let value = match self {
            Self::Record(record) => serde_json::to_vec(&(
                "relationship-import-evidence-v1",
                "record",
                record.id.0,
                record.revision,
                &record.kind,
                participant_evidence(&record.participants),
                memory_evidence(&record.evidence),
                &record.summary,
                &record.mutation_id,
                record.created_at_ns,
                record.updated_at_ns,
            )),
            Self::PrunedRecord { id, revision } => serde_json::to_vec(&(
                "relationship-import-evidence-v1",
                "pruned",
                id.0,
                revision.revision,
                &revision.kind,
                participant_evidence(&revision.participants),
                memory_evidence(&revision.evidence),
                &revision.mutation_id,
                revision.created_at_ns,
                revision.updated_at_ns,
            )),
            Self::Tombstone { id, revision } => serde_json::to_vec(&(
                "relationship-import-evidence-v1",
                "tombstone",
                id.0,
                revision,
            )),
        };
        value.map_err(|_| {
            RelationshipError::CorruptRecord("Relationship semantic evidence encoding")
        })
    }
}
fn participant_evidence(
    values: &[crate::RelationshipParticipant],
) -> Vec<(&str, [u8; 32], Option<&str>)> {
    values
        .iter()
        .map(|v| {
            (
                v.entity.owner_id.as_str(),
                v.entity.entity_id.0,
                v.role.as_deref(),
            )
        })
        .collect()
}
fn memory_evidence(values: &[crate::RelationshipMemoryRef]) -> Vec<(&str, [u8; 32])> {
    values
        .iter()
        .map(|v| (v.owner_id.as_str(), v.memory_id))
        .collect()
}
impl RelationshipStore {
    pub fn publication_at(
        &self,
        global: u64,
        local: u64,
    ) -> Result<Option<RelationshipReplayPublication>, RelationshipError> {
        match self.retained_publication_at(global, local)? {
            Some(RelationshipReplayPublication::PrunedRecord { .. }) => {
                Err(RelationshipError::PayloadUnavailable(
                    crate::RelationshipPayloadUnavailable::Pruned,
                ))
            }
            value => Ok(value),
        }
    }
    pub fn retained_publication_at(
        &self,
        global: u64,
        local: u64,
    ) -> Result<Option<RelationshipReplayPublication>, RelationshipError> {
        if let Some(record) = self
            .records
            .iter()
            .find(|v| v.global_version == global && v.relationship_version == local)
        {
            if self
                .pruned_revisions
                .contains(&(record.id, record.revision))
            {
                return Ok(Some(RelationshipReplayPublication::PrunedRecord {
                    id: record.id,
                    revision: RelationshipRetainedRevision {
                        revision: record.revision,
                        global_version: global,
                        relationship_version: local,
                        kind: record.kind.clone(),
                        participants: record.participants.clone(),
                        evidence: record.evidence.clone(),
                        created_at_ns: record.created_at_ns,
                        updated_at_ns: record.updated_at_ns,
                        mutation_id: record.mutation_id.clone(),
                    },
                }));
            }
            return Ok(Some(RelationshipReplayPublication::Record(record.clone())));
        }
        for (id, coordinates) in &self.tombstones {
            if coordinates.global_version == Some(global)
                && coordinates.domain_version == Some(local)
            {
                return Ok(Some(RelationshipReplayPublication::Tombstone {
                    id: *id,
                    revision: *self
                        .revisions
                        .get(id)
                        .ok_or(RelationshipError::InvalidVersion)?,
                }));
            }
        }
        Ok(None)
    }
    pub fn replay_publication(
        &mut self,
        container: &mut Container,
        publication: RelationshipReplayPublication,
    ) -> Result<bool, RelationshipError> {
        match publication {
            RelationshipReplayPublication::Record(record) => {
                validate_record(&record)?;
                if self.tombstones.contains_key(&record.id) {
                    return Err(RelationshipError::TerminalTombstone);
                }
                if let Some(index) = self.by_mutation.get(&record.mutation_id).copied() {
                    let existing = &self.records[index];
                    return if self.current.contains_key(&record.id)
                        && same_replay_record(existing, &record)
                    {
                        Ok(false)
                    } else {
                        Err(RelationshipError::MutationConflict)
                    };
                }
                self.persist_compact_revision(container, record)?;
                Ok(true)
            }
            RelationshipReplayPublication::PrunedRecord { .. } => {
                Err(RelationshipError::PayloadUnavailable(
                    crate::RelationshipPayloadUnavailable::Pruned,
                ))
            }
            RelationshipReplayPublication::Tombstone { id, revision } => {
                if self.tombstones.contains_key(&id) {
                    return if self.revisions.get(&id).copied() == Some(revision) {
                        Ok(false)
                    } else {
                        Err(RelationshipError::TerminalTombstone)
                    };
                }
                let expected = revision
                    .checked_sub(1)
                    .ok_or(RelationshipError::RevisionConflict)?;
                match self.tombstone(container, id, expected)? {
                    crate::RelationshipTombstoneOutcome::Published(_) => Ok(true),
                    crate::RelationshipTombstoneOutcome::AlreadyTombstoned(_) => Ok(false),
                }
            }
        }
    }
}
fn same_replay_record(left: &RelationshipRecord, right: &RelationshipRecord) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.global_version = 0;
    left.relationship_version = 0;
    right.global_version = 0;
    right.relationship_version = 0;
    left == right
}
