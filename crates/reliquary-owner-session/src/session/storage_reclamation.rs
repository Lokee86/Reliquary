//! Verified reclamation of unreachable objects in the admitted 2.1 families.

use super::*;
use crate::{VerifiedOwnerCopyPlan, staged_copy::StagedCopy};
use reliquary_vector_store::generation::{generation_receipt_target, relocate_generation_receipt};
use sha2::{Digest, Sha256};
use std::fs;

mod external_index;
mod sanitize;
mod scan;
use external_index::scan_indexes;
use sanitize::write_filtered_without_manifest;
use scan::{for_each_frame, open_state, removable_backing, unactivated_generation_offsets};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageReclamationReport {
    pub removed_frames: usize,
    pub source_bytes: u64,
    pub candidate_bytes: u64,
}

#[derive(Clone, Copy, Default)]
struct ReclamationFaults {
    copy: bool,
    sync: bool,
    reopen: bool,
}

/// Rebuild a separately admitted copy. Keep every accepted generation and
/// its mapping, including historical bindings under retired profiles.
pub fn reclaim_storage_copy(
    plan: &VerifiedOwnerCopyPlan,
    source_path: &Path,
    destination: &Path,
    admission: &GraphAdmission,
) -> Result<StorageReclamationReport, String> {
    reclaim_storage_copy_inner(
        plan,
        source_path,
        destination,
        admission,
        ReclamationFaults::default(),
    )
}

fn reclaim_storage_copy_inner(
    plan: &VerifiedOwnerCopyPlan,
    source_path: &Path,
    destination: &Path,
    admission: &GraphAdmission,
    faults: ReclamationFaults,
) -> Result<StorageReclamationReport, String> {
    let source = Container::open_read_only(source_path).map_err(|e| e.to_string())?;
    plan.verify_source(&source, source_path)?;
    let orphan_generations = unactivated_generation_offsets(&source)?;
    // A valid but unreceipted generation cannot be opened through the normal
    // owner reader. Inspect its fully admitted semantic state after filtering
    // only those physically validated, unpublished generation frames.
    let scrubbed = if orphan_generations.is_empty() {
        None
    } else {
        let temp = StagedCopy::create(destination)?;
        write_filtered_without_manifest(&source, &temp.path, &orphan_generations)?;
        Some(temp)
    };
    let semantic_copy = if let Some(temp) = &scrubbed {
        Some(Container::open_read_only(&temp.path).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let (common, archive_vectors, retained_archive_vectors, archive_files) =
        open_state(semantic_copy.as_ref().unwrap_or(&source), admission)?;
    let external = scan_indexes(&source)?;
    let mut referenced_packed = common
        .memory_vectors
        .infos()
        .into_iter()
        .map(|info| info.packed_vector_id.0)
        .collect::<BTreeSet<_>>();
    referenced_packed.extend(
        archive_vectors
            .into_iter()
            .filter(|info| retained_archive_vectors.contains(&info.id.0))
            .map(|info| info.packed_vector_id.0),
    );
    referenced_packed.extend(external.packed_ids.iter().copied());
    let mut removed = BTreeSet::new();
    let mut gross = 0u64;
    for_each_frame(&source, |object, bytes| {
        if orphan_generations.contains(&object.offset)
            || external.discard_offsets.contains(&object.offset)
            || removable_backing(
                bytes,
                &common.memories,
                &referenced_packed,
                &retained_archive_vectors,
                &archive_files,
            )?
        {
            removed.insert(object.offset);
            gross += object.length + 8;
        }
        Ok(())
    })?;
    let source_bytes = fs::metadata(source_path).map_err(|e| e.to_string())?.len();
    let staged = StagedCopy::create(destination)?;
    // 8 bytes of frame overhead plus the fixed manifest body.
    if gross <= 8 + MANIFEST_BYTES {
        plan.copy_to(source_path, &staged.path)?;
        if faults.copy {
            return Err("injected reclamation copy failure".into());
        }
        if faults.sync {
            return Err("injected reclamation sync failure".into());
        }
        if faults.reopen {
            return Err("injected reclamation reopen failure".into());
        }
        let copy = Container::open_read_only(&staged.path).map_err(|e| e.to_string())?;
        open_state(&copy, admission)?;
        drop(copy);
        removed.clear();
    } else {
        write_candidate(plan, &source, &staged.path, &removed, admission, faults)?;
    }
    let candidate_bytes = fs::metadata(&staged.path).map_err(|e| e.to_string())?.len();
    plan.verify_source(&source, source_path)?;
    fs::hard_link(&staged.path, destination).map_err(|e| e.to_string())?;
    Ok(StorageReclamationReport {
        removed_frames: removed.len(),
        source_bytes,
        candidate_bytes,
    })
}

const MANIFEST_BYTES: u64 = 8 + 16 + 32 + 8 + 8 + 32;

fn write_candidate(
    plan: &VerifiedOwnerCopyPlan,
    source: &Container,
    path: &Path,
    removed: &BTreeSet<u64>,
    admission: &GraphAdmission,
    faults: ReclamationFaults,
) -> Result<(), String> {
    let mut target =
        Container::create(path, source.identity(), source.owner_id()).map_err(|e| e.to_string())?;
    let mut relocated = BTreeMap::new();
    let mut digest = Sha256::new();
    let mut copied_frame = false;
    for_each_frame(source, |object, bytes| {
        if removed.contains(&object.offset) {
            return Ok(());
        }
        let receipt;
        let bytes =
            if let Some(old) = generation_receipt_target(bytes).map_err(|e| e.to_string())? {
                let new = *relocated
                    .get(&old.offset)
                    .ok_or("generation receipt lost its target")?;
                receipt = relocate_generation_receipt(bytes, new).map_err(|e| e.to_string())?;
                receipt.as_slice()
            } else {
                bytes
            };
        let written = target.append(bytes).map_err(|e| e.to_string())?;
        if !copied_frame && faults.copy {
            return Err("injected reclamation copy failure".into());
        }
        copied_frame = true;
        if bytes.starts_with(b"RELVGEN1") {
            relocated.insert(object.offset, written);
        }
        hash_frame(&mut digest, bytes);
        Ok(())
    })?;
    let retained_digest = digest.finalize();
    let mut evidence = b"RELRECL1".to_vec();
    evidence.extend_from_slice(&source.owner_id());
    evidence.extend_from_slice(&plan.source_digest());
    evidence.extend_from_slice(&source.latest_version().to_le_bytes());
    evidence.extend_from_slice(&(removed.len() as u64).to_le_bytes());
    evidence.extend_from_slice(&retained_digest);
    target.append(&evidence).map_err(|e| e.to_string())?;
    if faults.sync {
        return Err("injected reclamation sync failure".into());
    }
    target.sync().map_err(|e| e.to_string())?;
    drop(target);

    if faults.reopen {
        return Err("injected reclamation reopen failure".into());
    }

    let copy = Container::open_read_only(path).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut saw_manifest = false;
    for_each_frame(&copy, |_, bytes| {
        if saw_manifest {
            return Err("reclamation manifest must terminate the candidate".into());
        }
        if bytes == evidence {
            saw_manifest = true;
        } else {
            hash_frame(&mut digest, bytes);
        }
        Ok(())
    })?;
    if !saw_manifest
        || digest.finalize() != retained_digest
        || copy.identity() != source.identity()
        || copy.owner_id() != source.owner_id()
        || copy.latest_version() != source.latest_version()
    {
        return Err("reclamation candidate changed retained owner history".into());
    }
    open_state(&copy, admission)?;
    if source.identity().file_kind == FileKind::Reliquary {
        let before = ExternalIndexStore::open(source)?;
        let after = ExternalIndexStore::open(&copy)?;
        if before.sources != after.sources || before.damaged != after.damaged {
            return Err("reclamation changed retained external-index state".into());
        }
    }
    Ok(())
}

fn hash_frame(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

#[cfg(test)]
#[path = "storage_reclamation/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "storage_reclamation/tests_external.rs"]
mod tests_external;
#[cfg(test)]
#[path = "storage_reclamation/tests_generations.rs"]
mod tests_generations;
