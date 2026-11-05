//! Explicit transformed-copy pruning for exclusively owned terminal Memory bodies.
//! The first supported transform is intentionally confined to pointer-free
//! Memory format families. Unsupported source families fail closed.
use super::*;
use crate::records::memory_prune_proof::{
    MEMORY_BODY, frame_history_digest, memory_structure_digest, safe_memory_only_frames,
    tombstone_digest,
};
use crate::records::{
    HistoryPruneManifest, HistoryRetentionDescriptor, PrunedPayload, RetentionDomain,
    RetentionTarget, VerifiedOwnerCopyPlan,
};
use reliquary_memory_core::{MemoryBodyId, PayloadState};
use std::{collections::BTreeSet, fs, path::Path};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrunedOwnerCopyReport {
    pub owner_id: [u8; 16],
    pub pruned_memories: usize,
    pub source_bytes: u64,
    pub transformed_bytes: u64,
    pub manifest_digest: [u8; 32],
}

/// Write a new REL/PHY, never mutate or promote the source. Both the source
/// and output undergo owner admission. Physical rewrites are limited to known
/// pointer-free frame families, rather than guessing how to relocate offsets.
pub fn prune_terminal_memory_copy(
    plan: &VerifiedOwnerCopyPlan,
    source_path: &Path,
    destination_path: &Path,
    selected_memory_ids: &[MemoryId],
    graph_admission: &GraphAdmission,
) -> Result<PrunedOwnerCopyReport, String> {
    if selected_memory_ids.is_empty() || selected_memory_ids.len() > 128 {
        return Err("pruning requires between 1 and 128 explicit terminal Memories".into());
    }
    let source = Container::open_read_only(source_path).map_err(|e| e.to_string())?;
    plan.verify_source(&source, source_path)?;
    safe_memory_only_frames(&source)?;
    if !OwnerMaintenanceStore::open(&source)?.is_empty() {
        return Err("repeated destructive pruning requires a new lineage contract".into());
    }
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
    for id in selected {
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
                .collect::<Vec<_>>()
                != [id]
            || source_state
                .memory_vectors
                .referenced_body_ids()
                .any(|x| x.0 == body.0)
            || !bodies.insert(body)
        {
            return Err("selected Memory body is missing, shared, or referenced by vectors".into());
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
    // Identify every physical body before writing. Grouped or duplicate body
    // carriers cannot be removed by this format.
    let mut found = BTreeSet::new();
    for frame in source.read_frames().map_err(|e| e.to_string())? {
        if frame.payload.starts_with(MEMORY_BODY) && frame.payload.len() >= 40 {
            let id = MemoryBodyId(frame.payload[8..40].try_into().unwrap());
            if bodies.contains(&id) && !found.insert(id) {
                return Err("duplicate selected body frame".into());
            }
        }
    }
    if found != bodies {
        return Err("selected body is not an independently removable frame".into());
    }
    let source_bytes = fs::metadata(source_path).map_err(|e| e.to_string())?.len();
    let mut destination_created = false;
    let result = (|| {
        let mut target = Container::create(destination_path, source.identity(), source.owner_id())
            .map_err(|e| e.to_string())?;
        destination_created = true;
        let mut skipped = BTreeSet::new();
        for frame in source.read_frames().map_err(|e| e.to_string())? {
            if frame.payload.starts_with(MEMORY_BODY) && frame.payload.len() >= 40 {
                let id = MemoryBodyId(frame.payload[8..40].try_into().unwrap());
                if bodies.contains(&id) {
                    skipped.insert(id);
                    continue;
                }
            }
            target.append(&frame.payload).map_err(|e| e.to_string())?;
        }
        if skipped != bodies {
            return Err("prune copy skipped an unexpected set of bodies".into());
        }
        target.append(&manifest_bytes).map_err(|e| e.to_string())?;
        target.sync().map_err(|e| e.to_string())?;
        drop(target);

        plan.verify_source(&source, source_path)?;
        let copy = Container::open_read_only_as(destination_path, plan.file_kind())
            .map_err(|e| e.to_string())?;
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
        Ok(PrunedOwnerCopyReport {
            owner_id: source.owner_id(),
            pruned_memories: bodies.len(),
            source_bytes,
            transformed_bytes: fs::metadata(destination_path)
                .map_err(|e| e.to_string())?
                .len(),
            manifest_digest,
        })
    })();
    if result.is_err() && destination_created {
        let _ = fs::remove_file(destination_path);
    }
    result
}
#[cfg(test)]
#[path = "maintenance_prune/tests.rs"]
mod tests;
