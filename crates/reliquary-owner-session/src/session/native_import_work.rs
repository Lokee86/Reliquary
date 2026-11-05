//! Imported Episode claim and scoped publication.
use super::*;
use reliquary_archive::EpisodeOrigin;

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
        self.session.commit_scoped_grouped_completion(
            &mut self.container,
            token,
            completion,
            now_ns,
            receipts,
            Some(import_id),
        )
    }

    fn validate_import_episode_identity(
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
        if !turns.iter().any(|turn| {
            receipt.conversations.iter().any(|conversation| {
                conversation.conversation_id == episode.conversation_id
                    && conversation
                        .messages
                        .iter()
                        .any(|message| message.node_id.as_deref() == Some(turn.node_id.as_str()))
            })
        }) {
            return Err(OwnerSessionError::Record(
                "import Episode is not in the epoch source receipt".into(),
            ));
        }
        Ok(())
    }
}
