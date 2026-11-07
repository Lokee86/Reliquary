//! Semantic admission for transformed, tombstone-backed Memory history.
use super::*;
use crate::records::memory_prune_proof::{
    frame_history_digest, memory_structure_digest, safe_memory_only_frames, tombstone_digest,
};
use crate::records::{OwnerMaintenanceStore, PrunedPayload, RetentionDomain};
use reliquary_memory_core::{MemoryBodyId, PayloadState};
use std::collections::BTreeSet;

pub(super) fn validate_memory_prune(
    container: &Container,
    memories: &MemoryStore,
    memory_vectors: &MemoryVectorStore,
    maintenance: &OwnerMaintenanceStore,
    grouped_body_ids: &BTreeSet<MemoryBodyId>,
) -> Result<BTreeSet<MemoryBodyId>, OwnerSessionError> {
    let absent: BTreeSet<_> = memories.unbacked_body_ids().collect();
    let mut manifests = maintenance.manifests().values().filter(|manifest| {
        manifest
            .descriptors
            .iter()
            .any(|descriptor| descriptor.target.domain == RetentionDomain::Memory)
    });
    let Some(manifest) = manifests.next() else {
        // Standalone provenance metadata has always been allowed without local
        // body backing. The relaxed grouped replay requires prune authority for
        // its missing bodies; do not broaden that rule to standalone history.
        return if absent.is_empty()
            || (maintenance.is_empty() && absent.is_disjoint(grouped_body_ids))
        {
            Ok(BTreeSet::new())
        } else {
            Err(invalid("missing Memory prune retention evidence"))
        };
    };
    if manifests.next().is_some() {
        return Err(invalid(
            "multiple prune manifests have no supported reconciliation",
        ));
    }
    if manifest.owner_uuid != container.owner_id()
        || manifest.source_global_version > container.latest_version()
    {
        return Err(invalid("prune owner or global watermark mismatch"));
    }
    let prefix_version =
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
    if prefix_version != manifest.source_global_version
        || manifest.source_history_digest
            != frame_history_digest(container, &selected).map_err(OwnerSessionError::Record)?
    {
        return Err(invalid(
            "transformed copy changes the retained physical prefix",
        ));
    }
    Ok(selected)
}

fn invalid(message: &'static str) -> OwnerSessionError {
    OwnerSessionError::Record(message.into())
}
