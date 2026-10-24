use crate::archive_vector_store::ArchiveVectorStore;
use crate::community_store::CommunityStore;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::conversation_compaction_store::ConversationCompactionStore;
use crate::dream_duplicate_index::DuplicateIndex;
use crate::dream_pair_history::DreamPairStore;
use crate::echo_store::EchoStore;
use crate::ego_store::EgoStore;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::insomnia::store::InsomniaStore;
use crate::interaction_stream_store::InteractionStreamStore;
use crate::lexical_index::LexicalIndex;
use crate::memory_store::MemoryStore;
use crate::memory_vector_store::MemoryVectorStore;
use crate::packed_vector_store::PackedVectorStore;
use crate::processing_epoch_store::ProcessingEpochStore;
use crate::project_file_binding_store::ProjectFileStore;
use crate::project_history_store::ProjectHistoryStore;
use crate::rel_metadata_store::RelMetadataStore;
use crate::relationship_store::RelationshipStore;
use crate::vector_generation_store::VectorGenerationStore;
use crate::{
    Archive, ArchiveError, ArchiveRecordVersion, ArchiveStats, Branch, Container,
    ConversationMetadata, CvaError, Episode, EpisodeBoundary, EpisodeBuildResult, EpisodeConfig,
    EpisodeId, EpisodeOrigin, FileId, Fragment, FragmentConfig, FragmentId, Memory, MemoryDraft,
    MemoryError, MemoryId, MemoryStats, Node, ResolvedTurn, StoredFile,
};

pub use crate::rel_execution_owner::RelExecutionOwner as Cva;

impl Cva {
    pub fn legacy_scope_kind(&self) -> Option<crate::ReliquaryScopeKind> {
        self.container
            .identity()
            .and_then(|identity| identity.scope)
    }

    pub fn rel_metadata(&self) -> crate::RelMetadata {
        self.runtime_rel_metadata()
    }

    pub fn set_rel_metadata(
        &mut self,
        type_label: Option<String>,
        mut dependencies: Vec<String>,
    ) -> Result<bool, CvaError> {
        let type_label = type_label.and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        });
        dependencies = dependencies
            .into_iter()
            .map(|value| value.trim().to_owned())
            .collect();
        dependencies.sort();
        dependencies.dedup();
        if let Some(owner_id) = self.owner_id()
            && dependencies
                .iter()
                .any(|dependency| dependency == &owner_id)
        {
            return Err(CvaError::RelMetadata(
                "a REL cannot depend on itself".into(),
            ));
        }
        let metadata = crate::RelMetadata {
            type_label,
            dependencies,
        };
        self.owner
            .rel_metadata
            .put(&mut self.owner.container, metadata)
            .map_err(CvaError::RelMetadata)
    }

    pub fn owner_id(&self) -> Option<String> {
        self.runtime_owner_id()
    }

    pub fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.owner.container.owner_uuid()
    }

    pub fn latest_global_version(&self) -> u64 {
        self.container.latest_version()
    }

    pub fn transaction_time_ns(&self, version: u64) -> Option<i64> {
        self.container.transaction_time_ns(version)
    }

    pub fn version_at_or_before(&self, transaction_time_ns: i64) -> Option<u64> {
        self.container.version_at_or_before(transaction_time_ns)
    }

    pub fn archive(&self) -> &Archive {
        &self.archive
    }

    pub fn append_node(
        &mut self,
        id: String,
        conversation_id: String,
        parent_id: Option<String>,
        role: String,
        timestamp_ns: i64,
        content: &str,
    ) -> Result<Node, ArchiveError> {
        self.append_node_with_principal(
            id,
            conversation_id,
            parent_id,
            role,
            None,
            timestamp_ns,
            content,
        )
    }

    pub fn append_node_with_principal(
        &mut self,
        id: String,
        conversation_id: String,
        parent_id: Option<String>,
        role: String,
        principal_id: Option<String>,
        timestamp_ns: i64,
        content: &str,
    ) -> Result<Node, ArchiveError> {
        let accepted = self.owner.archive.append_node(
            &mut self.owner.container,
            id,
            conversation_id,
            parent_id,
            role,
            principal_id,
            timestamp_ns,
            content,
        )?;
        self.freshness_on_accepted_activity(self.archive.rel_turn_count());
        Ok(accepted.value)
    }

    pub fn append_branch(&mut self, branch: Branch) -> Result<(), ArchiveError> {
        self.runtime_append_branch(branch)
    }

    pub fn set_conversation_title(
        &mut self,
        conversation_id: &str,
        title: String,
    ) -> Result<bool, ArchiveError> {
        let mut metadata = self
            .archive
            .conversation_metadata(conversation_id)
            .cloned()
            .unwrap_or_else(|| ConversationMetadata::new(conversation_id));
        metadata.title = Some(title);
        self.owner
            .archive
            .put_conversation_metadata(&mut self.owner.container, metadata)
    }

    pub fn set_conversation_active(
        &mut self,
        conversation_id: &str,
        active: bool,
    ) -> Result<bool, ArchiveError> {
        let mut metadata = self
            .archive
            .conversation_metadata(conversation_id)
            .cloned()
            .unwrap_or_else(|| ConversationMetadata::new(conversation_id));
        metadata.active = active;
        self.owner
            .archive
            .put_conversation_metadata(&mut self.owner.container, metadata)
    }

    /// Canonical durable policy; absent legacy metadata is sequential at revision 1.
    pub fn conversation_policy(&self, conversation_id: &str) -> (crate::ConversationPolicy, u64) {
        self.runtime_conversation_policy(conversation_id)
    }

    /// Publish a policy change for an existing conversation. Synchronize before acknowledgement.
    pub fn set_conversation_policy(
        &mut self,
        conversation_id: &str,
        policy: crate::ConversationPolicy,
    ) -> Result<bool, ArchiveError> {
        self.owner.archive.set_conversation_policy(
            &mut self.owner.container,
            conversation_id,
            policy,
        )
    }

    pub fn conversation_metadata(&self, conversation_id: &str) -> Option<&ConversationMetadata> {
        self.archive.conversation_metadata(conversation_id)
    }

    pub fn historical_conversation_metadata(
        &self,
        conversation_id: &str,
    ) -> Option<&ConversationMetadata> {
        self.archive
            .historical_conversation_metadata(conversation_id)
    }

    pub fn branch_turns(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.owner
            .archive
            .branch_turns(&mut self.owner.container, conversation_id, branch_id)
    }

    pub fn historical_branch_turns(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.owner.archive.historical_branch_turns(
            &mut self.owner.container,
            conversation_id,
            branch_id,
        )
    }

    pub fn branch_at(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
        archive_version: u64,
    ) -> Result<Option<Branch>, ArchiveError> {
        self.owner.archive.branch_at(
            &mut self.owner.container,
            conversation_id,
            branch_id,
            archive_version,
        )
    }

    pub fn materialize_branch_fragments(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
        config: FragmentConfig,
        close_tail: bool,
    ) -> Result<Vec<Fragment>, ArchiveError> {
        self.owner.archive.materialize_branch_fragments(
            &mut self.owner.container,
            conversation_id,
            branch_id,
            config,
            close_tail,
        )
    }

    pub fn materialize_path_fragments(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        config: FragmentConfig,
        close_tail: bool,
    ) -> Result<Vec<Fragment>, ArchiveError> {
        self.owner.archive.materialize_path_fragments(
            &mut self.owner.container,
            conversation_id,
            leaf_node_id,
            config,
            close_tail,
        )
    }

    pub fn fragment_turns(&mut self, id: FragmentId) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.owner
            .archive
            .fragment_turns(&mut self.owner.container, id)
    }

    pub fn fragment_text(&mut self, id: FragmentId) -> Result<String, ArchiveError> {
        self.owner
            .archive
            .fragment_text(&mut self.owner.container, id)
    }

    pub fn materialize_branch_episodes(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
        config: EpisodeConfig,
        origin: EpisodeOrigin,
        close_tail: Option<(EpisodeBoundary, i64)>,
    ) -> Result<EpisodeBuildResult, ArchiveError> {
        self.owner.archive.materialize_branch_episodes(
            &mut self.owner.container,
            conversation_id,
            branch_id,
            config,
            origin,
            close_tail,
        )
    }

    pub fn materialize_path_episodes(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        config: EpisodeConfig,
        origin: EpisodeOrigin,
        close_tail: Option<(EpisodeBoundary, i64)>,
    ) -> Result<EpisodeBuildResult, ArchiveError> {
        self.owner.archive.materialize_path_episodes(
            &mut self.owner.container,
            conversation_id,
            leaf_node_id,
            config,
            origin,
            close_tail,
        )
    }

    pub fn episodes(&self) -> Vec<Episode> {
        self.archive.episodes()
    }

    pub fn episodes_for_conversation(&self, conversation_id: &str) -> Vec<Episode> {
        self.archive.episodes_for_conversation(conversation_id)
    }

    pub fn historical_episodes(&self) -> Vec<Episode> {
        self.archive.historical_episodes()
    }

    pub fn historical_episodes_for_conversation(&self, conversation_id: &str) -> Vec<Episode> {
        self.archive
            .historical_episodes_for_conversation(conversation_id)
    }

    pub fn episode(&self, id: EpisodeId) -> Option<&Episode> {
        self.archive.episode(id)
    }

    pub fn historical_episode(&self, id: EpisodeId) -> Option<&Episode> {
        self.archive.historical_episode(id)
    }

    pub fn episode_turns(&mut self, id: EpisodeId) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.owner
            .archive
            .episode_turns(&mut self.owner.container, id)
    }

    pub fn publish_memory(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
    ) -> Result<(Memory, bool), MemoryError> {
        self.runtime_publish_knowledge_memory(id, expected_revision, draft)
    }

    /// Read Memory content without access reinforcement or REL activity advancement.
    /// Internal processing and candidate delivery use this same passive primitive.
    /// Only a trusted host's confirmed external consumption may separately call
    /// `record_accepted_memory_use`; fetching content is never that confirmation.
    pub fn memory(&mut self, id: MemoryId) -> Result<Memory, MemoryError> {
        self.runtime_memory(id)
    }

    pub fn tombstone_memory(
        &mut self,
        id: MemoryId,
        expected_revision: u64,
    ) -> Result<crate::owner_lifecycle::TombstoneOutcome, MemoryError> {
        let result =
            self.owner
                .memories
                .tombstone(&mut self.owner.container, id, expected_revision)?;
        self.owner
            .graph
            .exclude_from_current(crate::SemanticNodeRef::memory(id))
            .map_err(|_| MemoryError::InvalidField("Memory graph projection"))?;
        Ok(result)
    }

    pub fn memory_retention_descriptor(
        &self,
        id: MemoryId,
    ) -> Result<crate::MemoryRetentionDescriptor, MemoryError> {
        self.memories.retention_descriptor(id)
    }

    pub fn historical_memory_revision(
        &mut self,
        id: MemoryId,
        revision: u64,
    ) -> Result<
        crate::owner_lifecycle::HistoricalPayload<Memory, crate::MemoryRetainedRevision>,
        MemoryError,
    > {
        let value = match self.memory_revision(id, revision) {
            Ok(value) => value,
            Err(MemoryError::PayloadUnavailable(reason)) => {
                let structure = self
                    .memory_retention_descriptor(id)?
                    .revisions
                    .into_iter()
                    .find(|r| r.revision == revision)
                    .ok_or(MemoryError::MissingMemory)?;
                return Ok(crate::owner_lifecycle::HistoricalPayload::Pruned {
                    structural_history: structure,
                    reason,
                });
            }
            Err(error) => return Err(error),
        };
        Ok(match self.memory_lifecycle_state(id)? {
            crate::owner_lifecycle::LifecycleState::Active => {
                crate::owner_lifecycle::HistoricalPayload::Available(value)
            }
            crate::owner_lifecycle::LifecycleState::Tombstoned(_) => {
                crate::owner_lifecycle::HistoricalPayload::Tombstoned(value)
            }
        })
    }

    pub fn memory_lifecycle_state(
        &self,
        id: MemoryId,
    ) -> Result<crate::owner_lifecycle::LifecycleState, MemoryError> {
        self.memories.lifecycle_state(id)
    }

    pub fn set_memory_temporal_status(
        &mut self,
        id: MemoryId,
        expected_revision: u64,
        temporal_status: &str,
        mutation_id: String,
    ) -> Result<(Memory, bool), MemoryError> {
        if !matches!(temporal_status, "current" | "future" | "historical") {
            return Err(MemoryError::InvalidField("temporal status"));
        }
        let current = self.memory(id)?;
        if current.revision != expected_revision {
            return Err(MemoryError::RevisionConflict);
        }
        if current.temporal_status == temporal_status {
            return Ok((current, false));
        }
        let draft = MemoryDraft {
            category: current.category.clone(),
            memory_type: current.memory_type.clone(),
            authority_kind: current.authority_kind.clone(),
            temporal_status: temporal_status.to_owned(),
            title: current.title.clone(),
            content: current.content.clone(),
            scope: current.scope.clone(),
            lifecycle_state: current.lifecycle_state.clone(),
            archived: current.archived,
            superseded_by: current.superseded_by,
            parent_id: current.parent_id,
            source_node_id: current.source_node_id.clone(),
            content_source_conversation_id: current.content_source_conversation_id.clone(),
            content_source_node_id: current.content_source_node_id.clone(),
            grounding_source_conversation_id: current.grounding_source_conversation_id.clone(),
            grounding_source_node_id: current.grounding_source_node_id.clone(),
            source_episode_id: current.source_episode_id,
            source_time_ns: current.source_time_ns,
            mutation_id,
            created_at_ns: current.created_at_ns,
            updated_at_ns: current.updated_at_ns,
        };
        self.publish_memory(Some(id), expected_revision, draft)
    }

    pub fn memory_revision(&mut self, id: MemoryId, revision: u64) -> Result<Memory, MemoryError> {
        self.runtime_memory_revision(id, revision)
    }

    pub fn memory_stats(&self) -> MemoryStats {
        self.memories.stats()
    }

    pub fn memory_ids(&self) -> Vec<MemoryId> {
        self.runtime_memory_ids()
    }

    pub fn memory_version(&self) -> u64 {
        self.memories.memory_version()
    }

    pub fn memory_body_id(&self, id: MemoryId) -> Result<crate::MemoryBodyId, MemoryError> {
        self.runtime_memory_body_id(id)
    }

    pub fn memory_routing_metadata(&self, id: MemoryId) -> Option<&crate::MemoryRoutingMetadata> {
        self.runtime_memory_routing_metadata(id)
    }

    pub fn put_memory_routing_metadata(
        &mut self,
        metadata: crate::MemoryRoutingMetadata,
    ) -> Result<bool, MemoryError> {
        self.owner
            .memories
            .put_routing_metadata(&mut self.owner.container, metadata)
    }

    pub fn stats(&self) -> ArchiveStats {
        self.archive.stats()
    }

    pub fn archive_version(&self) -> u64 {
        self.archive.archive_version()
    }

    pub fn record_versions(&self) -> &[ArchiveRecordVersion] {
        self.archive.record_versions()
    }

    pub fn record_version(&self, archive_version: u64) -> Option<&ArchiveRecordVersion> {
        self.archive.record_version(archive_version)
    }

    pub fn branches(&self) -> Vec<Branch> {
        self.archive.branches()
    }

    pub fn historical_branches(&self) -> Vec<Branch> {
        self.archive.historical_branches()
    }

    pub fn fragments(&self) -> Vec<Fragment> {
        self.archive.fragments()
    }

    pub fn historical_fragments(&self) -> Vec<Fragment> {
        self.archive.historical_fragments()
    }

    pub fn store_file(
        &mut self,
        filename: String,
        mime_type: Option<String>,
        bytes: &[u8],
    ) -> Result<StoredFile, ArchiveError> {
        self.owner
            .archive
            .store_file(&mut self.owner.container, filename, mime_type, bytes)
    }

    pub fn file(&self, id: FileId) -> Option<&StoredFile> {
        self.archive.file(id)
    }

    pub fn files(&self) -> Vec<StoredFile> {
        self.archive.files()
    }

    pub fn file_bytes(&mut self, id: FileId) -> Result<Vec<u8>, ArchiveError> {
        self.runtime_file_bytes(id)
    }

    pub fn conversation_compactions(
        &self,
        conversation_id: &str,
    ) -> Vec<crate::ConversationCompaction> {
        self.runtime_conversation_compactions(conversation_id)
    }

    pub fn put_conversation_compaction(
        &mut self,
        conversation_id: String,
        through_message_id: String,
        summary: String,
        supersedes_through_message_id: Option<&str>,
    ) -> Result<crate::ConversationCompaction, CvaError> {
        let record = self.owner.conversation_compactions.put(
            &mut self.owner.container,
            conversation_id,
            through_message_id,
            summary,
            supersedes_through_message_id,
        )?;
        Ok(record)
    }

    pub(crate) fn put_interaction_stream(
        &mut self,
        record: crate::InteractionStreamRecord,
    ) -> Result<(), CvaError> {
        self.owner
            .interaction_streams
            .put(&mut self.owner.container, record)
            .map_err(CvaError::from)
    }

    pub(crate) fn interaction_stream_record(
        &self,
        message_id: &str,
    ) -> Option<&crate::InteractionStreamRecord> {
        self.interaction_streams.record(message_id)
    }

    pub(crate) fn interaction_streams_for_session(
        &self,
        session_id: &str,
    ) -> Vec<crate::InteractionStreamRecord> {
        self.interaction_streams.records_for_session(session_id)
    }

    pub(crate) fn borrowed_interaction_streams_for_session(
        &self,
        session_id: &str,
    ) -> impl Iterator<Item = &crate::InteractionStreamRecord> {
        self.interaction_streams
            .borrowed_records_for_session(session_id)
    }

    pub(crate) fn interaction_stream_records(&self) -> Vec<crate::InteractionStreamRecord> {
        self.interaction_streams.all_records()
    }

    pub fn put_echo_event(&mut self, event: crate::EchoEvent) -> Result<bool, CvaError> {
        self.runtime_put_echo_event(event).map_err(CvaError::from)
    }

    pub fn echo_events(&self, conversation_id: &str, message_id: &str) -> Vec<crate::EchoEvent> {
        self.runtime_echo_events(conversation_id, message_id)
    }

    pub(crate) fn conversation_echo_events(&self, conversation_id: &str) -> Vec<crate::EchoEvent> {
        self.runtime_conversation_echo_events(conversation_id)
    }

    pub(crate) fn echo_records(&self) -> Vec<crate::EchoEvent> {
        self.echo.all_records()
    }

    pub fn current_rel_semantic_cut(&self) -> crate::RelSemanticCut {
        crate::RelSemanticCut {
            global_version: self.container.latest_version(),
            archive_version: self.archive.archive_version(),
            memory_version: self.memories.memory_version(),
        }
    }

    pub fn correlate_project_revision(
        &mut self,
        project_revision: crate::ProjectRevisionRef,
    ) -> Result<(crate::ProjectRevisionCorrelation, bool), CvaError> {
        let management =
            crate::ProjectRepositoryManagement::default_for(project_revision.repository.kind);
        self.correlate_project_revision_with_management(project_revision, management)
    }

    pub fn correlate_project_revision_with_management(
        &mut self,
        project_revision: crate::ProjectRevisionRef,
        repository_management: crate::ProjectRepositoryManagement,
    ) -> Result<(crate::ProjectRevisionCorrelation, bool), CvaError> {
        let rel_cut = self.current_rel_semantic_cut();
        self.owner
            .project_history
            .put(
                &mut self.owner.container,
                rel_cut,
                project_revision,
                repository_management,
            )
            .map_err(CvaError::from)
    }

    pub fn project_revision_correlations(&self) -> Vec<crate::ProjectRevisionCorrelation> {
        self.runtime_project_revision_correlations()
    }

    pub fn latest_project_revision_correlation(&self) -> Option<crate::ProjectRevisionCorrelation> {
        self.runtime_latest_project_revision_correlation()
    }

    pub fn register_project_file(
        &mut self,
        filename: String,
        mime_type: Option<String>,
        byte_length: u64,
        reference: crate::ProjectFileRef,
    ) -> Result<StoredFile, CvaError> {
        let content_hash = reference.content_hash.ok_or_else(|| {
            CvaError::ProjectFile("project-backed attachments require a content hash".into())
        })?;
        let file = crate::file_store::build_project_stored_file(
            filename,
            mime_type,
            crate::ContentId(content_hash),
            byte_length,
            &reference,
        )?;
        self.owner
            .project_files
            .put(&mut self.owner.container, file.id, reference)?;
        self.owner
            .archive
            .register_file(&mut self.owner.container, file.clone())?;
        Ok(file)
    }

    pub fn bind_legacy_project_file(
        &mut self,
        file_id: crate::FileId,
        reference: crate::ProjectFileRef,
    ) -> Result<bool, CvaError> {
        let file = self
            .archive
            .file(file_id)
            .cloned()
            .ok_or_else(|| CvaError::ProjectFile("legacy attachment file is missing".into()))?;
        let content_hash = reference.content_hash.ok_or_else(|| {
            CvaError::ProjectFile("project-backed attachments require a content hash".into())
        })?;
        if content_hash != file.content_id.0 {
            return Err(CvaError::ProjectFile(
                "legacy attachment content hash does not match the project reference".into(),
            ));
        }
        let legacy = crate::file_store::build_stored_file_from_content_id(
            file.filename.clone(),
            file.mime_type.clone(),
            file.content_id,
            file.byte_length,
        )?;
        if legacy.id != file.id {
            return Err(CvaError::ProjectFile(
                "only legacy embedded attachment identities can be rebound".into(),
            ));
        }
        self.owner
            .project_files
            .put(&mut self.owner.container, file.id, reference)
            .map_err(CvaError::from)
    }

    pub fn project_file_ref(&self, file_id: crate::FileId) -> Option<crate::ProjectFileRef> {
        self.runtime_project_file_ref(file_id)
    }

    pub fn sync(&self) -> Result<(), CvaError> {
        self.container.sync()?;
        Ok(())
    }
}
