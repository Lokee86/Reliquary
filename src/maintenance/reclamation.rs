//! Conservative reclamation policy over a frozen owner-lifecycle source.
use super::reclamation_scan::{ReclamationPlan, ReclamationScan};
use crate::ObjectRef;
use crate::owner_lifecycle::{
    HistorySource, PhylacteryReclamationCopy, PhylacteryReclamationSource, ReclamationCopyStats,
    ReliquaryReclamationCopy, ReliquaryReclamationSource,
};
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
