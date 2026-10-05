use super::*;
use crate::EmbeddingMode;

impl GatewayRuntime {
    /// Enumerate only mounted RELs explicitly granted to this attachment.
    pub fn viewable_rel_owners(
        &self,
        instance: &InstanceRuntime,
    ) -> Result<Vec<String>, ReliquaryRuntimeHostError> {
        let authorization = instance.authorization().map_err(operation)?;
        self.with_instance_delivery(instance, authorization, || {
            let grants = instance.grants_snapshot().map_err(operation)?;
            let mounted = self.mounted_rel_ids()?;
            Ok(mounted
                .into_iter()
                .filter(|owner| {
                    grants
                        .get(owner)
                        .is_some_and(|caps| caps.contains(&OwnerGrant::ViewRel))
                })
                .collect())
        })
    }

    pub fn conversation_transcript_page(
        &self,
        instance: &InstanceRuntime,
        conversation_id: &str,
        leaf_node_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<ResolvedInteractionTurn>, ReliquaryRuntimeHostError> {
        if limit == 0 {
            return Err(operation("transcript page size must be positive"));
        }
        Ok(self
            .conversation_transcript(instance, conversation_id, leaf_node_id)?
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect())
    }

    pub fn conversation_transcript_page_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        leaf_node_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<ResolvedInteractionTurn>, ReliquaryRuntimeHostError> {
        if limit == 0 {
            return Err(operation("transcript page size must be positive"));
        }
        Ok(self
            .conversation_transcript_for(instance, owner_id, conversation_id, leaf_node_id)?
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect())
    }

    /// Search each owner in the authorized dependency closure separately. The
    /// closure filters grants before any dependent owner is queried.
    pub fn search_dependency_archive_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, Vec<ArchiveSearchHit>)>, ReliquaryRuntimeHostError> {
        self.dependency_closure_for(instance, owner_id)?
            .into_iter()
            .map(|owner| {
                let hits = self.search_archive_for(instance, &owner, query, limit)?;
                Ok((owner, hits))
            })
            .collect()
    }

    pub fn search_dependency_archive(
        &self,
        instance: &InstanceRuntime,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, Vec<ArchiveSearchHit>)>, ReliquaryRuntimeHostError> {
        let owner_id = self
            .active_rel_id(instance)?
            .ok_or_else(|| operation("instance has no selected REL"))?;
        self.search_dependency_archive_for(instance, &owner_id, query, limit)
    }

    /// Paginate the summaries visible from the selected REL.
    pub fn conversation_summaries_page(
        &self,
        instance: &InstanceRuntime,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<ConversationSummary>, ReliquaryRuntimeHostError> {
        if limit == 0 {
            return Err(operation("conversation summary page size must be positive"));
        }
        let mut summaries = self.conversation_summaries(instance)?;
        summaries.sort_by(|a, b| a.conversation_id.cmp(&b.conversation_id));
        Ok(summaries.into_iter().skip(offset).take(limit).collect())
    }

    /// Paginate within the canonical top-ten archive search result window.
    pub fn search_archive_page(
        &self,
        instance: &InstanceRuntime,
        query: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<ArchiveSearchHit>, ReliquaryRuntimeHostError> {
        if limit == 0 {
            return Err(operation("archive search page size must be positive"));
        }
        let hits = self.search_archive(instance, query, crate::MAX_ARCHIVE_SEARCH_RESULTS)?;
        Ok(hits.into_iter().skip(offset).take(limit).collect())
    }

    /// Replace a Memory through the canonical revision-checked publication path.
    pub fn replace_knowledge_memory_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        memory_id: MemoryId,
        expected_revision: u64,
        title: String,
        content: String,
        now_ns: i64,
    ) -> Result<Memory, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageRel)?;
        let mut draft =
            super::user_creation_draft(title, content, "knowledge".into(), "fact".into(), now_ns);
        draft.mutation_id = format!("user_replace:{}", Uuid::new_v4());
        let updated = self.with_rel(&token, |runtime| {
            let (memory, _) = runtime
                .cva
                .publish_memory(Some(memory_id), expected_revision, draft)
                .map_err(operation)?;
            runtime.cva.sync().map_err(operation)?;
            Ok(memory)
        })?;
        self.wake_rel(owner_id)?;
        Ok(updated)
    }

    /// Embed outside the owner store lock, then revalidate the attachment grant
    /// at the retrieval boundary.
    pub fn search_memories_by_text_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        profile: crate::CompatibilityProfileId,
        query: &str,
        config: crate::MemoryRetrievalConfig,
    ) -> Result<crate::MemorySearchLane, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        let query = query.trim();
        if query.is_empty() || query.len() > crate::MAX_MEMORY_SEARCH_QUERY_BYTES {
            return Err(operation(format!(
                "memory search requires a query of at most {} bytes",
                crate::MAX_MEMORY_SEARCH_QUERY_BYTES
            )));
        }
        let endpoint = self
            .routes
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .embedding()
            .ok_or_else(|| operation("memory search requires an embedding route"))?;
        let vectors = endpoint
            .embed(EmbeddingMode::Query, &[query.to_owned()])
            .map_err(operation)?;
        let vector = vectors
            .into_iter()
            .next()
            .ok_or_else(|| operation("embedding route returned no query vector"))?;
        // Reuse the original token after provider I/O. A grant replacement or
        // detach during embedding must not silently start a new authorization.
        let lane = self.retrieve_reliquary_memories_with_token(&token, profile, &vector, config)?;
        self.deliver_rel(instance, &token, lane)
    }

    pub fn search_memories_by_text(
        &self,
        instance: &InstanceRuntime,
        profile: crate::CompatibilityProfileId,
        query: &str,
        config: crate::MemoryRetrievalConfig,
    ) -> Result<crate::MemorySearchLane, ReliquaryRuntimeHostError> {
        let owner_id = self
            .active_rel_id(instance)?
            .ok_or_else(|| operation("instance has no selected REL"))?;
        self.search_memories_by_text_for(instance, &owner_id, profile, query, config)
    }

    pub fn retrieve_reliquary_memories(
        &self,
        instance: &InstanceRuntime,
        profile: crate::CompatibilityProfileId,
        query_vector: &[f32],
        config: crate::MemoryRetrievalConfig,
    ) -> Result<crate::MemorySearchLane, ReliquaryRuntimeHostError> {
        let owner_id = self
            .active_rel_id(instance)?
            .ok_or_else(|| operation("instance has no selected REL"))?;
        self.retrieve_reliquary_memories_for(instance, &owner_id, profile, query_vector, config)
    }

    pub fn set_rel_processing_enabled_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        enabled: bool,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageRel)?;
        let attachment = self.attachment_entry(instance)?;
        let _attachment_lease = attachment.leases.acquire_current()?;
        attachment
            .instance
            .authorize(owner_id, OwnerGrant::ManageRel, token.authorization)
            .map_err(operation)?;
        let rels = self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let rel = rels
            .get(owner_id)
            .ok_or_else(|| operation("REL owner is not mounted"))?;
        if rel.mount_epoch.load(std::sync::atomic::Ordering::SeqCst) != token.mount_epoch {
            return Err(operation("owner token belongs to a previous REL mount"));
        }
        let _owner_lease = rel.fence.acquire(token.fence_generation)?;
        let owner = Arc::clone(&rel.owner);
        drop(rels);
        let execution = owner
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        attachment
            .instance
            .authorize(owner_id, OwnerGrant::ManageRel, token.authorization)
            .map_err(operation)?;
        execution
            .insomnia_enabled
            .store(enabled, std::sync::atomic::Ordering::SeqCst);
        let signal = Arc::clone(&execution.signal);
        drop(execution);
        crate::runtime_host::notify_work(&signal)
    }

    pub fn quiesce_rel_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageRel)?;
        // Lifecycle first, then attachment lease: detach/regrant cannot race the
        // grant check and the drain, and a lease is never held while waiting.
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if self.shutting_down.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let attachment = self.attachment_entry(instance)?;
        let _attachment_lease = attachment.leases.acquire_current()?;
        attachment
            .instance
            .authorize(owner_id, OwnerGrant::ManageRel, token.authorization)
            .map_err(operation)?;
        let (fence, owner, sequencer) = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let rel = rels
                .get(owner_id)
                .ok_or_else(|| operation("REL owner is not mounted"))?;
            if rel.mount_epoch.load(std::sync::atomic::Ordering::SeqCst) != token.mount_epoch {
                return Err(operation("owner token belongs to a previous REL mount"));
            }
            if rel.fence.snapshot()?.1 != token.fence_generation {
                return Err(operation(
                    "owner token belongs to a previous fence generation",
                ));
            }
            (
                Arc::clone(&rel.fence),
                Arc::clone(&rel.owner),
                Arc::clone(&rel.sequencer),
            )
        };
        fence.close_and_drain()?;
        sequencer.close()?;
        owner
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .stop_workers()?;
        attachment
            .instance
            .authorize(owner_id, OwnerGrant::ManageRel, token.authorization)
            .map_err(operation)?;
        Ok(())
    }

    pub fn reopen_rel_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<u64, ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if self.shutting_down.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let attachment = self.attachment_entry(instance)?;
        let _attachment_lease = attachment.leases.acquire_current()?;
        let authorization = attachment.instance.authorization().map_err(operation)?;
        attachment
            .instance
            .authorize(owner_id, OwnerGrant::ManageRel, authorization)
            .map_err(operation)?;
        let (fence, owner, sequencer) = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let rel = rels
                .get(owner_id)
                .ok_or_else(|| operation("REL owner is not mounted"))?;
            (
                Arc::clone(&rel.fence),
                Arc::clone(&rel.owner),
                Arc::clone(&rel.sequencer),
            )
        };
        if fence.snapshot()?.0 != super::super::OwnerAdmission::Closed {
            return Err(operation("REL must be quiesced before it can be reopened"));
        }
        // Preflight before reconciliation or restarting workers: an exhausted
        // fence must leave this owner fully closed and stopped.
        fence.validate_reopen()?;
        self.reconcile_rel(owner_id)?;
        let epoch = self.allocate_mount_epoch()?;
        {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let rel = rels
                .get(owner_id)
                .ok_or_else(|| operation("REL owner is not mounted"))?;
            rel.mount_epoch
                .store(epoch, std::sync::atomic::Ordering::SeqCst);
        }
        attachment
            .instance
            .authorize(owner_id, OwnerGrant::ManageRel, authorization)
            .map_err(operation)?;
        sequencer.reset_epoch(epoch)?;
        let (generation, restarted) = {
            let mut execution = owner
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let generation = fence.reopen()?;
            (generation, execution.restart_workers())
        };
        if let Err(error) = restarted {
            sequencer.close()?;
            fence.close_and_drain()?;
            owner
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .stop_workers()?;
            return Err(error);
        }
        Ok(generation)
    }

    /// Reconcile coordinator-owned conversation/archive state only through the
    /// established coordinator lifecycle seam while the REL is quiesced.
    pub fn reconcile_quiesced_rel_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let authorization = instance.authorization().map_err(operation)?;
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if self.shutting_down.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let attachment = self.attachment_entry(instance)?;
        let _attachment_lease = attachment.leases.acquire_current()?;
        attachment
            .instance
            .authorize(owner_id, OwnerGrant::ManageRel, authorization)
            .map_err(operation)?;
        let (fence, owner) = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let rel = rels
                .get(owner_id)
                .ok_or_else(|| operation("REL owner is not mounted"))?;
            (Arc::clone(&rel.fence), Arc::clone(&rel.owner))
        };
        let (admission, _, active) = fence.snapshot_full()?;
        if admission != super::super::OwnerAdmission::Closed || active != 0 {
            return Err(operation(
                "coordinator reconciliation requires a drained, quiesced REL",
            ));
        }
        if !owner
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .workers_stopped()
        {
            return Err(operation(
                "REL workers must be stopped before coordinator reconciliation",
            ));
        }
        attachment
            .instance
            .authorize(owner_id, OwnerGrant::ManageRel, authorization)
            .map_err(operation)?;
        self.reconcile_rel(owner_id)
    }
}
