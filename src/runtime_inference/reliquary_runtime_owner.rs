//! Canonical mounted REL runtime aggregate.
//!
//! Persistent and semantic store ownership remains in `ReliquaryOwner`. This
//! aggregate keeps rebuildable runtime indexes and projections beside that
//! owner so lower lifecycle storage does not depend on Retrieval or Dream.

use crate::MemoryId;
use crate::dream_duplicate_index::DuplicateIndex;
use crate::lexical_index::LexicalIndex;
use crate::owner_lifecycle::ReliquaryOwner;
use std::ops::{Deref, DerefMut};

pub struct RelExecutionOwner {
    pub(crate) owner: ReliquaryOwner,
    pub(crate) duplicate_index: DuplicateIndex,
    pub(crate) lexical_index: LexicalIndex,
    pub(crate) freshness_due: crate::FreshnessDueDispatcher,
    pub(crate) freshness_notifications:
        std::collections::BTreeMap<MemoryId, crate::freshness_due::FreshnessNotificationState>,
}

impl RelExecutionOwner {
    pub(crate) fn open_mounted(path: &std::path::Path) -> Result<Self, crate::CvaError> {
        let mut replay = crate::owner_lifecycle::open_reliquary_replay(path)?;
        let reconstruction = crate::history::reconstruct_reliquary(&mut replay)?;
        let (owner, due) = replay.into_owner(reconstruction);
        Ok(Self::from_reconstructed(owner, due))
    }

    pub(crate) fn runtime_memory(
        &mut self,
        id: MemoryId,
    ) -> Result<crate::Memory, crate::MemoryError> {
        self.owner.memories.memory(&self.owner.container, id)
    }

    pub(crate) fn runtime_memory_ids(&self) -> Vec<MemoryId> {
        self.owner.memories.current_ids()
    }

    pub(crate) fn runtime_rel_turn_count(&self) -> u64 {
        self.owner.archive.rel_turn_count()
    }

    pub(crate) fn runtime_graph_relations(&self) -> Vec<crate::GraphRelation> {
        self.owner.graph.active_relations()
    }

    pub(crate) fn runtime_semantic_graph_relations(&self) -> Vec<crate::SemanticGraphRelation> {
        self.owner
            .graph
            .active_semantic_relations()
            .into_iter()
            .filter(|relation| {
                relation.kind != crate::SemanticGraphRelationKind::PrincipalAssociation
            })
            .collect()
    }

    pub(crate) fn runtime_file_bytes(
        &mut self,
        id: crate::FileId,
    ) -> Result<Vec<u8>, crate::ArchiveError> {
        self.owner.archive.file_bytes(&mut self.owner.container, id)
    }

    pub(crate) fn runtime_project_file_ref(
        &self,
        id: crate::FileId,
    ) -> Option<crate::ProjectFileRef> {
        self.owner.project_files.get(id).cloned()
    }

    pub(crate) fn runtime_project_revision_correlations(
        &self,
    ) -> Vec<crate::ProjectRevisionCorrelation> {
        self.owner.project_history.records()
    }

    pub(crate) fn runtime_latest_project_revision_correlation(
        &self,
    ) -> Option<crate::ProjectRevisionCorrelation> {
        self.owner.project_history.latest()
    }

    pub(crate) fn runtime_entity_stats(&self) -> crate::EntityStats {
        self.owner.entities.stats()
    }

    pub(crate) fn runtime_entities(&self) -> Vec<crate::Entity> {
        self.owner.entities.current()
    }

    pub(crate) fn runtime_memory_routing_metadata(
        &self,
        id: MemoryId,
    ) -> Option<&crate::MemoryRoutingMetadata> {
        self.owner.memories.routing_metadata(id)
    }

    pub(crate) fn runtime_graph_version(&self) -> u64 {
        self.owner.graph.graph_version()
    }

    pub(crate) fn runtime_owner_id(&self) -> Option<String> {
        self.owner.container.owner_id()
    }

    pub(crate) fn runtime_memory_revision(
        &self,
        id: MemoryId,
        revision: u64,
    ) -> Result<crate::Memory, crate::MemoryError> {
        self.owner
            .memories
            .memory_revision(&self.owner.container, id, revision)
    }

    pub(crate) fn runtime_memory_body_id(
        &self,
        id: MemoryId,
    ) -> Result<crate::MemoryBodyId, crate::MemoryError> {
        self.owner.memories.current_body_id(id)
    }

    pub(crate) fn runtime_memory_vector_location(
        &self,
        profile: crate::CompatibilityProfileId,
        body: crate::MemoryBodyId,
    ) -> Option<crate::MemoryVectorLocation> {
        self.owner.memory_vectors.location(profile, body)
    }

    pub(crate) fn runtime_insomnia_attempts(
        &self,
        episode_id: crate::EpisodeId,
    ) -> Vec<crate::InsomniaAttempt> {
        self.owner.insomnia.attempts(episode_id)
    }

    pub(crate) fn runtime_external_memory_evidence(
        &self,
        id: MemoryId,
    ) -> Result<
        Option<crate::external_source::ExternalMemoryEvidence>,
        crate::external_source::ExternalSourceError,
    > {
        let body = self
            .owner
            .memories
            .current_body_id(id)
            .map_err(|_| crate::external_source::ExternalSourceError::Unavailable)?;
        Ok(self
            .owner
            .memories
            .external_evidence
            .get(&(id, body))
            .cloned())
    }

    pub(crate) fn runtime_rel_metadata(&self) -> crate::RelMetadata {
        self.owner.rel_metadata.effective(&self.owner.container)
    }

    pub(crate) fn runtime_echo_events(
        &self,
        conversation_id: &str,
        message_id: &str,
    ) -> Vec<crate::EchoEvent> {
        self.owner.echo.for_turn(conversation_id, message_id)
    }

    pub(crate) fn runtime_conversation_echo_events(
        &self,
        conversation_id: &str,
    ) -> Vec<crate::EchoEvent> {
        self.owner.echo.for_conversation(conversation_id)
    }

    pub(crate) fn runtime_files_for_source(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Vec<crate::StoredFile> {
        self.owner
            .archive
            .files_for_source(conversation_id, node_id)
    }

    pub(crate) fn runtime_conversation_compactions(
        &self,
        conversation_id: &str,
    ) -> Vec<crate::ConversationCompaction> {
        self.owner
            .conversation_compactions
            .for_conversation(conversation_id)
    }

    pub(crate) fn runtime_conversation_policy(
        &self,
        conversation_id: &str,
    ) -> (crate::ConversationPolicy, u64) {
        self.owner.archive.conversation_policy(conversation_id)
    }

    pub(crate) fn runtime_archive_has_node(&self, conversation_id: &str, node_id: &str) -> bool {
        self.owner.archive.has_node(conversation_id, node_id)
    }

    pub(crate) fn runtime_archive_branches(&self) -> Vec<crate::Branch> {
        self.owner.archive.branches()
    }

    pub(crate) fn runtime_append_branch(
        &mut self,
        branch: crate::Branch,
    ) -> Result<(), crate::ArchiveError> {
        self.owner
            .archive
            .append_branch(&mut self.owner.container, branch)
    }

    pub(crate) fn runtime_put_echo_event(
        &mut self,
        event: crate::EchoEvent,
    ) -> Result<bool, crate::EchoError> {
        self.owner.echo.put(&mut self.owner.container, event)
    }

    pub(crate) fn runtime_community_snapshot(&self) -> Option<crate::CommunitySnapshot> {
        self.owner
            .communities
            .latest_if_live_members(&self.owner.graph)
            .cloned()
    }

    pub(crate) fn runtime_community_semantic_names(&self) -> Vec<crate::CommunitySemanticName> {
        self.owner
            .communities
            .current_semantic_names(&self.owner.graph)
    }

    pub(crate) fn runtime_open_import_epoch(&self) -> bool {
        self.owner.import_epochs.open().is_some()
    }

    pub(crate) fn runtime_open_import_start(&self) -> Option<crate::imports::ImportStart> {
        self.owner.import_epochs.open().cloned()
    }

    pub(crate) fn runtime_current_vector_generation(
        &self,
        profile_id: crate::CompatibilityProfileId,
    ) -> Option<crate::VectorGeneration> {
        if self.owner.container.owner_uuid().is_none() || self.runtime_open_import_epoch() {
            return None;
        }
        self.owner.compatibility_profiles.get_active(profile_id)?;
        let generation = self
            .owner
            .vector_generations
            .current_generation(profile_id)?;
        let fragments = self
            .owner
            .archive
            .fragments()
            .iter()
            .map(|fragment| fragment.id)
            .collect::<Vec<_>>();
        (generation.source_archive_version == self.owner.archive.archive_version()
            && self
                .owner
                .archive_vectors
                .has_exact_fragment_coverage(generation.archive_vector_id, &fragments))
        .then_some(generation)
    }

    pub(crate) fn runtime_active_memory_compatibility_profiles(
        &self,
    ) -> Vec<crate::CompatibilityProfile> {
        if self.owner.container.owner_uuid().is_none() || self.runtime_open_import_epoch() {
            return Vec::new();
        }
        let body_ids = self.owner.memories.current_body_ids();
        self.owner
            .compatibility_profiles
            .profiles()
            .into_iter()
            .filter(|profile| {
                self.owner
                    .compatibility_profiles
                    .get_active(profile.id)
                    .is_some()
                    && self
                        .owner
                        .memory_vectors
                        .has_complete_coverage(profile.id, &body_ids)
            })
            .collect()
    }

    pub(crate) fn runtime_search_archive(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::ArchiveSearchHit>, crate::SearchError> {
        use crate::archive_search::{MAX_ARCHIVE_SEARCH_QUERY_BYTES, MAX_ARCHIVE_SEARCH_RESULTS};
        let query = query.trim();
        if query.is_empty() || query.len() > MAX_ARCHIVE_SEARCH_QUERY_BYTES {
            return Err(crate::SearchError::EmptyQuery);
        }
        if !(1..=MAX_ARCHIVE_SEARCH_RESULTS).contains(&limit) {
            return Err(crate::SearchError::InvalidConfig);
        }
        let view = crate::archive::ArchiveReadView::new(&self.owner.archive, &self.owner.container);
        let hits = crate::lexical_search::raw_archive_candidates(
            &mut self.lexical_index,
            &view,
            query,
            MAX_ARCHIVE_SEARCH_RESULTS,
        )?
        .into_iter()
        .filter(|hit| {
            !self
                .owner
                .archive
                .is_conversation_tombstoned(&hit.fragment.conversation_id)
        })
        .take(limit)
        .collect::<Vec<_>>();
        hits.into_iter()
            .map(|hit| {
                let text = self
                    .owner
                    .archive
                    .fragment_text_for(&mut self.owner.container, &hit.fragment)?;
                Ok(crate::ArchiveSearchHit {
                    fragment: hit.fragment,
                    text,
                    score: hit.score,
                })
            })
            .collect()
    }

    pub(crate) fn from_owner(owner: ReliquaryOwner) -> Self {
        Self {
            owner,
            duplicate_index: DuplicateIndex::empty(),
            lexical_index: LexicalIndex::default(),
            freshness_due: crate::FreshnessDueDispatcher::default(),
            freshness_notifications: std::collections::BTreeMap::new(),
        }
    }

    pub(crate) fn from_reconstructed(
        owner: ReliquaryOwner,
        freshness_due: crate::FreshnessDueDispatcher,
    ) -> Self {
        Self {
            freshness_due,
            ..Self::from_owner(owner)
        }
    }

    pub(crate) fn into_owner(self) -> ReliquaryOwner {
        self.owner
    }

    pub(crate) fn sync_owner(&self) -> Result<(), crate::CvaError> {
        self.owner.container.sync()?;
        Ok(())
    }

    /// Accept a turn through Archive and advance its derived Freshness notices.
    pub(crate) fn accept_interaction_turn(
        &mut self,
        turn: crate::IncomingTurn,
    ) -> Result<crate::archive_activity::TurnAcceptance<crate::IngestedTurn>, crate::ArchiveError>
    {
        let accepted = self.owner.archive.ingest_turn(
            &mut self.owner.container,
            self.owner.project_files.accepted_facts(),
            turn,
        )?;
        self.freshness_on_accepted_activity(self.owner.archive.rel_turn_count());
        Ok(accepted)
    }

    pub(crate) fn freshness_on_accepted_activity(&mut self, accepted_turn: u64) {
        for _ in 0..64 {
            let Some(due) = self
                .freshness_due
                .drain_due(accepted_turn, 1)
                .into_iter()
                .next()
            else {
                break;
            };
            if self.freshness_due.advance(due).is_err() {
                let _ = self.freshness_due.defer(due);
                break;
            }
            self.buffer_freshness_crossing(due);
        }
    }

    pub(crate) fn buffer_freshness_crossing(&mut self, due: crate::freshness_due::FreshnessDue) {
        let state = self.freshness_notifications.entry(due.memory).or_default();
        if state.crossings.len() < 2 {
            state.crossings.push_back(due);
        } else if let Some(latest) = state.crossings.back_mut() {
            *latest = due;
        }
    }

    pub fn establish_compatibility_profile(
        &mut self,
        endpoint: &impl crate::EmbeddingEndpoint,
    ) -> Result<crate::CompatibilityProfile, crate::CompatibilityProfileError> {
        let candidate = crate::compatibility_profile_probe::profile_from_endpoint(endpoint)?;
        if let Some(existing) = self.compatibility_profiles.compatible_with(&candidate) {
            return Ok(existing.clone());
        }
        self.owner
            .compatibility_profiles
            .put(&mut self.owner.container, candidate.clone())?;
        Ok(candidate)
    }

    pub fn put_packed_vectors(
        &mut self,
        packed: lodestone_packed::PackedVectors,
    ) -> Result<crate::PackedVectorId, crate::PackedVectorError> {
        self.owner
            .packed_vectors
            .put(&mut self.owner.container, packed)
    }

    pub fn put_memory_vectors(
        &mut self,
        compatibility_profile_id: crate::CompatibilityProfileId,
        packed_vector_id: crate::PackedVectorId,
        memory_body_ids: Vec<crate::MemoryBodyId>,
    ) -> Result<crate::MemoryVectorId, crate::MemoryVectorError> {
        self.compatibility_profiles
            .get_active(compatibility_profile_id)
            .ok_or(crate::MemoryVectorError::MissingProfile)?;
        crate::memory_vector_store::validate_mapping_shape(
            &self.compatibility_profiles,
            &self.packed_vectors,
            compatibility_profile_id,
            packed_vector_id,
            &memory_body_ids,
        )?;
        let mut seen = std::collections::HashSet::new();
        for body in &memory_body_ids {
            if !self.memories.contains_body(*body) {
                return Err(crate::MemoryVectorError::MissingMemoryBody);
            }
            if !seen.insert(*body) {
                return Err(crate::MemoryVectorError::DuplicateMemoryBody);
            }
        }
        self.owner.memory_vectors.put(
            &mut self.owner.container,
            &self.owner.compatibility_profiles,
            &self.owner.packed_vectors,
            compatibility_profile_id,
            packed_vector_id,
            memory_body_ids,
        )
    }

    pub fn build_missing_memory_vectors(
        &mut self,
        compatibility_profile_id: crate::CompatibilityProfileId,
        endpoint: &impl crate::EmbeddingEndpoint,
    ) -> Result<crate::MemoryVectorBuildResult, crate::MemoryVectorError> {
        use crate::compatibility_profile_probe::{validate_embedding_batch, verify_endpoint};
        use lodestone_packed::{PackedVectors, ScalarType, VectorSchema};

        let profile = self
            .compatibility_profiles
            .get_active(compatibility_profile_id)
            .cloned()
            .ok_or(crate::MemoryVectorError::MissingProfile)?;
        let compatibility = verify_endpoint(&profile, endpoint)?;
        if !compatibility.compatible {
            return Err(crate::MemoryVectorError::IncompatibleEndpoint);
        }
        let body_ids = self.memories.current_body_ids();
        if body_ids.is_empty() {
            return Err(crate::MemoryVectorError::EmptyPopulation);
        }
        let missing = self
            .memory_vectors
            .missing_body_ids(compatibility_profile_id, &body_ids);
        let already_present = body_ids.len().saturating_sub(missing.len());
        if missing.is_empty() {
            return Ok(crate::MemoryVectorBuildResult {
                created_set: None,
                embedded: 0,
                already_present,
            });
        }
        let mut texts = Vec::with_capacity(missing.len());
        for body_id in &missing {
            texts.push(
                self.owner
                    .memories
                    .body_embedding_text(&mut self.owner.container, *body_id)?,
            );
        }
        let vectors = endpoint.embed(crate::EmbeddingMode::Document, &texts)?;
        validate_embedding_batch(&profile, &vectors, texts.len())?;
        let schema = VectorSchema::new(profile.dimensions, ScalarType::F32)
            .map_err(|_| crate::MemoryVectorError::InvalidEmbeddingMatrix)?;
        let row_bytes = usize::try_from(profile.dimensions)
            .ok()
            .and_then(|dimensions| dimensions.checked_mul(4))
            .ok_or(crate::MemoryVectorError::InvalidEmbeddingMatrix)?;
        let capacity = vectors
            .len()
            .checked_mul(row_bytes)
            .ok_or(crate::MemoryVectorError::InvalidEmbeddingMatrix)?;
        let mut bytes = Vec::with_capacity(capacity);
        for vector in vectors {
            for value in vector {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let packed = PackedVectors::from_bytes(schema, bytes)
            .map_err(|_| crate::MemoryVectorError::InvalidEmbeddingMatrix)?;
        let packed_vector_id = self
            .owner
            .packed_vectors
            .put(&mut self.owner.container, packed)?;
        let set_id = self.owner.memory_vectors.put(
            &mut self.owner.container,
            &self.owner.compatibility_profiles,
            &self.owner.packed_vectors,
            compatibility_profile_id,
            packed_vector_id,
            missing.clone(),
        )?;
        Ok(crate::MemoryVectorBuildResult {
            created_set: Some(set_id),
            embedded: missing.len(),
            already_present,
        })
    }

    pub(crate) fn dream_eligible_epoch(
        &mut self,
        id: MemoryId,
        now_ns: i64,
    ) -> Result<Option<u64>, crate::MemoryError> {
        crate::dream_cooldown::owner_eligible_epoch(
            crate::memory_store::MemoryReadView::new(&self.owner.memories, &self.owner.container),
            Some(&self.owner.archive),
            &self.owner.processing_epochs,
            id,
            now_ns,
        )
    }

    pub(crate) fn mark_dream_processed(
        &mut self,
        id: MemoryId,
        epoch: u64,
        processed_at_ns: i64,
    ) -> Result<bool, crate::MemoryError> {
        self.owner.processing_epochs.put_for_memory(
            &mut self.owner.container,
            &self.owner.memories,
            id,
            crate::dream_cooldown::DREAM_PROCESSING_LANE,
            crate::dream_cooldown::dream_processing_state(epoch, processed_at_ns),
        )
    }
}

impl Deref for RelExecutionOwner {
    type Target = ReliquaryOwner;

    fn deref(&self) -> &Self::Target {
        &self.owner
    }
}

impl DerefMut for RelExecutionOwner {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.owner
    }
}
