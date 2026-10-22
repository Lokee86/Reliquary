//! Selected physical reclamation and transformed-copy publication.
use super::reclamation_scan::ReclamationScan;
use crate::{Container, ObjectRef};
use std::{collections::BTreeMap, fs, path::Path};

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

pub(crate) fn reclaim_container(
    container: &mut Container,
    output_path: &Path,
) -> Result<(StorageReclamationReport, Container), String> {
    container.sync().map_err(|error| error.to_string())?;
    let source_file_bytes = fs::metadata(container.path())
        .map_err(|error| error.to_string())?
        .len();
    let objects = container.chunks().map_err(|error| error.to_string())?;

    let mut scan = ReclamationScan::default();
    for object in &objects {
        let payload = container.read(*object).map_err(|error| error.to_string())?;
        scan.ingest(*object, &payload)?;
    }
    let plan = scan.finish();

    let mut output = container
        .create_empty_like(output_path)
        .map_err(|error| error.to_string())?;
    let output_guard = output.read_source().map_err(|error| error.to_string())?;
    let mut relocated = BTreeMap::<ObjectRef, ObjectRef>::new();
    let mut copied_chunks = 0_usize;
    let mut reclaimed_payload_bytes = 0_u64;
    let copy_result = (|| {
        for object in objects {
            if plan.drop.contains(&object) {
                reclaimed_payload_bytes = reclaimed_payload_bytes
                    .checked_add(container.object_capacity(object))
                    .ok_or_else(|| "reclaimed payload byte overflow".to_owned())?;
                continue;
            }
            let payload = container.read(object).map_err(|error| error.to_string())?;
            let payload = crate::maintenance::relocation::relocate_payload(&payload, &relocated)?;
            let destination = output.append(&payload).map_err(|error| error.to_string())?;
            relocated.insert(object, destination);
            copied_chunks += 1;
        }
        output.sync().map_err(|error| error.to_string())
    })();
    if let Err(error) = copy_result {
        drop(output);
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        return Err(error);
    }
    let output_file_bytes = fs::metadata(output_path)
        .map_err(|error| error.to_string())?
        .len();
    if plan.drop.is_empty() && output_file_bytes != source_file_bytes {
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        return Err(format!(
            "reclamation changed file size without reclaimable chunks: source={source_file_bytes} output={output_file_bytes}"
        ));
    }
    if !plan.drop.is_empty() && output_file_bytes >= source_file_bytes {
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        return Err(format!(
            "reclamation did not shrink file: source={source_file_bytes} output={output_file_bytes}"
        ));
    }

    Ok((
        StorageReclamationReport {
            copied_chunks,
            reclaimed_chunks: plan.drop.len(),
            unpublished_backing_chunks: plan.unpublished_backing_chunks,
            orphan_content_chunks: plan.orphan_content_chunks,
            orphan_memory_body_chunks: plan.orphan_memory_body_chunks,
            unactivated_archive_vector_chunks: plan.unactivated_archive_vector_chunks,
            unreferenced_packed_vector_chunks: plan.unreferenced_packed_vector_chunks,
            free_compaction_chunks: plan.free_compaction_chunks,
            superseded_external_index_chunks: plan.superseded_external_index_chunks,
            reclaimed_payload_bytes,
            source_file_bytes,
            output_file_bytes,
        },
        output,
    ))
}
