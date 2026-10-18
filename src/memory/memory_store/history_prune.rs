//! Memory-owned unavailable markers for destructively pruned body chunks.
use crate::memory_model::MemoryBodyId;
use crate::{MemoryError, ObjectRef};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const MARKER_MAGIC: &[u8; 8] = b"CVAMPRN1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PrunedMemoryBody {
    pub id: MemoryBodyId,
    pub byte_length: u64,
    pub manifest_digest: [u8; 32],
    pub chunk: ObjectRef,
}

impl PrunedMemoryBody {
    pub(crate) fn encode(id: MemoryBodyId, byte_length: u64, manifest_digest: [u8; 32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(112);
        bytes.extend_from_slice(MARKER_MAGIC);
        bytes.extend_from_slice(&id.0);
        bytes.extend_from_slice(&byte_length.to_le_bytes());
        bytes.extend_from_slice(&manifest_digest);
        bytes.extend_from_slice(&Sha256::digest(&bytes));
        bytes
    }

    fn decode(chunk: ObjectRef, bytes: &[u8]) -> Result<Self, MemoryError> {
        if bytes.len() != 112 || Sha256::digest(&bytes[..80]).as_slice() != &bytes[80..] {
            return Err(MemoryError::CorruptRecord("pruned body marker"));
        }
        let marker = Self {
            id: MemoryBodyId(bytes[8..40].try_into().unwrap()),
            byte_length: u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
            manifest_digest: bytes[48..80].try_into().unwrap(),
            chunk,
        };
        if marker.manifest_digest == [0; 32] {
            return Err(MemoryError::CorruptRecord("pruned body marker digest"));
        }
        Ok(marker)
    }
}

#[derive(Default)]
pub(crate) struct MemoryPruneState {
    markers: HashMap<MemoryBodyId, PrunedMemoryBody>,
}

impl MemoryPruneState {
    pub(crate) fn ingest(&mut self, chunk: ObjectRef, bytes: &[u8]) -> Result<bool, MemoryError> {
        if !bytes.starts_with(MARKER_MAGIC) {
            return Ok(false);
        }
        let marker = PrunedMemoryBody::decode(chunk, bytes)?;
        if self.markers.insert(marker.id, marker).is_some() {
            return Err(MemoryError::CorruptRecord("duplicate pruned body marker"));
        }
        Ok(true)
    }

    pub(crate) fn get(&self, id: MemoryBodyId) -> Option<&PrunedMemoryBody> {
        self.markers.get(&id)
    }

    pub(crate) fn markers(&self) -> impl Iterator<Item = &PrunedMemoryBody> {
        self.markers.values()
    }
}

impl MemoryPruneState {
    pub(crate) fn validate(
        &self,
        store: &super::MemoryStore,
        container: &mut crate::Container,
        manifests: &std::collections::BTreeMap<
            [u8; 32],
            crate::history_prune_manifest::HistoryPruneManifest,
        >,
    ) -> Result<(), MemoryError> {
        use crate::history_prune_manifest::{PrunedPayload, RetentionDomain};
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
                    let marker = self.get(body).ok_or_else(invalid)?;
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
                    bodies.contains(&record.body_id)
                        && store.retention_descriptor(record.id).is_err()
                }) {
                    return Err(invalid());
                }
            }
        }
        let _ = container;
        if authorized.len() != self.markers.len() {
            return Err(invalid());
        }
        Ok(())
    }
}

pub(crate) fn descriptor(
    store: &super::MemoryStore,
    id: crate::MemoryId,
    bodies: &[MemoryBodyId],
) -> Result<crate::history_prune_manifest::HistoryRetentionDescriptor, MemoryError> {
    use crate::history_prune_manifest::*;
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
