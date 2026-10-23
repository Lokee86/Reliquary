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
        let history = crate::owner_lifecycle::HistoryReadAccess::new(&self.container);
        let admission = crate::history::import_admission::ImportAdmissionReadParts {
            access: crate::owner_lifecycle::HistoryReadAccess::new(&self.container),
            archive: &self.archive,
            memories: &self.memories,
            freshness: &self.freshness,
            dream: &self.dream_freshness,
        };
        let freshness =
            crate::owner_lifecycle::historical_freshness_read::HistoricalFreshnessReadAccess::new(
                &self.container,
                &self.freshness,
            );
        let dream = crate::history::dream_evidence::DreamEvidenceReadAccess::new(
            &history,
            &self.graph,
            &self.dream_freshness,
        );
        let vectors = crate::owner_lifecycle::semantic_source::HistoricalVectorReadAccess::new(
            crate::owner_lifecycle::semantic_source::SemanticSourceReadParts {
                container: &self.container,
                archive: &self.archive,
                freshness: &self.freshness,
                generations: &self.vector_generations,
                compatibility_profiles: &self.compatibility_profiles,
                archive_vectors: &self.archive_vectors,
                packed_vectors: &self.packed_vectors,
            },
            &self.memory_vectors,
        );
        #[cfg(feature = "freshness-performance-trace")]
        let mut traces = Vec::new();
        let result = crate::imports::historical_prepare::prepare(
            crate::imports::historical_manifest::HistoricalManifestReadViews {
                history: &history,
                archive: &self.archive,
                memories: &self.memories,
                admission: &admission,
                freshness: &freshness,
                dream: &dream,
                vectors: &vectors,
                cut: crate::imports::historical_manifest::ManifestOwnerCut {
                    owner_uuid: history.owner_uuid(),
                    global_version: self.latest_global_version(),
                    archive_version: self.archive_version(),
                    activity_cut: self.rel_turn_count(),
                    memory_version: self.memory_version(),
                    graph_version: self.graph_version(),
                    memory_graph_version: self.memory_graph_version(),
                    vector_version: self.vector_version(),
                    community_snapshot: self.community_snapshot(),
                    insomnia: self.insomnia_stats(),
                },
            },
            self.open_import_epoch(),
            requested_complete,
            #[cfg(feature = "freshness-performance-trace")]
            &mut traces,
        );
        #[cfg(feature = "freshness-performance-trace")]
        for trace in traces {
            crate::cva_freshness::record_dream_evidence_trace(trace);
        }
        result
    }
    /// Atomically replace provisional scoring. Exact accepted retries resolve before
    /// input drift checks and never erase legitimate post-checkpoint live events.
    fn commit_historical_import(
        &mut self,
        prepared: &PreparedHistoricalImport,
    ) -> Result<bool, CvaError> {
        let cut = crate::imports::historical_freeze::HistoricalOwnerCut {
            owner: prepared.settlement.owner,
            activity: self.rel_turn_count(),
            global: self.latest_global_version(),
            archive: self.archive_version(),
            graph: self.graph_version(),
            memory_graph: self.memory_graph_version(),
        };
        let open_epoch = self.open_import_epoch();
        let owners = crate::history::HistoricalSettlementReadOwners::new(
            &self.archive,
            &self.memories,
            &self.graph,
            &self.dream_freshness,
            &self.import_epochs,
            &self.freshness,
        );
        let mut writer =
            crate::owner_lifecycle::historical_settlement::HistoricalSettlementWriteAccess::new(
                &mut self.container,
                &self.freshness,
            );
        let Some(publication) = crate::imports::historical_commit::commit(
            &mut writer,
            &owners,
            prepared,
            cut,
            open_epoch,
        )?
        else {
            return Ok(false);
        };
        drop(writer);
        self.freshness_due = publication.install(&mut self.freshness);
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
