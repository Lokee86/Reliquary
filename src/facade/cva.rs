use crate::archive_vector_store::ArchiveVectorStore;
use crate::community_store::CommunityStore;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::conversation_compaction_store::ConversationCompactionStore;
use crate::cva_memory_publish::publish_memory_parts;
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

pub struct Cva {
    pub(crate) container: Container,
    pub(crate) archive: Archive,
    pub(crate) memories: MemoryStore,
    pub(crate) graph: GraphStore,
    pub(crate) communities: CommunityStore,
    pub(crate) duplicate_index: DuplicateIndex,
    pub(crate) processing_epochs: ProcessingEpochStore,
    pub(crate) dream_pairs: DreamPairStore,
    pub(crate) insomnia: InsomniaStore,
    pub(crate) lexical_index: LexicalIndex,
    pub(crate) packed_vectors: PackedVectorStore,
    pub(crate) memory_vectors: MemoryVectorStore,
    pub(crate) archive_vectors: ArchiveVectorStore,
    pub(crate) compatibility_profiles: CompatibilityProfileStore,
    pub(crate) vector_generations: VectorGenerationStore,
    pub(crate) conversation_compactions: ConversationCompactionStore,
    pub(crate) interaction_streams: InteractionStreamStore,
    pub(crate) echo: EchoStore,
    pub(crate) ego: EgoStore,
    pub(crate) entities: EntityStore,
    pub(crate) entity_resolutions: EntityResolutionStore,
    pub(crate) relationships: RelationshipStore,
    pub(crate) project_history: ProjectHistoryStore,
    pub(crate) project_files: ProjectFileStore,
    pub(crate) rel_metadata: RelMetadataStore,
}

impl Cva {
    pub fn legacy_scope_kind(&self) -> Option<crate::ReliquaryScopeKind> {
        self.container
            .identity()
            .and_then(|identity| identity.scope)
    }

    pub fn rel_metadata(&self) -> crate::RelMetadata {
        if let Some(metadata) = self.rel_metadata.current() {
            return metadata.clone();
        }
        crate::RelMetadata {
            type_label: self.legacy_scope_kind().map(|scope| match scope {
                crate::ReliquaryScopeKind::Organization => "Organization".to_owned(),
                crate::ReliquaryScopeKind::Project => "Project".to_owned(),
                crate::ReliquaryScopeKind::Connection => "Connection".to_owned(),
            }),
            dependencies: Vec::new(),
        }
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
        self.rel_metadata
            .put(&mut self.container, metadata)
            .map_err(CvaError::RelMetadata)
    }

    pub fn is_legacy_cva(&self) -> bool {
        self.container.identity().is_none()
    }

    pub fn owner_id(&self) -> Option<String> {
        self.container.owner_id()
    }

    pub fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.container.owner_uuid()
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

    pub fn append_branch(&mut self, branch: Branch) -> Result<(), ArchiveError> {
        self.archive.append_branch(&mut self.container, branch)
    }

    pub fn set_conversation_title(
        &mut self,
        conversation_id: &str,
        title: String,
    ) -> Result<bool, ArchiveError> {
        let active = self
            .archive
            .conversation_metadata(conversation_id)
            .is_some_and(|metadata| metadata.active);
        self.archive.put_conversation_metadata(
            &mut self.container,
            ConversationMetadata {
                conversation_id: conversation_id.to_owned(),
                title: Some(title),
                active,
            },
        )
    }

    pub fn set_conversation_active(
        &mut self,
        conversation_id: &str,
        active: bool,
    ) -> Result<bool, ArchiveError> {
        let title = self
            .archive
            .conversation_metadata(conversation_id)
            .and_then(|metadata| metadata.title.clone());
        self.archive.put_conversation_metadata(
            &mut self.container,
            ConversationMetadata {
                conversation_id: conversation_id.to_owned(),
                title,
                active,
            },
        )
    }

    pub fn conversation_metadata(&self, conversation_id: &str) -> Option<&ConversationMetadata> {
        self.archive.conversation_metadata(conversation_id)
    }

    pub fn branch_turns(
        &mut self,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.archive
            .branch_turns(&mut self.container, conversation_id, branch_id)
    }

    pub fn branch_at(
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

    pub fn materialize_branch_fragments(
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

    pub fn materialize_path_fragments(
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

    pub fn fragment_turns(&mut self, id: FragmentId) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.archive.fragment_turns(&mut self.container, id)
    }

    pub fn fragment_text(&mut self, id: FragmentId) -> Result<String, ArchiveError> {
        self.archive.fragment_text(&mut self.container, id)
    }

    pub fn materialize_branch_episodes(
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

    pub fn materialize_path_episodes(
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

    pub fn episodes(&self) -> Vec<Episode> {
        self.archive.episodes()
    }

    pub fn episodes_for_conversation(&self, conversation_id: &str) -> Vec<Episode> {
        self.archive.episodes_for_conversation(conversation_id)
    }

    pub fn episode(&self, id: EpisodeId) -> Option<&Episode> {
        self.archive.episode(id)
    }

    pub fn episode_turns(&mut self, id: EpisodeId) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.archive.episode_turns(&mut self.container, id)
    }

    pub fn publish_memory(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
    ) -> Result<(Memory, bool), MemoryError> {
        publish_memory_parts(
            &self.archive,
            &mut self.memories,
            &mut self.container,
            id,
            expected_revision,
            draft,
        )
    }

    pub fn memory(&mut self, id: MemoryId) -> Result<Memory, MemoryError> {
        self.memories.memory(&mut self.container, id)
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
        self.memories
            .memory_revision(&mut self.container, id, revision)
    }

    pub fn memory_stats(&self) -> MemoryStats {
        self.memories.stats()
    }

    pub fn memory_ids(&self) -> Vec<MemoryId> {
        self.memories.current_ids()
    }

    pub fn memory_version(&self) -> u64 {
        self.memories.memory_version()
    }

    pub fn memory_body_id(&self, id: MemoryId) -> Result<crate::MemoryBodyId, MemoryError> {
        self.memories.current_body_id(id)
    }

    pub fn memory_routing_metadata(&self, id: MemoryId) -> Option<&crate::MemoryRoutingMetadata> {
        self.memories.routing_metadata(id)
    }

    pub fn put_memory_routing_metadata(
        &mut self,
        metadata: crate::MemoryRoutingMetadata,
    ) -> Result<bool, MemoryError> {
        self.memories
            .put_routing_metadata(&mut self.container, metadata)
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

    pub fn fragments(&self) -> Vec<Fragment> {
        self.archive.fragments()
    }

    pub fn store_file(
        &mut self,
        filename: String,
        mime_type: Option<String>,
        bytes: &[u8],
    ) -> Result<StoredFile, ArchiveError> {
        self.archive
            .store_file(&mut self.container, filename, mime_type, bytes)
    }

    pub fn file(&self, id: FileId) -> Option<&StoredFile> {
        self.archive.file(id)
    }

    pub fn files(&self) -> Vec<StoredFile> {
        self.archive.files()
    }

    pub fn file_bytes(&mut self, id: FileId) -> Result<Vec<u8>, ArchiveError> {
        self.archive.file_bytes(&mut self.container, id)
    }

    pub fn conversation_compactions(
        &self,
        conversation_id: &str,
    ) -> Vec<crate::ConversationCompaction> {
        self.conversation_compactions
            .for_conversation(conversation_id)
    }

    pub fn put_conversation_compaction(
        &mut self,
        conversation_id: String,
        through_message_id: String,
        summary: String,
        supersedes_through_message_id: Option<&str>,
    ) -> Result<crate::ConversationCompaction, CvaError> {
        let record = self.conversation_compactions.put(
            &mut self.container,
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
        self.interaction_streams.put(&mut self.container, record)
    }

    pub(crate) fn interaction_streams_for_session(
        &self,
        session_id: &str,
    ) -> Vec<crate::InteractionStreamRecord> {
        self.interaction_streams.records_for_session(session_id)
    }

    pub(crate) fn interaction_stream_records(&self) -> Vec<crate::InteractionStreamRecord> {
        self.interaction_streams.all_records()
    }

    pub fn put_echo_event(&mut self, event: crate::EchoEvent) -> Result<bool, CvaError> {
        Ok(self.echo.put(&mut self.container, event)?)
    }

    pub fn echo_events(&self, conversation_id: &str, message_id: &str) -> Vec<crate::EchoEvent> {
        self.echo.for_turn(conversation_id, message_id)
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
        self.project_history.put(
            &mut self.container,
            rel_cut,
            project_revision,
            repository_management,
        )
    }

    pub fn project_revision_correlations(&self) -> Vec<crate::ProjectRevisionCorrelation> {
        self.project_history.records()
    }

    pub fn latest_project_revision_correlation(&self) -> Option<crate::ProjectRevisionCorrelation> {
        self.project_history.latest()
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
        self.project_files
            .put(&mut self.container, file.id, reference)?;
        self.archive
            .register_file(&mut self.container, file.clone())?;
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
        self.project_files
            .put(&mut self.container, file.id, reference)
    }

    pub fn project_file_ref(&self, file_id: crate::FileId) -> Option<crate::ProjectFileRef> {
        self.project_files.get(file_id).cloned()
    }

    pub fn sync(&self) -> Result<(), CvaError> {
        self.container.sync()?;
        Ok(())
    }
}
