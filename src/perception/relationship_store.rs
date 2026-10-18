#[path = "relationship_store_read.rs"]
mod read;
#[path = "relationship_store_validation.rs"]
mod validation;

use crate::entity_store::EntityStore;
use crate::memory_store::MemoryStore;
use crate::relationship_codec::{
    RelationshipTombstone, RelationshipVersion, encode_format, encode_record, encode_tombstone,
    encode_version,
};
use crate::relationship_model::RelationshipRecord;
use crate::{
    Container, EntityRef, Relationship, RelationshipDraft, RelationshipError, RelationshipId,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use validation::{normalize_draft, validate_record};

pub(crate) struct RelationshipStore {
    records: Vec<RelationshipRecord>,
    pub(crate) pruned_revisions: BTreeSet<(RelationshipId, u64)>,
    current: HashMap<RelationshipId, usize>,
    by_mutation: HashMap<String, usize>,
    by_participant: HashMap<EntityRef, BTreeSet<RelationshipId>>,
    revisions: HashMap<RelationshipId, u64>,
    tombstones: HashMap<RelationshipId, crate::owner_lifecycle::TombstoneCoordinates>,
    next_relationship_version: u64,
}

impl RelationshipStore {
    pub(crate) fn empty() -> Self {
        Self {
            records: Vec::new(),
            pruned_revisions: BTreeSet::new(),
            current: HashMap::new(),
            by_mutation: HashMap::new(),
            by_participant: HashMap::new(),
            revisions: HashMap::new(),
            tombstones: HashMap::new(),
            next_relationship_version: 1,
        }
    }

    pub(crate) fn initialize(&self, container: &mut Container) -> Result<(), RelationshipError> {
        container.append(&encode_format())?;
        Ok(())
    }

    pub(crate) fn publish(
        &mut self,
        container: &mut Container,
        owner_id: &str,
        memories: &MemoryStore,
        entities: &EntityStore,
        id: Option<RelationshipId>,
        expected_revision: u64,
        mut draft: RelationshipDraft,
    ) -> Result<(Relationship, bool), RelationshipError> {
        normalize_draft(&mut draft)?;
        validate_live_local_refs(
            owner_id,
            memories,
            entities,
            &draft.participants,
            &draft.evidence,
        )?;
        self.publish_normalized(container, id, expected_revision, draft)
    }

    pub(crate) fn publish_replayed(
        &mut self,
        container: &mut Container,
        id: Option<RelationshipId>,
        expected_revision: u64,
        mut draft: RelationshipDraft,
    ) -> Result<(Relationship, bool), RelationshipError> {
        normalize_draft(&mut draft)?;
        self.publish_normalized(container, id, expected_revision, draft)
    }

    fn publish_normalized(
        &mut self,
        container: &mut Container,
        id: Option<RelationshipId>,
        expected_revision: u64,
        draft: RelationshipDraft,
    ) -> Result<(Relationship, bool), RelationshipError> {
        let id = id.unwrap_or_else(|| relationship_id(&draft.mutation_id));
        if self.tombstones.contains_key(&id) {
            return Err(RelationshipError::TerminalTombstone);
        }
        if let Some(index) = self.by_mutation.get(&draft.mutation_id).copied() {
            let existing = resolve(&self.records[index]);
            return if existing.id == id
                && self.current.contains_key(&existing.id)
                && same_draft(&existing, &draft)
            {
                Ok((existing, false))
            } else {
                Err(RelationshipError::MutationConflict)
            };
        }
        let current = self.current.get(&id).map(|index| &self.records[*index]);
        let current_revision = self.revisions.get(&id).copied().unwrap_or(0);
        if current_revision != expected_revision {
            return Err(RelationshipError::RevisionConflict);
        }
        if current.is_some_and(|record| record.created_at_ns != draft.created_at_ns) {
            return Err(RelationshipError::InvalidField("Relationship created_at"));
        }

        let revision = expected_revision
            .checked_add(1)
            .ok_or(RelationshipError::VersionExhausted)?;
        let relationship_version = self.next_relationship_version;
        relationship_version
            .checked_add(1)
            .ok_or(RelationshipError::VersionExhausted)?;
        let mut record = RelationshipRecord {
            id,
            revision,
            kind: draft.kind,
            participants: draft.participants,
            evidence: draft.evidence,
            summary: draft.summary,
            mutation_id: draft.mutation_id,
            created_at_ns: draft.created_at_ns,
            updated_at_ns: draft.updated_at_ns,
            global_version: 0,
            relationship_version,
        };
        let record_ref = container.append(&encode_record(&record)?)?;
        let global_version = container.allocate_version()?;
        container.append(&encode_version(RelationshipVersion {
            global_version,
            relationship_version,
            record: record_ref,
        }))?;
        record.global_version = global_version;
        self.insert_rebuilt(record.clone())?;
        Ok((resolve(&record), true))
    }

    pub(crate) fn insert_rebuilt(
        &mut self,
        record: RelationshipRecord,
    ) -> Result<(), RelationshipError> {
        if self.tombstones.contains_key(&record.id) {
            return Err(RelationshipError::TerminalTombstone);
        }
        if record.revision == 0
            || record.relationship_version == 0
            || record.global_version == 0
            || record.relationship_version != self.next_relationship_version
        {
            return Err(RelationshipError::InvalidVersion);
        }
        if let Some(index) = self.by_mutation.get(&record.mutation_id).copied() {
            return if self.records[index] == record {
                Ok(())
            } else {
                Err(RelationshipError::MutationConflict)
            };
        }
        let expected_revision = self
            .revisions
            .get(&record.id)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(RelationshipError::VersionExhausted)?;
        let current = self
            .current
            .get(&record.id)
            .map(|index| &self.records[*index]);
        if record.revision != expected_revision
            || current.is_some_and(|prior| prior.created_at_ns != record.created_at_ns)
        {
            return Err(RelationshipError::RevisionConflict);
        }
        validate_record(&record)?;
        if let Some(index) = self.current.get(&record.id).copied() {
            self.deindex(index);
        }
        let index = self.records.len();
        for participant in &record.participants {
            self.by_participant
                .entry(participant.entity.clone())
                .or_default()
                .insert(record.id);
        }
        self.by_mutation.insert(record.mutation_id.clone(), index);
        self.current.insert(record.id, index);
        self.revisions.insert(record.id, record.revision);
        self.records.push(record);
        self.next_relationship_version = self
            .next_relationship_version
            .checked_add(1)
            .ok_or(RelationshipError::VersionExhausted)?;
        Ok(())
    }

    fn deindex(&mut self, index: usize) {
        let record = &self.records[index];
        for participant in &record.participants {
            if let Some(ids) = self.by_participant.get_mut(&participant.entity) {
                ids.remove(&record.id);
                if ids.is_empty() {
                    self.by_participant.remove(&participant.entity);
                }
            }
        }
    }

    pub(crate) fn tombstone(
        &mut self,
        container: &mut Container,
        id: RelationshipId,
        expected_revision: u64,
    ) -> Result<crate::owner_lifecycle::TombstoneOutcome, RelationshipError> {
        if let Some(&coordinates) = self.tombstones.get(&id) {
            if expected_revision.checked_add(1) != self.revisions.get(&id).copied() {
                return Err(RelationshipError::RevisionConflict);
            }
            return Ok(crate::owner_lifecycle::TombstoneOutcome::AlreadyTombstoned(
                coordinates,
            ));
        }

        let index = self
            .current
            .get(&id)
            .copied()
            .ok_or(RelationshipError::MissingRelationship)?;
        if self.records[index].revision != expected_revision {
            return Err(RelationshipError::RevisionConflict);
        }

        self.next_relationship_version
            .checked_add(1)
            .ok_or(RelationshipError::VersionExhausted)?;
        let tombstone = RelationshipTombstone {
            id,
            revision: expected_revision
                .checked_add(1)
                .ok_or(RelationshipError::VersionExhausted)?,
        };
        let record = container.append(&encode_tombstone(tombstone))?;
        let global_version = container.allocate_version()?;
        let relationship_version = self.next_relationship_version;
        container.append(&encode_version(RelationshipVersion {
            global_version,
            relationship_version,
            record,
        }))?;
        self.insert_tombstone_rebuilt(tombstone, global_version, relationship_version)?;

        Ok(crate::owner_lifecycle::TombstoneOutcome::Published(
            crate::owner_lifecycle::TombstoneCoordinates {
                global_version: Some(global_version),
                domain_version: Some(relationship_version),
            },
        ))
    }

    pub(crate) fn insert_tombstone_rebuilt(
        &mut self,
        tombstone: RelationshipTombstone,
        global_version: u64,
        relationship_version: u64,
    ) -> Result<(), RelationshipError> {
        if global_version == 0 || relationship_version != self.next_relationship_version {
            return Err(RelationshipError::InvalidVersion);
        }

        let index = self
            .current
            .get(&tombstone.id)
            .copied()
            .ok_or(RelationshipError::MissingRelationship)?;
        if tombstone.revision
            != self.records[index]
                .revision
                .checked_add(1)
                .ok_or(RelationshipError::VersionExhausted)?
        {
            return Err(RelationshipError::RevisionConflict);
        }

        self.current.remove(&tombstone.id);
        self.deindex(index);
        self.revisions.insert(tombstone.id, tombstone.revision);
        self.tombstones.insert(
            tombstone.id,
            crate::owner_lifecycle::TombstoneCoordinates {
                global_version: Some(global_version),
                domain_version: Some(relationship_version),
            },
        );
        self.next_relationship_version = relationship_version
            .checked_add(1)
            .ok_or(RelationshipError::VersionExhausted)?;
        Ok(())
    }

    pub(crate) fn validate_local_references(
        &self,
        owner_id: Option<&str>,
        memories: &MemoryStore,
        entities: &EntityStore,
    ) -> Result<(), RelationshipError> {
        if self.current.is_empty() {
            return Ok(());
        }
        let owner_id = owner_id.ok_or(RelationshipError::MissingOwnerIdentity)?;
        for index in self.current.values() {
            let record = &self.records[*index];
            validate_historical_local_refs(
                owner_id,
                memories,
                entities,
                &record.participants,
                &record.evidence,
            )?;
        }
        Ok(())
    }
}

fn validate_live_local_refs(
    owner_id: &str,
    memories: &MemoryStore,
    entities: &EntityStore,
    participants: &[crate::RelationshipParticipant],
    evidence: &[crate::MemoryRef],
) -> Result<(), RelationshipError> {
    validate_live_local_entity_refs(owner_id, entities, participants)?;
    for value in evidence {
        if value.owner_id == owner_id && !memories.contains_memory(value.memory_id) {
            return Err(RelationshipError::MissingLocalMemory(value.memory_id));
        }
    }
    Ok(())
}

fn validate_historical_local_refs(
    owner_id: &str,
    memories: &MemoryStore,
    entities: &EntityStore,
    participants: &[crate::RelationshipParticipant],
    evidence: &[crate::MemoryRef],
) -> Result<(), RelationshipError> {
    validate_historical_local_entity_refs(owner_id, entities, participants)?;
    for value in evidence {
        if value.owner_id == owner_id && !memories.contains_historical_memory(value.memory_id) {
            return Err(RelationshipError::MissingLocalMemory(value.memory_id));
        }
    }
    Ok(())
}

fn validate_live_local_entity_refs(
    owner_id: &str,
    entities: &EntityStore,
    participants: &[crate::RelationshipParticipant],
) -> Result<(), RelationshipError> {
    for participant in participants {
        if participant.entity.owner_id == owner_id
            && !entities.contains(participant.entity.entity_id)
        {
            return Err(RelationshipError::MissingLocalEntity(
                participant.entity.entity_id,
            ));
        }
    }
    Ok(())
}

fn validate_historical_local_entity_refs(
    owner_id: &str,
    entities: &EntityStore,
    participants: &[crate::RelationshipParticipant],
) -> Result<(), RelationshipError> {
    for participant in participants {
        if participant.entity.owner_id == owner_id
            && !entities.contains_historical(participant.entity.entity_id)
        {
            return Err(RelationshipError::MissingLocalEntity(
                participant.entity.entity_id,
            ));
        }
    }
    Ok(())
}

fn relationship_id(mutation_id: &str) -> RelationshipId {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-relationship-id\0");
    hash.update((mutation_id.len() as u64).to_le_bytes());
    hash.update(mutation_id.as_bytes());
    RelationshipId(hash.finalize().into())
}

fn resolve(record: &RelationshipRecord) -> Relationship {
    Relationship {
        id: record.id,
        revision: record.revision,
        kind: record.kind.clone(),
        participants: record.participants.clone(),
        evidence: record.evidence.clone(),
        summary: record.summary.clone(),
        mutation_id: record.mutation_id.clone(),
        created_at_ns: record.created_at_ns,
        updated_at_ns: record.updated_at_ns,
        global_version: record.global_version,
        relationship_version: record.relationship_version,
    }
}

fn same_draft(value: &Relationship, draft: &RelationshipDraft) -> bool {
    value.kind == draft.kind
        && value.participants == draft.participants
        && value.evidence == draft.evidence
        && value.summary == draft.summary
        && value.mutation_id == draft.mutation_id
        && value.created_at_ns == draft.created_at_ns
        && value.updated_at_ns == draft.updated_at_ns
}
