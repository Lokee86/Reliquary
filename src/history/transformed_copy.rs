//! Read-only authority comparison for a physically transformed REL copy.

use crate::archive::Archive;
use crate::freshness_storage::FreshnessStore;
use crate::graph_store::GraphStore;
use crate::imports::epoch_store::ImportEpochStore;
use crate::owner_lifecycle::{HistoryReadAccess, OwnerError};

/// Owner facts needed to decide whether a physical copy retained historical
/// settlement, scoring, and receipt authority.
pub(crate) struct HistoricalCopyView<'a> {
    pub archive: &'a Archive,
    pub graph: &'a GraphStore,
    pub freshness: &'a FreshnessStore,
    pub import_epochs: &'a ImportEpochStore,
    pub read_access: &'a HistoryReadAccess<'a>,
    pub accepted_global_head: u64,
}

/// Compare source and transformed-copy authority in the original short-circuit
/// order. Source snapshots are opened only after records_at succeeds on both.
pub(crate) fn historical_copy_preserves_import_authority(
    source: HistoricalCopyView<'_>,
    copy: HistoricalCopyView<'_>,
) -> Result<bool, OwnerError> {
    if source.freshness.import_settlements().is_empty()
        && copy.freshness.import_settlements().is_empty()
    {
        return Ok(true);
    }
    if source.freshness.import_settlements() != copy.freshness.import_settlements()
        || source.import_epochs.statuses() != copy.import_epochs.statuses()
        || source.freshness.policy() != copy.freshness.policy()
        || source.archive.rel_turn_count() != copy.archive.rel_turn_count()
        || source.accepted_global_head != copy.accepted_global_head
        || source.archive.archive_version() != copy.archive.archive_version()
        || source.graph.graph_version() != copy.graph.graph_version()
        || source.graph.memory_graph_version() != copy.graph.memory_graph_version()
        || source
            .freshness
            .records_at(source.archive.rel_turn_count())
            .map_err(OwnerError::Freshness)?
            != copy
                .freshness
                .records_at(copy.archive.rel_turn_count())
                .map_err(OwnerError::Freshness)?
        || crate::history::historical_owner_evidence_digest(
            &source
                .read_access
                .snapshot()
                .map_err(|error| OwnerError::Container(error.0))?,
        )
        .map_err(|error| OwnerError::Container(error.0))?
            != crate::history::historical_owner_evidence_digest(
                &copy
                    .read_access
                    .snapshot()
                    .map_err(|error| OwnerError::Container(error.0))?,
            )
            .map_err(|error| OwnerError::Container(error.0))?
    {
        return Ok(false);
    }
    Ok(true)
}
