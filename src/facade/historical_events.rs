//! Facade adapter: gather ordered read capabilities and dispatch Import planning.
use super::manifest::Manifest;
use crate::imports::historical_events::{self, HistoricalEventReadAccess};
use crate::owner_lifecycle::semantic_source::{
    HistoricalVectorReadAccess, SemanticSourceReadParts,
};
use crate::{Cva, CvaError};

pub(super) use crate::imports::historical_events::NativeHistoricalEvents;

pub(super) fn build(
    cva: &mut Cva,
    manifest: &Manifest,
) -> Result<NativeHistoricalEvents, CvaError> {
    let owner_uuid = crate::owner_lifecycle::HistoryReadAccess::new(&cva.container)
        .owner_uuid()
        .ok_or_else(|| CvaError::Freshness("missing historical event owner".into()))?;
    let owner_id = crate::owner_lifecycle::HistoryReadAccess::new(&cva.container)
        .owner_id()
        .ok_or_else(|| CvaError::Freshness("missing historical event owner identity".into()))?;
    // Preserve the original scope-resolution checkpoint: only after identity.
    let epoch_scope = if cva.open_import_epoch().is_some() {
        Some(cva.import_epoch_scope()?)
    } else {
        None
    };
    let history = crate::owner_lifecycle::HistoryReadAccess::new(&cva.container);
    let vectors = HistoricalVectorReadAccess::new(
        SemanticSourceReadParts {
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
    let dream = crate::history::dream_evidence::DreamEvidenceReadAccess::new(
        &history,
        &cva.graph,
        &cva.dream_freshness,
    );
    let access = HistoricalEventReadAccess {
        owner_uuid,
        owner_id: &owner_id,
        epoch_scope,
        history: &history,
        archive: &cva.archive,
        memories: &cva.memories,
        vectors: &vectors,
        dream: &dream,
    };
    let outcome = historical_events::build(&access, manifest);
    #[cfg(feature = "freshness-performance-trace")]
    for trace in outcome.traces {
        crate::cva_freshness::record_dream_evidence_trace(trace);
    }
    outcome.result
}
