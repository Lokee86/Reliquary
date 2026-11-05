use crate::memory_codec::{decode_format, decode_object, encode_format, encode_object};
use crate::{
    CompatibilityProfileId, CompatibilityProfileStore, MemoryBodyId, MemoryVectorError,
    MemoryVectorId, MemoryVectorInfo, MemoryVectorLocation, MemoryVectorRecord, MemoryVectorStats,
    PackedVectorError, PackedVectorId, PackedVectorStore,
};
use lodestone_packed::ScalarType;
use reliquary_container::{Container, ObjectRef};
use std::collections::{HashMap, HashSet};

pub struct AcceptedMemoryMapping {
    pub profile: CompatibilityProfileId,
    pub packed: PackedVectorId,
    pub packed_rows: u64,
    pub body_ids: Vec<MemoryBodyId>,
}

#[derive(Clone)]
struct Entry {
    info: MemoryVectorInfo,
    chunk: ObjectRef,
    bodies: Vec<MemoryBodyId>,
}

#[derive(Default)]
pub struct MemoryVectorStore {
    objects: HashMap<MemoryVectorId, Entry>,
    bindings: HashMap<(CompatibilityProfileId, MemoryBodyId), MemoryVectorLocation>,
    profile_counts: HashMap<CompatibilityProfileId, usize>,
}

impl MemoryVectorStore {
    pub fn initialize(&self, container: &mut Container) -> Result<(), MemoryVectorError> {
        let format = encode_format();
        if crate::has_format(container, &format)? {
            return Err(MemoryVectorError::ConflictingFormat);
        }
        container.append(&format)?;
        Ok(())
    }

    pub fn open(
        container: &Container,
        packed: &PackedVectorStore,
        profiles: &CompatibilityProfileStore,
    ) -> Result<Self, MemoryVectorError> {
        let mut state = MemoryVectorOpenState::new();
        let mut cursor = container.physical_cursor()?;
        while let Some(mut frame) = cursor.next_frame()? {
            let object = frame.object();
            let mut payload = Vec::new();
            std::io::Read::read_to_end(&mut frame, &mut payload)
                .map_err(|_| MemoryVectorError::CorruptRecord("memory vector frame read"))?;
            state.ingest(object, &payload)?;
        }
        let mut store = Self::default();
        for replay in state.finish_records()? {
            validate_replay_record(packed, profiles, replay.info)?;
            store.insert(replay.info, replay.chunk, replay.bodies)?;
        }
        Ok(store)
    }

    pub fn put(
        &mut self,
        container: &mut Container,
        packed: &PackedVectorStore,
        profiles: &CompatibilityProfileStore,
        accepted: AcceptedMemoryMapping,
    ) -> Result<MemoryVectorId, MemoryVectorError> {
        let rows =
            u64::try_from(accepted.body_ids.len()).map_err(|_| MemoryVectorError::SizeOverflow)?;
        let packed_info = packed
            .info(accepted.packed)
            .ok_or(MemoryVectorError::MissingPackedVector)?;
        let profile = profiles
            .get_active(accepted.profile)
            .ok_or(MemoryVectorError::MissingProfile)?;
        if rows != accepted.packed_rows || rows != packed_info.count {
            return Err(MemoryVectorError::RowCountMismatch);
        }
        if packed.is_pruned(accepted.packed) {
            return Err(
                PackedVectorError::PayloadUnavailable(crate::PayloadUnavailable::Pruned).into(),
            );
        }
        if packed_info.schema.dimensions != profile.dimensions {
            return Err(MemoryVectorError::DimensionMismatch);
        }
        if packed_info.schema.scalar != ScalarType::F32 {
            return Err(MemoryVectorError::UnsupportedScalar(
                packed_info.schema.scalar,
            ));
        }
        ensure_unique(&accepted.body_ids)?;
        let id = crate::memory_vector_id(accepted.profile, accepted.packed, &accepted.body_ids);
        if let Some(existing) = self.objects.get(&id) {
            if existing.info.compatibility_profile_id != accepted.profile
                || existing.info.packed_vector_id != accepted.packed
                || existing.bodies != accepted.body_ids
            {
                return Err(MemoryVectorError::HashCollision);
            }
            return Ok(id);
        }
        if accepted
            .body_ids
            .iter()
            .any(|body| self.bindings.contains_key(&(accepted.profile, *body)))
        {
            return Err(MemoryVectorError::DuplicateBinding);
        }
        let payload = encode_object(id, accepted.profile, accepted.packed, &accepted.body_ids)?;
        let chunk = container.append(&payload)?;
        let info = MemoryVectorInfo {
            id,
            compatibility_profile_id: accepted.profile,
            packed_vector_id: accepted.packed,
            rows,
        };
        self.insert(info, chunk, accepted.body_ids)?;
        Ok(id)
    }

    fn insert(
        &mut self,
        info: MemoryVectorInfo,
        chunk: ObjectRef,
        bodies: Vec<MemoryBodyId>,
    ) -> Result<(), MemoryVectorError> {
        ensure_unique(&bodies)?;
        if info.rows != u64::try_from(bodies.len()).map_err(|_| MemoryVectorError::SizeOverflow)?
            || crate::memory_vector_id(
                info.compatibility_profile_id,
                info.packed_vector_id,
                &bodies,
            ) != info.id
        {
            return Err(MemoryVectorError::HashCollision);
        }
        if let Some(existing) = self.objects.get(&info.id) {
            return if existing.info == info && existing.bodies == bodies {
                Ok(())
            } else {
                Err(MemoryVectorError::HashCollision)
            };
        }
        if bodies.iter().any(|body| {
            self.bindings
                .contains_key(&(info.compatibility_profile_id, *body))
        }) {
            return Err(MemoryVectorError::DuplicateBinding);
        }
        for (row, body) in bodies.iter().enumerate() {
            self.bindings.insert(
                (info.compatibility_profile_id, *body),
                MemoryVectorLocation {
                    set_id: info.id,
                    row: u64::try_from(row).map_err(|_| MemoryVectorError::SizeOverflow)?,
                },
            );
        }
        *self
            .profile_counts
            .entry(info.compatibility_profile_id)
            .or_default() += bodies.len();
        self.objects.insert(
            info.id,
            Entry {
                info,
                chunk,
                bodies,
            },
        );
        Ok(())
    }

    pub fn info(&self, id: MemoryVectorId) -> Option<MemoryVectorInfo> {
        self.objects.get(&id).map(|entry| entry.info)
    }
    pub fn get(&self, id: MemoryVectorId) -> Result<MemoryVectorRecord, MemoryVectorError> {
        let entry = self
            .objects
            .get(&id)
            .ok_or(MemoryVectorError::MissingObject)?;
        if crate::memory_vector_id(
            entry.info.compatibility_profile_id,
            entry.info.packed_vector_id,
            &entry.bodies,
        ) != id
        {
            return Err(MemoryVectorError::HashCollision);
        }
        Ok(MemoryVectorRecord {
            id,
            compatibility_profile_id: entry.info.compatibility_profile_id,
            packed_vector_id: entry.info.packed_vector_id,
            memory_body_ids: entry.bodies.clone(),
        })
    }

    pub fn location(
        &self,
        profile: CompatibilityProfileId,
        body: MemoryBodyId,
    ) -> Option<MemoryVectorLocation> {
        self.bindings.get(&(profile, body)).copied()
    }

    pub fn object_ref(&self, id: MemoryVectorId) -> Option<ObjectRef> {
        self.objects.get(&id).map(|entry| entry.chunk)
    }

    pub fn missing_body_ids(
        &self,
        profile: CompatibilityProfileId,
        bodies: &[MemoryBodyId],
    ) -> Vec<MemoryBodyId> {
        bodies
            .iter()
            .copied()
            .filter(|body| !self.bindings.contains_key(&(profile, *body)))
            .collect()
    }

    pub fn binding_count(&self, profile: CompatibilityProfileId) -> usize {
        self.profile_counts.get(&profile).copied().unwrap_or(0)
    }

    pub fn has_complete_coverage(
        &self,
        profile: CompatibilityProfileId,
        bodies: &[MemoryBodyId],
    ) -> bool {
        bodies
            .iter()
            .all(|body| self.bindings.contains_key(&(profile, *body)))
    }

    pub fn infos(&self) -> Vec<MemoryVectorInfo> {
        let mut infos: Vec<_> = self.objects.values().map(|entry| entry.info).collect();
        infos.sort_by_key(|info| info.id.0);
        infos
    }

    /// Yield the unique Memory body identities referenced by stored mappings.
    /// The result is stable across reopens and includes mappings whose packed
    /// payload has been pruned.
    pub fn referenced_body_ids(&self) -> impl Iterator<Item = MemoryBodyId> {
        let mut ids = self
            .objects
            .values()
            .flat_map(|entry| entry.bodies.iter().copied())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        ids.sort_by_key(|id| id.0);
        ids.into_iter()
    }

    pub fn stats(&self) -> MemoryVectorStats {
        MemoryVectorStats {
            objects: self.objects.len(),
            rows: self.objects.values().map(|entry| entry.info.rows).sum(),
            bindings: self.bindings.len(),
        }
    }
}

pub fn validate_replay_record(
    packed: &PackedVectorStore,
    profiles: &CompatibilityProfileStore,
    info: MemoryVectorInfo,
) -> Result<(), MemoryVectorError> {
    // Retirement prevents new mappings, but historical mappings remain valid
    // while their profile record is retained.
    let profile = profiles
        .get(info.compatibility_profile_id)
        .ok_or(MemoryVectorError::MissingProfile)?;
    let packed_info = packed
        .info(info.packed_vector_id)
        .ok_or(MemoryVectorError::MissingPackedVector)?;
    if packed_info.count != info.rows {
        return Err(MemoryVectorError::RowCountMismatch);
    }
    if packed_info.schema.dimensions != profile.dimensions {
        return Err(MemoryVectorError::DimensionMismatch);
    }
    if packed_info.schema.scalar != ScalarType::F32 {
        return Err(MemoryVectorError::UnsupportedScalar(
            packed_info.schema.scalar,
        ));
    }
    Ok(())
}

/// Read Memory body identities from a mapping frame for owner-prefix validation.
pub struct MemoryVectorSources {
    pub profile: CompatibilityProfileId,
    pub packed: PackedVectorId,
    pub bodies: Vec<MemoryBodyId>,
}

pub fn sources_in_frame(payload: &[u8]) -> Result<Option<MemoryVectorSources>, MemoryVectorError> {
    Ok(decode_object(payload)?.map(|record| MemoryVectorSources {
        profile: record.profile,
        packed: record.packed,
        bodies: record.bodies,
    }))
}

fn ensure_unique(bodies: &[MemoryBodyId]) -> Result<(), MemoryVectorError> {
    let mut seen = HashSet::with_capacity(bodies.len());
    if bodies.iter().any(|body| !seen.insert(*body)) {
        return Err(MemoryVectorError::DuplicateMemoryBody);
    }
    Ok(())
}

pub struct MemoryVectorOpenState {
    records: Vec<MemoryVectorReplayRecord>,
    format_seen: bool,
}

pub struct MemoryVectorReplayRecord {
    pub info: MemoryVectorInfo,
    pub chunk: ObjectRef,
    pub bodies: Vec<MemoryBodyId>,
}

impl MemoryVectorOpenState {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            format_seen: false,
        }
    }

    pub fn ingest(&mut self, chunk: ObjectRef, payload: &[u8]) -> Result<(), MemoryVectorError> {
        if decode_format(payload)? {
            if self.format_seen {
                return Err(MemoryVectorError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        let Some(decoded) = decode_object(payload)? else {
            return Ok(());
        };
        if crate::memory_vector_id(decoded.profile, decoded.packed, &decoded.bodies) != decoded.id {
            return Err(MemoryVectorError::HashCollision);
        }
        self.records.push(MemoryVectorReplayRecord {
            info: MemoryVectorInfo {
                id: decoded.id,
                compatibility_profile_id: decoded.profile,
                packed_vector_id: decoded.packed,
                rows: u64::try_from(decoded.bodies.len())
                    .map_err(|_| MemoryVectorError::SizeOverflow)?,
            },
            chunk,
            bodies: decoded.bodies,
        });
        Ok(())
    }

    pub fn finish_records(self) -> Result<Vec<MemoryVectorReplayRecord>, MemoryVectorError> {
        if !self.format_seen {
            return Err(MemoryVectorError::MissingFormat);
        }
        Ok(self.records)
    }
}

impl Default for MemoryVectorOpenState {
    fn default() -> Self {
        Self::new()
    }
}
