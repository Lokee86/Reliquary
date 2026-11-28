//! Imported Episode claim and scoped publication.
use super::*;
use reliquary_archive::EpisodeOrigin;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct InsomniaEpisodeStatus {
    pub id: reliquary_archive::EpisodeId,
    pub state: reliquary_insomnia_store::InsomniaWorkState,
    pub due: bool,
    pub rel_memories: usize,
    pub phy_memories: usize,
    pub rejected: u32,
    pub last_error: Option<String>,
}

pub(in crate::session) struct ActiveImportSource {
    key: [u8; 32],
    digest: [u8; 32],
    nodes: Arc<ImportSourceMap>,
}

pub(in crate::session) struct ImportSourceMap(BTreeMap<String, BTreeMap<String, bool>>);

impl ImportSourceMap {
    pub(in crate::session) fn from_receipt(receipt: &crate::model::ImportReceipt) -> Self {
        let mut mapped = BTreeMap::<String, BTreeMap<String, bool>>::new();
        for conversation in &receipt.conversations {
            let nodes = mapped
                .entry(conversation.conversation_id.clone())
                .or_default();
            for message in &conversation.messages {
                if let Some(node) = &message.node_id {
                    *nodes.entry(node.clone()).or_default() |= !message.deleted;
                }
            }
        }
        Self(mapped)
    }

    pub(in crate::session) fn contains(&self, conversation: &str, node: &str) -> bool {
        self.0
            .get(conversation)
            .is_some_and(|nodes| nodes.contains_key(node))
    }

    fn contains_live(&self, conversation: &str, node: &str) -> bool {
        self.0
            .get(conversation)
            .and_then(|nodes| nodes.get(node))
            .copied()
            .unwrap_or(false)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().flat_map(|(conversation, nodes)| {
            nodes
                .keys()
                .map(|node| (conversation.as_str(), node.as_str()))
        })
    }
}

impl RelSession {
    // Historic lookups never replace the one cache for the current epoch.
    pub(super) fn native_import_sources(
        &self,
        key: &[u8; 32],
    ) -> Result<Option<Arc<ImportSourceMap>>, OwnerSessionError> {
        let Some(digest) = self.native_import_receipt_digest(key) else {
            return Ok(None);
        };
        if self
            .imports
            .open_epoch()
            .and_then(|start| super::decode_import_key(&start.import_key))
            .as_ref()
            != Some(key)
        {
            return Ok(self
                .native_import_receipt(key)?
                .map(|receipt| Arc::new(ImportSourceMap::from_receipt(receipt.as_ref()))));
        }
        let mut cached = self.active_import_source.lock().map_err(|_| {
            OwnerSessionError::Record("active import source cache is unavailable".into())
        })?;
        if let Some(source) = cached
            .as_ref()
            .filter(|source| source.key == *key && source.digest == digest)
        {
            return Ok(Some(Arc::clone(&source.nodes)));
        }
        let receipt = self.native_import_receipt(key)?.ok_or_else(|| {
            OwnerSessionError::Record("accepted Archive import receipt is unavailable".into())
        })?;
        let nodes = Arc::new(ImportSourceMap::from_receipt(receipt.as_ref()));
        *cached = Some(ActiveImportSource {
            key: *key,
            digest,
            nodes: Arc::clone(&nodes),
        });
        Ok(Some(nodes))
    }

    pub(super) fn clear_active_import_source(&mut self) {
        // An exclusive owner write ends the epoch and discards its derived map.
        let entry = self
            .active_import_source
            .get_mut()
            .unwrap_or_else(|error| error.into_inner());
        *entry = None;
        self.active_import_source.clear_poison();
    }

    pub fn validate_import_processing_binding(
        &self,
        import_id: &str,
        model: &str,
        destination: Option<[u8; 16]>,
        mut checkpoint_matches: impl FnMut(&[u8]) -> bool,
    ) -> Result<(), OwnerSessionError> {
        self.validate_copied_import_destination(import_id, destination)?;
        let destination = destination.map(|owner| {
            owner
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        });
        for (key, completion) in &self.grouped_completions {
            if self
                .native_import_grouped_epochs
                .get(key)
                .is_none_or(|id| id != import_id)
            {
                continue;
            }
            if completion.extractor_model != model
                || completion
                    .external_memory_refs
                    .iter()
                    .any(|reference| destination.as_deref() != Some(reference.owner_id.as_str()))
            {
                return Err(OwnerSessionError::ManagementConflict);
            }
        }
        let mut matches = true;
        self.scoped_insomnia_status(Some(import_id), 0, |status| {
            if matches {
                let work = self.insomnia.work(WorkEpisodeId(status.id.0));
                matches = work
                    .and_then(|work| work.held_payload.as_deref())
                    .is_none_or(&mut checkpoint_matches);
            }
        })?;
        if matches {
            Ok(())
        } else {
            Err(OwnerSessionError::ManagementConflict)
        }
    }

    pub fn native_import_source_contains(
        &self,
        import_id: &str,
        conversation: &str,
        node: &str,
    ) -> Result<bool, OwnerSessionError> {
        let Some(key) = self
            .imports
            .open_epoch()
            .filter(|epoch| epoch.import_id == import_id)
            .and_then(|epoch| super::decode_import_key(&epoch.import_key))
        else {
            return Ok(false);
        };
        Ok(self
            .native_import_sources(&key)?
            .is_some_and(|nodes| nodes.contains_live(conversation, node)))
    }

    pub fn scoped_insomnia_status(
        &self,
        import_id: Option<&str>,
        now_ns: i64,
        mut visit: impl FnMut(InsomniaEpisodeStatus),
    ) -> Result<(), OwnerSessionError> {
        use crate::records::ImportEpochStatus;
        use reliquary_insomnia_store::InsomniaWorkState as State;
        let mapped = if let Some(id) = import_id {
            let start = match self.import_status(id) {
                Some(
                    ImportEpochStatus::Open(start) | ImportEpochStatus::Completed { start, .. },
                ) => start,
                None => return Err(OwnerSessionError::Record("unknown import ID".into())),
            };
            let key = super::decode_import_key(&start.import_key)
                .ok_or_else(|| OwnerSessionError::Record("invalid import source key".into()))?;
            self.native_import_sources(&key)?
        } else {
            None
        };
        let mut episodes = self.archive.episodes();
        episodes.sort_unstable_by_key(|episode| (episode.source_through_ns, episode.id));
        for episode in episodes {
            let matches = match &mapped {
                None => import_id.is_none() && episode.origin == EpisodeOrigin::Live,
                Some(mapped) => {
                    episode.origin == EpisodeOrigin::Import
                        && mapped.contains(&episode.conversation_id, &episode.start_node_id)
                }
            };
            if !matches {
                continue;
            }
            let work_id = WorkEpisodeId(episode.id.0);
            let work = self.insomnia.work(work_id);
            let receipt = work.and_then(|work| self.insomnia.receipt(work_id, work.attempt_count));
            let state = work.map_or(State::Pending, |work| work.state);
            let due = match (state, work) {
                (State::Pending, _) => true,
                (State::Processing, Some(work)) => {
                    work.lease_expires_ns.is_some_and(|t| t <= now_ns)
                }
                (State::Failed, Some(work)) => work.retry_after_ns.is_none_or(|t| t <= now_ns),
                _ => false,
            };
            visit(InsomniaEpisodeStatus {
                id: episode.id,
                state,
                due,
                rel_memories: receipt.map_or(0, |r| r.memory_ids.len()),
                phy_memories: receipt.map_or(0, |r| r.external_memory_refs.len()),
                rejected: receipt.map_or(0, |r| r.rejected_count),
                last_error: work.and_then(|work| work.last_error.clone()),
            });
        }
        Ok(())
    }
}

impl RelWriter {
    /// Claim a specific source-mapped import Episode while ordinary work
    /// remains fenced. Inference runs after the owner writer lock is released.
    pub fn claim_native_import_episode(
        &mut self,
        import_id: &str,
        episode_id: reliquary_archive::EpisodeId,
        worker: &str,
        now_ns: i64,
        lease_duration_ns: i64,
    ) -> Result<RelInsomniaClaimInput, OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        self.validate_import_episode_identity(import_id, episode_id)?;
        let result = self.session.insomnia.claim_episode(
            &mut self.container,
            WorkEpisodeId(episode_id.0),
            worker,
            now_ns,
            lease_duration_ns,
        );
        let work = self.finish_insomnia_write(result)?;
        let episode = self.session.archive.episode(episode_id).cloned().ok_or(
            OwnerSessionError::Insomnia(InsomniaError::UnacceptedEpisode),
        )?;
        let turns = self.session.archive_evidence_episode(episode_id)?.ok_or(
            OwnerSessionError::Insomnia(InsomniaError::UnacceptedEpisode),
        )?;
        Ok(RelInsomniaClaimInput {
            owner_id: self.session.owner_id,
            work,
            episode,
            turns,
            started_at_ns: now_ns,
        })
    }

    pub fn commit_native_import_completion_with_routed_receipts(
        &mut self,
        import_id: &str,
        token: InsomniaLeaseToken,
        completion: GroupedInsomniaCompletion,
        now_ns: i64,
        receipts: &[RoutedUserMemoryReceipt],
    ) -> Result<CompletionCheck, OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        self.validate_import_episode_identity(
            import_id,
            reliquary_archive::EpisodeId(completion.episode_id),
        )?;
        let key = (WorkEpisodeId(completion.episode_id), completion.attempt);
        let result = self.session.commit_scoped_grouped_completion(
            &mut self.container,
            token,
            completion,
            now_ns,
            receipts,
            Some(import_id),
        )?;
        if result == CompletionCheck::Apply {
            self.session
                .native_import_grouped_epochs
                .insert(key, import_id.to_owned());
        }
        Ok(result)
    }

    pub fn validate_native_import_claim(
        &self,
        import_id: &str,
        claim: &RelInsomniaClaimInput,
        now_ns: i64,
    ) -> Result<(), OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        self.validate_import_episode_identity(import_id, claim.episode.id)?;
        self.validate_scoped_insomnia_claim(claim, now_ns)
    }

    pub fn checkpoint_native_import_claim(
        &mut self,
        import_id: &str,
        claim: &RelInsomniaClaimInput,
        now_ns: i64,
        payload: Vec<u8>,
    ) -> Result<InsomniaWork, OwnerSessionError> {
        self.validate_native_import_claim(import_id, claim, now_ns)?;
        self.checkpoint_scoped_insomnia_claim(claim, now_ns, payload)
    }

    pub fn hold_native_import_claim(
        &mut self,
        import_id: &str,
        claim: &RelInsomniaClaimInput,
        now_ns: i64,
        reason: String,
        payload: Option<Vec<u8>>,
    ) -> Result<InsomniaWork, OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        self.validate_import_episode_identity(import_id, claim.episode.id)?;
        if claim.owner_id != self.owner_id() || claim.work.episode_id.0 != claim.episode.id.0 {
            return Err(OwnerSessionError::Insomnia(
                InsomniaError::UnacceptedEpisode,
            ));
        }
        let token = claim
            .work
            .lease_token
            .ok_or(OwnerSessionError::Insomnia(InsomniaError::InvalidLease))?;
        let result = self.session.insomnia.hold_unreconciled_attempt(
            &mut self.container,
            claim.work.episode_id,
            token,
            now_ns,
            reason,
            payload,
        );
        self.finish_insomnia_write(result)
    }

    pub fn retry_held_import_episode(
        &mut self,
        import_id: &str,
        episode: reliquary_archive::EpisodeId,
        now_ns: i64,
    ) -> Result<(), OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        self.validate_import_episode_identity(import_id, episode)?;
        let result =
            self.session
                .insomnia
                .retry_held(&mut self.container, WorkEpisodeId(episode.0), now_ns);
        self.finish_insomnia_write(result).map(|_| ())
    }

    /// Check that an Episode belongs to this open import before any routed publication.
    pub fn validate_import_episode_identity(
        &self,
        import_id: &str,
        id: reliquary_archive::EpisodeId,
    ) -> Result<(), OwnerSessionError> {
        let start = self
            .session
            .imports
            .open_epoch()
            .filter(|epoch| epoch.import_id == import_id)
            .ok_or_else(|| OwnerSessionError::Record("import episode has no open epoch".into()))?;
        let key = super::decode_import_key(&start.import_key)
            .ok_or_else(|| OwnerSessionError::Record("invalid import source key".into()))?;
        let mapped = self.session.native_import_sources(&key)?.ok_or_else(|| {
            OwnerSessionError::Record("import work requires a durable source receipt".into())
        })?;
        let episode = self
            .session
            .archive
            .episode(id)
            .ok_or(OwnerSessionError::Insomnia(
                InsomniaError::UnacceptedEpisode,
            ))?;
        let work = self
            .session
            .insomnia
            .work(WorkEpisodeId(id.0))
            .ok_or(OwnerSessionError::Insomnia(InsomniaError::MissingWork))?;
        if episode.origin != EpisodeOrigin::Import || work.priority != InsomniaPriority::Import {
            return Err(OwnerSessionError::Record(
                "episode is not import-priority source work".into(),
            ));
        }
        let turns = self
            .session
            .archive
            .episode_turns(id)?
            .ok_or(OwnerSessionError::Insomnia(
                InsomniaError::UnacceptedEpisode,
            ))?;
        if !source_path_is_mapped(&mapped, &episode.conversation_id, &turns) {
            return Err(OwnerSessionError::Record(
                "import Episode path escapes its receipt source set".into(),
            ));
        }
        Ok(())
    }
}

pub(super) fn source_path_is_mapped(
    mapped: &ImportSourceMap,
    conversation: &str,
    turns: &[reliquary_archive::ResolvedTurn],
) -> bool {
    !turns.is_empty()
        && turns
            .iter()
            .all(|turn| mapped.contains(conversation, &turn.node_id))
}
