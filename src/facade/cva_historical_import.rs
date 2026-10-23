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
    crate::owner_lifecycle::physical_fingerprint::file_digest(path)
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
        let accepted =
            crate::owner_lifecycle::historical_freshness_read::HistoricalFreshnessReadAccess::new(
                &self.container,
                &self.freshness,
            )
            .accepted_settlement(
                self.open_import_epoch()
                    .as_ref()
                    .map(|start| start.import_id.as_str()),
            );
        if let Some(settlement) = accepted {
            if self.open_import_epoch().is_some()
                && self.import_epoch_scope()?.start_digest != settlement.start_evidence_digest
            {
                return Err(fail("import settlement start evidence mismatch"));
            }
            return crate::imports::historical_freeze::accepted_retry(settlement);
        }
        let manifest = manifest::build_with_fill(self, requested_complete)?;
        let owner = self
            .owner_uuid()
            .ok_or_else(|| fail("historical import requires a durable REL owner"))?;
        let events = events::build(self, &manifest)?;
        self.container.sync()?;
        let mut frozen = crate::imports::historical_freeze::from_manifest(
            manifest,
            events,
            crate::imports::historical_freeze::HistoricalOwnerCut {
                owner,
                activity: self.rel_turn_count(),
                global: self.latest_global_version(),
                archive: self.archive_version(),
                graph: self.graph_version(),
                memory_graph: self.memory_graph_version(),
            },
        );
        if frozen.epoch {
            crate::imports::historical_freeze::bind_result_cohort(&mut frozen)?;
            let scope = self.import_epoch_scope()?;
            let replay = crate::imports::historical_freeze::framed_scope(&frozen, scope);
            crate::imports::historical_freeze::attach_semantic_evidence(
                &mut frozen,
                &crate::owner_lifecycle::historical_freshness_read::HistoricalFreshnessReadAccess::new(&self.container, &self.freshness),
                &replay,
            )?;
        } else {
            let source_nodes = input::required_nodes(&self.archive, frozen.cut())?
                .into_keys()
                .collect();
            let replay = crate::imports::historical_freeze::unframed_scope(&frozen, source_nodes);
            crate::imports::historical_freeze::attach_semantic_evidence(
                &mut frozen,
                &crate::owner_lifecycle::historical_freshness_read::HistoricalFreshnessReadAccess::new(&self.container, &self.freshness),
                &replay,
            )?;
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
        if let Some(committed) = crate::imports::historical_freeze::admit_prepared(
            self.owner_uuid(),
            prepared,
            self.freshness.import_settlement(settlement.import_id),
        )? {
            return Ok(committed);
        }
        crate::imports::historical_freeze::validate_owner_cut(
            prepared,
            crate::imports::historical_freeze::HistoricalOwnerCut {
                owner: settlement.owner,
                activity: self.rel_turn_count(),
                global: self.latest_global_version(),
                archive: self.archive_version(),
                graph: self.graph_version(),
                memory_graph: self.memory_graph_version(),
            },
        )?;
        crate::imports::historical_freeze::validate_physical_freeze(
            prepared,
            &crate::owner_lifecycle::HistoryReadAccess::new(&self.container),
        )?;
        if prepared.epoch {
            let current = self.import_epoch_scope()?;
            crate::imports::historical_freeze::validate_start_binding(
                prepared,
                current.start_digest,
            )?;
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
