//! Cross-owner authorization of Memory retention markers.
use crate::{MemoryBodyId, MemoryError};
use sha2::{Digest, Sha256};
pub(crate) fn validate_prune_history(
    store: &crate::memory_store::MemoryStore,
    container: &mut reliquary_container::Container,
    manifests: &std::collections::BTreeMap<
        [u8; 32],
        crate::owner_maintenance::HistoryPruneManifest,
    >,
) -> Result<(), MemoryError> {
    use crate::owner_maintenance::{PrunedPayload, RetentionDomain};
    let invalid = || MemoryError::CorruptRecord("Memory prune evidence mismatch");
    let mut authorized = std::collections::HashSet::new();
    for (digest, manifest) in manifests {
        for descriptor in &manifest.descriptors {
            if descriptor.target.domain != RetentionDomain::Memory {
                continue;
            }
            let raw: [u8; 32] = descriptor
                .target
                .identity
                .as_slice()
                .try_into()
                .map_err(|_| invalid())?;
            let retained = store.retention_descriptor(crate::MemoryId(raw))?;
            let mut bodies = Vec::new();
            for payload in &descriptor.payloads {
                let PrunedPayload::MemoryBody(id) = payload else {
                    return Err(invalid());
                };
                let body = MemoryBodyId(*id);
                bodies.push(body);
                let marker = store.prune_marker(body).ok_or_else(invalid)?;
                if marker.manifest_digest != *digest
                    || !retained.prunable_body_ids.contains(&body)
                    || !authorized.insert(body)
                {
                    return Err(invalid());
                }
            }
            if self::descriptor(store, crate::MemoryId(raw), &bodies)? != *descriptor {
                return Err(invalid());
            }
            if store.records().iter().any(|record| {
                bodies.contains(&record.body_id) && store.retention_descriptor(record.id).is_err()
            }) {
                return Err(invalid());
            }
        }
    }
    let _ = container;
    if authorized.len() != store.prune_markers().count() {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn descriptor(
    store: &crate::memory_store::MemoryStore,
    id: crate::MemoryId,
    bodies: &[MemoryBodyId],
) -> Result<crate::owner_maintenance::HistoryRetentionDescriptor, MemoryError> {
    use crate::owner_maintenance::*;
    let retained = store.retention_descriptor(id)?;
    if bodies.is_empty()
        || bodies
            .iter()
            .any(|body| !retained.prunable_body_ids.contains(body))
    {
        return Err(MemoryError::PayloadRequiredByLiveTarget);
    }
    let mut structure = Sha256::new();
    structure.update(b"Memory retained topology v1");
    for record in store.records().iter().filter(|record| record.id == id) {
        let raw = crate::memory_codec::encode_record(record)?;
        structure.update((raw.len() as u64).to_le_bytes());
        structure.update(raw);
        structure.update(record.global_version.to_le_bytes());
        structure.update(record.memory_version.to_le_bytes());
    }
    let mut tombstone = Sha256::new();
    tombstone.update(id.0);
    tombstone.update(retained.tombstone_revision.to_le_bytes());
    tombstone.update(retained.tombstone_global_version.to_le_bytes());
    tombstone.update(retained.tombstone_memory_version.to_le_bytes());
    let mut payloads = bodies
        .iter()
        .map(|id| PrunedPayload::MemoryBody(id.0))
        .collect::<Vec<_>>();
    payloads.sort();
    payloads.dedup();
    Ok(HistoryRetentionDescriptor {
        target: RetentionTarget {
            domain: RetentionDomain::Memory,
            identity: id.0.to_vec(),
        },
        tombstone_global_version: Some(retained.tombstone_global_version),
        tombstone_domain_version: Some(retained.tombstone_memory_version),
        tombstone_digest: tombstone.finalize().into(),
        retained_structure_digest: structure.finalize().into(),
        payloads,
    })
}
