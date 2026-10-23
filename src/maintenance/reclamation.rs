//! Conservative reclamation policy over a frozen owner-lifecycle source.
use super::reclamation_scan::{ReclamationPlan, ReclamationScan};
use crate::Container;
use crate::ObjectRef;
use crate::owner_lifecycle::{
    HistorySource, PhylacteryReclamationCopy, PhylacteryReclamationSource, ReclamationCopyStats,
    ReliquaryReclamationCopy, ReliquaryReclamationSource,
};
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageReclamationReport {
    pub copied_chunks: usize,
    pub reclaimed_chunks: usize,
    pub unpublished_backing_chunks: usize,
    pub orphan_content_chunks: usize,
    pub orphan_memory_body_chunks: usize,
    pub unactivated_archive_vector_chunks: usize,
    pub unreferenced_packed_vector_chunks: usize,
    pub free_compaction_chunks: usize,
    pub superseded_external_index_chunks: usize,
    pub reclaimed_payload_bytes: u64,
    pub source_file_bytes: u64,
    pub output_file_bytes: u64,
}

fn plan(source: &HistorySource<'_>, objects: &[ObjectRef]) -> Result<ReclamationPlan, String> {
    let mut scan = ReclamationScan::default();
    for object in objects {
        scan.ingest(*object, &source.read(*object)?)?;
    }
    Ok(scan.finish())
}

pub(crate) fn reclaim_reliquary(
    source: ReliquaryReclamationSource<'_>,
    output: &Path,
) -> Result<(StorageReclamationReport, ReliquaryReclamationCopy), String> {
    let plan = plan(&source.history_source()?, source.objects())?;
    let copy = source.copy(&plan.drop, output)?;
    Ok((report(&plan, copy.stats()), copy))
}
pub(crate) fn reclaim_phylactery(
    source: PhylacteryReclamationSource<'_>,
    output: &Path,
) -> Result<(StorageReclamationReport, PhylacteryReclamationCopy), String> {
    let plan = plan(&source.history_source()?, source.objects())?;
    let copy = source.copy(&plan.drop, output)?;
    Ok((report(&plan, copy.stats()), copy))
}
fn report(plan: &ReclamationPlan, stats: ReclamationCopyStats) -> StorageReclamationReport {
    StorageReclamationReport {
        copied_chunks: stats.copied_chunks,
        reclaimed_chunks: plan.drop.len(),
        unpublished_backing_chunks: plan.unpublished_backing_chunks,
        orphan_content_chunks: plan.orphan_content_chunks,
        orphan_memory_body_chunks: plan.orphan_memory_body_chunks,
        unactivated_archive_vector_chunks: plan.unactivated_archive_vector_chunks,
        unreferenced_packed_vector_chunks: plan.unreferenced_packed_vector_chunks,
        free_compaction_chunks: plan.free_compaction_chunks,
        superseded_external_index_chunks: plan.superseded_external_index_chunks,
        reclaimed_payload_bytes: stats.reclaimed_payload_bytes,
        source_file_bytes: stats.source_file_bytes,
        output_file_bytes: stats.output_file_bytes,
    }
}

/// Borrowed accepted REL owner facts used to validate a reclaimed copy.
/// The physical source is prepared only after the semantic-evidence refusal.
pub(crate) struct ReliquaryReclamationParts<'a> {
    pub container: &'a mut Container,
    pub archive: &'a crate::Archive,
    pub memories: &'a crate::memory_store::MemoryStore,
    pub entities: &'a crate::entity_store::EntityStore,
    pub relationships: &'a crate::relationship_store::RelationshipStore,
    pub graph: &'a crate::graph_store::GraphStore,
    pub vector_generations: &'a crate::vector_generation_store::VectorGenerationStore,
    pub freshness: &'a crate::freshness_storage::FreshnessStore,
    pub import_epochs: &'a crate::imports::epoch_store::ImportEpochStore,
}

pub(crate) fn reclaim_reliquary_parts(
    parts: ReliquaryReclamationParts<'_>,
    output_path: &Path,
) -> Result<StorageReclamationReport, crate::owner_lifecycle::OwnerError> {
    use crate::owner_lifecycle::{HistoryReadAccess, OwnerError, ReliquaryReclamationSource};

    if parts
        .freshness
        .import_settlements()
        .iter()
        .any(|settlement| settlement.semantic.is_none())
    {
        return Err(OwnerError::Repack(
            "current import settlement semantic evidence is missing".into(),
        ));
    }
    let expected = ReclamationSemanticState {
        owner_uuid: parts.container.owner_uuid(),
        global_version: parts.container.latest_version(),
        memory_version: parts.memories.memory_version(),
        entity_version: parts.entities.entity_version(),
        relationship_version: parts.relationships.relationship_version(),
        graph_version: parts.graph.graph_version(),
        archive_version: parts.archive.archive_version(),
        vector_version: parts.vector_generations.vector_version(),
    };
    let source_archive = parts.archive;
    let (report, copy) = reclaim_reliquary(
        ReliquaryReclamationSource::prepare(parts.container).map_err(OwnerError::Repack)?,
        output_path,
    )
    .map_err(OwnerError::Repack)?;
    let source_read = HistoryReadAccess::new(parts.container);
    let output = copy.into_reliquary_container();
    let output_guard = output.read_source()?;
    let (reopened, _due) = match super::reliquary_prune::reopen_reliquary(output) {
        Ok(value) => value,
        Err(error) => {
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            return Err(error);
        }
    };
    let source_view = crate::history::HistoricalCopyView {
        archive: source_archive,
        graph: parts.graph,
        freshness: parts.freshness,
        import_epochs: parts.import_epochs,
        read_access: &source_read,
        accepted_global_head: expected.global_version,
    };
    let reopened_read = reopened.history_read_access();
    let copy_view = crate::history::HistoricalCopyView {
        archive: &reopened.archive,
        graph: &reopened.graph,
        freshness: &reopened.freshness,
        import_epochs: &reopened.import_epochs,
        read_access: &reopened_read,
        accepted_global_head: reopened.latest_global_version(),
    };
    let preserved =
        match crate::history::historical_copy_preserves_import_authority(source_view, copy_view) {
            Ok(value) => value,
            Err(error) => {
                drop(reopened);
                if output_guard.validate_path_identity().is_ok() {
                    let _ = fs::remove_file(output_path);
                }
                return Err(error);
            }
        };
    if !preserved {
        drop(reopened);
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        return Err(OwnerError::Repack(
            "physical transformation changed historical checkpoint scoring or receipt/proof authority"
                .into(),
        ));
    }
    let actual = ReclamationSemanticState {
        owner_uuid: reopened.owner_uuid(),
        global_version: reopened.latest_global_version(),
        memory_version: reopened.memories.memory_version(),
        entity_version: reopened.entities.entity_version(),
        relationship_version: reopened.relationships.relationship_version(),
        graph_version: reopened.graph.graph_version(),
        archive_version: reopened.archive.archive_version(),
        vector_version: reopened.vector_generations.vector_version(),
    };
    let activity_matches = source_archive.activity_history_matches(&reopened.archive);
    drop(reopened);
    if actual != expected || !activity_matches {
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        return Err(OwnerError::Repack(
            "reclaimed REL changed owner, semantic versions or activity history".into(),
        ));
    }
    output_guard.validate_path_identity()?;
    Ok(report)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReclamationSemanticState {
    owner_uuid: Option<[u8; 16]>,
    global_version: u64,
    memory_version: u64,
    entity_version: u64,
    relationship_version: u64,
    graph_version: u64,
    archive_version: u64,
    vector_version: u64,
}
