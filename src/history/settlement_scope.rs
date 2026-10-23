//! Accepted settlement availability, epoch binding and legacy birth proof.
use crate::Archive;
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::{FreshnessStore, HistoricalImportFreshnessSettlement};
use crate::graph_store::GraphStore;
use crate::imports::epoch_store::ImportEpochStore;
use crate::memory_store::MemoryStore;
use crate::owner_lifecycle::{HistoryReadAccess, OwnerError};
use std::collections::BTreeSet;

pub(crate) enum SettlementHistoryKind {
    Framed,
    Initial,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_settlement_scope(
    access: &HistoryReadAccess<'_>,
    archive: &Archive,
    memories: &MemoryStore,
    graph: &GraphStore,
    freshness: &FreshnessStore,
    dream: &DreamFreshnessJournal,
    epochs: &ImportEpochStore,
    cp: &HistoricalImportFreshnessSettlement,
) -> Result<SettlementHistoryKind, OwnerError> {
    let fail = |message: &str| OwnerError::Freshness(message.into());
    if access.owner_uuid() != Some(cp.owner)
        || cp.settlement_cut > archive.rel_turn_count()
        || cp.global_version > access.latest_global_version()
        || cp.graph_version > graph.graph_version()
        || cp.memory_graph_version > graph.memory_graph_version()
        || cp.archive_version > archive.archive_version()
    {
        return Err(fail("import checkpoint input cut is unavailable"));
    }
    let scope = cp
        .semantic
        .as_ref()
        .ok_or_else(|| fail("accepted settlement lacks semantic authority"))?;
    cp.validate().map_err(OwnerError::Freshness)?;
    match super::prepare_semantic_import_scope(access.owner_uuid(), archive, epochs, cp, scope)? {
        super::PreparedSemanticImportScope::InitialHistory => {}
        super::PreparedSemanticImportScope::Framed(facts) => {
            let source = access
                .snapshot()
                .map_err(|error| OwnerError::Container(error.0))?;
            super::validate_semantic_import_births(&source, archive, memories, facts)?;
        }
    }
    freshness
        .validate_semantic_import_evidence(scope)
        .map_err(OwnerError::Freshness)?;
    let import_id: String = cp
        .import_id
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if epochs.status(&import_id).is_some() {
        return Ok(SettlementHistoryKind::Framed);
    }
    let original: BTreeSet<_> = memories
        .current_ids()
        .into_iter()
        .filter(|id| {
            memories
                .first_global_version(*id)
                .is_some_and(|version| version <= cp.global_version)
        })
        .collect();
    if original != cp.cohort {
        return Err(fail(
            "import checkpoint cohort differs from its accepted Memory prefix",
        ));
    }
    let source = access
        .snapshot()
        .map_err(|error| OwnerError::Container(error.0))?;
    let births =
        super::legacy_import_births(&source, archive, memories, cp.global_version, &cp.cohort)?;
    super::validate_legacy_singleton_births(
        &source,
        memories,
        dream,
        cp.global_version,
        &cp.cohort,
        &births,
    )?;
    Ok(SettlementHistoryKind::Initial)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_historical_settlement(
    access: &HistoryReadAccess<'_>,
    archive: &Archive,
    memories: &MemoryStore,
    graph: &GraphStore,
    freshness: &FreshnessStore,
    dream: &DreamFreshnessJournal,
    epochs: &ImportEpochStore,
    cp: &HistoricalImportFreshnessSettlement,
) -> Result<(), OwnerError> {
    match validate_settlement_scope(
        access, archive, memories, graph, freshness, dream, epochs, cp,
    )? {
        SettlementHistoryKind::Framed => return Ok(()),
        SettlementHistoryKind::Initial => {}
    }
    super::source_proof::accepted_archive_timeline(archive, cp.settlement_cut)?;
    let nodes = super::source_proof::required_nodes(archive, cp.settlement_cut)?;
    let digest = super::source_proof::corpus_digest_with_content(
        access,
        archive,
        nodes.into_values().collect(),
        "import source content proof mismatch",
    )?;
    if digest != cp.corpus_digest {
        return Err(OwnerError::Freshness(
            "import settlement corpus identity differs from its source prefix".into(),
        ));
    }
    Ok(())
}

/// Immutable owners needed for one retained historical settlement proof.
pub(crate) struct HistoricalSettlementReadOwners<'a> {
    archive: &'a crate::Archive,
    memories: &'a crate::memory_store::MemoryStore,
    graph: &'a crate::graph_store::GraphStore,
    dream: &'a crate::freshness_dream_journal::DreamFreshnessJournal,
    epochs: &'a crate::imports::ImportEpochStore,
    freshness: &'a crate::freshness_storage::FreshnessStore,
}
impl<'a> HistoricalSettlementReadOwners<'a> {
    pub(crate) fn new(
        archive: &'a crate::Archive,
        memories: &'a crate::memory_store::MemoryStore,
        graph: &'a crate::graph_store::GraphStore,
        dream: &'a crate::freshness_dream_journal::DreamFreshnessJournal,
        epochs: &'a crate::imports::ImportEpochStore,
        freshness: &'a crate::freshness_storage::FreshnessStore,
    ) -> Self {
        Self {
            archive,
            memories,
            graph,
            dream,
            epochs,
            freshness,
        }
    }
    pub(crate) fn epoch_scope(
        &self,
        source: &crate::owner_lifecycle::HistoryReadAccess<'_>,
        start: crate::imports::ImportStart,
    ) -> Result<crate::history::ImportEpochScope, crate::owner_lifecycle::OwnerError> {
        crate::history::read_import_epoch_scope(source, self.archive, self.memories, start)
    }
    pub(crate) fn validate(
        &self,
        source: &crate::owner_lifecycle::HistoryReadAccess<'_>,
        settlement: &crate::freshness_storage::HistoricalImportFreshnessSettlement,
    ) -> Result<(), crate::owner_lifecycle::OwnerError> {
        validate_historical_settlement(
            source,
            self.archive,
            self.memories,
            self.graph,
            self.freshness,
            self.dream,
            self.epochs,
            settlement,
        )
    }
}
