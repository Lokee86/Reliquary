use super::*;

impl RelSession {
    pub fn commit_grouped_insomnia_completion(
        &mut self,
        container: &mut Container,
        token: InsomniaLeaseToken,
        completion: GroupedInsomniaCompletion,
        now_ns: i64,
    ) -> Result<CompletionCheck, OwnerSessionError> {
        if !completion.external_memory_refs.is_empty() {
            return Err(OwnerSessionError::Record(
                "routed completion requires an authenticated PHY publication receipt".into(),
            ));
        }
        self.commit_grouped_insomnia_completion_with_routed_receipts(
            container,
            token,
            completion,
            now_ns,
            &[],
        )
    }

    pub fn commit_grouped_insomnia_completion_with_routed_receipts(
        &mut self,
        container: &mut Container,
        token: InsomniaLeaseToken,
        completion: GroupedInsomniaCompletion,
        now_ns: i64,
        routed_receipts: &[RoutedUserMemoryReceipt],
    ) -> Result<CompletionCheck, OwnerSessionError> {
        self.commit_scoped_grouped_completion(
            container,
            token,
            completion,
            now_ns,
            routed_receipts,
            None,
        )
    }

    pub(super) fn commit_scoped_grouped_completion(
        &mut self,
        container: &mut Container,
        token: InsomniaLeaseToken,
        completion: GroupedInsomniaCompletion,
        now_ns: i64,
        routed_receipts: &[RoutedUserMemoryReceipt],
        import_id: Option<&str>,
    ) -> Result<CompletionCheck, OwnerSessionError> {
        if self.write_poisoned {
            return Err(OwnerSessionError::Record(
                "owner session requires reopen after an uncertain grouped write".into(),
            ));
        }
        match (self.imports.open_epoch(), import_id) {
            (Some(epoch), Some(scope))
                if epoch.import_id == scope
                    && epoch.format_version == 2
                    && self.imports.settlement(scope).is_none() => {}
            (None, None) => {}
            _ => {
                return Err(OwnerSessionError::Record(
                    "owner mutation is fenced by an open import epoch".into(),
                ));
            }
        }
        if container.identity().file_kind != FileKind::Reliquary {
            return Err(OwnerSessionError::WrongFileKind {
                expected: FileKind::Reliquary,
                actual: container.identity().file_kind,
            });
        }
        if container.owner_id() != self.owner_id {
            return Err(OwnerSessionError::Record(
                "grouped completion writer belongs to another owner".into(),
            ));
        }
        if container.identity() != self.container_identity
            || canonical_container_path(container)? != self.container_path
            || container.physical_file_identity()? != self.container_file_identity
        {
            return Err(OwnerSessionError::Record(
                "grouped completion writer is not bound to this owner session file".into(),
            ));
        }
        let expected_writer_proof = self.writer_lease_proof.ok_or_else(|| {
            OwnerSessionError::Record(
                "grouped completion requires a session opened with a writer lease".into(),
            )
        })?;
        let current_writer_proof = container.writer_lease_proof().ok_or_else(|| {
            OwnerSessionError::Record(
                "grouped completion requires the writer lease held at session open".into(),
            )
        })?;
        if current_writer_proof != expected_writer_proof {
            return Err(OwnerSessionError::Record(
                "owner session is stale; reopen before grouped writes".into(),
            ));
        }
        let episode_id = WorkEpisodeId(completion.episode_id);
        let key = (episode_id, completion.attempt);
        if let Some(accepted) = self.grouped_completions.get(&key) {
            return if accepted == &completion {
                Ok(CompletionCheck::AlreadyAccepted)
            } else {
                Err(OwnerSessionError::Insomnia(InsomniaError::ReceiptConflict))
            };
        }
        let archive_episode = self
            .archive
            .episode(reliquary_archive::EpisodeId(completion.episode_id))
            .ok_or_else(|| {
                OwnerSessionError::Record("grouped completion Episode is not admitted".into())
            })?;
        let accepted_order =
            self.insomnia
                .accepted_order(episode_id)
                .ok_or(OwnerSessionError::Insomnia(
                    InsomniaError::UnacceptedEpisode,
                ))?;
        if import_id.is_some()
            && (archive_episode.origin != reliquary_archive::EpisodeOrigin::Import
                || self
                    .insomnia
                    .work(episode_id)
                    .is_none_or(|work| work.priority != InsomniaPriority::Import))
        {
            return Err(OwnerSessionError::Record(
                "import-scoped completion requires import-priority Archive work".into(),
            ));
        }
        if accepted_order.source_through_ns != archive_episode.source_through_ns
            || accepted_order.conversation_id != archive_episode.conversation_id
            || accepted_order.start_node_id != archive_episode.start_node_id
        {
            return Err(OwnerSessionError::Record(
                "grouped completion Episode order differs from Archive evidence".into(),
            ));
        }
        completion
            .validate_replay_episode(completion.episode_id)
            .map_err(OwnerSessionError::Record)?;
        validate_routed_completion_receipts(
            self.owner_id,
            completion.episode_id,
            &completion.external_memory_refs,
            routed_receipts,
        )?;
        if !completion.routing_metadata.is_empty() {
            return Err(OwnerSessionError::Record(
                "grouped routing metadata commit is unsupported".into(),
            ));
        }
        if completion.global_version_start
            != container
                .latest_version()
                .checked_add(1)
                .ok_or_else(|| OwnerSessionError::Record("physical version exhausted".into()))?
        {
            return Err(OwnerSessionError::Record(
                "grouped completion has a stale physical version range".into(),
            ));
        }
        if !completion.memory_records.is_empty()
            && completion.freshness_birth_turn != Some(self.archive.rel_turn_count())
        {
            return Err(OwnerSessionError::Record(
                "grouped Memory births require the current accepted Freshness turn".into(),
            ));
        }

        let receipt = grouped_completion_receipt(&completion);
        let birth_ids = completion
            .memory_records
            .iter()
            .filter_map(|bytes| reliquary_memory_core::decode_grouped_creation(bytes).ok())
            .map(|record| record.id.0)
            .collect::<std::collections::BTreeSet<_>>();
        if completion.memory_ids.iter().any(|id| {
            !self.common.memories.contains_memory(MemoryId(*id)) && !birth_ids.contains(id)
        }) {
            return Err(OwnerSessionError::Record(
                "grouped completion references an absent owner Memory".into(),
            ));
        }
        let bodies = completion
            .bodies
            .iter()
            .map(|body| (body.id, body.bytes.clone()))
            .collect::<Vec<_>>();
        self.common
            .memories
            .preflight_grouped_completion(bodies.clone(), completion.memory_records.clone())?;
        if let Some(turn) = completion.freshness_birth_turn {
            self.freshness
                .preflight_grouped_births(
                    self.owner_id,
                    turn,
                    completion
                        .freshness_birth_memory_ids
                        .iter()
                        .copied()
                        .map(FreshnessMemoryId)
                        .collect(),
                )
                .map_err(OwnerSessionError::Record)?;
        }
        if !self
            .insomnia
            .preflight_grouped_completion(container, token, &receipt, now_ns)?
        {
            return Err(OwnerSessionError::Insomnia(InsomniaError::ReceiptConflict));
        }
        let payload = completion.encode().map_err(OwnerSessionError::Record)?;

        if let Err(error) = container.append(&payload) {
            self.write_poisoned = true;
            return Err(error.into());
        }
        if let Err(error) = container.commit_embedded_version_range_at(
            completion.global_version_start,
            completion.memory_records.len(),
            Some(completion.transaction_time_ns),
        ) {
            self.write_poisoned = true;
            return Err(error.into());
        }
        if let Err(error) = container.sync() {
            self.write_poisoned = true;
            return Err(error.into());
        }
        #[cfg(test)]
        if self.fail_after_grouped_sync {
            self.fail_after_grouped_sync = false;
            self.write_poisoned = true;
            return Err(OwnerSessionError::Record(
                "injected failure after grouped sync".into(),
            ));
        }

        if let Err(error) = self
            .common
            .memories
            .apply_grouped_completion(bodies, completion.memory_records.clone())
        {
            self.write_poisoned = true;
            return Err(error.into());
        }
        if let Some(turn) = completion.freshness_birth_turn {
            if let Err(error) = self.freshness.apply_grouped_births(
                self.owner_id,
                turn,
                completion
                    .freshness_birth_memory_ids
                    .iter()
                    .copied()
                    .map(FreshnessMemoryId)
                    .collect(),
            ) {
                self.write_poisoned = true;
                return Err(OwnerSessionError::Record(error));
            }
        }
        match self.insomnia.apply_grouped_completion(container, receipt) {
            Ok(CompletionCheck::Apply) => {}
            Ok(CompletionCheck::AlreadyAccepted) => {
                self.write_poisoned = true;
                return Err(OwnerSessionError::Insomnia(InsomniaError::ReceiptConflict));
            }
            Err(error) => {
                self.write_poisoned = true;
                return Err(error.into());
            }
        }
        if let Err(error) = self.common.refresh_dream_index(
            &completion
                .memory_ids
                .iter()
                .map(|id| MemoryId(*id))
                .collect::<Vec<_>>(),
        ) {
            self.write_poisoned = true;
            return Err(error);
        }
        self.grouped_completions.insert(key, completion);
        let Some(next_proof) = container.writer_lease_proof() else {
            self.write_poisoned = true;
            return Err(OwnerSessionError::Record(
                "grouped writer lease disappeared after sync".into(),
            ));
        };
        self.writer_lease_proof = Some(next_proof);
        Ok(CompletionCheck::Apply)
    }
}

fn validate_routed_completion_receipts(
    source_owner_id: [u8; 16],
    source_episode_id: [u8; 32],
    references: &[crate::records::GroupedExternalMemoryRef],
    receipts: &[RoutedUserMemoryReceipt],
) -> Result<(), OwnerSessionError> {
    if references.len() != receipts.len() {
        return Err(OwnerSessionError::Record(
            "routed completion references lack matching PHY publication receipts".into(),
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for (reference, receipt) in references.iter().zip(receipts) {
        if receipt.source_owner_id != source_owner_id
            || receipt.source_episode_id != source_episode_id
            || receipt.destination_owner_id == source_owner_id
            || reference != &receipt.external_ref()
            || !seen.insert((receipt.destination_owner_id, receipt.memory_id))
        {
            return Err(OwnerSessionError::Record(
                "routed completion does not match authenticated PHY publication receipts".into(),
            ));
        }
    }
    Ok(())
}
