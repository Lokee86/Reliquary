use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::memory_store::MemoryStore;
use crate::memory_vector_codec::{decode_object, encode_format, encode_object};
use crate::packed_vector_store::PackedVectorStore;
use crate::{
    CompatibilityProfileId, Container, MemoryBodyId, MemoryVectorError, MemoryVectorId,
    MemoryVectorInfo, MemoryVectorLocation, MemoryVectorSet, MemoryVectorStats, ObjectRef,
    PackedVectorId, ScalarType,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct MemoryVectorStore {
    objects: HashMap<MemoryVectorId, MemoryVectorEntry>,
    bindings: HashMap<(CompatibilityProfileId, MemoryBodyId), MemoryVectorLocation>,
    profile_binding_counts: HashMap<CompatibilityProfileId, usize>,
}

#[derive(Clone, Copy)]
struct MemoryVectorEntry {
    info: MemoryVectorInfo,
    chunk: ObjectRef,
}

impl MemoryVectorStore {
    pub(crate) fn initialize(&self, container: &mut Container) -> Result<(), MemoryVectorError> {
        container.append(&encode_format())?;
        Ok(())
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        memories: &MemoryStore,
        profiles: &CompatibilityProfileStore,
        packed_vectors: &PackedVectorStore,
        compatibility_profile_id: CompatibilityProfileId,
        packed_vector_id: PackedVectorId,
        memory_body_ids: Vec<MemoryBodyId>,
    ) -> Result<MemoryVectorId, MemoryVectorError> {
        profiles
            .get_active(compatibility_profile_id)
            .ok_or(MemoryVectorError::MissingProfile)?;
        validate_mapping(
            memories,
            profiles,
            packed_vectors,
            compatibility_profile_id,
            packed_vector_id,
            &memory_body_ids,
        )?;
        if packed_vectors.is_pruned(packed_vector_id) {
            return Err(crate::PackedVectorError::PayloadUnavailable(
                crate::owner_lifecycle::PayloadUnavailable::Pruned,
            )
            .into());
        }
        let id = memory_vector_id(compatibility_profile_id, packed_vector_id, &memory_body_ids);
        if let Some(existing) = self.objects.get(&id).copied() {
            let payload = container.read(existing.chunk)?;
            let decoded = decode_object(&payload)?.ok_or(MemoryVectorError::MissingObject)?;
            if decoded.id != id
                || decoded.compatibility_profile_id != compatibility_profile_id
                || decoded.packed_vector_id != packed_vector_id
                || decoded.memory_body_ids != memory_body_ids
            {
                return Err(MemoryVectorError::HashCollision);
            }
            return Ok(id);
        }
        for body_id in &memory_body_ids {
            if self
                .bindings
                .contains_key(&(compatibility_profile_id, *body_id))
            {
                return Err(MemoryVectorError::DuplicateBinding);
            }
        }
        let payload = encode_object(
            id,
            compatibility_profile_id,
            packed_vector_id,
            &memory_body_ids,
        )?;
        let chunk = container.append(&payload)?;
        let info = MemoryVectorInfo {
            id,
            compatibility_profile_id,
            packed_vector_id,
            rows: u64::try_from(memory_body_ids.len())
                .map_err(|_| MemoryVectorError::SizeOverflow)?,
        };
        self.insert_entry(info, chunk, &memory_body_ids)?;
        Ok(id)
    }

    pub(crate) fn get(
        &self,
        container: &mut Container,
        id: MemoryVectorId,
    ) -> Result<MemoryVectorSet, MemoryVectorError> {
        let entry = self
            .objects
            .get(&id)
            .ok_or(MemoryVectorError::MissingObject)?;
        let payload = container.read(entry.chunk)?;
        let decoded = decode_object(&payload)?.ok_or(MemoryVectorError::MissingObject)?;
        if decoded.id != id
            || memory_vector_id(
                decoded.compatibility_profile_id,
                decoded.packed_vector_id,
                &decoded.memory_body_ids,
            ) != id
        {
            return Err(MemoryVectorError::HashCollision);
        }
        Ok(MemoryVectorSet {
            id,
            compatibility_profile_id: decoded.compatibility_profile_id,
            packed_vector_id: decoded.packed_vector_id,
            memory_body_ids: decoded.memory_body_ids,
        })
    }

    pub(crate) fn insert_rebuilt(
        &mut self,
        info: MemoryVectorInfo,
        chunk: ObjectRef,
        body_ids: &[MemoryBodyId],
    ) -> Result<(), MemoryVectorError> {
        if let Some(existing) = self.objects.get(&info.id) {
            if existing.info != info {
                return Err(MemoryVectorError::HashCollision);
            }
            return Ok(());
        }
        self.insert_entry(info, chunk, body_ids)
    }

    fn insert_entry(
        &mut self,
        info: MemoryVectorInfo,
        chunk: ObjectRef,
        body_ids: &[MemoryBodyId],
    ) -> Result<(), MemoryVectorError> {
        if body_ids.iter().any(|body_id| {
            self.bindings
                .contains_key(&(info.compatibility_profile_id, *body_id))
        }) {
            return Err(MemoryVectorError::DuplicateBinding);
        }
        for (row, body_id) in body_ids.iter().enumerate() {
            let key = (info.compatibility_profile_id, *body_id);
            self.bindings.insert(
                key,
                MemoryVectorLocation {
                    set_id: info.id,
                    row: u64::try_from(row).map_err(|_| MemoryVectorError::SizeOverflow)?,
                },
            );
        }
        *self
            .profile_binding_counts
            .entry(info.compatibility_profile_id)
            .or_default() += body_ids.len();
        self.objects
            .insert(info.id, MemoryVectorEntry { info, chunk });
        Ok(())
    }

    pub(crate) fn location(
        &self,
        profile_id: CompatibilityProfileId,
        body_id: MemoryBodyId,
    ) -> Option<MemoryVectorLocation> {
        self.bindings.get(&(profile_id, body_id)).copied()
    }

    pub(crate) fn missing_body_ids(
        &self,
        profile_id: CompatibilityProfileId,
        body_ids: &[MemoryBodyId],
    ) -> Vec<MemoryBodyId> {
        body_ids
            .iter()
            .copied()
            .filter(|body_id| !self.bindings.contains_key(&(profile_id, *body_id)))
            .collect()
    }

    pub(crate) fn binding_count(&self, profile_id: CompatibilityProfileId) -> usize {
        self.profile_binding_counts
            .get(&profile_id)
            .copied()
            .unwrap_or(0)
    }

    pub(crate) fn has_complete_coverage(
        &self,
        profile_id: CompatibilityProfileId,
        body_ids: &[MemoryBodyId],
    ) -> bool {
        body_ids
            .iter()
            .all(|body_id| self.bindings.contains_key(&(profile_id, *body_id)))
    }

    pub(crate) fn infos(&self) -> Vec<MemoryVectorInfo> {
        let mut infos: Vec<_> = self.objects.values().map(|entry| entry.info).collect();
        infos.sort_by_key(|info| info.id.0);
        infos
    }

    pub(crate) fn stats(&self) -> MemoryVectorStats {
        MemoryVectorStats {
            objects: self.objects.len(),
            rows: self.objects.values().map(|entry| entry.info.rows).sum(),
            bindings: self.bindings.len(),
        }
    }
}

pub(crate) fn validate_mapping(
    memories: &MemoryStore,
    profiles: &CompatibilityProfileStore,
    packed_vectors: &PackedVectorStore,
    compatibility_profile_id: CompatibilityProfileId,
    packed_vector_id: PackedVectorId,
    memory_body_ids: &[MemoryBodyId],
) -> Result<(), MemoryVectorError> {
    let profile = profiles
        .get(compatibility_profile_id)
        .ok_or(MemoryVectorError::MissingProfile)?;
    let packed = packed_vectors
        .info(packed_vector_id)
        .ok_or(MemoryVectorError::MissingPackedVector)?;
    let rows = u64::try_from(memory_body_ids.len()).map_err(|_| MemoryVectorError::SizeOverflow)?;
    if packed.count != rows {
        return Err(MemoryVectorError::RowCountMismatch);
    }
    if packed.schema.dimensions != profile.dimensions {
        return Err(MemoryVectorError::DimensionMismatch);
    }
    if packed.schema.scalar != ScalarType::F32 {
        return Err(MemoryVectorError::UnsupportedScalar(packed.schema.scalar));
    }
    let mut seen = HashSet::with_capacity(memory_body_ids.len());
    for body_id in memory_body_ids {
        if !memories.contains_body(*body_id) {
            return Err(MemoryVectorError::MissingMemoryBody);
        }
        if !seen.insert(*body_id) {
            return Err(MemoryVectorError::DuplicateMemoryBody);
        }
    }
    Ok(())
}

pub(crate) fn memory_vector_id(
    compatibility_profile_id: CompatibilityProfileId,
    packed_vector_id: PackedVectorId,
    body_ids: &[MemoryBodyId],
) -> MemoryVectorId {
    let mut hash = Sha256::new();
    hash.update(b"CVA-MEMORY-VECTORS-V1\0");
    hash.update(compatibility_profile_id.0);
    hash.update(packed_vector_id.0);
    for body_id in body_ids {
        hash.update(body_id.0);
    }
    MemoryVectorId(hash.finalize().into())
}
