//! Import-owned historical reconstruction for a concrete replayed REL owner.
use super::historical_reconstruction_model::{
    HistoricalReconstructionError, HistoricalReconstructionReport,
};
use super::historical_settlement_plan::{FrozenHistoricalImport, PreparedHistoricalImport};
use crate::owner_lifecycle::{OwnerError, ReliquaryOwner};

fn freeze(owner: &ReliquaryOwner) -> Result<FrozenHistoricalImport, OwnerError> {
    let requested_complete = false;
    let history = crate::owner_lifecycle::HistoryReadAccess::new(&owner.container);
    let admission = crate::history::import_admission::ImportAdmissionReadParts {
        access: crate::owner_lifecycle::HistoryReadAccess::new(&owner.container),
        archive: &owner.archive,
        memories: &owner.memories,
        freshness: &owner.freshness,
        dream: &owner.dream_freshness,
    };
    let freshness =
        crate::owner_lifecycle::historical_freshness_read::HistoricalFreshnessReadAccess::new(
            &owner.container,
            &owner.freshness,
        );
    let dream = crate::history::dream_evidence::DreamEvidenceReadAccess::new(
        &history,
        &owner.graph,
        &owner.dream_freshness,
    );
    let vectors = crate::owner_lifecycle::semantic_source::HistoricalVectorReadAccess::new(
        crate::owner_lifecycle::semantic_source::SemanticSourceReadParts {
            container: &owner.container,
            archive: &owner.archive,
            freshness: &owner.freshness,
            generations: &owner.vector_generations,
            compatibility_profiles: &owner.compatibility_profiles,
            archive_vectors: &owner.archive_vectors,
            packed_vectors: &owner.packed_vectors,
        },
        &owner.memory_vectors,
    );
    #[cfg(feature = "freshness-performance-trace")]
    let mut traces = Vec::new();
    let result = crate::imports::historical_prepare::prepare(
        crate::imports::historical_manifest::HistoricalManifestReadViews {
            history: &history,
            archive: &owner.archive,
            memories: &owner.memories,
            admission: &admission,
            freshness: &freshness,
            dream: &dream,
            vectors: &vectors,
            cut: crate::imports::historical_manifest::ManifestOwnerCut {
                owner_uuid: history.owner_uuid(),
                global_version: history.latest_global_version(),
                archive_version: owner.archive.archive_version(),
                activity_cut: owner.archive.rel_turn_count(),
                memory_version: owner.memories.memory_version(),
                graph_version: owner.graph.graph_version(),
                memory_graph_version: owner.graph.memory_graph_version(),
                vector_version: owner.vector_generations.vector_version(),
                community_snapshot: owner
                    .communities
                    .latest_if_live_members(&owner.graph)
                    .cloned(),
                insomnia: owner.insomnia.stats(),
            },
        },
        owner.import_epochs.open().cloned(),
        requested_complete,
        #[cfg(feature = "freshness-performance-trace")]
        &mut traces,
    );
    #[cfg(feature = "freshness-performance-trace")]
    let _ = traces;
    result
}

fn commit(
    owner: &mut ReliquaryOwner,
    prepared: &PreparedHistoricalImport,
) -> Result<bool, OwnerError> {
    let cut = crate::imports::historical_freeze::HistoricalOwnerCut {
        owner: prepared.settlement.owner,
        activity: owner.archive.rel_turn_count(),
        global: crate::owner_lifecycle::HistoryReadAccess::new(&owner.container)
            .latest_global_version(),
        archive: owner.archive.archive_version(),
        graph: owner.graph.graph_version(),
        memory_graph: owner.graph.memory_graph_version(),
    };
    let open_epoch = owner.import_epochs.open().cloned();
    let owners = crate::history::HistoricalSettlementReadOwners::new(
        &owner.archive,
        &owner.memories,
        &owner.graph,
        &owner.dream_freshness,
        &owner.import_epochs,
        &owner.freshness,
    );
    let mut writer =
        crate::owner_lifecycle::historical_settlement::HistoricalSettlementWriteAccess::new(
            &mut owner.container,
            &owner.freshness,
        );
    let Some(publication) =
        crate::imports::historical_commit::commit(&mut writer, &owners, prepared, cut, open_epoch)?
    else {
        return Ok(false);
    };
    drop(writer);
    let _ = publication.install(&mut owner.freshness);
    Ok(true)
}

pub(crate) fn reconstruct(
    owner: &mut ReliquaryOwner,
) -> Result<HistoricalReconstructionReport, HistoricalReconstructionError> {
    let frozen = freeze(owner)?;
    let prepared = super::historical_reconstruction::prepare_without_endpoint(frozen, false)?;
    let committed = commit(owner, &prepared)?;
    Ok(super::historical_reconstruction::report(
        &prepared, committed, None,
    ))
}
