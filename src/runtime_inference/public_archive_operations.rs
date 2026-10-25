//! Native runtime contracts for the public REL Archive and owner APIs.
use crate::{
    Archive, ArchiveError, ArchiveRecordVersion, ArchiveStats, Branch, ConversationMetadata,
    ConversationPolicy, CvaError, Episode, EpisodeBoundary, EpisodeBuildResult, EpisodeConfig,
    EpisodeId, EpisodeOrigin, FileId, Fragment, FragmentConfig, FragmentId, Node, ResolvedTurn,
    StoredFile,
};

impl crate::RelExecutionOwner {
    pub(crate) fn runtime_required_node_timestamp(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Result<i64, crate::ArchiveError> {
        self.owner
            .native_required_node_timestamp(conversation_id, node_id)
    }

    pub(crate) fn runtime_archive(&self) -> &Archive {
        &self.owner.archive
    }

    pub(crate) fn native_append_archive_node(
        &mut self,
        id: String,
        conversation_id: String,
        parent_id: Option<String>,
        role: String,
        principal_id: Option<String>,
        timestamp_ns: i64,
        content: &str,
    ) -> Result<Node, ArchiveError> {
        let value = self.owner.append_archive_node(
            id,
            conversation_id,
            parent_id,
            role,
            principal_id,
            timestamp_ns,
            content,
        )?;
        self.freshness_on_accepted_activity(self.owner.archive.rel_turn_count());
        Ok(value)
    }

    pub(crate) fn native_set_conversation_title(
        &mut self,
        conversation_id: &str,
        title: String,
    ) -> Result<bool, ArchiveError> {
        let mut metadata = self
            .owner
            .archive
            .conversation_metadata(conversation_id)
            .cloned()
            .unwrap_or_else(|| ConversationMetadata::new(conversation_id));
        metadata.title = Some(title);
        self.owner.put_conversation_metadata(metadata)
    }

    pub(crate) fn native_set_conversation_active(
        &mut self,
        conversation_id: &str,
        active: bool,
    ) -> Result<bool, ArchiveError> {
        let mut metadata = self
            .owner
            .archive
            .conversation_metadata(conversation_id)
            .cloned()
            .unwrap_or_else(|| ConversationMetadata::new(conversation_id));
        metadata.active = active;
        self.owner.put_conversation_metadata(metadata)
    }

    pub(crate) fn native_set_conversation_policy(
        &mut self,
        conversation_id: &str,
        policy: ConversationPolicy,
    ) -> Result<bool, ArchiveError> {
        self.owner.set_conversation_policy(conversation_id, policy)
    }

    pub(crate) fn runtime_conversation_metadata(
        &self,
        conversation_id: &str,
    ) -> Option<&ConversationMetadata> {
        self.owner.archive.conversation_metadata(conversation_id)
    }

    pub(crate) fn runtime_historical_conversation_metadata(
        &self,
        conversation_id: &str,
    ) -> Option<&ConversationMetadata> {
        self.owner
            .archive
            .historical_conversation_metadata(conversation_id)
    }

    pub(crate) fn native_branch_turns(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.owner.branch_turns(conversation_id, branch_id)
    }

    pub(crate) fn native_historical_branch_turns(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.owner
            .historical_branch_turns(conversation_id, branch_id)
    }

    pub(crate) fn native_branch_at(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
        archive_version: u64,
    ) -> Result<Option<Branch>, ArchiveError> {
        self.owner
            .branch_at(conversation_id, branch_id, archive_version)
    }

    pub(crate) fn native_materialize_branch_fragments(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
        config: FragmentConfig,
        close_tail: bool,
    ) -> Result<Vec<Fragment>, ArchiveError> {
        self.owner
            .materialize_branch_fragments(conversation_id, branch_id, config, close_tail)
    }

    pub(crate) fn native_materialize_path_fragments(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        config: FragmentConfig,
        close_tail: bool,
    ) -> Result<Vec<Fragment>, ArchiveError> {
        self.owner
            .materialize_path_fragments(conversation_id, leaf_node_id, config, close_tail)
    }

    pub(crate) fn native_fragment_turns(
        &mut self,
        id: FragmentId,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.owner.fragment_turns(id)
    }

    pub(crate) fn native_fragment_text(&mut self, id: FragmentId) -> Result<String, ArchiveError> {
        self.owner.fragment_text(id)
    }

    pub(crate) fn native_materialize_branch_episodes(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
        config: EpisodeConfig,
        origin: EpisodeOrigin,
        close_tail: Option<(EpisodeBoundary, i64)>,
    ) -> Result<EpisodeBuildResult, ArchiveError> {
        self.owner.materialize_branch_episodes(
            conversation_id,
            branch_id,
            config,
            origin,
            close_tail,
        )
    }

    pub(crate) fn native_materialize_path_episodes(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        config: EpisodeConfig,
        origin: EpisodeOrigin,
        close_tail: Option<(EpisodeBoundary, i64)>,
    ) -> Result<EpisodeBuildResult, ArchiveError> {
        self.owner.materialize_path_episodes(
            conversation_id,
            leaf_node_id,
            config,
            origin,
            close_tail,
        )
    }

    pub(crate) fn runtime_episodes(&self) -> Vec<Episode> {
        self.owner.archive.episodes()
    }

    pub(crate) fn runtime_episodes_for_conversation(&self, conversation_id: &str) -> Vec<Episode> {
        self.owner
            .archive
            .episodes_for_conversation(conversation_id)
    }

    pub(crate) fn runtime_historical_episodes(&self) -> Vec<Episode> {
        self.owner.archive.historical_episodes()
    }

    pub(crate) fn runtime_historical_episodes_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Vec<Episode> {
        self.owner
            .archive
            .historical_episodes_for_conversation(conversation_id)
    }

    pub(crate) fn runtime_episode(&self, id: EpisodeId) -> Option<&Episode> {
        self.owner.archive.episode(id)
    }

    pub(crate) fn runtime_historical_episode(&self, id: EpisodeId) -> Option<&Episode> {
        self.owner.archive.historical_episode(id)
    }

    pub(crate) fn native_episode_turns(
        &mut self,
        id: EpisodeId,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.owner.episode_turns(id)
    }

    pub(crate) fn runtime_archive_stats(&self) -> ArchiveStats {
        self.owner.archive.stats()
    }

    pub(crate) fn runtime_archive_version(&self) -> u64 {
        self.owner.archive.archive_version()
    }

    pub(crate) fn runtime_archive_record_versions(&self) -> &[ArchiveRecordVersion] {
        self.owner.archive.record_versions()
    }

    pub(crate) fn runtime_archive_record_version(
        &self,
        archive_version: u64,
    ) -> Option<&ArchiveRecordVersion> {
        self.owner.archive.record_version(archive_version)
    }

    pub(crate) fn runtime_branches(&self) -> Vec<Branch> {
        self.owner.archive.branches()
    }

    pub(crate) fn runtime_historical_branches(&self) -> Vec<Branch> {
        self.owner.archive.historical_branches()
    }

    pub(crate) fn runtime_fragments(&self) -> Vec<Fragment> {
        self.owner.archive.fragments()
    }

    pub(crate) fn runtime_historical_fragments(&self) -> Vec<Fragment> {
        self.owner.archive.historical_fragments()
    }

    pub(crate) fn native_store_file(
        &mut self,
        filename: String,
        mime_type: Option<String>,
        bytes: &[u8],
    ) -> Result<StoredFile, ArchiveError> {
        self.owner.store_file(filename, mime_type, bytes)
    }

    pub(crate) fn runtime_file(&self, id: FileId) -> Option<&StoredFile> {
        self.owner.archive.file(id)
    }

    pub(crate) fn runtime_files(&self) -> Vec<StoredFile> {
        self.owner.archive.files()
    }

    pub(crate) fn native_put_conversation_compaction(
        &mut self,
        conversation_id: String,
        through_message_id: String,
        summary: String,
        supersedes_through_message_id: Option<&str>,
    ) -> Result<crate::ConversationCompaction, CvaError> {
        self.owner
            .put_conversation_compaction(
                conversation_id,
                through_message_id,
                summary,
                supersedes_through_message_id,
            )
            .map_err(CvaError::from)
    }

    pub(crate) fn native_put_interaction_stream(
        &mut self,
        record: crate::InteractionStreamRecord,
    ) -> Result<(), CvaError> {
        self.owner
            .put_interaction_stream(record)
            .map_err(CvaError::from)
    }

    pub(crate) fn runtime_interaction_stream_record(
        &self,
        message_id: &str,
    ) -> Option<&crate::InteractionStreamRecord> {
        self.owner.interaction_streams.record(message_id)
    }

    pub(crate) fn runtime_interaction_streams_for_session(
        &self,
        session_id: &str,
    ) -> Vec<crate::InteractionStreamRecord> {
        self.owner
            .interaction_streams
            .records_for_session(session_id)
    }

    pub(crate) fn runtime_borrowed_interaction_streams_for_session(
        &self,
        session_id: &str,
    ) -> impl Iterator<Item = &crate::InteractionStreamRecord> {
        self.owner
            .interaction_streams
            .borrowed_records_for_session(session_id)
    }

    pub(crate) fn runtime_interaction_stream_records(&self) -> Vec<crate::InteractionStreamRecord> {
        self.owner.interaction_streams.all_records()
    }

    pub(crate) fn runtime_echo_records(&self) -> Vec<crate::EchoEvent> {
        self.owner.echo.all_records()
    }

    pub(crate) fn runtime_current_rel_semantic_cut(&self) -> crate::RelSemanticCut {
        crate::RelSemanticCut {
            global_version: self.runtime_latest_global_version(),
            archive_version: self.runtime_archive_version(),
            memory_version: self.native_memory_version(),
        }
    }

    pub(crate) fn native_correlate_project_revision(
        &mut self,
        project_revision: crate::ProjectRevisionRef,
    ) -> Result<(crate::ProjectRevisionCorrelation, bool), CvaError> {
        let repository_management =
            crate::ProjectRepositoryManagement::default_for(project_revision.repository.kind);
        self.native_correlate_project_revision_with_management(
            project_revision,
            repository_management,
        )
    }

    pub(crate) fn native_correlate_project_revision_with_management(
        &mut self,
        project_revision: crate::ProjectRevisionRef,
        repository_management: crate::ProjectRepositoryManagement,
    ) -> Result<(crate::ProjectRevisionCorrelation, bool), CvaError> {
        let cut = self.runtime_current_rel_semantic_cut();
        self.owner
            .put_project_revision_correlation(cut, project_revision, repository_management)
            .map_err(CvaError::from)
    }

    pub(crate) fn native_register_project_file(
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
            .put_project_file_reference(file.id, reference)
            .map_err(CvaError::from)?;
        self.owner.register_file(file.clone())?;
        Ok(file)
    }

    pub(crate) fn native_bind_legacy_project_file(
        &mut self,
        file_id: FileId,
        reference: crate::ProjectFileRef,
    ) -> Result<bool, CvaError> {
        let file = self
            .owner
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
            .put_project_file_reference(file.id, reference)
            .map_err(CvaError::from)
    }
}

// Canonical Gateway bindings for Archive lifecycle operations.
impl crate::RelExecutionOwner {
    pub(crate) fn runtime_historical_files_for_source(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Vec<crate::StoredFile> {
        self.owner
            .native_historical_files_for_source(conversation_id, node_id)
    }

    pub(crate) fn runtime_activity_cut_at_archive_version(
        &self,
        version: u64,
    ) -> Result<u64, crate::ArchiveError> {
        self.owner.native_activity_cut_at_archive_version(version)
    }

    pub(crate) fn runtime_activity_position_for_turn(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Option<u64> {
        self.owner
            .native_activity_position_for_turn(conversation_id, node_id)
    }

    pub(crate) fn runtime_file_memory_links(
        &self,
        file_id: crate::FileId,
    ) -> Vec<crate::FileMemoryLink> {
        self.owner.native_file_memory_links(file_id)
    }

    pub(crate) fn runtime_files_for_memory(
        &self,
        memory_id: crate::MemoryId,
    ) -> Vec<crate::StoredFile> {
        self.owner.native_files_for_memory(memory_id)
    }

    pub(crate) fn runtime_conversation_summaries(&self) -> Vec<crate::ConversationSummary> {
        self.owner.native_conversation_summaries()
    }

    pub(crate) fn native_historical_conversation_turns(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<crate::ResolvedTurn>, crate::ArchiveError> {
        self.owner
            .native_historical_conversation_turns(conversation_id, leaf_node_id)
    }

    pub(crate) fn native_conversation_turn_page(
        &mut self,
        conversation_id: &str,
        end_node_id: &str,
        limit: usize,
    ) -> Result<(Vec<crate::ResolvedTurn>, Option<String>), crate::ArchiveError> {
        self.owner
            .native_conversation_turn_page(conversation_id, end_node_id, limit)
    }

    pub(crate) fn native_tombstone_conversation(
        &mut self,
        conversation_id: &str,
    ) -> Result<bool, crate::ArchiveError> {
        self.owner.native_tombstone_conversation(conversation_id)
    }

    pub(crate) fn native_conversation_tombstone(
        &self,
        conversation_id: &str,
    ) -> Option<&crate::ConversationTombstone> {
        self.owner.native_conversation_tombstone(conversation_id)
    }

    pub(crate) fn native_conversation_lifecycle_state(
        &self,
        conversation_id: &str,
    ) -> Result<crate::owner_lifecycle::LifecycleState, crate::owner_lifecycle::LifecycleError>
    {
        self.owner
            .native_conversation_lifecycle_state(conversation_id)
    }
}
