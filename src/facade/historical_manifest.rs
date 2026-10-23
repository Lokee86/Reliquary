//! Facade read-view gathering for native historical Import manifest planning.
use crate::{Cva, CvaError};

pub(super) use crate::history::community_proof::community_proof;
pub(super) use crate::imports::historical_manifest::Manifest;

#[cfg(test)]
pub(super) fn build(cva: &mut Cva) -> Result<Manifest, CvaError> {
    build_with_fill(cva, false)
}

pub(super) fn build_with_fill(
    cva: &mut Cva,
    requested_complete: bool,
) -> Result<Manifest, CvaError> {
    let scope = if cva.open_import_epoch().is_some() {
        Some(cva.import_epoch_scope()?)
    } else {
        None
    };
    let history = crate::owner_lifecycle::HistoryReadAccess::new(&cva.container);
    let admission = crate::history::import_admission::ImportAdmissionReadParts {
        access: crate::owner_lifecycle::HistoryReadAccess::new(&cva.container),
        archive: &cva.archive,
        memories: &cva.memories,
        freshness: &cva.freshness,
        dream: &cva.dream_freshness,
    };
    let freshness =
        crate::owner_lifecycle::historical_freshness_read::HistoricalFreshnessReadAccess::new(
            &cva.container,
            &cva.freshness,
        );
    let dream = crate::history::dream_evidence::DreamEvidenceReadAccess::new(
        &history,
        &cva.graph,
        &cva.dream_freshness,
    );
    let vectors = crate::owner_lifecycle::semantic_source::HistoricalVectorReadAccess::new(
        crate::owner_lifecycle::semantic_source::SemanticSourceReadParts {
            container: &cva.container,
            archive: &cva.archive,
            freshness: &cva.freshness,
            generations: &cva.vector_generations,
            compatibility_profiles: &cva.compatibility_profiles,
            archive_vectors: &cva.archive_vectors,
            packed_vectors: &cva.packed_vectors,
        },
        &cva.memory_vectors,
    );
    #[cfg(feature = "freshness-performance-trace")]
    let mut traces = Vec::new();
    let result = crate::imports::historical_manifest::build(
        crate::imports::historical_manifest::HistoricalManifestReadViews {
            scope,
            history: &history,
            archive: &cva.archive,
            memories: &cva.memories,
            admission: &admission,
            freshness: &freshness,
            dream: &dream,
            vectors: &vectors,
            cut: crate::imports::historical_manifest::ManifestOwnerCut {
                owner_uuid: history.owner_uuid(),
                global_version: cva.latest_global_version(),
                archive_version: cva.archive_version(),
                activity_cut: cva.rel_turn_count(),
                memory_version: cva.memory_version(),
                graph_version: cva.graph_version(),
                memory_graph_version: cva.memory_graph_version(),
                vector_version: cva.vector_version(),
                community_snapshot: cva.community_snapshot(),
                insomnia: cva.insomnia_stats(),
            },
        },
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

pub(super) fn historical_import_id(owner: [u8; 16], corpus: [u8; 32]) -> [u8; 32] {
    crate::imports::historical_manifest::historical_import_id(owner, corpus)
}
