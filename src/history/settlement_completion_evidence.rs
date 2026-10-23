//! Read-only accepted evidence used by the single Import completion sequence.
use crate::Archive;
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::FreshnessStore;
use crate::freshness_storage::HistoricalImportFreshnessSettlement;
use crate::graph_store::GraphStore;
use crate::imports::ImportEpochStore;
use crate::memory_store::MemoryStore;
use crate::owner_lifecycle::{HistoryReadAccess, OwnerError};

/// Narrow immutable source capability for retained-settlement validation and
/// the one settlement selected for an Import completion request.
pub(crate) struct SettlementCompletionEvidence<'a> {
    archive: &'a Archive,
    memories: &'a MemoryStore,
    graph: &'a GraphStore,
    freshness: &'a FreshnessStore,
    dream: &'a DreamFreshnessJournal,
}

impl<'a> SettlementCompletionEvidence<'a> {
    pub(crate) fn new(
        archive: &'a Archive,
        memories: &'a MemoryStore,
        graph: &'a GraphStore,
        freshness: &'a FreshnessStore,
        dream: &'a DreamFreshnessJournal,
    ) -> Self {
        Self {
            archive,
            memories,
            graph,
            freshness,
            dream,
        }
    }

    pub(crate) fn validate_retained(
        &self,
        source: &HistoryReadAccess<'_>,
        epochs: &ImportEpochStore,
    ) -> Result<(), OwnerError> {
        for settlement in self.freshness.import_settlements().to_vec() {
            crate::history::validate_historical_settlement(
                source,
                self.archive,
                self.memories,
                self.graph,
                self.freshness,
                self.dream,
                epochs,
                &settlement,
            )?;
        }
        Ok(())
    }

    pub(crate) fn settlement(
        &self,
        import_id: &str,
    ) -> Option<HistoricalImportFreshnessSettlement> {
        self.freshness
            .import_settlements()
            .iter()
            .find(|settlement| settlement_id(settlement) == import_id)
            .cloned()
    }
}

fn settlement_id(settlement: &HistoricalImportFreshnessSettlement) -> String {
    settlement
        .import_id
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
