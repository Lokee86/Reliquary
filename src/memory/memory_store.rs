use crate::memory_codec::{MemoryTombstone, decode_body, encode_body, encode_format};
use crate::memory_compact_codec::publication::{self, MemoryPublication};
use crate::memory_model::{
    ExternalMemoryBinding, ExternalMemoryEvidence, MemoryRecord, memory_body_bytes, memory_body_id,
    memory_id,
};
use crate::memory_model::{GroupedBodyEncoding, GroupedBodySpan};
use crate::{
    Container, MAX_MEMORY_ENTITY_MENTIONS, MAX_MEMORY_ROUTING_TEXT_BYTES, Memory, MemoryBodyId,
    MemoryDraft, MemoryEntityMentionKey, MemoryError, MemoryId, MemoryRoutingMetadata,
    MemorySourceRef, MemoryStats, MemoryTemporalInference, MemoryTextField, ObjectRef,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

#[derive(Clone, Debug)]
enum BodyLocation {
    Frame(ObjectRef),
    GroupedFrame {
        frame: ObjectRef,
        range: Range<usize>,
        encoding: GroupedBodyEncoding,
    },
}

impl BodyLocation {
    fn frame(&self) -> ObjectRef {
        match self {
            Self::Frame(frame) | Self::GroupedFrame { frame, .. } => *frame,
        }
    }
}

#[cfg(test)]
#[path = "memory_rebuild_overflow_tests.rs"]
mod rebuild_overflow_tests;

#[path = "memory_store/grouped_publish.rs"]
mod grouped_publish;
#[path = "memory_store/history_prune.rs"]
pub(crate) mod history_prune;

pub(crate) struct MemoryStore {
    pub(crate) external_evidence: HashMap<(MemoryId, MemoryBodyId), ExternalMemoryEvidence>,
    pub(crate) external_windows: HashMap<[u8; 32], Vec<MemoryId>>,
    bodies: HashMap<MemoryBodyId, BodyLocation>,
    routing_metadata: HashMap<MemoryId, MemoryRoutingMetadata>,
    entity_mentions_by_surface: HashMap<String, HashSet<MemoryEntityMentionKey>>,
    records: Vec<MemoryRecord>,
    current: HashMap<MemoryId, usize>,
    by_mutation: HashMap<String, usize>,
    revisions: HashMap<MemoryId, u64>,
    tombstones: HashMap<MemoryId, crate::owner_lifecycle::TombstoneCoordinates>,
    pub(crate) prune: history_prune::MemoryPruneState,
    next_memory_version: u64,
}

/// A Container-neutral, owner-qualified view of current Memory read facts.
/// Construction is inert; bodies are loaded only when `memory` is requested.
#[derive(Clone, Copy)]
pub(crate) struct MemoryReadView<'a> {
    store: &'a MemoryStore,
    container: &'a Container,
}

impl<'a> MemoryReadView<'a> {
    pub(crate) fn current_body_id(&self, id: MemoryId) -> Result<MemoryBodyId, MemoryError> {
        self.store.current_body_id(id)
    }

    pub(crate) fn new(store: &'a MemoryStore, container: &'a Container) -> Self {
        Self { store, container }
    }

    pub(crate) fn memory(&self, id: MemoryId) -> Result<Memory, MemoryError> {
        self.store.memory(self.container, id)
    }

    pub(crate) fn current_ids(&self) -> Vec<MemoryId> {
        self.store.current_ids()
    }

    pub(crate) fn current_body_ids(&self) -> Vec<MemoryBodyId> {
        self.store.current_body_ids()
    }

    pub(crate) fn routing_metadata(&self, id: MemoryId) -> Option<&MemoryRoutingMetadata> {
        self.store.routing_metadata(id)
    }

    pub(crate) fn memory_version(&self) -> u64 {
        self.store.memory_version()
    }

    pub(crate) fn contains_memory(&self, id: MemoryId) -> bool {
        self.store.contains_memory(id)
    }

    pub(crate) fn validate_entity_mention_key(
        &self,
        key: MemoryEntityMentionKey,
    ) -> Result<(), MemoryError> {
        self.store.validate_entity_mention_key(key)
    }

    pub(crate) fn resolution_source_inactive(&self, id: MemoryId) -> bool {
        self.store.resolution_source_inactive(id)
    }

    pub(crate) fn body_embedding_text(&self, id: MemoryBodyId) -> Result<String, MemoryError> {
        self.store.body_embedding_text(self.container, id)
    }

    pub(crate) fn records(&self) -> &[MemoryRecord] {
        self.store.records()
    }
}

pub(crate) enum PublishPreflight {
    Retry(Memory),
    New {
        record: MemoryRecord,
        next_memory_version: u64,
        body_id: MemoryBodyId,
    },
}

impl MemoryStore {
    pub(crate) fn empty() -> Self {
        Self {
            external_evidence: HashMap::new(),
            external_windows: HashMap::new(),
            bodies: HashMap::new(),
            routing_metadata: HashMap::new(),
            entity_mentions_by_surface: HashMap::new(),
            records: Vec::new(),
            current: HashMap::new(),
            by_mutation: HashMap::new(),
            revisions: HashMap::new(),
            tombstones: HashMap::new(),
            prune: history_prune::MemoryPruneState::default(),
            next_memory_version: 1,
        }
    }

    pub(crate) fn initialize(&self, container: &mut Container) -> Result<(), MemoryError> {
        container.append(&encode_format())?;
        Ok(())
    }

    pub(crate) fn publish(
        &mut self,
        container: &mut Container,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
    ) -> Result<(Memory, bool), MemoryError> {
        self.publish_with_source_ref_and_temporal_inference(
            container,
            id,
            expected_revision,
            draft,
            None,
            None,
        )
    }

    pub(crate) fn publish_with_source_ref(
        &mut self,
        container: &mut Container,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
        source_ref: Option<MemorySourceRef>,
    ) -> Result<(Memory, bool), MemoryError> {
        self.publish_with_source_ref_and_temporal_inference(
            container,
            id,
            expected_revision,
            draft,
            source_ref,
            None,
        )
    }

    pub(crate) fn publish_with_source_ref_and_temporal_inference(
        &mut self,
        container: &mut Container,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
        source_ref: Option<MemorySourceRef>,
        temporal_inference: Option<MemoryTemporalInference>,
    ) -> Result<(Memory, bool), MemoryError> {
        let preflight = self.preflight_publish(
            container,
            id,
            expected_revision,
            &draft,
            source_ref.as_ref(),
            temporal_inference.as_ref(),
        )?;
        let (mut record, next_memory_version) = match preflight {
            PublishPreflight::Retry(existing) => return Ok((existing, false)),
            PublishPreflight::New {
                record,
                next_memory_version,
                ..
            } => (record, next_memory_version),
        };
        self.put_body(container, &draft.title, &draft.content)?;
        record.global_version = container.next_version_candidate();
        let encoded = if let Some(index) = self.current.get(&record.id) {
            let delta = crate::memory_compact_contract::MemoryDelta::between(
                &self.records[*index],
                &record,
            )?;
            publication::encode(&MemoryPublication::Delta(delta))?
        } else {
            publication::encode(&MemoryPublication::Creation(
                crate::memory_compact_contract::MemoryCreation::new(record.clone())?,
            ))?
        };
        let (global_version, _) = container.append_publication_envelope(&encoded)?;
        record.global_version = global_version;
        self.insert(record.clone())?;
        self.next_memory_version = next_memory_version;
        self.resolve_record(container, &record)
            .map(|memory| (memory, true))
    }

    pub(crate) fn preflight_publish(
        &self,
        container: &mut Container,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: &MemoryDraft,
        source_ref: Option<&MemorySourceRef>,
        temporal_inference: Option<&MemoryTemporalInference>,
    ) -> Result<PublishPreflight, MemoryError> {
        validate_draft(draft)?;
        validate_source_ref(source_ref)?;
        let body_id = memory_body_id(&draft.title, &draft.content);
        if let Some(inference) = temporal_inference {
            validate_temporal_inference_binding(body_id, draft.source_time_ns, inference)?;
        }
        let target_id = id.unwrap_or_else(|| memory_id(&draft.mutation_id));
        if self.tombstones.contains_key(&target_id) {
            return Err(MemoryError::TerminalTombstone);
        }
        if let Some(index) = self.by_mutation.get(&draft.mutation_id).copied() {
            if self.records[index].id != target_id
                && self.tombstones.contains_key(&self.records[index].id)
            {
                return Err(MemoryError::MutationConflict);
            }
            let existing = self.resolve_record(container, &self.records[index])?;
            let source_matches = source_ref
                .is_none_or(|source_ref| existing.source_ref.as_ref() == Some(source_ref));
            let inference_matches = temporal_inference
                .is_none_or(|inference| existing.temporal_inference.as_ref() == Some(inference));
            return if same_draft(&existing, draft) && source_matches && inference_matches {
                Ok(PublishPreflight::Retry(existing))
            } else {
                Err(MemoryError::MutationConflict)
            };
        }
        let current_revision = self
            .current
            .get(&target_id)
            .map(|index| self.records[*index].revision)
            .unwrap_or(0);
        if current_revision != expected_revision {
            return Err(MemoryError::RevisionConflict);
        }
        if let Some(index) = self.current.get(&target_id).copied()
            && self.records[index].body_id != body_id
        {
            return Err(MemoryError::SemanticMutation);
        }
        let revision = expected_revision
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        let memory_version = self.next_memory_version;
        let next_memory_version = memory_version
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        let bytes = memory_body_bytes(&draft.title, &draft.content);
        if self.bodies.contains_key(&body_id) && self.body_bytes(container, body_id)? != bytes {
            return Err(MemoryError::HashCollision);
        }
        let source_ref = source_ref.cloned().or_else(|| {
            self.current
                .get(&target_id)
                .and_then(|index| self.records[*index].source_ref.clone())
        });
        let temporal_inference = temporal_inference.cloned().or_else(|| {
            self.current.get(&target_id).and_then(|index| {
                self.records[*index]
                    .temporal_inference
                    .clone()
                    .filter(|value| {
                        value.body_id == body_id && value.source_time_ns == draft.source_time_ns
                    })
            })
        });
        let record = MemoryRecord {
            id: target_id,
            revision,
            body_id,
            category: draft.category.clone(),
            memory_type: draft.memory_type.clone(),
            authority_kind: draft.authority_kind.clone(),
            temporal_status: draft.temporal_status.clone(),
            scope: draft.scope.clone(),
            lifecycle_state: draft.lifecycle_state.clone(),
            archived: draft.archived,
            superseded_by: draft.superseded_by,
            parent_id: draft.parent_id,
            source_node_id: draft.source_node_id.clone(),
            content_source_conversation_id: draft.content_source_conversation_id.clone(),
            content_source_node_id: draft.content_source_node_id.clone(),
            grounding_source_conversation_id: draft.grounding_source_conversation_id.clone(),
            grounding_source_node_id: draft.grounding_source_node_id.clone(),
            source_episode_id: draft.source_episode_id,
            source_time_ns: draft.source_time_ns,
            source_ref,
            temporal_inference,
            mutation_id: draft.mutation_id.clone(),
            created_at_ns: draft.created_at_ns,
            updated_at_ns: draft.updated_at_ns,
            global_version: 0,
            memory_version,
        };
        encode_body(body_id, &bytes)?;
        crate::memory_compact_codec::validate_record_values(&record)?;
        if container.next_version_candidate().checked_add(1).is_none() {
            return Err(MemoryError::VersionExhausted);
        }
        Ok(PublishPreflight::New {
            record,
            next_memory_version,
            body_id,
        })
    }

    pub(crate) fn memory(
        &self,
        container: &Container,
        id: MemoryId,
    ) -> Result<Memory, MemoryError> {
        let index = self.current.get(&id).ok_or(MemoryError::MissingMemory)?;
        self.resolve_record(container, &self.records[*index])
    }

    pub(crate) fn memory_revision(
        &self,
        container: &Container,
        id: MemoryId,
        revision: u64,
    ) -> Result<Memory, MemoryError> {
        let record = self
            .records
            .iter()
            .find(|record| record.id == id && record.revision == revision)
            .ok_or(MemoryError::MissingMemory)?;
        self.resolve_record(container, record)
    }

    pub(crate) fn tombstone(
        &mut self,
        container: &mut Container,
        id: MemoryId,
        expected_revision: u64,
    ) -> Result<crate::owner_lifecycle::TombstoneOutcome, MemoryError> {
        if let Some(&coordinates) = self.tombstones.get(&id) {
            if expected_revision.checked_add(1) != self.revisions.get(&id).copied() {
                return Err(MemoryError::RevisionConflict);
            }
            return Ok(crate::owner_lifecycle::TombstoneOutcome::AlreadyTombstoned(
                coordinates,
            ));
        }
        let index = self
            .current
            .get(&id)
            .copied()
            .ok_or(MemoryError::MissingMemory)?;
        if self.records[index].revision != expected_revision {
            return Err(MemoryError::RevisionConflict);
        }
        self.next_memory_version
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        let tombstone = MemoryTombstone {
            id,
            revision: expected_revision
                .checked_add(1)
                .ok_or(MemoryError::VersionExhausted)?,
        };
        let memory_version = self.next_memory_version;
        let encoded = publication::encode(&MemoryPublication::Tombstone {
            id,
            revision: tombstone.revision,
            memory_version,
        })?;
        let (global_version, _) = container.append_publication_envelope(&encoded)?;
        self.insert_tombstone_rebuilt(tombstone, global_version, memory_version)?;
        Ok(crate::owner_lifecycle::TombstoneOutcome::Published(
            crate::owner_lifecycle::TombstoneCoordinates {
                global_version: Some(global_version),
                domain_version: Some(memory_version),
            },
        ))
    }

    pub(crate) fn insert_tombstone_rebuilt(
        &mut self,
        tombstone: MemoryTombstone,
        global_version: u64,
        memory_version: u64,
    ) -> Result<(), MemoryError> {
        if global_version == 0 || memory_version != self.next_memory_version {
            return Err(MemoryError::InvalidVersion);
        }
        let next_memory_version = memory_version
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        let index = self
            .current
            .get(&tombstone.id)
            .copied()
            .ok_or(MemoryError::MissingMemory)?;
        if tombstone.revision
            != self.records[index]
                .revision
                .checked_add(1)
                .ok_or(MemoryError::VersionExhausted)?
        {
            return Err(MemoryError::RevisionConflict);
        }
        self.current.remove(&tombstone.id);
        self.revisions.insert(tombstone.id, tombstone.revision);
        self.tombstones.insert(
            tombstone.id,
            crate::owner_lifecycle::TombstoneCoordinates {
                global_version: Some(global_version),
                domain_version: Some(memory_version),
            },
        );
        self.next_memory_version = next_memory_version;
        Ok(())
    }

    pub(crate) fn lifecycle_state(
        &self,
        id: MemoryId,
    ) -> Result<crate::owner_lifecycle::LifecycleState, MemoryError> {
        if let Some(&coordinates) = self.tombstones.get(&id) {
            return Ok(crate::owner_lifecycle::LifecycleState::Tombstoned(
                coordinates,
            ));
        }
        self.current
            .contains_key(&id)
            .then_some(crate::owner_lifecycle::LifecycleState::Active)
            .ok_or(MemoryError::MissingMemory)
    }

    pub(crate) fn retention_descriptor(
        &self,
        id: MemoryId,
    ) -> Result<crate::memory_model::MemoryRetentionDescriptor, MemoryError> {
        Ok(self.retention_proof(id)?.descriptor)
    }

    pub(crate) fn retention_proof(
        &self,
        id: MemoryId,
    ) -> Result<crate::memory_model::MemoryRetentionProof, MemoryError> {
        let coordinates = self.tombstones.get(&id).copied().ok_or_else(|| {
            if self.contains_historical_memory(id) {
                MemoryError::PayloadRequiredByLiveTarget
            } else {
                MemoryError::MissingMemory
            }
        })?;
        let mut structure = Sha256::new();
        structure.update(b"Memory retained topology v1");
        for record in self.records.iter().filter(|record| record.id == id) {
            let raw = crate::memory_codec::encode_record(record)?;
            structure.update((raw.len() as u64).to_le_bytes());
            structure.update(raw);
            structure.update(record.global_version.to_le_bytes());
            structure.update(record.memory_version.to_le_bytes());
        }
        let mut tombstone = Sha256::new();
        tombstone.update(id.0);
        tombstone.update(self.revisions[&id].to_le_bytes());
        tombstone.update(coordinates.global_version.unwrap().to_le_bytes());
        tombstone.update(coordinates.domain_version.unwrap().to_le_bytes());
        let revisions = self
            .records
            .iter()
            .filter(|record| record.id == id)
            .map(|record| crate::memory_model::MemoryRetainedRevision {
                revision: record.revision,
                global_version: record.global_version,
                memory_version: record.memory_version,
                body_id: record.body_id,
                parent_id: record.parent_id,
                superseded_by: record.superseded_by,
                source_episode_id: record.source_episode_id,
                source_node_id: record.source_node_id.clone(),
                content_source_conversation_id: record.content_source_conversation_id.clone(),
                content_source_node_id: record.content_source_node_id.clone(),
                grounding_source_conversation_id: record.grounding_source_conversation_id.clone(),
                grounding_source_node_id: record.grounding_source_node_id.clone(),
                source_ref: record.source_ref.clone(),
                source_time_ns: record.source_time_ns,
                created_at_ns: record.created_at_ns,
                updated_at_ns: record.updated_at_ns,
                mutation_id: record.mutation_id.clone(),
            })
            .collect();
        let live_bodies: HashSet<_> = self
            .current
            .values()
            .map(|index| self.records[*index].body_id)
            .collect();
        let mut prunable_body_ids: Vec<_> = self
            .records
            .iter()
            .filter(|record| record.id == id && !live_bodies.contains(&record.body_id))
            .map(|record| record.body_id)
            .collect();
        prunable_body_ids.sort_by_key(|body_id| body_id.0);
        prunable_body_ids.dedup();
        Ok(crate::memory_model::MemoryRetentionProof {
            descriptor: crate::memory_model::MemoryRetentionDescriptor {
                id,
                prunable_body_ids,
                tombstone_revision: self.revisions[&id],
                tombstone_global_version: coordinates.global_version.unwrap(),
                tombstone_memory_version: coordinates.domain_version.unwrap(),
                revisions,
            },
            tombstone_digest: tombstone.finalize().into(),
            retained_structure_digest: structure.finalize().into(),
        })
    }

    pub(crate) fn prune_body_chunk(&self, id: crate::MemoryBodyId) -> Option<ObjectRef> {
        self.bodies.get(&id).map(BodyLocation::frame)
    }

    pub(crate) fn records(&self) -> &[MemoryRecord] {
        &self.records
    }

    /// Highest Memory-local version visible at the accepted global cut,
    /// including an eligible tombstone coordinate.
    pub(crate) fn local_version_at_global_cut(&self, global: u64) -> u64 {
        self.records
            .iter()
            .filter(|record| record.global_version <= global)
            .map(|record| record.memory_version)
            .chain(self.tombstoned_ids().into_iter().filter_map(|id| {
                match self.lifecycle_state(id).ok()? {
                    crate::owner_lifecycle::LifecycleState::Tombstoned(coordinates)
                        if coordinates.global_version? <= global =>
                    {
                        coordinates.domain_version
                    }
                    _ => None,
                }
            }))
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn pruned_body_markers(
        &self,
    ) -> impl Iterator<Item = &history_prune::PrunedMemoryBody> {
        self.prune.markers()
    }

    pub(crate) fn apply_external_bindings(
        &mut self,
        window: [u8; 32],
        bindings: &[ExternalMemoryBinding],
    ) -> Result<(), MemoryError> {
        if self.external_windows.contains_key(&window) {
            return Err(MemoryError::MutationConflict);
        }
        for binding in bindings {
            if binding.evidence.processing_window != window {
                return Err(MemoryError::CorruptRecord("external publication"));
            }
        }
        for binding in bindings {
            self.external_evidence.insert(
                (MemoryId(binding.memory), MemoryBodyId(binding.body)),
                binding.evidence.clone(),
            );
        }
        self.external_windows.insert(
            window,
            bindings
                .iter()
                .map(|binding| MemoryId(binding.memory))
                .collect(),
        );
        Ok(())
    }

    pub(crate) fn external_window(
        &self,
        container: &mut Container,
        window: [u8; 32],
    ) -> Result<Option<Vec<Memory>>, MemoryError> {
        self.external_windows
            .get(&window)
            .map(|ids| ids.iter().map(|id| self.memory(container, *id)).collect())
            .transpose()
    }

    /// Resolve an already validated owner publication independently of its
    /// physical legacy/creation/delta carrier. Local versions are ordered even
    /// when terminal records leave gaps in the retained revision array.
    pub(crate) fn record_at_publication(
        &self,
        global_version: u64,
        memory_version: u64,
    ) -> Option<&MemoryRecord> {
        let index = self
            .records
            .binary_search_by_key(&memory_version, |record| record.memory_version)
            .ok()?;
        let record = &self.records[index];
        (record.global_version == global_version).then_some(record)
    }

    /// Produce canonical comparison evidence for a validated publication when
    /// its local version is no longer represented by a retained live record.
    /// The Memory owner owns decoding publication envelopes and terminal facts.
    pub(crate) fn reconciliation_publication_evidence(
        &self,
        global_version: u64,
        memory_version: u64,
        payload: &[u8],
    ) -> Result<Option<Vec<u8>>, MemoryError> {
        if let Some(record) = self.record_at_publication(global_version, memory_version) {
            let mut record = record.clone();
            record.global_version = 0;
            record.memory_version = 0;
            return Ok(Some(crate::memory_codec::encode_record(&record)?));
        }

        let Some(envelope) = crate::container::publication_envelope::decode(payload)? else {
            return Ok(None);
        };
        let Some(MemoryPublication::Tombstone { id, revision, .. }) =
            publication::decode(envelope.body, global_version)?
        else {
            return Ok(None);
        };
        Ok(Some(crate::memory_codec::encode_tombstone(
            crate::memory_codec::MemoryTombstone { id, revision },
        )))
    }

    /// Return accepted first-publication identities at a Memory-local cut.
    /// The result is ordered by each identity's first local version and keeps
    /// that revision's exact global coordinate, independently of current
    /// liveness, later revisions, and terminal tombstones.
    pub(crate) fn first_publication_prefix(
        &self,
        cut: u64,
    ) -> Result<Vec<(MemoryId, u64)>, MemoryError> {
        if cut > self.memory_version() {
            return Err(MemoryError::InvalidVersion);
        }
        let mut prefix = self
            .records
            .iter()
            .filter(|record| record.revision == 1 && record.memory_version <= cut)
            .map(|record| (record.memory_version, record.id, record.global_version))
            .collect::<Vec<_>>();
        prefix.sort_by_key(|(local_version, _, _)| *local_version);
        Ok(prefix
            .into_iter()
            .map(|(_, id, global_version)| (id, global_version))
            .collect())
    }

    pub(crate) fn current_record_for_replay(
        &self,
        id: MemoryId,
    ) -> Result<&MemoryRecord, MemoryError> {
        if self.tombstones.contains_key(&id) {
            return Err(MemoryError::TerminalTombstone);
        }
        self.current
            .get(&id)
            .map(|index| &self.records[*index])
            .ok_or(MemoryError::MissingMemory)
    }

    pub(crate) fn validate_compact_record(record: &MemoryRecord) -> Result<(), MemoryError> {
        if let Some(inference) = &record.temporal_inference {
            validate_temporal_inference_binding(record.body_id, record.source_time_ns, inference)?;
        }
        Ok(())
    }

    pub(crate) fn tombstone_global_versions(&self) -> impl Iterator<Item = u64> + '_ {
        self.tombstones
            .values()
            .filter_map(|coordinates| coordinates.global_version)
    }

    pub(crate) fn memory_version(&self) -> u64 {
        self.next_memory_version.saturating_sub(1)
    }

    pub(crate) fn contains_memory(&self, id: MemoryId) -> bool {
        self.current.contains_key(&id)
    }

    pub(crate) fn current_mutation_id(&self, id: MemoryId) -> Option<&str> {
        self.current
            .get(&id)
            .map(|index| self.records[*index].mutation_id.as_str())
    }

    pub(crate) fn first_mutation_id(&self, id: MemoryId) -> Option<&str> {
        self.records
            .iter()
            .find(|record| record.id == id && record.revision == 1)
            .map(|record| record.mutation_id.as_str())
    }

    pub(crate) fn first_global_version(&self, id: MemoryId) -> Option<u64> {
        self.records
            .iter()
            .find(|record| record.id == id && record.revision == 1)
            .map(|record| record.global_version)
    }

    pub(crate) fn contains_body(&self, id: MemoryBodyId) -> bool {
        self.bodies.contains_key(&id)
    }

    pub(crate) fn current_ids(&self) -> Vec<MemoryId> {
        let mut ids: Vec<_> = self.current.keys().copied().collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub(crate) fn tombstoned_ids(&self) -> Vec<MemoryId> {
        let mut ids: Vec<_> = self.tombstones.keys().copied().collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub(crate) fn current_body_ids(&self) -> Vec<MemoryBodyId> {
        let mut ids: Vec<_> = self
            .current
            .values()
            .map(|index| self.records[*index].body_id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids.dedup();
        ids
    }

    pub(crate) fn current_body_id(&self, id: MemoryId) -> Result<MemoryBodyId, MemoryError> {
        let index = self.current.get(&id).ok_or(MemoryError::MissingMemory)?;
        Ok(self.records[*index].body_id)
    }

    pub(crate) fn contains_historical_memory(&self, id: MemoryId) -> bool {
        self.revisions.contains_key(&id)
    }

    pub(crate) fn entity_mention_keys_for_memory(
        &self,
        memory_id: MemoryId,
    ) -> Vec<MemoryEntityMentionKey> {
        self.routing_metadata(memory_id)
            .map(|metadata| {
                metadata
                    .entity_mentions
                    .iter()
                    .map(|mention| MemoryEntityMentionKey::new(memory_id, mention))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn routing_metadata(&self, id: MemoryId) -> Option<&MemoryRoutingMetadata> {
        self.routing_metadata
            .get(&id)
            .filter(|_| self.current.contains_key(&id))
    }

    pub(crate) fn entity_mention_keys_for_surface(
        &self,
        surface: &str,
    ) -> Vec<MemoryEntityMentionKey> {
        let normalized = normalize_entity_surface(surface);
        let mut keys: Vec<_> = self
            .entity_mentions_by_surface
            .get(&normalized)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        keys.retain(|key| self.current.contains_key(&key.memory_id));
        keys.sort_by_key(|key| {
            (
                key.memory_id.0,
                key.field.tag(),
                key.start_byte,
                key.end_byte,
            )
        });
        keys
    }

    pub(crate) fn validate_entity_mention_key(
        &self,
        key: MemoryEntityMentionKey,
    ) -> Result<(), MemoryError> {
        if !self.current.contains_key(&key.memory_id) {
            return Err(MemoryError::MissingMemory);
        }
        self.validate_historical_entity_mention_key(key)
    }

    pub(crate) fn validate_historical_entity_mention_key(
        &self,
        key: MemoryEntityMentionKey,
    ) -> Result<(), MemoryError> {
        let metadata = self
            .routing_metadata
            .get(&key.memory_id)
            .ok_or(MemoryError::InvalidField("Entity resolution mention"))?;
        if metadata.entity_mentions.iter().any(|mention| {
            mention.field == key.field
                && mention.start_byte == key.start_byte
                && mention.end_byte == key.end_byte
        }) {
            Ok(())
        } else {
            Err(MemoryError::InvalidField("Entity resolution mention"))
        }
    }

    pub(crate) fn resolution_source_inactive(&self, id: MemoryId) -> bool {
        self.current
            .get(&id)
            .map(|index| {
                let record = &self.records[*index];
                record.archived || record.superseded_by.is_some()
            })
            .unwrap_or(true)
    }

    pub(crate) fn put_routing_metadata(
        &mut self,
        container: &mut Container,
        metadata: MemoryRoutingMetadata,
    ) -> Result<bool, MemoryError> {
        self.current_body_id(metadata.memory_id)?;
        self.validate_routing_binding(&metadata)?;
        self.validate_routing_text(container, &metadata)?;
        if let Some(existing) = self.routing_metadata.get(&metadata.memory_id) {
            return if existing == &metadata {
                Ok(false)
            } else {
                Err(MemoryError::RoutingMetadataConflict)
            };
        }
        container.append(&crate::memory_routing_codec::encode(&metadata)?)?;
        self.index_routing_metadata(&metadata);
        self.routing_metadata.insert(metadata.memory_id, metadata);
        Ok(true)
    }

    pub(crate) fn insert_routing_metadata_rebuilt(
        &mut self,
        metadata: MemoryRoutingMetadata,
    ) -> Result<(), MemoryError> {
        self.validate_routing_binding(&metadata)?;
        match self.routing_metadata.get(&metadata.memory_id) {
            Some(existing) if existing == &metadata => Ok(()),
            Some(_) => Err(MemoryError::RoutingMetadataConflict),
            None => {
                self.index_routing_metadata(&metadata);
                self.routing_metadata.insert(metadata.memory_id, metadata);
                Ok(())
            }
        }
    }

    fn index_routing_metadata(&mut self, metadata: &MemoryRoutingMetadata) {
        for mention in &metadata.entity_mentions {
            let key = MemoryEntityMentionKey::new(metadata.memory_id, mention);
            self.entity_mentions_by_surface
                .entry(normalize_entity_surface(&mention.text))
                .or_default()
                .insert(key);
        }
    }

    pub(crate) fn validate_routing_metadata(
        &self,
        container: &mut Container,
    ) -> Result<(), MemoryError> {
        for metadata in self.routing_metadata.values() {
            self.validate_routing_binding(metadata)?;
            self.validate_routing_text(container, metadata)?;
        }
        Ok(())
    }

    pub(crate) fn body_embedding_text(
        &self,
        container: &Container,
        id: MemoryBodyId,
    ) -> Result<String, MemoryError> {
        let body = self.body_bytes(container, id)?;
        let (title, content) = decode_memory_body(&body)?;
        Ok(format!("{title}\n\n{content}"))
    }

    pub(crate) fn stats(&self) -> MemoryStats {
        MemoryStats {
            memories: self.current.len(),
            revisions: self.records.len(),
            bodies: self.bodies.len(),
            memory_version: self.memory_version(),
        }
    }

    pub(crate) fn insert_body(&mut self, id: MemoryBodyId, chunk: ObjectRef) -> bool {
        if self.bodies.contains_key(&id) {
            return false;
        }
        self.bodies.insert(id, BodyLocation::Frame(chunk));
        true
    }

    pub(crate) fn insert_grouped_body_span(
        &mut self,
        span: &GroupedBodySpan,
        chunk: ObjectRef,
    ) -> Result<(), MemoryError> {
        if span.range.start > span.range.end || self.bodies.contains_key(&span.id) {
            return Err(MemoryError::CorruptBody);
        }
        self.bodies.insert(
            span.id,
            BodyLocation::GroupedFrame {
                frame: chunk,
                range: span.range.clone(),
                encoding: span.encoding,
            },
        );
        Ok(())
    }

    pub(crate) fn insert_rebuilt(&mut self, record: MemoryRecord) -> Result<(), MemoryError> {
        if record.memory_version != self.next_memory_version {
            return Err(MemoryError::InvalidVersion);
        }
        let next_memory_version = self
            .next_memory_version
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        self.insert(record)?;
        self.next_memory_version = next_memory_version;
        Ok(())
    }

    pub(crate) fn validate_bodies(&self) -> Result<(), MemoryError> {
        for record in &self.records {
            if !self.bodies.contains_key(&record.body_id)
                && self.prune.get(record.body_id).is_none()
            {
                return Err(MemoryError::MissingBody);
            }
            if let Some(inference) = &record.temporal_inference {
                validate_temporal_inference_binding(
                    record.body_id,
                    record.source_time_ns,
                    inference,
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn validate_provenance_fields(&self) -> Result<(), MemoryError> {
        for record in &self.records {
            if !paired_values(record.source_episode_id, record.source_node_id.as_deref())
                || !paired_values(
                    record.content_source_conversation_id.as_deref(),
                    record.content_source_node_id.as_deref(),
                )
                || !paired_values(
                    record.grounding_source_conversation_id.as_deref(),
                    record.grounding_source_node_id.as_deref(),
                )
            {
                return Err(MemoryError::InvalidProvenance);
            }
        }
        Ok(())
    }

    fn insert(&mut self, record: MemoryRecord) -> Result<(), MemoryError> {
        if self.tombstones.contains_key(&record.id) {
            return Err(MemoryError::TerminalTombstone);
        }
        if record.revision == 0 || record.memory_version == 0 || record.global_version == 0 {
            return Err(MemoryError::InvalidVersion);
        }
        if let Some(index) = self.by_mutation.get(&record.mutation_id).copied() {
            return if self.records[index] == record {
                Ok(())
            } else {
                Err(MemoryError::MutationConflict)
            };
        }
        let expected = self
            .current
            .get(&record.id)
            .map(|index| self.records[*index].revision + 1)
            .unwrap_or(1);
        if record.revision != expected {
            return Err(MemoryError::RevisionConflict);
        }
        if let Some(index) = self.current.get(&record.id).copied() {
            if self.records[index].body_id != record.body_id {
                return Err(MemoryError::SemanticMutation);
            }
        }
        let index = self.records.len();
        self.by_mutation.insert(record.mutation_id.clone(), index);
        self.current.insert(record.id, index);
        self.revisions.insert(record.id, record.revision);
        self.records.push(record);
        Ok(())
    }

    fn body_bytes(&self, container: &Container, id: MemoryBodyId) -> Result<Vec<u8>, MemoryError> {
        if self.prune.get(id).is_some() {
            return Err(MemoryError::PayloadUnavailable(
                crate::owner_lifecycle::PayloadUnavailable::Pruned,
            ));
        }
        let location = self.bodies.get(&id).ok_or(MemoryError::MissingBody)?;
        let payload = container.read(location.frame())?;
        let body = match location {
            BodyLocation::Frame(_) => {
                let (stored_id, body) = decode_body(&payload)?.ok_or(MemoryError::CorruptBody)?;
                if stored_id != id {
                    return Err(MemoryError::CorruptBody);
                }
                body
            }
            BodyLocation::GroupedFrame {
                range, encoding, ..
            } => {
                let span = payload.get(range.clone()).ok_or(MemoryError::CorruptBody)?;
                match encoding {
                    GroupedBodyEncoding::Raw => span.to_vec(),
                    GroupedBodyEncoding::JsonByteArray => {
                        serde_json::from_slice(span).map_err(|_| MemoryError::CorruptBody)?
                    }
                }
            }
        };
        let (title, content) = decode_memory_body(&body)?;
        if memory_body_id(&title, &content) != id {
            return Err(MemoryError::CorruptBody);
        }
        Ok(body)
    }

    pub(crate) fn retained_body_bytes(
        &self,
        container: &Container,
        id: MemoryBodyId,
    ) -> Result<Vec<u8>, MemoryError> {
        self.body_bytes(container, id)
    }

    pub(crate) fn validate_body_bytes(id: MemoryBodyId, bytes: &[u8]) -> Result<(), MemoryError> {
        let (title, content) = decode_memory_body(bytes)?;
        if memory_body_id(&title, &content) != id {
            return Err(MemoryError::CorruptBody);
        }
        Ok(())
    }

    fn validate_routing_binding(
        &self,
        metadata: &MemoryRoutingMetadata,
    ) -> Result<(), MemoryError> {
        if metadata.entity_mentions.len() > MAX_MEMORY_ENTITY_MENTIONS {
            return Err(MemoryError::InvalidField("Memory routing metadata count"));
        }
        if !self
            .records
            .iter()
            .any(|record| record.id == metadata.memory_id && record.body_id == metadata.body_id)
        {
            return Err(MemoryError::InvalidField("Memory routing metadata binding"));
        }
        for mention in &metadata.entity_mentions {
            if mention.text.is_empty()
                || mention.text.len() > MAX_MEMORY_ROUTING_TEXT_BYTES
                || mention.start_byte >= mention.end_byte
            {
                return Err(MemoryError::InvalidField("Memory entity mention"));
            }
        }
        Ok(())
    }

    fn validate_routing_text(
        &self,
        container: &mut Container,
        metadata: &MemoryRoutingMetadata,
    ) -> Result<(), MemoryError> {
        let body = self.body_bytes(container, metadata.body_id)?;
        let (title, content) = decode_memory_body(&body)?;
        for mention in &metadata.entity_mentions {
            let source = match mention.field {
                MemoryTextField::Title => title.as_str(),
                MemoryTextField::Content => content.as_str(),
            };
            let start = usize::try_from(mention.start_byte)
                .map_err(|_| MemoryError::InvalidField("Memory entity mention span"))?;
            let end = usize::try_from(mention.end_byte)
                .map_err(|_| MemoryError::InvalidField("Memory entity mention span"))?;
            if !source.is_char_boundary(start)
                || !source.is_char_boundary(end)
                || source.get(start..end) != Some(mention.text.as_str())
            {
                return Err(MemoryError::InvalidField("Memory entity mention span"));
            }
        }
        Ok(())
    }

    fn put_body(
        &mut self,
        container: &mut Container,
        title: &str,
        content: &str,
    ) -> Result<MemoryBodyId, MemoryError> {
        let id = memory_body_id(title, content);
        let bytes = memory_body_bytes(title, content);
        if self.bodies.contains_key(&id) {
            let stored = self.body_bytes(container, id)?;
            return if stored == bytes {
                Ok(id)
            } else {
                Err(MemoryError::HashCollision)
            };
        }
        let chunk = container.append(&encode_body(id, &bytes)?)?;
        self.bodies.insert(id, BodyLocation::Frame(chunk));
        Ok(id)
    }

    fn resolve_record(
        &self,
        container: &Container,
        record: &MemoryRecord,
    ) -> Result<Memory, MemoryError> {
        let body = self.body_bytes(container, record.body_id)?;
        let (title, content) = decode_memory_body(&body)?;
        Ok(Memory {
            id: record.id,
            revision: record.revision,
            category: record.category.clone(),
            memory_type: record.memory_type.clone(),
            authority_kind: record.authority_kind.clone(),
            temporal_status: record.temporal_status.clone(),
            title,
            content,
            scope: record.scope.clone(),
            lifecycle_state: record.lifecycle_state.clone(),
            archived: record.archived,
            superseded_by: record.superseded_by,
            parent_id: record.parent_id,
            source_node_id: record.source_node_id.clone(),
            content_source_conversation_id: record.content_source_conversation_id.clone(),
            content_source_node_id: record.content_source_node_id.clone(),
            grounding_source_conversation_id: record.grounding_source_conversation_id.clone(),
            grounding_source_node_id: record.grounding_source_node_id.clone(),
            source_episode_id: record.source_episode_id,
            source_time_ns: record.source_time_ns,
            source_ref: record.source_ref.clone(),
            temporal_inference: record.temporal_inference.clone(),
            routing_metadata: self.routing_metadata.get(&record.id).cloned(),
            mutation_id: record.mutation_id.clone(),
            created_at_ns: record.created_at_ns,
            updated_at_ns: record.updated_at_ns,
            global_version: record.global_version,
            memory_version: record.memory_version,
        })
    }
}

fn paired_values<L, R>(left: Option<L>, right: Option<R>) -> bool {
    matches!((left, right), (Some(_), Some(_)) | (None, None))
}

fn normalize_entity_surface(value: &str) -> String {
    value.trim().to_lowercase()
}

fn decode_memory_body(bytes: &[u8]) -> Result<(String, String), MemoryError> {
    if bytes.len() < 16 {
        return Err(MemoryError::CorruptBody);
    }
    let title_len = u64::from_le_bytes(bytes[0..8].try_into().unwrap()) as usize;
    let title_end = 8usize
        .checked_add(title_len)
        .ok_or(MemoryError::FieldTooLarge)?;
    let content_len_end = title_end.checked_add(8).ok_or(MemoryError::FieldTooLarge)?;
    let title = bytes.get(8..title_end).ok_or(MemoryError::CorruptBody)?;
    let content_len_raw = bytes
        .get(title_end..content_len_end)
        .ok_or(MemoryError::CorruptBody)?;
    let content_len = u64::from_le_bytes(content_len_raw.try_into().unwrap()) as usize;
    let content_end = content_len_end
        .checked_add(content_len)
        .ok_or(MemoryError::FieldTooLarge)?;
    let content = bytes
        .get(content_len_end..content_end)
        .ok_or(MemoryError::CorruptBody)?;
    if content_end != bytes.len() {
        return Err(MemoryError::CorruptBody);
    }
    Ok((
        String::from_utf8(title.to_vec()).map_err(|_| MemoryError::InvalidUtf8)?,
        String::from_utf8(content.to_vec()).map_err(|_| MemoryError::InvalidUtf8)?,
    ))
}

fn validate_draft(draft: &MemoryDraft) -> Result<(), MemoryError> {
    for (value, field) in [
        (&draft.category, "category"),
        (&draft.memory_type, "memory type"),
        (&draft.authority_kind, "authority kind"),
        (&draft.temporal_status, "temporal status"),
        (&draft.title, "title"),
        (&draft.content, "content"),
        (&draft.scope, "scope"),
        (&draft.lifecycle_state, "lifecycle state"),
        (&draft.mutation_id, "mutation id"),
    ] {
        if value.trim().is_empty() {
            return Err(MemoryError::InvalidField(field));
        }
    }
    if !matches!(
        draft.authority_kind.as_str(),
        "direct" | "correction" | "adoption" | "retention" | "unknown"
    ) {
        return Err(MemoryError::InvalidField("authority kind"));
    }
    if !matches!(
        draft.temporal_status.as_str(),
        "current" | "future" | "historical" | "unknown"
    ) {
        return Err(MemoryError::InvalidField("temporal status"));
    }
    if draft.updated_at_ns < draft.created_at_ns {
        return Err(MemoryError::InvalidField("updated_at_ns"));
    }
    Ok(())
}

fn validate_temporal_inference_binding(
    body_id: MemoryBodyId,
    source_time_ns: Option<i64>,
    inference: &MemoryTemporalInference,
) -> Result<(), MemoryError> {
    if inference.body_id != body_id
        || inference.source_time_ns != source_time_ns
        || inference.inference.resolutions.is_empty()
    {
        return Err(MemoryError::InvalidField("temporal inference binding"));
    }
    Ok(())
}

fn validate_source_ref(source_ref: Option<&MemorySourceRef>) -> Result<(), MemoryError> {
    let Some(source_ref) = source_ref else {
        return Ok(());
    };
    if source_ref.owner_id.trim().is_empty() || source_ref.source_node_id.trim().is_empty() {
        return Err(MemoryError::InvalidProvenance);
    }
    for (conversation_id, node_id) in [
        (
            source_ref.content_source_conversation_id.as_deref(),
            source_ref.content_source_node_id.as_deref(),
        ),
        (
            source_ref.grounding_source_conversation_id.as_deref(),
            source_ref.grounding_source_node_id.as_deref(),
        ),
    ] {
        match (conversation_id, node_id) {
            (None, None) => {}
            (Some(conversation_id), Some(node_id))
                if !conversation_id.trim().is_empty() && !node_id.trim().is_empty() => {}
            _ => return Err(MemoryError::InvalidProvenance),
        }
    }
    Ok(())
}

fn same_draft(memory: &Memory, draft: &MemoryDraft) -> bool {
    memory.category == draft.category
        && memory.memory_type == draft.memory_type
        && memory.authority_kind == draft.authority_kind
        && memory.temporal_status == draft.temporal_status
        && memory.title == draft.title
        && memory.content == draft.content
        && memory.scope == draft.scope
        && memory.lifecycle_state == draft.lifecycle_state
        && memory.archived == draft.archived
        && memory.superseded_by == draft.superseded_by
        && memory.parent_id == draft.parent_id
        && memory.source_node_id == draft.source_node_id
        && memory.content_source_conversation_id == draft.content_source_conversation_id
        && memory.content_source_node_id == draft.content_source_node_id
        && memory.grounding_source_conversation_id == draft.grounding_source_conversation_id
        && memory.grounding_source_node_id == draft.grounding_source_node_id
        && memory.source_episode_id == draft.source_episode_id
        && memory.source_time_ns == draft.source_time_ns
        && memory.mutation_id == draft.mutation_id
        && memory.created_at_ns == draft.created_at_ns
        && memory.updated_at_ns == draft.updated_at_ns
}

#[cfg(test)]
#[path = "memory_publication_writer_tests.rs"]
mod publication_writer_tests;
