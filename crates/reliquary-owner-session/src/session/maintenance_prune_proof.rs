//! Semantic admission for transformed, tombstone-backed Memory history.
use super::*;
use crate::records::memory_prune_proof::{
    MEMORY_BODY, frame_history_digest, memory_structure_digest, safe_memory_only_frames,
    tombstone_digest,
};
use crate::records::{OwnerMaintenanceStore, PrunedPayload, RetentionDomain};
use reliquary_memory_core::{MemoryBodyId, PayloadState};
use std::{collections::BTreeSet, io::Read};

pub(super) fn validate_memory_prune(
    container: &Container,
    memories: &MemoryStore,
    memory_vectors: &MemoryVectorStore,
    maintenance: &OwnerMaintenanceStore,
) -> Result<(), OwnerSessionError> {
    let manifests: Vec<_> = maintenance
        .manifests()
        .values()
        .filter(|manifest| {
            manifest
                .descriptors
                .iter()
                .any(|descriptor| descriptor.target.domain == RetentionDomain::Memory)
        })
        .collect();
    let absent: BTreeSet<_> = memories.unbacked_body_ids().collect();
    if manifests.is_empty() {
        // Legacy grouped and externally routed Memory provenance can retain
        // metadata without a local body. No destructive-copy proof is in scope.
        return Ok(());
    }
    // The first destructive-copy format accepts one immutable source cut.
    // Incremental re-pruning needs distinct ancestry and is not yet admitted.
    if manifests.len() != 1 || maintenance.manifests().len() != 1 {
        return Err(invalid(
            "multiple prune manifests have no supported reconciliation",
        ));
    }
    let manifest = manifests[0];
    if manifest.owner_uuid != container.owner_id()
        || manifest.source_global_version != container.latest_version()
    {
        return Err(invalid("prune owner or global watermark mismatch"));
    }
    safe_memory_only_frames(container, true).map_err(OwnerSessionError::Record)?;
    let mut selected = BTreeSet::new();
    for descriptor in &manifest.descriptors {
        if descriptor.target.domain != RetentionDomain::Memory
            || descriptor.target.identity.len() != 32
            || descriptor.payloads.len() != 1
        {
            return Err(invalid("unsupported prune descriptor domain or shape"));
        }
        let id = MemoryId(
            descriptor
                .target
                .identity
                .as_slice()
                .try_into()
                .map_err(|_| invalid("invalid prune Memory identity"))?,
        );
        let history = memories
            .history(id)
            .map_err(|_| invalid("pruned Memory history is missing"))?;
        let tombstone = history
            .tombstone
            .ok_or_else(|| invalid("pruned Memory is not terminally tombstoned"))?;
        let body_id = history
            .revisions
            .first()
            .ok_or_else(|| invalid("pruned Memory has no retained revision"))?
            .record
            .body_id;
        if descriptor.payloads != [PrunedPayload::MemoryBody(body_id.0)]
            || !history.revisions.iter().all(|revision| {
                revision.record.body_id == body_id
                    && matches!(revision.payload, PayloadState::Absent)
            })
            || memories.memory_ids_for_body(body_id).any(|reference| {
                !memories
                    .history(reference)
                    .is_ok_and(|history| history.tombstone.is_some())
            })
            || memory_vectors
                .referenced_body_ids()
                .any(|value| value.0 == body_id.0)
            || descriptor.tombstone_digest != tombstone_digest(id, tombstone)
            || descriptor.tombstone_domain_version != Some(tombstone.memory_version)
            || descriptor.tombstone_global_version.is_some()
            || descriptor.retained_structure_digest
                != memory_structure_digest(container, id).map_err(OwnerSessionError::Record)?
            || !selected.insert(body_id)
        {
            return Err(invalid("pruned body lacks terminal source proof"));
        }
    }
    if absent != selected {
        return Err(invalid(
            "transformed owner has unapproved missing Memory bodies",
        ));
    }
    let mut found_manifest = false;
    let mut cursor = container.physical_cursor()?;
    while let Some(mut frame) = cursor.next_frame()? {
        if found_manifest {
            return Err(invalid(
                "transformed owner contains records after the prune manifest",
            ));
        }
        let mut magic = [0u8; 8];
        frame
            .read_exact(&mut magic)
            .map_err(|e| OwnerSessionError::Record(e.to_string()))?;
        if &magic == b"RELHPR01" {
            if found_manifest {
                return Err(invalid("multiple transformed history manifests"));
            }
            found_manifest = true;
            continue;
        }
        if !found_manifest && &magic == MEMORY_BODY && frame.object().length >= 40 {
            let mut raw_id = [0u8; 32];
            frame
                .read_exact(&mut raw_id)
                .map_err(|e| OwnerSessionError::Record(e.to_string()))?;
            if selected.contains(&MemoryBodyId(raw_id)) {
                return Err(invalid("pruned body remains present before manifest"));
            }
        }
    }
    if !found_manifest
        || manifest.source_history_digest
            != frame_history_digest(container, &selected).map_err(OwnerSessionError::Record)?
    {
        return Err(invalid(
            "transformed copy changes the retained physical prefix",
        ));
    }
    Ok(())
}

fn invalid(message: &'static str) -> OwnerSessionError {
    OwnerSessionError::Record(message.into())
}
