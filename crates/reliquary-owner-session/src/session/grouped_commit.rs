use super::*;

impl RelSession {
    pub fn commit_grouped_insomnia_completion(
        &mut self,
        container: &mut Container,
        token: InsomniaLeaseToken,
        completion: GroupedInsomniaCompletion,
        now_ns: i64,
    ) -> Result<CompletionCheck, OwnerSessionError> {
        if self.write_poisoned {
            return Err(OwnerSessionError::Record(
                "owner session requires reopen after an uncertain grouped write".into(),
            ));
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
        if completion
            .memory_ids
            .iter()
            .any(|id| !self.common.memories.contains_memory(MemoryId(*id)))
        {
            let birth_ids = completion
                .memory_records
                .iter()
                .filter_map(|bytes| reliquary_memory_core::decode_grouped_creation(bytes).ok())
                .map(|record| record.id.0)
                .collect::<std::collections::BTreeSet<_>>();
            if completion
                .memory_ids
                .iter()
                .any(|id| !birth_ids.contains(id))
            {
                return Err(OwnerSessionError::Record(
                    "grouped completion references an absent owner Memory".into(),
                ));
            }
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
