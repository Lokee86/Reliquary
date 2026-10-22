//! Match Maintenance manifests to the Memory owner's exact retained proofs.
use crate::memory_store::MemoryStore;
use crate::owner_maintenance::{
    HistoryRetentionDescriptor, OwnerMaintenanceStore, PrunedPayload, RetentionDomain,
    RetentionTarget,
};
use crate::{MemoryBodyId, MemoryError, MemoryId};

pub(crate) fn descriptor(
    store: &MemoryStore,
    id: MemoryId,
    bodies: &[MemoryBodyId],
) -> Result<HistoryRetentionDescriptor, MemoryError> {
    let proof = store.retention_proof(id)?;
    let retained = proof.descriptor;
    if bodies.is_empty()
        || bodies
            .iter()
            .any(|body| !retained.prunable_body_ids.contains(body))
    {
        return Err(MemoryError::PayloadRequiredByLiveTarget);
    }
    let mut payloads: Vec<_> = bodies
        .iter()
        .map(|id| PrunedPayload::MemoryBody(id.0))
        .collect();
    payloads.sort();
    payloads.dedup();
    Ok(HistoryRetentionDescriptor {
        target: RetentionTarget {
            domain: RetentionDomain::Memory,
            identity: id.0.to_vec(),
        },
        tombstone_global_version: Some(retained.tombstone_global_version),
        tombstone_domain_version: Some(retained.tombstone_memory_version),
        tombstone_digest: proof.tombstone_digest,
        retained_structure_digest: proof.retained_structure_digest,
        payloads,
    })
}

pub(crate) fn validate(
    store: &MemoryStore,
    maintenance: &OwnerMaintenanceStore,
) -> Result<(), MemoryError> {
    let invalid = || MemoryError::CorruptRecord("Memory prune evidence mismatch");
    let markers: std::collections::HashMap<_, _> = store
        .pruned_body_markers()
        .map(|marker| (marker.id, marker))
        .collect();
    let mut authorized = std::collections::HashSet::new();
    for (digest, manifest) in maintenance.manifests() {
        for expected in &manifest.descriptors {
            if expected.target.domain != RetentionDomain::Memory {
                continue;
            }
            let raw: [u8; 32] = expected
                .target
                .identity
                .as_slice()
                .try_into()
                .map_err(|_| invalid())?;
            let retained = store.retention_descriptor(MemoryId(raw))?;
            let mut bodies = Vec::new();
            for payload in &expected.payloads {
                let PrunedPayload::MemoryBody(id) = payload else {
                    return Err(invalid());
                };
                let body = MemoryBodyId(*id);
                bodies.push(body);
                let marker = markers.get(&body).ok_or_else(invalid)?;
                if marker.manifest_digest != *digest
                    || !retained.prunable_body_ids.contains(&body)
                    || !authorized.insert(body)
                {
                    return Err(invalid());
                }
            }
            if descriptor(store, MemoryId(raw), &bodies)? != *expected {
                return Err(invalid());
            }
            if store.records().iter().any(|record| {
                bodies.contains(&record.body_id) && store.retention_descriptor(record.id).is_err()
            }) {
                return Err(invalid());
            }
        }
    }
    if authorized.len() != markers.len() {
        return Err(invalid());
    }
    Ok(())
}
