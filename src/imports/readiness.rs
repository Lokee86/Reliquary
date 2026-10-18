use super::{
    ImportReadinessBlocker, ImportReadinessOwner as Owner, ImportReadinessReport,
    ImportReadinessStatus as Status, ImportReceipt, ImportStart, ImportVectorReadiness,
};
use crate::freshness_storage::HistoricalImportFreshnessSettlement;
use crate::{Cva, CvaError, EpisodeId, EpisodeOrigin, InsomniaWorkState};

fn blocker(owner: Owner, identity: impl Into<String>, status: Status) -> ImportReadinessBlocker {
    ImportReadinessBlocker {
        owner,
        identity: identity.into(),
        status,
    }
}

impl Cva {
    /// Inspect owner-derived readiness for the exact currently open native import.
    /// This is read-only and never consults unrelated owner-wide pending counts.
    pub fn native_import_readiness(
        &mut self,
        import_id: &str,
    ) -> Result<ImportReadinessReport, CvaError> {
        let epochs = self.completion_epochs_from_bytes()?;
        let start = match epochs.status(import_id) {
            Some(super::ImportEpochStatus::Open(start)) => start,
            Some(super::ImportEpochStatus::Completed { .. }) => {
                return Err(CvaError::Import(super::ImportError::Epoch(
                    "readiness is available only for an open native import",
                )));
            }
            None => {
                return Err(CvaError::Import(super::ImportError::Epoch(
                    "readiness identity is not the current open native import",
                )));
            }
        };
        let Some(receipt) = self.archive.import_receipts.get(&start.import_key).cloned() else {
            return Ok(ImportReadinessReport {
                import_id: start.import_id,
                blocker: Some(blocker(Owner::Archive, "import-receipt", Status::Missing)),
                vectors: ImportVectorReadiness::SupportedUnavailable,
            });
        };

        let scope = self.import_epoch_scope()?;
        let settlement = self
            .freshness
            .import_settlements()
            .iter()
            .find(|settlement| {
                settlement
                    .import_id
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
                    == start.import_id
            })
            .cloned();
        let mut readiness = self.derive_import_readiness(&start, &receipt, settlement.as_ref())?;
        if readiness.is_none() && settlement.is_none() {
            readiness = Some(blocker(
                Owner::Freshness,
                start.import_id.clone(),
                Status::Missing,
            ));
        }
        let has_outputs = scope
            .outputs
            .values()
            .any(|output| !output.memory_ids.is_empty());
        let vectors = if scope.source_nodes.is_empty() && !has_outputs {
            ImportVectorReadiness::NotRequired
        } else {
            ImportVectorReadiness::SupportedUnavailable
        };
        Ok(ImportReadinessReport {
            import_id: start.import_id,
            blocker: readiness,
            vectors,
        })
    }

    /// Derive readiness from exact owner-owned import evidence. This checks only
    /// Episodes and Memory outputs attributable to this epoch; unrelated queues
    /// and owner-wide watermarks do not participate.
    pub(crate) fn derive_import_readiness(
        &mut self,
        start: &ImportStart,
        receipt: &ImportReceipt,
        settlement: Option<&HistoricalImportFreshnessSettlement>,
    ) -> Result<Option<ImportReadinessBlocker>, CvaError> {
        let scope = self.import_epoch_scope()?;
        if scope.start != *start
            || receipt.import_key != start.import_key
            || receipt.source_namespace != start.source_namespace
            || receipt.source_format != start.source_format
        {
            return Ok(Some(blocker(
                Owner::Semantic,
                start.import_id.clone(),
                Status::ConflictingEvidence,
            )));
        }

        // A newly imported source path without a complete Import Episode has
        // not established that its import work was exhausted. Reused exact
        // source Nodes from an earlier accepted import do not create new work.
        if scope.episodes.is_empty() && !scope.source_nodes.is_empty() {
            let mut uncovered = scope.source_nodes.clone();
            let statuses = self.completion_epochs_from_bytes()?.statuses();
            for (key, prior) in &self.archive.import_receipts {
                if key == &start.import_key || uncovered.is_empty() {
                    continue;
                }
                let was_completed = statuses.iter().any(|status| match status {
                    super::ImportEpochStatus::Completed {
                        start: prior_start, ..
                    } => prior_start.import_key == *key,
                    super::ImportEpochStatus::Open(_) => false,
                });
                if !was_completed {
                    continue;
                }
                for conversation in &prior.conversations {
                    for message in &conversation.messages {
                        if let Some(node_id) = &message.node_id {
                            uncovered
                                .remove(&(conversation.conversation_id.clone(), node_id.clone()));
                        }
                    }
                }
            }
            if !uncovered.is_empty() {
                return Ok(Some(blocker(
                    Owner::Insomnia,
                    "import-episode-set",
                    Status::Missing,
                )));
            }
        }

        let mut perception_candidates = std::collections::BTreeSet::new();

        // Native import schedules only its Import-origin Episodes through
        // Insomnia. Require exact durable completion for those Episodes.
        for raw_id in &scope.episodes {
            let id = EpisodeId(*raw_id);
            let Some(episode) = self.archive.historical_episode(id) else {
                return Ok(Some(blocker(Owner::Insomnia, hex(raw_id), Status::Missing)));
            };
            if episode.origin != EpisodeOrigin::Import {
                return Ok(Some(blocker(
                    Owner::Insomnia,
                    hex(raw_id),
                    Status::ConflictingEvidence,
                )));
            }
            let Some(work) = self.insomnia.work(id) else {
                return Ok(Some(blocker(Owner::Insomnia, hex(raw_id), Status::Missing)));
            };
            match work.state {
                InsomniaWorkState::Complete => {}
                InsomniaWorkState::Pending | InsomniaWorkState::Processing => {
                    return Ok(Some(blocker(Owner::Insomnia, hex(raw_id), Status::Pending)));
                }
                InsomniaWorkState::Failed | InsomniaWorkState::Terminal => {
                    return Ok(Some(blocker(Owner::Insomnia, hex(raw_id), Status::Failed)));
                }
            }
            let Some(output) = scope.outputs.get(raw_id) else {
                return Ok(Some(blocker(Owner::Insomnia, hex(raw_id), Status::Missing)));
            };
            if output.episode_id != *raw_id {
                return Ok(Some(blocker(
                    Owner::Insomnia,
                    hex(raw_id),
                    Status::ConflictingEvidence,
                )));
            }

            // External Memory publications belong to another owner and cannot
            // be authenticated by this REL's MemoryStore.
            if let Some(reference) = output.external_memory_refs.iter().next() {
                return Ok(Some(blocker(
                    Owner::Memory,
                    format!("{}:{}", reference.owner_id, hex(&reference.memory_id.0)),
                    Status::Unsupported,
                )));
            }
            if output
                .records
                .keys()
                .any(|id| !output.memory_ids.contains(id))
            {
                return Ok(Some(blocker(
                    Owner::Memory,
                    hex(raw_id),
                    Status::ConflictingEvidence,
                )));
            }

            // Completion memory_ids can include an exact same-mutation
            // idempotent result. That is not fresh import publication evidence:
            // every nonzero output ID needs a retained first-publication record
            // from this epoch and this Episode. Caller-supplied IDs or Memory
            // provenance fields alone cannot prove accepted grouped publication.
            for memory_id in &output.memory_ids {
                if !self.memories.contains_historical_memory(*memory_id) {
                    return Ok(Some(blocker(
                        Owner::Memory,
                        hex(&memory_id.0),
                        Status::Missing,
                    )));
                }
                let Some(record) = output.records.get(memory_id) else {
                    return Ok(Some(blocker(
                        Owner::Memory,
                        hex(&memory_id.0),
                        Status::ConflictingEvidence,
                    )));
                };
                if record.global_version <= start.pre.global_version
                    || record.revision != 1
                    || record.source_episode_id != Some(id)
                    || !scope.cohort.contains(memory_id)
                    || self.memories.first_mutation_id(*memory_id)
                        != Some(record.mutation_id.as_str())
                {
                    return Ok(Some(blocker(
                        Owner::Memory,
                        hex(&memory_id.0),
                        Status::ConflictingEvidence,
                    )));
                }
                // Terminal Memories retain this first-publication proof; only
                // current Memories can contribute downstream Perception work.
                if self.memories.contains_memory(*memory_id) {
                    let current = self.memories.memory(&mut self.container, *memory_id)?;
                    if current.revision < record.revision {
                        return Ok(Some(blocker(
                            Owner::Memory,
                            hex(&memory_id.0),
                            Status::ConflictingEvidence,
                        )));
                    }
                    if current.source_episode_id != Some(id) {
                        return Ok(Some(blocker(
                            Owner::Memory,
                            hex(&memory_id.0),
                            Status::ConflictingEvidence,
                        )));
                    }
                    if !current.archived && current.superseded_by.is_none() {
                        perception_candidates.insert(*memory_id);
                    }
                }
            }
        }

        // The scoped CVAFRS05 settlement is the semantic work receipt. Its
        // authenticated source/cohort scope and store validator bind import
        // events, exact source identities and sparse affected recipients.
        // Settlement.cohort intentionally may also include existing affected
        // recipients; only epoch.born_memories is compared to the birth cohort.

        // Each first-publication Memory has a deterministic initial Dream pass
        // identity. The Dream owner records its accepted no-link or linked result.
        for id in &scope.cohort {
            // A pass begun before archival remains owner work. The Dream owner
            // resumes such a pass before applying its new-pass eligibility rules.
            if let Some(pass) = self.pending_initial_dream_pass(*id) {
                return Ok(Some(blocker(Owner::Dream, pass.pass_id, Status::Pending)));
            }
            if !self.memories.contains_memory(*id) {
                continue;
            }
            let current = self.memories.memory(&mut self.container, *id)?;
            // Dream intentionally does not start an initial pass for archived
            // publication births. Superseded Memories remain eligible in the
            // Dream owner, so their deterministic pass is still required.
            if current.archived {
                continue;
            }
            let pass_id = crate::dream_freshness::initial_pass_id(start.owner_uuid, *id);
            if !self.initial_dream_pass_settled(&pass_id) {
                return Ok(Some(blocker(Owner::Dream, pass_id, Status::Pending)));
            }
        }

        // Perception schedules mention resolution only after initial Dream
        // processing. Check exact imported publications and require durable
        // terminal outcomes; unrelated Memories do not participate.
        for id in &perception_candidates {
            if scope.cohort.contains(id) && !self.dream_was_processed(*id) {
                return Ok(Some(blocker(
                    Owner::Dream,
                    hex(&id.0),
                    Status::ConflictingEvidence,
                )));
            }
            if !self.dream_was_processed(*id) {
                continue;
            }
            for key in self.entity_mention_keys_for_memory(*id) {
                let Some(resolution) = self.entity_resolution(key) else {
                    return Ok(Some(blocker(
                        Owner::Perception,
                        mention_identity(key),
                        Status::Pending,
                    )));
                };
                match resolution.status {
                    crate::MemoryEntityResolutionStatus::Resolved { .. }
                    | crate::MemoryEntityResolutionStatus::Rejected { .. } => {}
                    crate::MemoryEntityResolutionStatus::Pending(_)
                    | crate::MemoryEntityResolutionStatus::Dormant(_) => {
                        return Ok(Some(blocker(
                            Owner::Perception,
                            mention_identity(key),
                            Status::Pending,
                        )));
                    }
                }
            }
        }

        if let Some(settlement) = settlement {
            let Some(epoch) = settlement.epoch.as_ref() else {
                return Ok(Some(blocker(
                    Owner::Freshness,
                    start.import_id.clone(),
                    Status::ConflictingEvidence,
                )));
            };
            if settlement.owner != start.owner_uuid
                || settlement
                    .import_id
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
                    != start.import_id
                || epoch.owner != start.owner_uuid
                || epoch.import_id != settlement.import_id
                || epoch.start_evidence_digest != scope.start_digest
                || epoch.base_cut != scope.base_cut
                || epoch.source_nodes != scope.source_nodes
                || epoch.born_memories != scope.cohort
            {
                return Ok(Some(blocker(
                    Owner::Freshness,
                    start.import_id.clone(),
                    Status::ConflictingEvidence,
                )));
            }
            if receipt.conversations.is_empty()
                && (!epoch.source_nodes.is_empty()
                    || !epoch.born_memories.is_empty()
                    || !settlement.cohort.is_empty()
                    || !settlement.records.is_empty()
                    || !settlement.superseded.is_empty()
                    || settlement.base_cut != settlement.settlement_cut)
            {
                return Ok(Some(blocker(
                    Owner::Freshness,
                    start.import_id.clone(),
                    Status::ConflictingEvidence,
                )));
            }
            if !super::completion::settlement_is_current(self, settlement) {
                return Ok(Some(blocker(
                    Owner::Freshness,
                    start.import_id.clone(),
                    Status::Pending,
                )));
            }
        }

        // Native import schedules no Ego mutations and enqueues no vector job.
        // Missing vector rows remain supported-unavailable; they are not work.
        Ok(None)
    }
}

fn mention_identity(key: crate::MemoryEntityMentionKey) -> String {
    format!(
        "{}:{:?}:{}:{}",
        hex(&key.memory_id.0),
        key.field,
        key.start_byte,
        key.end_byte
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
