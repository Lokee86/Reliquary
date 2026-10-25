//! A frozen one-shot native REL import and its atomic Freshness publication.
#[cfg(test)]
use crate::freshness::FreshnessPolicy;
use crate::freshness_storage::{
    HistoricalImportFreshnessScope, HistoricalImportFreshnessSettlement,
};
use crate::history::manifest_encoding as encoding;
use crate::{Cva, CvaError, MemoryId, freshness::FreshnessRecord};
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
#[cfg(test)]
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
#[cfg(test)]
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
        self.runtime_freeze_historical_import(requested_complete)
    }
    /// Atomically replace provisional scoring. Exact accepted retries resolve before
    /// input drift checks and never erase legitimate post-checkpoint live events.
    fn commit_historical_import(
        &mut self,
        prepared: &PreparedHistoricalImport,
    ) -> Result<bool, CvaError> {
        self.runtime_commit_historical_import(prepared)
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
        crate::imports::historical_manifest_plan::validate_legacy_owner_policy(
            self.owner_uuid(),
            self.rel_turn_count(),
            self.open_import_epoch().is_some(),
            self.freshness_policy(),
            self.freshness_has_pending_dream_passes(),
            self.insomnia_stats(),
        )
    }
}

#[cfg(test)]
#[path = "historical_import_attribution_tests.rs"]
mod attribution_tests;
