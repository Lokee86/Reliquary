use crate::{
    Archive, ArchiveError, ArchiveRecordVersion, ArchiveStats, Branch, ConversationMetadata,
    CvaError, Episode, EpisodeBoundary, EpisodeBuildResult, EpisodeConfig, EpisodeId,
    EpisodeOrigin, FileId, Fragment, FragmentConfig, FragmentId, Memory, MemoryDraft, MemoryError,
    MemoryId, MemoryStats, Node, ResolvedTurn, StoredFile,
};

pub use crate::rel_execution_owner::RelExecutionOwner as Cva;

impl Cva {
    pub fn legacy_scope_kind(&self) -> Option<crate::ReliquaryScopeKind> {
        self.runtime_legacy_scope_kind()
    }

    pub fn rel_metadata(&self) -> crate::RelMetadata {
        self.runtime_rel_metadata()
    }

    pub fn set_rel_metadata(
        &mut self,
        type_label: Option<String>,
        dependencies: Vec<String>,
    ) -> Result<bool, CvaError> {
        self.native_put_rel_metadata(type_label, dependencies)
    }

    pub fn owner_id(&self) -> Option<String> {
        self.runtime_owner_id()
    }

    pub fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.runtime_owner_uuid()
    }

    pub fn latest_global_version(&self) -> u64 {
        self.runtime_latest_global_version()
    }

    pub fn transaction_time_ns(&self, version: u64) -> Option<i64> {
        self.runtime_transaction_time_ns(version)
    }

    pub fn version_at_or_before(&self, transaction_time_ns: i64) -> Option<u64> {
        self.runtime_version_at_or_before(transaction_time_ns)
    }

    pub fn archive(&self) -> &Archive {
        self.runtime_archive()
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
        self.native_append_archive_node(
            id,
            conversation_id,
            parent_id,
            role,
            principal_id,
            timestamp_ns,
            content,
        )
    }

    pub fn append_branch(&mut self, branch: Branch) -> Result<(), ArchiveError> {
        self.runtime_append_branch(branch)
    }

    pub fn set_conversation_title(
        &mut self,
        conversation_id: &str,
        title: String,
    ) -> Result<bool, ArchiveError> {
        self.native_set_conversation_title(conversation_id, title)
    }

    pub fn set_conversation_active(
        &mut self,
        conversation_id: &str,
        active: bool,
    ) -> Result<bool, ArchiveError> {
        self.native_set_conversation_active(conversation_id, active)
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
        self.native_set_conversation_policy(conversation_id, policy)
    }

    pub fn conversation_metadata(&self, conversation_id: &str) -> Option<&ConversationMetadata> {
        self.runtime_conversation_metadata(conversation_id)
    }

    pub fn historical_conversation_metadata(
        &self,
        conversation_id: &str,
    ) -> Option<&ConversationMetadata> {
        self.runtime_historical_conversation_metadata(conversation_id)
    }

    pub fn branch_turns(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.native_branch_turns(conversation_id, branch_id)
    }

    pub fn historical_branch_turns(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.native_historical_branch_turns(conversation_id, branch_id)
    }

    pub fn branch_at(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
        archive_version: u64,
    ) -> Result<Option<Branch>, ArchiveError> {
        self.native_branch_at(conversation_id, branch_id, archive_version)
    }

    pub fn materialize_branch_fragments(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
        config: FragmentConfig,
        close_tail: bool,
    ) -> Result<Vec<Fragment>, ArchiveError> {
        self.native_materialize_branch_fragments(conversation_id, branch_id, config, close_tail)
    }

    pub fn materialize_path_fragments(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        config: FragmentConfig,
        close_tail: bool,
    ) -> Result<Vec<Fragment>, ArchiveError> {
        self.native_materialize_path_fragments(conversation_id, leaf_node_id, config, close_tail)
    }

    pub fn fragment_turns(&mut self, id: FragmentId) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.native_fragment_turns(id)
    }

    pub fn fragment_text(&mut self, id: FragmentId) -> Result<String, ArchiveError> {
        self.native_fragment_text(id)
    }

    pub fn materialize_branch_episodes(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
        config: EpisodeConfig,
        origin: EpisodeOrigin,
        close_tail: Option<(EpisodeBoundary, i64)>,
    ) -> Result<EpisodeBuildResult, ArchiveError> {
        self.native_materialize_branch_episodes(
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
        self.native_materialize_path_episodes(
            conversation_id,
            leaf_node_id,
            config,
            origin,
            close_tail,
        )
    }

    pub fn episodes(&self) -> Vec<Episode> {
        self.runtime_episodes()
    }

    pub fn episodes_for_conversation(&self, conversation_id: &str) -> Vec<Episode> {
        self.runtime_episodes_for_conversation(conversation_id)
    }

    pub fn historical_episodes(&self) -> Vec<Episode> {
        self.runtime_historical_episodes()
    }

    pub fn historical_episodes_for_conversation(&self, conversation_id: &str) -> Vec<Episode> {
        self.runtime_historical_episodes_for_conversation(conversation_id)
    }

    pub fn episode(&self, id: EpisodeId) -> Option<&Episode> {
        self.runtime_episode(id)
    }

    pub fn historical_episode(&self, id: EpisodeId) -> Option<&Episode> {
        self.runtime_historical_episode(id)
    }

    pub fn episode_turns(&mut self, id: EpisodeId) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.native_episode_turns(id)
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
        self.native_tombstone_memory(id, expected_revision)
    }

    pub fn memory_retention_descriptor(
        &self,
        id: MemoryId,
    ) -> Result<crate::MemoryRetentionDescriptor, MemoryError> {
        self.native_memory_retention_descriptor(id)
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
        self.native_memory_lifecycle_state(id)
    }

    pub fn set_memory_temporal_status(
        &mut self,
        id: MemoryId,
        expected_revision: u64,
        temporal_status: &str,
        mutation_id: String,
    ) -> Result<(Memory, bool), MemoryError> {
        self.native_set_memory_temporal_status(id, expected_revision, temporal_status, mutation_id)
    }

    pub fn memory_revision(&mut self, id: MemoryId, revision: u64) -> Result<Memory, MemoryError> {
        self.runtime_memory_revision(id, revision)
    }

    pub fn memory_stats(&self) -> MemoryStats {
        self.native_memory_stats()
    }

    pub fn memory_ids(&self) -> Vec<MemoryId> {
        self.runtime_memory_ids()
    }

    pub fn memory_version(&self) -> u64 {
        self.native_memory_version()
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
        self.native_put_memory_routing_metadata(metadata)
    }

    pub fn stats(&self) -> ArchiveStats {
        self.runtime_archive_stats()
    }

    pub fn archive_version(&self) -> u64 {
        self.runtime_archive_version()
    }

    pub fn record_versions(&self) -> &[ArchiveRecordVersion] {
        self.runtime_archive_record_versions()
    }

    pub fn record_version(&self, archive_version: u64) -> Option<&ArchiveRecordVersion> {
        self.runtime_archive_record_version(archive_version)
    }

    pub fn branches(&self) -> Vec<Branch> {
        self.runtime_branches()
    }

    pub fn historical_branches(&self) -> Vec<Branch> {
        self.runtime_historical_branches()
    }

    pub fn fragments(&self) -> Vec<Fragment> {
        self.runtime_fragments()
    }

    pub fn historical_fragments(&self) -> Vec<Fragment> {
        self.runtime_historical_fragments()
    }

    pub fn store_file(
        &mut self,
        filename: String,
        mime_type: Option<String>,
        bytes: &[u8],
    ) -> Result<StoredFile, ArchiveError> {
        self.native_store_file(filename, mime_type, bytes)
    }

    pub fn file(&self, id: FileId) -> Option<&StoredFile> {
        self.runtime_file(id)
    }

    pub fn files(&self) -> Vec<StoredFile> {
        self.runtime_files()
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
        self.native_put_conversation_compaction(
            conversation_id,
            through_message_id,
            summary,
            supersedes_through_message_id,
        )
    }

    pub(crate) fn put_interaction_stream(
        &mut self,
        record: crate::InteractionStreamRecord,
    ) -> Result<(), CvaError> {
        self.native_put_interaction_stream(record)
    }

    pub(crate) fn interaction_stream_record(
        &self,
        message_id: &str,
    ) -> Option<&crate::InteractionStreamRecord> {
        self.runtime_interaction_stream_record(message_id)
    }

    pub(crate) fn interaction_streams_for_session(
        &self,
        session_id: &str,
    ) -> Vec<crate::InteractionStreamRecord> {
        self.runtime_interaction_streams_for_session(session_id)
    }

    pub(crate) fn borrowed_interaction_streams_for_session(
        &self,
        session_id: &str,
    ) -> impl Iterator<Item = &crate::InteractionStreamRecord> {
        self.runtime_borrowed_interaction_streams_for_session(session_id)
    }

    pub(crate) fn interaction_stream_records(&self) -> Vec<crate::InteractionStreamRecord> {
        self.runtime_interaction_stream_records()
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
        self.runtime_echo_records()
    }

    pub fn current_rel_semantic_cut(&self) -> crate::RelSemanticCut {
        self.runtime_current_rel_semantic_cut()
    }

    pub fn correlate_project_revision(
        &mut self,
        project_revision: crate::ProjectRevisionRef,
    ) -> Result<(crate::ProjectRevisionCorrelation, bool), CvaError> {
        self.native_correlate_project_revision(project_revision)
    }

    pub fn correlate_project_revision_with_management(
        &mut self,
        project_revision: crate::ProjectRevisionRef,
        repository_management: crate::ProjectRepositoryManagement,
    ) -> Result<(crate::ProjectRevisionCorrelation, bool), CvaError> {
        self.native_correlate_project_revision_with_management(
            project_revision,
            repository_management,
        )
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
        self.native_register_project_file(filename, mime_type, byte_length, reference)
    }

    pub fn bind_legacy_project_file(
        &mut self,
        file_id: crate::FileId,
        reference: crate::ProjectFileRef,
    ) -> Result<bool, CvaError> {
        self.native_bind_legacy_project_file(file_id, reference)
    }

    pub fn project_file_ref(&self, file_id: crate::FileId) -> Option<crate::ProjectFileRef> {
        self.runtime_project_file_ref(file_id)
    }

    pub fn sync(&self) -> Result<(), CvaError> {
        self.runtime_sync_public_owner()
    }
}
