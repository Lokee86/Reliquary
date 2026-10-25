//! Canonical runtime execution for small public facade entrypoints.
use crate::{EmbeddingEndpoint, EmbeddingMode, MemoryVectorError, RelExecutionOwner};

impl RelExecutionOwner {
    pub(crate) fn runtime_memory_lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::lexical_index::MemoryLexicalHit>, crate::MemoryError> {
        let memory_read = self.owner.memory_read_view();
        let mut view =
            crate::lexical_search::MemoryLexicalReadView::new(&mut self.lexical_index, memory_read);
        crate::lexical_search::memory_lexical_candidates(&mut view, query, limit)
    }

    pub(crate) fn runtime_current_dream_begin_payload(
        &mut self,
        pass: &crate::InitialDreamPass,
    ) -> Result<Vec<u8>, crate::CvaError> {
        crate::owner_lifecycle::dream_begin_transport::current_dream_begin_payload(
            &self.container,
            &self.archive,
            &self.memories,
            pass,
        )
    }
    pub(crate) fn runtime_community_stats(&self) -> crate::CommunityStats {
        self.owner.community_stats()
    }

    pub(crate) fn runtime_memory_graph_version(&self) -> u64 {
        self.owner.memory_graph_version()
    }

    pub(crate) fn runtime_insomnia_stats(&self) -> crate::InsomniaStats {
        self.owner.insomnia_stats()
    }

    pub(crate) fn runtime_ego_version(&self) -> u64 {
        self.owner.ego_version()
    }

    pub(crate) fn runtime_conversation_turns(
        &mut self,
        conversation: &str,
        leaf: &str,
    ) -> Result<Vec<crate::ResolvedTurn>, crate::ArchiveError> {
        self.owner.conversation_turns(conversation, leaf)
    }

    pub(crate) fn runtime_conversation_branch_start_node_ids(
        &self,
        conversation: &str,
        leaf: &str,
    ) -> Result<Vec<String>, crate::ArchiveError> {
        self.owner
            .conversation_branch_start_node_ids(conversation, leaf)
    }

    pub(crate) fn runtime_entity_version(&self) -> u64 {
        crate::perception_entity_merge_runtime::entity_version_rel(self)
    }

    pub(crate) fn runtime_record_accepted_memory_use(
        &mut self,
        receipt: crate::AcceptedMemoryUseReceipt,
        workers: usize,
    ) -> Result<usize, crate::CvaError> {
        crate::freshness_operations_runtime::record_accepted_memory_use(
            self,
            receipt.owner_uuid,
            receipt.use_id,
            receipt.accepted_turn,
            receipt.memories,
            workers,
        )
    }

    pub(crate) fn runtime_ingest_turn(
        &mut self,
        turn: crate::IncomingTurn,
    ) -> Result<crate::IngestedTurn, crate::ArchiveError> {
        self.accept_interaction_turn(turn)
            .map(|receipt| receipt.value)
    }

    pub(crate) fn runtime_link_file_to_memory(
        &mut self,
        file_id: crate::FileId,
        memory_id: crate::MemoryId,
    ) -> Result<crate::FileMemoryLink, crate::ArchiveError> {
        self.owner.link_file_to_memory(file_id, memory_id)
    }

    pub(crate) fn runtime_semantic_memory_targets(
        &self,
        profile_id: crate::CompatibilityProfileId,
    ) -> Result<
        Vec<crate::owner_lifecycle::public_consumer_operations::SemanticMemoryTarget>,
        crate::MemoryError,
    > {
        crate::owner_lifecycle::public_consumer_operations::semantic_memory_targets(
            &self.owner.memories,
            &self.owner.memory_vectors,
            profile_id,
        )
    }

    pub(crate) fn runtime_rebuild_derived_vectors(
        &mut self,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<(crate::CompatibilityProfileId, usize, bool), crate::CvaError> {
        let candidate = crate::compatibility_profile_probe::profile_from_endpoint(endpoint)?;
        let profile = crate::owner_lifecycle::vector_publication::accept_profile(
            &mut self.owner.container,
            &mut self.owner.compatibility_profiles,
            candidate,
        )?;
        let batch = crate::vector_publication::prepare_batch(
            &crate::memory_store::MemoryReadView::new(&self.owner.memories, &self.owner.container),
            &self.owner.compatibility_profiles,
            &self.owner.memory_vectors,
            profile.id,
        )?;
        let memory_vectors_embedded = match batch {
            Some(batch) => {
                let vectors = endpoint
                    .embed(EmbeddingMode::Document, &batch.texts)
                    .map_err(MemoryVectorError::from)?;
                let plan = crate::runtime_vector_step_policy::publication_plan(
                    &self.owner.memories,
                    &self.owner.compatibility_profiles,
                    &self.owner.memory_vectors,
                    batch,
                    vectors,
                )?;
                match plan {
                    Some(plan) => {
                        crate::owner_lifecycle::vector_publication::publish_memory_vectors(
                            &mut self.owner.container,
                            &self.owner.compatibility_profiles,
                            &mut self.owner.packed_vectors,
                            &mut self.owner.memory_vectors,
                            plan,
                        )?
                    }
                    None => 0,
                }
            }
            None => 0,
        };
        let archive_generation_rebuilt = if self.owner.archive.fragments().is_empty() {
            false
        } else {
            let archive_version = self.owner.archive.archive_version();
            let rebuild = self
                .runtime_current_vector_generation(profile.id)
                .is_none_or(|generation| generation.source_archive_version != archive_version);
            if rebuild {
                self.public_build_archive_vector_generation_verified(profile.clone(), endpoint)?;
            }
            rebuild
        };
        self.sync_owner()?;
        Ok((
            profile.id,
            memory_vectors_embedded,
            archive_generation_rebuilt,
        ))
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn runtime_memory_lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::lexical_index::MemoryLexicalHit>, crate::MemoryError> {
        let memory_read = self.owner.memory_read_view();
        let mut view =
            crate::lexical_search::MemoryLexicalReadView::new(&mut self.lexical_index, memory_read);
        crate::lexical_search::memory_lexical_candidates(&mut view, query, limit)
    }
    pub(crate) fn runtime_community_snapshot(&self) -> Option<crate::CommunitySnapshot> {
        self.owner.community_snapshot()
    }

    pub(crate) fn runtime_community_semantic_names(&self) -> Vec<crate::CommunitySemanticName> {
        self.owner.community_semantic_names()
    }

    pub(crate) fn runtime_community_stats(&self) -> crate::CommunityStats {
        self.owner.native_community_stats()
    }

    pub(crate) fn runtime_community_semantic_name(
        &self,
        community_id: crate::CommunityId,
    ) -> Option<crate::CommunitySemanticName> {
        self.owner.native_community_semantic_name(community_id)
    }

    pub(crate) fn runtime_community_lineage(&self) -> Option<crate::CommunityLineageTransition> {
        self.owner.native_community_lineage()
    }

    pub(crate) fn runtime_publish_community_semantic_name(
        &mut self,
        record: crate::CommunitySemanticName,
    ) -> Result<bool, crate::CommunityError> {
        self.owner.native_publish_community_semantic_name(record)
    }

    pub(crate) fn runtime_refresh_communities_leiden(
        &mut self,
    ) -> Result<crate::CommunitySnapshot, crate::CommunityError> {
        self.owner.native_refresh_communities_leiden()
    }

    pub(crate) fn runtime_semantic_graph_relations(&self) -> Vec<crate::SemanticGraphRelation> {
        self.owner.semantic_graph_relations()
    }

    pub(crate) fn runtime_entities(&self) -> Vec<crate::Entity> {
        crate::perception_entity_merge_runtime::entities_phy(self)
    }

    pub(crate) fn runtime_entity_stats(&self) -> crate::EntityStats {
        crate::perception_entity_merge_runtime::entity_stats_phy(self)
    }

    pub(crate) fn runtime_entity_version(&self) -> u64 {
        crate::perception_entity_merge_runtime::entity_version_phy(self)
    }
}

impl RelExecutionOwner {
    pub(crate) fn runtime_import_native(
        &mut self,
        input: &crate::imports::NativeImport,
        now_ns: i64,
    ) -> Result<crate::imports::ImportReceipt, crate::imports::ImportError> {
        let watermarks =
            crate::owner_lifecycle::import_reconciliation_restore::import_watermarks(&self.owner);
        let mut activity = Vec::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::imports::native_commit::import_native(
                crate::owner_lifecycle::native_import::parts(&mut self.owner),
                input,
                now_ns,
                watermarks,
                &mut activity,
            )
        }));
        for turn in activity {
            self.freshness_on_accepted_activity(turn);
        }
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(crate) fn runtime_import_epoch_status(
        &self,
        import_id: &str,
    ) -> Option<crate::imports::ImportEpochStatus> {
        self.owner.import_epochs.status(import_id)
    }

    pub(crate) fn runtime_import_epoch_statuses(&self) -> Vec<crate::imports::ImportEpochStatus> {
        self.owner.import_epochs.statuses()
    }

    pub(crate) fn runtime_native_import_readiness(
        &mut self,
        import_id: &str,
    ) -> Result<crate::imports::ImportReadinessReport, crate::owner_lifecycle::OwnerError> {
        crate::imports::owner_completion::readiness(&mut self.owner, import_id)
    }
}

macro_rules! lineage_read_bindings {
    ($runtime:ty) => {
        impl $runtime {
            pub(crate) fn runtime_history_generation(&self) -> crate::HistoryGenerationId {
                self.owner.owner_history_generation()
            }
            pub(crate) fn runtime_rewrite_genesis(&self) -> Option<&crate::RewriteGenesis> {
                self.owner.owner_rewrite_genesis()
            }
        }
    };
}
lineage_read_bindings!(crate::RelExecutionOwner);
lineage_read_bindings!(crate::phy_execution_owner::PhyExecutionOwner);
