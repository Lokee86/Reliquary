use super::*;

impl RelWriter {
    pub(super) fn ensure_write_ready(&self) -> Result<(), OwnerSessionError> {
        if self.session.write_poisoned {
            Err(OwnerSessionError::Record(
                "owner session requires reopen after an uncertain write".into(),
            ))
        } else if self.session.imports.open_epoch().is_some() {
            Err(OwnerSessionError::Record(
                "owner mutation is fenced by an open import epoch".into(),
            ))
        } else {
            Ok(())
        }
    }
    pub(super) fn finish_insomnia_write<T>(
        &mut self,
        result: Result<T, InsomniaError>,
    ) -> Result<T, OwnerSessionError> {
        match result {
            Ok(value) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(value)
            }
            Err(error) => {
                if matches!(error, InsomniaError::Container(_))
                    || self.session.writer_lease_proof != self.container.writer_lease_proof()
                {
                    self.session.write_poisoned = true;
                }
                Err(error.into())
            }
        }
    }
    /// Create and initialize a new REL owner through the public owner scope.
    pub fn create_current_path(
        path: impl AsRef<Path>,
        scope: OwnerScope,
        owner_id: [u8; 16],
    ) -> Result<Self, OwnerSessionError> {
        Self::create_path(path, scope.into_container_scope(), owner_id)
    }

    /// Create and initialize a new REL owner, then open its canonical writer.
    pub fn create_path(
        path: impl AsRef<Path>,
        scope: reliquary_container::ReliquaryScope,
        owner_id: [u8; 16],
    ) -> Result<Self, OwnerSessionError> {
        let mut container = Container::create(path, ContainerIdentity::reliquary(scope), owner_id)?;
        super::create::initialize_owner(&mut container)?;
        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let session = RelSession::open(&container, &admission, [], [])?;
        Ok(Self { container, session })
    }

    pub fn open_path(
        path: impl AsRef<Path>,
        graph_admission: GraphAdmission,
        accepted_episodes: impl IntoIterator<Item = AcceptedEpisodeOrder>,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<Self, OwnerSessionError> {
        let accepted_episodes = accepted_episodes.into_iter().collect::<Vec<_>>();
        let dream_admissions = dream_admissions.into_iter().collect::<Vec<_>>();
        let container = Container::open_writer_as(path, FileKind::Reliquary)?;
        let session = RelSession::open(
            &container,
            &graph_admission,
            accepted_episodes.clone(),
            dream_admissions.clone(),
        )?;
        Ok(Self { container, session })
    }

    /// Acquire the exclusive writer lease after current-format durable admission.
    pub fn open_path_current(path: impl AsRef<Path>) -> Result<Self, OwnerSessionError> {
        let container = Container::open_writer_as(path, FileKind::Reliquary)?;
        let session = RelSession::open_current(&container)?;
        Ok(Self { container, session })
    }

    pub fn owner_id(&self) -> [u8; 16] {
        self.session.owner_id()
    }
    pub fn tombstone_conversation(
        &mut self,
        conversation_id: &str,
    ) -> Result<bool, OwnerSessionError> {
        self.ensure_write_ready()?;
        let result = self
            .session
            .archive
            .tombstone_conversation(&mut self.container, conversation_id);
        match result {
            Ok(value) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(value)
            }
            Err(error) => {
                if matches!(error, reliquary_archive::ArchiveError::Container(_)) {
                    self.session.write_poisoned = true;
                }
                Err(error.into())
            }
        }
    }
    /// Record an evaluated pair after the application has accepted its result.
    pub fn record_dream_pair(
        &mut self,
        left: MemoryId,
        right: MemoryId,
    ) -> Result<bool, OwnerSessionError> {
        self.ensure_write_ready()?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "owner session is stale; reopen before Dream writes".into(),
            ));
        }
        let result = self.session.common.dream_pairs.append(
            &mut self.container,
            &self.session.common.memories,
            left,
            right,
        );
        match result {
            Ok(value) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(value)
            }
            Err(error) => {
                if matches!(error, OwnerSessionError::Container(_))
                    || before != self.container.writer_lease_proof()
                {
                    self.session.write_poisoned = true;
                }
                Err(error)
            }
        }
    }
    pub fn read_session(&self) -> &RelSession {
        &self.session
    }

    /// Revalidate an inference snapshot against the live durable claim before
    /// Gateway performs any routed PHY write.
    pub fn validate_insomnia_claim(
        &self,
        claim: &RelInsomniaClaimInput,
        now_ns: i64,
    ) -> Result<(), OwnerSessionError> {
        self.ensure_write_ready()?;
        self.validate_scoped_insomnia_claim(claim, now_ns)
    }

    pub(super) fn validate_scoped_insomnia_claim(
        &self,
        claim: &RelInsomniaClaimInput,
        now_ns: i64,
    ) -> Result<(), OwnerSessionError> {
        if self.session.writer_lease_proof.is_none()
            || self.session.writer_lease_proof != self.container.writer_lease_proof()
        {
            return Err(OwnerSessionError::Record(
                "owner session is stale; reopen before routed writes".into(),
            ));
        }
        if claim.owner_id != self.session.owner_id {
            return Err(OwnerSessionError::Insomnia(
                InsomniaError::UnacceptedEpisode,
            ));
        }
        let stored = self
            .session
            .insomnia
            .work(claim.work.episode_id)
            .ok_or(OwnerSessionError::Insomnia(InsomniaError::MissingWork))?;
        if stored.state != reliquary_insomnia_store::InsomniaWorkState::Processing
            || stored.attempt_count == 0
            || stored != &claim.work
            || stored.lease_token.is_none()
            || stored
                .lease_expires_ns
                .is_none_or(|expires| expires <= now_ns)
            || now_ns < stored.updated_at_ns
            || now_ns < claim.started_at_ns
            || claim.episode.id.0 != stored.episode_id.0
            || self.session.archive.episode(claim.episode.id) != Some(&claim.episode)
        {
            return Err(OwnerSessionError::Insomnia(
                InsomniaError::InvalidTransition,
            ));
        }
        let turns = self.session.archive_evidence_episode(claim.episode.id)?;
        if turns.as_ref() != Some(&claim.turns) {
            return Err(OwnerSessionError::Insomnia(
                InsomniaError::UnacceptedEpisode,
            ));
        }
        Ok(())
    }

    /// Admit a bounded batch of previously accepted Episodes, claim the next
    /// due work item, and return Archive-verified input. This method performs
    /// only local journal/projection work; callers run inference after it
    /// returns, when the Gateway's owner lock can be released.
    pub fn claim_next_insomnia_input(
        &mut self,
        worker: &str,
        started_at_ns: i64,
        lease_duration_ns: i64,
        max_admissions: usize,
    ) -> Result<Option<RelInsomniaClaimInput>, OwnerSessionError> {
        self.ensure_write_ready()?;
        if max_admissions == 0 || lease_duration_ns <= 0 || worker.trim().is_empty() {
            return Err(OwnerSessionError::Insomnia(InsomniaError::InvalidField(
                "Insomnia worker limits",
            )));
        }
        for order in self.session.insomnia.unqueued_orders(max_admissions) {
            let episode = self
                .session
                .archive
                .episode(reliquary_archive::EpisodeId(order.episode_id.0))
                .ok_or(OwnerSessionError::Insomnia(
                    InsomniaError::UnacceptedEpisode,
                ))?;
            if episode.source_through_ns != order.source_through_ns
                || episode.conversation_id != order.conversation_id
                || episode.start_node_id != order.start_node_id
            {
                return Err(OwnerSessionError::Insomnia(
                    InsomniaError::UnacceptedEpisode,
                ));
            }
            let queued = self.session.insomnia.queue(
                &mut self.container,
                order,
                InsomniaPriority::Live,
                started_at_ns,
            );
            self.finish_insomnia_write(queued)?;
        }
        let claimed = self.session.insomnia.claim_next(
            &mut self.container,
            worker,
            started_at_ns,
            lease_duration_ns,
        );
        let Some(work) = self.finish_insomnia_write(claimed)? else {
            self.session.writer_lease_proof = self.container.writer_lease_proof();
            return Ok(None);
        };
        let episode_id = reliquary_archive::EpisodeId(work.episode_id.0);
        let episode = self.session.archive.episode(episode_id).cloned().ok_or(
            OwnerSessionError::Insomnia(InsomniaError::UnacceptedEpisode),
        )?;
        let turns = match self.session.archive_evidence_episode(episode_id) {
            Ok(Some(turns)) => turns,
            Ok(None) => {
                return Err(OwnerSessionError::Insomnia(
                    InsomniaError::UnacceptedEpisode,
                ));
            }
            Err(
                error @ OwnerSessionError::Archive(
                    reliquary_archive::ArchiveError::EvidenceLimitExceeded,
                ),
            ) => {
                let held_claim = RelInsomniaClaimInput {
                    owner_id: self.owner_id(),
                    work: work.clone(),
                    episode: episode.clone(),
                    turns: Vec::new(),
                    started_at_ns,
                };
                self.hold_insomnia_claim(
                    &held_claim,
                    started_at_ns,
                    "Episode exceeds bounded native evidence limits".into(),
                )?;
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        Ok(Some(RelInsomniaClaimInput {
            owner_id: self.owner_id(),
            work,
            episode,
            turns,
            started_at_ns,
        }))
    }

    pub fn hold_insomnia_claim(
        &mut self,
        claim: &RelInsomniaClaimInput,
        held_at_ns: i64,
        reason: String,
    ) -> Result<InsomniaWork, OwnerSessionError> {
        self.hold_insomnia_claim_with_payload(claim, held_at_ns, reason, None)
    }

    pub fn hold_insomnia_claim_with_payload(
        &mut self,
        claim: &RelInsomniaClaimInput,
        held_at_ns: i64,
        reason: String,
        held_payload: Option<Vec<u8>>,
    ) -> Result<InsomniaWork, OwnerSessionError> {
        self.ensure_write_ready()?;
        self.hold_scoped_insomnia_claim(claim, held_at_ns, reason, held_payload)
    }

    pub(super) fn hold_scoped_insomnia_claim(
        &mut self,
        claim: &RelInsomniaClaimInput,
        held_at_ns: i64,
        reason: String,
        held_payload: Option<Vec<u8>>,
    ) -> Result<InsomniaWork, OwnerSessionError> {
        if claim.owner_id != self.owner_id() || claim.work.episode_id.0 != claim.episode.id.0 {
            return Err(OwnerSessionError::Insomnia(
                InsomniaError::UnacceptedEpisode,
            ));
        }
        let token = claim
            .work
            .lease_token
            .ok_or(OwnerSessionError::Insomnia(InsomniaError::InvalidLease))?;
        let result = self.session.insomnia.hold_with_payload(
            &mut self.container,
            claim.work.episode_id,
            token,
            held_at_ns,
            reason,
            held_payload,
        );
        self.finish_insomnia_write(result)
    }

    pub fn checkpoint_insomnia_claim(
        &mut self,
        claim: &RelInsomniaClaimInput,
        checkpointed_at_ns: i64,
        payload: Vec<u8>,
    ) -> Result<InsomniaWork, OwnerSessionError> {
        self.ensure_write_ready()?;
        self.checkpoint_scoped_insomnia_claim(claim, checkpointed_at_ns, payload)
    }

    pub(super) fn checkpoint_scoped_insomnia_claim(
        &mut self,
        claim: &RelInsomniaClaimInput,
        checkpointed_at_ns: i64,
        payload: Vec<u8>,
    ) -> Result<InsomniaWork, OwnerSessionError> {
        if claim.owner_id != self.owner_id() || claim.work.episode_id.0 != claim.episode.id.0 {
            return Err(OwnerSessionError::Insomnia(
                InsomniaError::UnacceptedEpisode,
            ));
        }
        let token = claim
            .work
            .lease_token
            .ok_or(OwnerSessionError::Insomnia(InsomniaError::InvalidLease))?;
        let result = self.session.insomnia.checkpoint_payload(
            &mut self.container,
            claim.work.episode_id,
            token,
            checkpointed_at_ns,
            payload,
        );
        self.finish_insomnia_write(result)
    }

    pub fn next_global_version(&self) -> Result<u64, OwnerSessionError> {
        self.container
            .latest_version()
            .checked_add(1)
            .ok_or_else(|| OwnerSessionError::Record("physical version exhausted".into()))
    }

    /// Accept a live source turn and advance its branch through the owner’s
    /// Archive projection. Archive operations are idempotent, so a retry after
    /// a queue journal failure can safely confirm the already synced turn.
    pub fn accept_turn_and_advance_branch(
        &mut self,
        input: reliquary_archive::TurnInput,
        branch: reliquary_archive::Branch,
    ) -> Result<reliquary_archive::Node, OwnerSessionError> {
        self.ensure_write_ready()?;
        self.accept_scoped_turn_and_advance_branch(input, branch)
    }

    pub(super) fn accept_scoped_turn_and_advance_branch(
        &mut self,
        input: reliquary_archive::TurnInput,
        branch: reliquary_archive::Branch,
    ) -> Result<reliquary_archive::Node, OwnerSessionError> {
        self.validate_turn_branch(&input, &branch)?;
        let already_on_branch = if let Some(current) = self
            .session
            .archive
            .branch(&branch.conversation_id, &branch.id)
        {
            let ancestry = self
                .session
                .archive
                .historical_turns(&branch.conversation_id, &current.leaf_node_id)?;
            let found = ancestry.iter().any(|turn| turn.node_id == input.id);
            if found {
                self.validate_existing_turn(&input)?;
            }
            found
        } else {
            false
        };
        let accepted = match self.session.archive.accept_turn(&mut self.container, input) {
            Ok(value) => value,
            Err(error) => {
                if matches!(error, reliquary_archive::ArchiveError::Container(_)) {
                    self.session.write_poisoned = true;
                }
                return Err(error.into());
            }
        };
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        if already_on_branch {
            return Ok(accepted.value);
        }
        if let Err(error) = self.session.archive.put_branch(&mut self.container, branch) {
            if matches!(error, reliquary_archive::ArchiveError::Container(_)) {
                self.session.write_poisoned = true;
            }
            return Err(error.into());
        }
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        Ok(accepted.value)
    }

    /// Finalize a native Episode and admit its Archive-derived order for work.
    pub fn finalize_episode(
        &mut self,
        conversation_id: &str,
        start_node_id: &str,
        end_node_id: &str,
        origin: reliquary_archive::EpisodeOrigin,
        boundary: reliquary_archive::EpisodeBoundary,
        finalized_at_ns: i64,
    ) -> Result<reliquary_archive::EpisodeId, OwnerSessionError> {
        self.ensure_write_ready()?;
        let id = match self.session.archive.put_episode(
            &mut self.container,
            conversation_id,
            start_node_id,
            end_node_id,
            origin,
            boundary,
            finalized_at_ns,
        ) {
            Ok(id) => id,
            Err(error) => {
                if matches!(error, reliquary_archive::ArchiveError::Container(_)) {
                    self.session.write_poisoned = true;
                }
                return Err(error.into());
            }
        };
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        let episode = self
            .session
            .archive
            .episode(id)
            .expect("finalized Episode is present");
        let order = AcceptedEpisodeOrder {
            episode_id: WorkEpisodeId(id.0),
            source_through_ns: episode.source_through_ns,
            conversation_id: episode.conversation_id.clone(),
            start_node_id: episode.start_node_id.clone(),
        };
        if let Err(error) = self.session.insomnia.admit_order(order) {
            self.session.write_poisoned = true;
            return Err(error.into());
        }
        Ok(id)
    }

    /// Check that an accepted turn would advance this owner's current branch
    /// before any Archive frame is appended. The caller must retain the same
    /// writer lock from validation through `accept_turn_and_advance_branch`.
    pub fn validate_turn_branch(
        &self,
        input: &reliquary_archive::TurnInput,
        branch: &reliquary_archive::Branch,
    ) -> Result<(), OwnerSessionError> {
        if input.conversation_id != branch.conversation_id || input.id != branch.leaf_node_id {
            return Err(OwnerSessionError::Archive(
                reliquary_archive::ArchiveError::InvalidBranchRevision,
            ));
        }
        let Some(current) = self
            .session
            .archive
            .branch(&branch.conversation_id, &branch.id)
        else {
            return Ok(());
        };
        if self
            .session
            .archive
            .node(&input.conversation_id, &input.id)
            .is_some()
        {
            self.validate_existing_turn(input)?;
        }
        let current_ancestry = self
            .session
            .archive
            .historical_turns(&branch.conversation_id, &current.leaf_node_id)?;
        if current_ancestry.iter().any(|turn| turn.node_id == input.id) {
            return Ok(());
        }
        let Some(parent_id) = input.parent_id.as_deref() else {
            return Err(OwnerSessionError::Archive(
                reliquary_archive::ArchiveError::InvalidBranchRevision,
            ));
        };
        let ancestry = self
            .session
            .archive
            .historical_turns(&input.conversation_id, parent_id)?;
        if ancestry
            .iter()
            .any(|turn| turn.node_id == current.leaf_node_id)
        {
            Ok(())
        } else {
            Err(OwnerSessionError::Archive(
                reliquary_archive::ArchiveError::InvalidBranchRevision,
            ))
        }
    }

    fn validate_existing_turn(
        &self,
        input: &reliquary_archive::TurnInput,
    ) -> Result<(), OwnerSessionError> {
        let Some(node) = self.session.archive.node(&input.conversation_id, &input.id) else {
            return Ok(());
        };
        let content_matches = self
            .session
            .archive
            .content_for_node(&input.conversation_id, &input.id)
            == Some(input.content.as_str());
        if node.parent_id != input.parent_id
            || node.role != input.role
            || node.principal_id != input.principal_id
            || node.timestamp_ns != input.timestamp_ns
            || !content_matches
        {
            return Err(OwnerSessionError::Archive(
                reliquary_archive::ArchiveError::ConflictingNode,
            ));
        }
        Ok(())
    }

    /// Queue and lease work for an admitted Archive Episode. The Episode's
    /// scheduling identity is re-derived from this owner's Archive projection;
    /// caller-supplied order data cannot redirect the claim.
    pub fn queue_and_claim_insomnia_episode(
        &mut self,
        episode_id: reliquary_archive::EpisodeId,
        worker: &str,
        now_ns: i64,
        lease_duration_ns: i64,
    ) -> Result<InsomniaWork, OwnerSessionError> {
        self.ensure_write_ready()?;
        let episode =
            self.session
                .archive
                .episode(episode_id)
                .ok_or(OwnerSessionError::Insomnia(
                    InsomniaError::UnacceptedEpisode,
                ))?;
        let work_id = WorkEpisodeId(episode_id.0);
        let order = self
            .session
            .insomnia
            .accepted_order(work_id)
            .cloned()
            .ok_or(OwnerSessionError::Insomnia(
                InsomniaError::UnacceptedEpisode,
            ))?;
        if order.source_through_ns != episode.source_through_ns
            || order.conversation_id != episode.conversation_id
            || order.start_node_id != episode.start_node_id
        {
            return Err(OwnerSessionError::Insomnia(
                InsomniaError::UnacceptedEpisode,
            ));
        }
        let queued =
            self.session
                .insomnia
                .queue(&mut self.container, order, InsomniaPriority::Live, now_ns);
        self.finish_insomnia_write(queued)?;
        let result = self.session.insomnia.claim_episode(
            &mut self.container,
            work_id,
            worker,
            now_ns,
            lease_duration_ns,
        );
        self.finish_insomnia_write(result)
    }

    pub fn commit_grouped_insomnia_completion(
        &mut self,
        token: InsomniaLeaseToken,
        completion: GroupedInsomniaCompletion,
        now_ns: i64,
    ) -> Result<CompletionCheck, OwnerSessionError> {
        self.session.commit_grouped_insomnia_completion(
            &mut self.container,
            token,
            completion,
            now_ns,
        )
    }

    pub fn commit_grouped_insomnia_completion_with_routed_receipts(
        &mut self,
        token: InsomniaLeaseToken,
        completion: GroupedInsomniaCompletion,
        now_ns: i64,
        routed_receipts: &[RoutedUserMemoryReceipt],
    ) -> Result<CompletionCheck, OwnerSessionError> {
        self.session
            .commit_grouped_insomnia_completion_with_routed_receipts(
                &mut self.container,
                token,
                completion,
                now_ns,
                routed_receipts,
            )
    }
}
