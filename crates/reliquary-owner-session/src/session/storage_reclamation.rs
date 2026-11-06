//! Verified reclamation of unreachable objects in the admitted 2.1 families.

use super::*;
use crate::{VerifiedOwnerCopyPlan, staged_copy::StagedCopy};
use reliquary_vector_store::generation::{generation_receipt_target, relocate_generation_receipt};
use sha2::{Digest, Sha256};
use std::fs;

mod scan;
use scan::{for_each_frame, open_state, reject_unactivated_generations, removable_backing};

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

/// Rebuild a separately admitted copy. Keep all accepted generations and
/// mappings, including historical bindings under retired profiles.
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
    reject_unactivated_generations(&source)?;
    let (common, archive_vectors, archive_files) = open_state(&source, admission)?;
    let mut referenced_packed = common
        .memory_vectors
        .infos()
        .into_iter()
        .map(|info| info.packed_vector_id.0)
        .collect::<BTreeSet<_>>();
    referenced_packed.extend(
        archive_vectors
            .into_iter()
            .map(|info| info.packed_vector_id.0),
    );
    let mut removed = BTreeSet::new();
    let mut gross = 0u64;
    for_each_frame(&source, |object, bytes| {
        if removable_backing(bytes, &common.memories, &referenced_packed, &archive_files)? {
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
    Ok(())
}

fn hash_frame(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

#[cfg(test)]
#[path = "storage_reclamation/tests.rs"]
mod tests;
