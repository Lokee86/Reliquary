//! Explicit transformed-copy pruning for terminal Memory bodies.
//! Supported pointer-free payload families include standalone Memory bodies
//! and owner-admitted RELINSC7 grouped bodies. Unsupported source families fail closed.
use super::*;
use crate::records::memory_prune_proof::{
    MEMORY_BODY, frame_history_digest, grouped_completion_without_bodies, memory_structure_digest,
    safe_memory_only_frames, tombstone_digest,
};
use crate::records::{
    HistoryPruneManifest, HistoryRetentionDescriptor, PrunedPayload, RetentionDomain,
    RetentionTarget, VerifiedOwnerCopyPlan,
};
use crate::staged_copy::StagedCopy;
use reliquary_memory_core::{MemoryBodyId, PayloadState};
use reliquary_vector_store::generation::{generation_receipt_target, relocate_generation_receipt};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrunedOwnerCopyReport {
    pub owner_id: [u8; 16],
    pub pruned_memories: usize,
    pub source_bytes: u64,
    pub transformed_bytes: u64,
    pub manifest_digest: [u8; 32],
}

/// Publish a verified REL/PHY snapshot at a new path. Both the source
/// and output undergo owner admission. Physical rewrites are limited to known
/// pointer-free frame families, rather than guessing how to relocate offsets.
pub fn prune_terminal_memory_copy(
    plan: &VerifiedOwnerCopyPlan,
    source_path: &Path,
    destination_path: &Path,
    selected_memory_ids: &[MemoryId],
    graph_admission: &GraphAdmission,
) -> Result<PrunedOwnerCopyReport, String> {
    transform_terminal_memory_copy(
        plan,
        source_path,
        destination_path,
        selected_memory_ids,
        graph_admission,
        false,
    )
}

/// Replace an offline owner with an independently validated transformed copy.
/// The backup is created without overwriting any existing path and retains
/// the original inode. A failure before replacement leaves the source intact.
pub fn promote_terminal_memory_copy(
    plan: &VerifiedOwnerCopyPlan,
    source_path: &Path,
    backup_path: &Path,
    selected_memory_ids: &[MemoryId],
    graph_admission: &GraphAdmission,
) -> Result<PrunedOwnerCopyReport, String> {
    transform_terminal_memory_copy(
        plan,
        source_path,
        backup_path,
        selected_memory_ids,
        graph_admission,
        true,
    )
}

fn transform_terminal_memory_copy(
    plan: &VerifiedOwnerCopyPlan,
    source_path: &Path,
    destination_path: &Path,
    selected_memory_ids: &[MemoryId],
    graph_admission: &GraphAdmission,
    replace_source: bool,
) -> Result<PrunedOwnerCopyReport, String> {
    if selected_memory_ids.is_empty() || selected_memory_ids.len() > 128 {
        return Err("pruning requires between 1 and 128 explicit terminal Memories".into());
    }
    if destination_path.exists() {
        return Err("maintenance destination already exists".into());
    }
    let source = if replace_source {
        if fs::symlink_metadata(source_path)
            .map_err(|error| error.to_string())?
            .is_symlink()
        {
            return Err("promotion requires the physical source path".into());
        }
        Container::open_writer_as(source_path, plan.file_kind())
    } else {
        Container::open_read_only(source_path)
    }
    .map_err(|e| e.to_string())?;
    plan.verify_source(&source, source_path)?;
    let owner_id = source.owner_id();
    if !OwnerMaintenanceStore::open(&source)?.is_empty() {
        return Err("repeated destructive pruning requires a new lineage contract".into());
    }
    safe_memory_only_frames(&source, false)?;
    let source_state = match plan.file_kind() {
        FileKind::Reliquary => {
            let rel =
                RelSession::open(&source, graph_admission, [], []).map_err(|e| e.to_string())?;
            rel.common
        }
        FileKind::Phylactery => {
            let phy = PhySession::open(&source, graph_admission, []).map_err(|e| e.to_string())?;
            phy.common
        }
    };
    let selected: BTreeSet<_> = selected_memory_ids.iter().copied().collect();
    if selected.len() != selected_memory_ids.len() {
        return Err("duplicate selected Memory identity".into());
    }
    let mut bodies = BTreeSet::<MemoryBodyId>::new();
    let mut descriptors = Vec::with_capacity(selected.len());
    for &id in &selected {
        let history = source_state
            .memories
            .history(id)
            .map_err(|e| e.to_string())?;
        let tombstone = history
            .tombstone
            .ok_or("prune target must have a terminal Memory tombstone")?;
        let first = history
            .revisions
            .first()
            .ok_or("tombstoned history has no creation")?;
        let body = first.record.body_id;
        if !matches!(first.payload, PayloadState::Available(_))
            || !history
                .revisions
                .iter()
                .all(|revision| revision.record.body_id == body)
            || source_state
                .memories
                .memory_ids_for_body(body)
                .any(|reference| !selected.contains(&reference))
            || source_state
                .memory_vectors
                .referenced_body_ids()
                .any(|x| x.0 == body.0)
        {
            return Err("selected Memory body is missing, has an unselected reference, or is referenced by vectors".into());
        }
        // Each physical body has one descriptor. Every referring Memory is
        // checked above and its terminal records remain in the prefix digest.
        if !bodies.insert(body) {
            continue;
        }
        descriptors.push(HistoryRetentionDescriptor {
            target: RetentionTarget {
                domain: RetentionDomain::Memory,
                identity: id.0.to_vec(),
            },
            tombstone_global_version: None,
            tombstone_domain_version: Some(tombstone.memory_version),
            tombstone_digest: tombstone_digest(id, tombstone),
            retained_structure_digest: memory_structure_digest(&source, id)?,
            payloads: vec![PrunedPayload::MemoryBody(body.0)],
        });
    }
    descriptors.sort_by(|a, b| a.target.cmp(&b.target));
    let manifest = HistoryPruneManifest {
        owner_uuid: source.owner_id(),
        source_global_version: source.latest_version(),
        source_history_digest: frame_history_digest(&source, &bodies)?,
        descriptors,
    };
    let manifest_bytes = manifest.encode()?;
    let manifest_digest = manifest.digest()?;
    let source_bytes = fs::metadata(source_path).map_err(|e| e.to_string())?.len();
    let staged = StagedCopy::create(if replace_source {
        source_path
    } else {
        destination_path
    })?;
    let mut target = Container::create(&staged.path, source.identity(), source.owner_id())
        .map_err(|e| e.to_string())?;
    let mut skipped = BTreeSet::new();
    let mut removed_grouped_bodies = BTreeSet::new();
    let mut relocated_generations = BTreeMap::new();
    let mut cursor = source.physical_cursor().map_err(|e| e.to_string())?;
    let mut payload = Vec::new();
    while let Some(mut frame) = cursor.next_frame().map_err(|e| e.to_string())? {
        payload.clear();
        frame.read_to_end(&mut payload).map_err(|e| e.to_string())?;
        if payload.starts_with(b"RELINSC7") {
            let (rewritten, removed) = grouped_completion_without_bodies(&payload, &bodies)?;
            removed_grouped_bodies.extend(removed);
            payload = rewritten;
        }
        if payload.starts_with(MEMORY_BODY) && payload.len() >= 40 {
            let id = MemoryBodyId(payload[8..40].try_into().unwrap());
            if bodies.contains(&id) {
                if !skipped.insert(id) {
                    return Err("duplicate selected body frame".into());
                }
                continue;
            }
        }
        let is_generation = payload.starts_with(b"RELVGEN1");
        let relocated = if let Some(old) =
            generation_receipt_target(&payload).map_err(|error| error.to_string())?
        {
            let new = *relocated_generations
                .get(&old.offset)
                .ok_or("Memory prune lost vector generation")?;
            std::borrow::Cow::Owned(
                relocate_generation_receipt(&payload, new)
                    .map_err(|error| error.to_string())?
                    .to_vec(),
            )
        } else {
            std::borrow::Cow::Borrowed(payload.as_slice())
        };
        let written = target.append(&relocated).map_err(|e| e.to_string())?;
        if is_generation {
            relocated_generations.insert(frame.object().offset, written);
        }
    }
    skipped.extend(removed_grouped_bodies);
    if skipped != bodies {
        return Err("selected body has no recognized removable payload".into());
    }
    target.append(&manifest_bytes).map_err(|e| e.to_string())?;
    target.sync().map_err(|e| e.to_string())?;
    drop(target);

    plan.verify_source(&source, source_path)?;
    let copy =
        Container::open_read_only_as(&staged.path, plan.file_kind()).map_err(|e| e.to_string())?;
    if copy.identity() != source.identity()
        || copy.owner_id() != source.owner_id()
        || copy.latest_version() != source.latest_version()
        || frame_history_digest(&copy, &bodies)? != manifest.source_history_digest
    {
        return Err("transformed copy changed owner clocks or retained source history".into());
    }
    match plan.file_kind() {
        FileKind::Reliquary => {
            RelSession::open(&copy, graph_admission, [], []).map_err(|e| e.to_string())?;
        }
        FileKind::Phylactery => {
            PhySession::open(&copy, graph_admission, []).map_err(|e| e.to_string())?;
        }
    }
    plan.verify_source(&source, source_path)?;
    let transformed_bytes = fs::metadata(&staged.path).map_err(|e| e.to_string())?.len();
    if replace_source {
        fs::hard_link(source_path, destination_path)
            .map_err(|error| format!("could not retain original owner as backup: {error}"))?;
        plan.verify_source(&source, destination_path)?;
        plan.verify_source(&source, source_path)?;
        fs::rename(&staged.path, source_path).map_err(|error| {
            format!("could not replace verified owner; original retained at backup: {error}")
        })?;
    } else {
        let publication = VerifiedOwnerCopyPlan::prepare(&staged.path)?
            .copy_to(&staged.path, destination_path)
            .map_err(|error| format!("could not publish verified maintenance snapshot: {error}"))?;
        if publication.owner_id != source.owner_id()
            || publication.identity != source.identity()
            || publication.byte_len != transformed_bytes
        {
            let _ = fs::remove_file(destination_path);
            return Err("published maintenance snapshot differs from verified candidate".into());
        }
    }
    Ok(PrunedOwnerCopyReport {
        owner_id,
        pruned_memories: selected.len(),
        source_bytes,
        transformed_bytes,
        manifest_digest,
    })
}
#[cfg(test)]
#[path = "maintenance_prune/tests.rs"]
mod tests;
