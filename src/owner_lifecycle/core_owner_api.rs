//! Canonical lifecycle operations for supported public REL and PHY owner APIs.
use crate::{
    ArchiveError, Branch, ConversationMetadata, ConversationPolicy, Episode, EpisodeBoundary,
    EpisodeBuildResult, EpisodeConfig, EpisodeId, EpisodeOrigin, FileId, Fragment, FragmentConfig,
    FragmentId, Node, ResolvedTurn, StoredFile,
};

impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn legacy_scope_kind(&self) -> Option<crate::ReliquaryScopeKind> {
        self.container
            .identity()
            .and_then(|identity| identity.scope)
    }

    pub(crate) fn transaction_time_ns(&self, version: u64) -> Option<i64> {
        self.container.transaction_time_ns(version)
    }

    pub(crate) fn version_at_or_before(&self, transaction_time_ns: i64) -> Option<u64> {
        self.container.version_at_or_before(transaction_time_ns)
    }

    pub(crate) fn put_rel_metadata(
        &mut self,
        metadata: crate::RelMetadata,
    ) -> Result<bool, String> {
        self.rel_metadata
            .put(&mut self.container, metadata)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn append_archive_node(
        &mut self,
        id: String,
        conversation_id: String,
        parent_id: Option<String>,
        role: String,
        principal_id: Option<String>,
        timestamp_ns: i64,
        content: &str,
    ) -> Result<Node, ArchiveError> {
        self.archive
            .append_node(
                &mut self.container,
                id,
                conversation_id,
                parent_id,
                role,
                principal_id,
                timestamp_ns,
                content,
            )
            .map(|accepted| accepted.value)
    }

    pub(crate) fn put_conversation_metadata(
        &mut self,
        metadata: ConversationMetadata,
    ) -> Result<bool, ArchiveError> {
        self.archive
            .put_conversation_metadata(&mut self.container, metadata)
    }

    pub(crate) fn set_conversation_policy(
        &mut self,
        conversation_id: &str,
        policy: ConversationPolicy,
    ) -> Result<bool, ArchiveError> {
        self.archive
            .set_conversation_policy(&mut self.container, conversation_id, policy)
    }

    pub(crate) fn branch_turns(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.archive
            .branch_turns(&mut self.container, conversation_id, branch_id)
    }

    pub(crate) fn historical_branch_turns(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.archive
            .historical_branch_turns(&mut self.container, conversation_id, branch_id)
    }

    pub(crate) fn branch_at(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
        archive_version: u64,
    ) -> Result<Option<Branch>, ArchiveError> {
        self.archive.branch_at(
            &mut self.container,
            conversation_id,
            branch_id,
            archive_version,
        )
    }

    pub(crate) fn materialize_branch_fragments(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
        config: FragmentConfig,
        close_tail: bool,
    ) -> Result<Vec<Fragment>, ArchiveError> {
        self.archive.materialize_branch_fragments(
            &mut self.container,
            conversation_id,
            branch_id,
            config,
            close_tail,
        )
    }

    pub(crate) fn materialize_path_fragments(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        config: FragmentConfig,
        close_tail: bool,
    ) -> Result<Vec<Fragment>, ArchiveError> {
        self.archive.materialize_path_fragments(
            &mut self.container,
            conversation_id,
            leaf_node_id,
            config,
            close_tail,
        )
    }

    pub(crate) fn fragment_turns(
        &mut self,
        id: FragmentId,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.archive.fragment_turns(&mut self.container, id)
    }

    pub(crate) fn fragment_text(&mut self, id: FragmentId) -> Result<String, ArchiveError> {
        self.archive.fragment_text(&mut self.container, id)
    }

    pub(crate) fn materialize_branch_episodes(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
        config: EpisodeConfig,
        origin: EpisodeOrigin,
        close_tail: Option<(EpisodeBoundary, i64)>,
    ) -> Result<EpisodeBuildResult, ArchiveError> {
        self.archive.materialize_branch_episodes(
            &mut self.container,
            conversation_id,
            branch_id,
            config,
            origin,
            close_tail,
        )
    }

    pub(crate) fn materialize_path_episodes(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        config: EpisodeConfig,
        origin: EpisodeOrigin,
        close_tail: Option<(EpisodeBoundary, i64)>,
    ) -> Result<EpisodeBuildResult, ArchiveError> {
        self.archive.materialize_path_episodes(
            &mut self.container,
            conversation_id,
            leaf_node_id,
            config,
            origin,
            close_tail,
        )
    }

    pub(crate) fn episode_turns(
        &mut self,
        id: EpisodeId,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.archive.episode_turns(&mut self.container, id)
    }

    pub(crate) fn store_file(
        &mut self,
        filename: String,
        mime_type: Option<String>,
        bytes: &[u8],
    ) -> Result<StoredFile, ArchiveError> {
        self.archive
            .store_file(&mut self.container, filename, mime_type, bytes)
    }

    pub(crate) fn register_file(&mut self, file: StoredFile) -> Result<StoredFile, ArchiveError> {
        self.archive.register_file(&mut self.container, file)
    }

    pub(crate) fn put_conversation_compaction(
        &mut self,
        conversation_id: String,
        through_message_id: String,
        summary: String,
        supersedes_through_message_id: Option<&str>,
    ) -> Result<crate::ConversationCompaction, crate::ConversationCompactionError> {
        self.conversation_compactions.put(
            &mut self.container,
            conversation_id,
            through_message_id,
            summary,
            supersedes_through_message_id,
        )
    }

    pub(crate) fn put_interaction_stream(
        &mut self,
        record: crate::InteractionStreamRecord,
    ) -> Result<(), crate::interaction_stream_error::InteractionStreamError> {
        self.interaction_streams.put(&mut self.container, record)
    }

    pub(crate) fn put_project_revision_correlation(
        &mut self,
        cut: crate::RelSemanticCut,
        revision: crate::ProjectRevisionRef,
        management: crate::ProjectRepositoryManagement,
    ) -> Result<
        (crate::ProjectRevisionCorrelation, bool),
        crate::project_history_store::ProjectHistoryStoreError,
    > {
        self.project_history
            .put(&mut self.container, cut, revision, management)
    }

    pub(crate) fn put_project_file_reference(
        &mut self,
        file_id: FileId,
        reference: crate::ProjectFileRef,
    ) -> Result<bool, crate::project_file_binding_store::ProjectFileStoreError> {
        self.project_files
            .put(&mut self.container, file_id, reference)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn transaction_time_ns(&self, version: u64) -> Option<i64> {
        self.container.transaction_time_ns(version)
    }

    pub(crate) fn version_at_or_before(&self, transaction_time_ns: i64) -> Option<u64> {
        self.container.version_at_or_before(transaction_time_ns)
    }

    pub(crate) fn set_profile(&mut self, display_name: Option<String>) -> Result<bool, String> {
        let display_name = display_name.and_then(|value| {
            let trimmed = value.trim().to_owned();
            (!trimmed.is_empty()).then_some(trimmed)
        });
        self.profile.put(
            &mut self.container,
            crate::PhylacteryProfile { display_name },
        )
    }
}
