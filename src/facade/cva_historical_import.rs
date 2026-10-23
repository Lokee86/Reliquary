//! A frozen one-shot native REL import and its atomic Freshness publication.
use crate::freshness_storage::{
    HistoricalImportFreshnessScope, HistoricalImportFreshnessSettlement,
};
use crate::history::manifest_encoding as encoding;
use crate::{
    Cva, CvaError, MemoryId,
    freshness::{FreshnessPolicy, FreshnessRecord},
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;
use std::io::Read;
#[path = "historical_embedding.rs"]
mod embedding;
#[path = "historical_embedding_model.rs"]
mod embedding_model;
#[path = "historical_embedding_plan.rs"]
mod embedding_plan;
#[path = "historical_events.rs"]
mod events;
pub use embedding_model::{
    HistoricalEmbeddingCoverage, HistoricalEmbeddingDisposition, HistoricalEmbeddingFailure,
    HistoricalEmbeddingLimits, HistoricalEmbeddingReport,
};
#[path = "historical_import_base.rs"]
mod historical_base;
#[path = "historical_import_input.rs"]
mod input;
#[path = "historical_manifest.rs"]
mod manifest;
#[path = "historical_import_transform.rs"]
mod transform;

use crate::imports::historical_settlement_plan::{
    FrozenHistoricalImport, PreparedHistoricalImport,
};
fn fail(message: &str) -> CvaError {
    CvaError::Freshness(message.into())
}

impl From<crate::history::manifest_encoding::HistoryProofError> for CvaError {
    fn from(error: crate::history::manifest_encoding::HistoryProofError) -> Self {
        Self::Freshness(error.to_string())
    }
}

fn file_digest(path: &std::path::Path) -> Result<[u8; 32], CvaError> {
    let mut file = std::fs::File::open(path).map_err(|e| fail(&e.to_string()))?;
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let size = file.read(&mut bytes).map_err(|e| fail(&e.to_string()))?;
        if size == 0 {
            break;
        }
        hash.update(&bytes[..size]);
    }
    Ok(hash.finalize().into())
}
// All ordinary backing records are immutable except Archive conversation
// compaction allocations. Bind physical coordinates as well as payloads so
// relocation cannot silently preserve an invalid epoch boundary.

impl Cva {
    /// Private checkpoint primitive. The owner coordinator derives the
    /// verified manifest and frozen owner cut; this is not a caller-facing API.
    fn freeze_historical_import(&mut self) -> Result<FrozenHistoricalImport, CvaError> {
        self.freeze_historical_import_with_fill(false)
    }
    fn freeze_historical_import_with_fill(
        &mut self,
        requested_complete: bool,
    ) -> Result<FrozenHistoricalImport, CvaError> {
        // An earlier import's settlement is not a retry of the open epoch.
        // Resolve the durable epoch identity before considering accepted evidence.
        let accepted = if let Some(start) = self.open_import_epoch() {
            self.freshness
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
        } else {
            self.freshness.latest_import_settlement()
        };
        if let Some(settlement) = accepted {
            let settlement = settlement.clone();
            if self.open_import_epoch().is_some()
                && self.import_epoch_scope()?.start_digest != settlement.start_evidence_digest
            {
                return Err(fail("import settlement start evidence mismatch"));
            }
            let mut initial = BTreeMap::new();
            for (id, record) in &settlement.records {
                let mut birth = FreshnessRecord::created();
                if let Some(at) = record.admitted_at_turn {
                    birth.admit(at).map_err(|e| fail(&e.to_string()))?;
                }
                initial.insert(*id, birth);
            }
            return Ok(FrozenHistoricalImport {
                source_bytes: 0,
                source_digest: [0; 32],
                retained: Vec::new(),
                epoch: false,
                events: None,
                propagation: None,
                settlement,
                initial,
            });
        }
        let manifest = manifest::build_with_fill(self, requested_complete)?;
        let owner = self
            .owner_uuid()
            .ok_or_else(|| fail("historical import requires a durable REL owner"))?;
        let events = events::build(self, &manifest)?;
        let cohort = manifest.cohort.clone();
        self.container.sync()?;
        let settlement = HistoricalImportFreshnessSettlement::from_import_scope(
            HistoricalImportFreshnessScope {
                owner,
                import_id: manifest.import_id,
                start_evidence_digest: manifest.start_evidence_digest,
                base_cut: manifest.base_cut,
                settlement_cut: self.rel_turn_count(),
            },
            manifest.corpus_digest,
            manifest.digest,
            manifest.digest,
            cohort,
            self.latest_global_version(),
            manifest.input_bytes,
            self.archive_version(),
            self.graph_version(),
            self.memory_graph_version(),
            BTreeMap::new(),
            manifest.superseded,
        );
        let mut frozen = FrozenHistoricalImport {
            source_bytes: manifest.input_bytes,
            source_digest: manifest.input_digest,
            retained: Vec::new(),
            epoch: manifest.epoch,
            events: Some(events),
            propagation: Some(manifest.propagation),
            settlement,
            initial: manifest.initial,
        };
        if frozen.epoch {
            // The manifest binds provisional attribution and the full recipient proof.
            // Bind planned propagation recipients separately before retaining result records.
            let affected = frozen
                .native_scores()?
                .into_iter()
                .map(|event| event.recipient);
            frozen.settlement.cohort.extend(affected);
            frozen
                .initial
                .retain(|id, _| frozen.settlement.cohort.contains(id));
            let mut binding = Sha256::new();
            binding.update(b"import-epoch-result-cohort-v1");
            binding.update(frozen.settlement.manifest_digest);
            for id in &frozen.settlement.cohort {
                binding.update(id.0);
            }
            frozen.settlement.manifest_digest = binding.finalize().into();
            let scope = self.import_epoch_scope()?;
            let replay = crate::freshness_storage::ImportEventScope {
                owner,
                import_id: frozen.settlement.import_id,
                start_evidence_digest: scope.start_digest,
                born_memories: scope.cohort,
                affected_recipients: frozen
                    .propagation
                    .as_ref()
                    .unwrap()
                    .availability
                    .keys()
                    .copied()
                    .collect(),
                source_nodes: scope.source_nodes,
                start_prefix: scope.start_offset,
                freeze_prefix: frozen.settlement.input_bytes,
                base_cut: scope.base_cut,
                freeze_cut: frozen.cut(),
            };
            let mut history = self.freshness.clone();
            history
                .bind_history_source(&self.container, replay.freeze_prefix)
                .map_err(CvaError::Freshness)?;
            frozen.retained = history
                .retained_import_scores(
                    &replay,
                    &frozen.settlement.cohort,
                    &frozen.settlement.superseded,
                )
                .map_err(CvaError::Freshness)?;
            frozen.settlement.semantic = Some(
                history
                    .semantic_import_evidence(
                        &replay,
                        &frozen.settlement.cohort,
                        &frozen.settlement.superseded,
                    )
                    .map_err(CvaError::Freshness)?,
            );
        } else {
            let source_nodes = input::required_nodes(&self.archive, frozen.cut())?
                .into_keys()
                .collect();
            let replay = crate::freshness_storage::ImportEventScope {
                owner,
                import_id: frozen.settlement.import_id,
                start_evidence_digest: frozen.settlement.start_evidence_digest,
                born_memories: frozen.settlement.cohort.clone(),
                affected_recipients: frozen.settlement.cohort.clone(),
                source_nodes,
                start_prefix: 0,
                freeze_prefix: frozen.source_bytes,
                base_cut: frozen.settlement.base_cut,
                freeze_cut: frozen.cut(),
            };
            let mut history = self.freshness.clone();
            history
                .bind_history_source(&self.container, frozen.source_bytes)
                .map_err(CvaError::Freshness)?;
            frozen.settlement.semantic = Some(
                history
                    .semantic_import_evidence(
                        &replay,
                        &frozen.settlement.cohort,
                        &frozen.settlement.superseded,
                    )
                    .map_err(CvaError::Freshness)?,
            );
        }
        frozen.settlement.input_bytes = 0;
        Ok(frozen)
    }
    /// Atomically replace provisional scoring. Exact accepted retries resolve before
    /// input drift checks and never erase legitimate post-checkpoint live events.
    fn commit_historical_import(
        &mut self,
        prepared: &PreparedHistoricalImport,
    ) -> Result<bool, CvaError> {
        let settlement = &prepared.settlement;
        if self.owner_uuid() != Some(settlement.owner) {
            return Err(fail("import settlement owner mismatch"));
        }
        if let Some(old) = self.freshness.import_settlement(settlement.import_id) {
            return if old == settlement {
                Ok(false)
            } else {
                Err(fail("ImportManifestConflict"))
            };
        }
        if settlement.semantic.is_none() {
            return Err(fail("historical settlement representation/scope mismatch"));
        }
        if self.latest_global_version() != settlement.global_version
            || self.archive_version() != settlement.archive_version
            || self.rel_turn_count() != settlement.settlement_cut
            || self.graph_version() != settlement.graph_version
            || self.memory_graph_version() != settlement.memory_graph_version
            || std::fs::metadata(self.container.path())
                .map_err(|e| fail(&e.to_string()))?
                .len()
                != prepared.source_bytes
            || file_digest(self.container.path())? != prepared.source_digest
        {
            return Err(fail("ImportInputDrift: discard and refreeze the attempt"));
        }
        if prepared.epoch {
            let current = self.import_epoch_scope()?;
            if current.start_digest != settlement.start_evidence_digest {
                return Err(fail(
                    "historical settlement does not belong to the open epoch",
                ));
            }
            input::validate_settlement(self, settlement)?;
        }
        let (staged, due) = crate::owner_lifecycle::historical_settlement::publish_settlement(
            &mut self.container,
            &self.freshness,
            settlement,
            prepared.source_bytes,
            prepared.epoch,
        )?;
        self.freshness = staged;
        self.freshness_due = due;
        self.freshness_notifications.clear();
        Ok(true)
    }
}

impl Cva {
    pub(crate) fn semantic_scope_for_import_settlement(
        &self,
        settlement: &HistoricalImportFreshnessSettlement,
    ) -> Result<crate::freshness_storage::ImportSemanticScope, CvaError> {
        settlement
            .semantic
            .clone()
            .ok_or_else(|| fail("accepted settlement lacks semantic authority"))
    }

    pub(crate) fn validate_historical_import_settlements(&mut self) -> Result<(), CvaError> {
        for settlement in self.freshness.import_settlements().to_vec() {
            input::validate_settlement(self, &settlement)?;
        }
        Ok(())
    }
}

#[path = "historical_reconstruction.rs"]
mod reconstruction;
pub use reconstruction::{
    HistoricalReconstructionError, HistoricalReconstructionOptions, HistoricalReconstructionReport,
};

#[cfg(test)]
#[path = "historical_import_tests.rs"]
mod tests;

impl Cva {
    fn historical_reconstruction_owner(&self) -> Result<[u8; 16], CvaError> {
        let owner = self
            .owner_uuid()
            .ok_or_else(|| fail("historical import requires a durable REL owner"))?;
        if (self.rel_turn_count() == 0 && self.open_import_epoch().is_none())
            || self.freshness_policy() != FreshnessPolicy::default_v1()
            || self.freshness_has_pending_dream_passes()
        {
            return Err(fail(
                "unsupported import owner/policy or unfinished generation",
            ));
        }
        let stats = self.insomnia_stats();
        if stats.pending + stats.processing + stats.failed + stats.terminal != 0 {
            return Err(fail("import Insomnia generation is not complete"));
        }
        Ok(owner)
    }
}

#[cfg(test)]
#[path = "historical_import_attribution_tests.rs"]
mod attribution_tests;
