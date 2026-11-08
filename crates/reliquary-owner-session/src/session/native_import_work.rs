//! Imported Episode claim and scoped publication.
use super::*;
use reliquary_archive::EpisodeOrigin;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug)]
pub struct InsomniaEpisodeStatus {
    pub id: reliquary_archive::EpisodeId,
    pub state: reliquary_insomnia_store::InsomniaWorkState,
    pub due: bool,
    pub rel_memories: usize,
    pub phy_memories: usize,
    pub rejected: u32,
}

impl RelSession {
    pub fn validate_import_processing_binding(
        &self,
        import_id: &str,
        model: &str,
        destination: Option<[u8; 16]>,
        mut checkpoint_matches: impl FnMut(&[u8]) -> bool,
    ) -> Result<(), OwnerSessionError> {
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
    ) -> bool {
        self.imports
            .open_epoch()
            .filter(|epoch| epoch.import_id == import_id)
            .and_then(|epoch| super::decode_import_key(&epoch.import_key))
            .and_then(|key| self.native_import_receipt(&key))
            .and_then(|receipt| {
                receipt
                    .conversations
                    .iter()
                    .find(|entry| entry.conversation_id == conversation)
            })
            .is_some_and(|entry| {
                entry
                    .messages
                    .iter()
                    .any(|message| !message.deleted && message.node_id.as_deref() == Some(node))
            })
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
            Some(
                self.native_import_receipt(&key)
                    .map(mapped_source_nodes)
                    .unwrap_or_default(),
            )
        } else {
            None
        };
        let mut episodes = self.archive.episodes();
        episodes.sort_unstable_by_key(|episode| (episode.source_through_ns, episode.id));
        for episode in episodes {
            let matches = match &mapped {
                None => episode.origin == EpisodeOrigin::Live,
                Some(mapped) => {
                    episode.origin == EpisodeOrigin::Import
                        && mapped.contains(&(
                            episode.conversation_id.as_str(),
                            episode.start_node_id.as_str(),
                        ))
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
        let receipt = self.session.archive.import_receipt(&key).ok_or_else(|| {
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
        let mapped = mapped_source_nodes(receipt);
        if !source_path_is_mapped(&mapped, &episode.conversation_id, &turns) {
            return Err(OwnerSessionError::Record(
                "import Episode path escapes its receipt source set".into(),
            ));
        }
        Ok(())
    }
}

pub(super) fn mapped_source_nodes(receipt: &crate::model::ImportReceipt) -> BTreeSet<(&str, &str)> {
    receipt
        .conversations
        .iter()
        .flat_map(|conversation| {
            conversation.messages.iter().filter_map(|message| {
                Some((
                    conversation.conversation_id.as_str(),
                    message.node_id.as_deref()?,
                ))
            })
        })
        .collect()
}

pub(super) fn source_path_is_mapped(
    mapped: &BTreeSet<(&str, &str)>,
    conversation: &str,
    turns: &[reliquary_archive::ResolvedTurn],
) -> bool {
    !turns.is_empty()
        && turns
            .iter()
            .all(|turn| mapped.contains(&(conversation, turn.node_id.as_str())))
}
