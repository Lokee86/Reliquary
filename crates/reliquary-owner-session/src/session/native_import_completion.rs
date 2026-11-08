//! Terminal import admission, with owner-derived Freshness evidence.

use super::work::{mapped_source_nodes, source_path_is_mapped};
use super::*;
use crate::records::import_settlement::source_prefix_digest;
use crate::records::{
    ImportComplete, ImportEpochStatus, ImportFreshnessAuthority, ImportFreshnessSettlement,
    ImportWatermarks, hex_digest,
};
use reliquary_archive::EpisodeOrigin;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

impl RelWriter {
    /// Persist the frozen import Freshness proof independently of completion.
    /// If interruption occurs after this sync, the same owner can reopen and
    /// complete from the exact accepted settlement without reprocessing work.
    pub fn settle_native_import(
        &mut self,
        import_id: &str,
    ) -> Result<ImportFreshnessSettlement, OwnerSessionError> {
        self.settle_native_import_with_authority(import_id, None)
    }

    fn settle_native_import_with_authority(
        &mut self,
        import_id: &str,
        replay_source_authority: Option<ImportFreshnessAuthority>,
    ) -> Result<ImportFreshnessSettlement, OwnerSessionError> {
        if self.session.write_poisoned
            || self.container.writer_lease_proof() != self.session.writer_lease_proof
            || self.session.writer_lease_proof.is_none()
        {
            return Err(OwnerSessionError::Record(
                "native import settlement requires a current unpoisoned owner writer".into(),
            ));
        }
        let start = match self.session.imports.status(import_id) {
            Some(ImportEpochStatus::Open(start)) if start.format_version == 2 => start,
            _ => {
                return Err(OwnerSessionError::Record(
                    "native import settlement requires an open current-format epoch".into(),
                ));
            }
        };
        let key = super::decode_import_key(&start.import_key).ok_or_else(|| {
            OwnerSessionError::Record("import epoch has an invalid source key".into())
        })?;
        let receipt = self.session.archive.import_receipt(&key).ok_or_else(|| {
            OwnerSessionError::Record("import completion requires a durable Archive receipt".into())
        })?;
        if receipt.source_namespace != start.source_namespace
            || receipt.source_format != start.source_format
            || receipt.source_digest != key
        {
            return Err(OwnerSessionError::Record(
                "import receipt differs from the opened owner epoch".into(),
            ));
        }
        let receipt_digest = hex_digest_bytes(&receipt.receipt_digest);
        if self
            .session
            .imports
            .processing_binding(import_id)
            .is_some_and(|binding| {
                binding.profile.is_none()
                    || replay_source_authority.is_none()
                        && binding.profile.is_some_and(|profile| {
                            self.session
                                .active_compatibility_profile(CompatibilityProfileId(profile))
                                .is_none()
                        })
            })
        {
            return Err(OwnerSessionError::Record(
                "import processing plan lacks its accepted embedding profile".into(),
            ));
        }
        self.session.validate_import_work_complete(receipt)?;

        let settlement = if let Some(accepted) = self.session.imports.settlement(import_id) {
            if accepted.receipt_digest != receipt_digest
                || accepted.global_version != self.container.latest_version()
                || accepted.rel_turn != self.session.archive.rel_turn_count()
                || accepted.memory_version != self.session.common.memories.memory_version()
                || accepted.graph_version != self.session.common.graph.graph_version()
                || accepted.replay_source_authority != replay_source_authority
            {
                return Err(OwnerSessionError::Record(
                    "native import Freshness settlement is stale or mismatched".into(),
                ));
            }
            accepted.clone()
        } else {
            let records = self
                .session
                .freshness
                .records_through_turn(self.session.archive.rel_turn_count())
                .map_err(OwnerSessionError::Record)?;
            let cut = self.session.archive.rel_turn_count();
            let mut hash = Sha256::new();
            hash.update(b"reliquary-import-freshness-snapshot-v1\0");
            hash.update(self.session.owner_id);
            hash.update(cut.to_le_bytes());
            for (id, record) in &records {
                hash.update(id.0);
                hash.update(
                    record
                        .score_at_with_policy(cut, &self.session.freshness.policy())
                        .map_err(|e| OwnerSessionError::Record(e.to_string()))?
                        .to_le_bytes(),
                );
                for anchor in [record.admitted_at_turn, record.accounted_turn] {
                    hash.update([u8::from(anchor.is_some())]);
                    if let Some(value) = anchor {
                        hash.update(value.to_le_bytes());
                    }
                }
            }
            let record = ImportFreshnessSettlement {
                owner_uuid: self.session.owner_id,
                import_id: import_id.to_owned(),
                start_digest: hex_digest(
                    &serde_json::to_vec(&start)
                        .map_err(|e| OwnerSessionError::Record(e.to_string()))?,
                ),
                receipt_digest,
                prefix_digest: source_prefix_digest(&self.container)
                    .map_err(OwnerSessionError::Record)?,
                rel_turn: cut,
                global_version: self.container.latest_version(),
                memory_version: self.session.common.memories.memory_version(),
                graph_version: self.session.common.graph.graph_version(),
                freshness_state_digest: format!("{:x}", hash.finalize()),
                freshness_memory_count: records.len() as u64,
                replay_source_authority,
            };
            if let Err(error) = self
                .session
                .imports
                .append_settlement(&mut self.container, record.clone())
            {
                self.session.write_poisoned = true;
                return Err(OwnerSessionError::Record(error));
            }
            self.session.writer_lease_proof = self.container.writer_lease_proof();
            record
        };

        Ok(settlement)
    }

    /// Complete the selected import only after its source receipt and every
    /// imported Episode's durable extraction have been accepted. The settlement
    /// is committed first and replays independently after interruption.
    pub fn complete_native_import(
        &mut self,
        import_id: &str,
        completed_at_ns: i64,
    ) -> Result<(ImportComplete, bool), OwnerSessionError> {
        self.complete_native_import_with_authority(import_id, completed_at_ns, None)
    }

    pub(crate) fn complete_replayed_native_import(
        &mut self,
        import_id: &str,
        completed_at_ns: i64,
        source_authority: ImportFreshnessAuthority,
    ) -> Result<(ImportComplete, bool), OwnerSessionError> {
        self.complete_native_import_with_authority(
            import_id,
            completed_at_ns,
            Some(source_authority),
        )
    }

    fn complete_native_import_with_authority(
        &mut self,
        import_id: &str,
        completed_at_ns: i64,
        replay_source_authority: Option<ImportFreshnessAuthority>,
    ) -> Result<(ImportComplete, bool), OwnerSessionError> {
        if self.session.write_poisoned
            || self.container.writer_lease_proof() != self.session.writer_lease_proof
            || self.session.writer_lease_proof.is_none()
        {
            return Err(OwnerSessionError::Record(
                "native import completion requires a current unpoisoned owner writer".into(),
            ));
        }
        let status = self
            .session
            .imports
            .status(import_id)
            .ok_or_else(|| OwnerSessionError::Record("unknown native import epoch".into()))?;
        let start = match status {
            ImportEpochStatus::Completed { complete, .. } => return Ok((complete, false)),
            ImportEpochStatus::Open(start) if start.format_version == 2 => start,
            _ => {
                return Err(OwnerSessionError::Record(
                    "legacy import epoch cannot be completed under the current contract".into(),
                ));
            }
        };
        if completed_at_ns < start.started_at_ns {
            return Err(OwnerSessionError::Record(
                "import completion precedes durable start".into(),
            ));
        }
        let settlement =
            self.settle_native_import_with_authority(import_id, replay_source_authority)?;

        let complete = ImportComplete {
            format_version: 2,
            owner_uuid: self.session.owner_id,
            import_id: import_id.to_owned(),
            start_digest: settlement.start_digest.clone(),
            receipt_digest: settlement.receipt_digest.clone(),
            completed_at_ns,
            post: ImportWatermarks {
                global_version: self.container.latest_version(),
                archive_version: Some(self.session.archive.archive_version()),
                memory_version: self.session.common.memories.memory_version(),
                entity_version: None,
                relationship_version: None,
                graph_version: self.session.common.graph.graph_version(),
                memory_graph_version: None,
                vector_version: Some(self.session.generations.vector_version()),
            },
            historical_freshness_settlement_digest: settlement
                .digest()
                .map_err(OwnerSessionError::Record)?,
        };
        let written = match self
            .session
            .imports
            .append_complete(&mut self.container, complete.clone())
        {
            Ok(value) => value,
            Err(error) => {
                self.session.write_poisoned = true;
                return Err(OwnerSessionError::Record(error));
            }
        };
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        Ok((complete, written))
    }
}

impl RelSession {
    pub(super) fn validate_import_work_complete(
        &self,
        receipt: &crate::model::ImportReceipt,
    ) -> Result<(), OwnerSessionError> {
        let mapped = mapped_source_nodes(receipt);
        let mut covered = BTreeSet::new();
        for episode in self.archive.episodes() {
            if episode.origin != EpisodeOrigin::Import {
                continue;
            }
            let turns = self.archive.episode_turns(episode.id)?.ok_or_else(|| {
                OwnerSessionError::Record("import Episode path is unavailable".into())
            })?;
            let covered_by_receipt = turns.iter().any(|turn| {
                mapped.contains(&(episode.conversation_id.as_str(), turn.node_id.as_str()))
            });
            if !covered_by_receipt {
                continue;
            }
            if !source_path_is_mapped(&mapped, &episode.conversation_id, &turns) {
                return Err(OwnerSessionError::Record(
                    "import Episode path escapes its receipt source set".into(),
                ));
            }
            let work_id = WorkEpisodeId(episode.id.0);
            let work = self.insomnia.work(work_id).ok_or_else(|| {
                OwnerSessionError::Record("import Episode has no durable Insomnia work".into())
            })?;
            if work.priority != InsomniaPriority::Import
                || work.state != reliquary_insomnia_store::InsomniaWorkState::Complete
            {
                return Err(OwnerSessionError::Record(
                    "import completion requires settled imported Episode work".into(),
                ));
            }
            for turn in turns {
                covered.insert((episode.conversation_id.clone(), turn.node_id));
            }
        }
        if mapped.iter().any(|(conversation, node)| {
            !covered.contains(&(conversation.to_string(), node.to_string()))
        }) {
            return Err(OwnerSessionError::Record(
                "import receipt has source turns without completed import Episodes".into(),
            ));
        }
        Ok(())
    }
}

fn hex_digest_bytes(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
