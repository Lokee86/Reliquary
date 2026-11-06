use crate::{
    ArchiveVectorId, ArchiveVectorStore, CompatibilityProfileId, CompatibilityProfileStore,
    GenerationStoreError, VectorGeneration, VectorGenerationId, VectorGenerationStats,
};
use reliquary_container::{Container, ObjectRef};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const FORMAT_MAGIC: &[u8; 8] = b"RELVGFM1";
const GENERATION_MAGIC: &[u8; 8] = b"RELVGEN1";
const VERSION_MAGIC: &[u8; 8] = b"RELVGRC1";
const FORMAT_SCHEMA: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationPayload {
    pub id: VectorGenerationId,
    pub compatibility_profile_id: CompatibilityProfileId,
    pub archive_vector_id: ArchiveVectorId,
    pub source_archive_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GenerationVersion {
    vector_version: u64,
    generation_record: ObjectRef,
}

pub fn encode_format() -> [u8; 12] {
    let mut bytes = [0; 12];
    bytes[..8].copy_from_slice(FORMAT_MAGIC);
    bytes[8..].copy_from_slice(&FORMAT_SCHEMA.to_le_bytes());
    bytes
}

fn decode_format(bytes: &[u8]) -> Result<bool, GenerationStoreError> {
    if !bytes.starts_with(FORMAT_MAGIC) {
        return Ok(false);
    }
    if bytes.len() != 12 || u32_at(bytes, 8)? != FORMAT_SCHEMA {
        return Err(GenerationStoreError::CorruptRecord(
            "generation format marker",
        ));
    }
    Ok(true)
}

pub fn encode_generation(payload: GenerationPayload) -> [u8; 112] {
    let mut bytes = [0; 112];
    bytes[..8].copy_from_slice(GENERATION_MAGIC);
    bytes[8..40].copy_from_slice(&payload.id.0);
    bytes[40..72].copy_from_slice(&payload.compatibility_profile_id.0);
    bytes[72..104].copy_from_slice(&payload.archive_vector_id.0);
    bytes[104..112].copy_from_slice(&payload.source_archive_version.to_le_bytes());
    bytes
}

pub fn decode_generation(bytes: &[u8]) -> Result<Option<GenerationPayload>, GenerationStoreError> {
    if !bytes.starts_with(GENERATION_MAGIC) {
        return Ok(None);
    }
    if bytes.len() != 112 {
        return Err(GenerationStoreError::CorruptRecord("generation payload"));
    }
    Ok(Some(GenerationPayload {
        id: VectorGenerationId(bytes[8..40].try_into().expect("generation identity width")),
        compatibility_profile_id: CompatibilityProfileId(
            bytes[40..72].try_into().expect("profile identity width"),
        ),
        archive_vector_id: ArchiveVectorId(
            bytes[72..104]
                .try_into()
                .expect("archive vector identity width"),
        ),
        source_archive_version: u64_at(bytes, 104)?,
    }))
}

fn encode_version(version: GenerationVersion) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes[..8].copy_from_slice(VERSION_MAGIC);
    bytes[8..16].copy_from_slice(&version.vector_version.to_le_bytes());
    bytes[16..24].copy_from_slice(&version.generation_record.offset.to_le_bytes());
    bytes[24..32].copy_from_slice(&version.generation_record.length.to_le_bytes());
    bytes
}

fn decode_version(bytes: &[u8]) -> Result<Option<GenerationVersion>, GenerationStoreError> {
    if !bytes.starts_with(VERSION_MAGIC) {
        return Ok(None);
    }
    if bytes.len() != 32 {
        return Err(GenerationStoreError::CorruptRecord("generation version"));
    }
    Ok(Some(GenerationVersion {
        vector_version: u64_at(bytes, 8)?,
        generation_record: ObjectRef {
            offset: u64_at(bytes, 16)?,
            length: u64_at(bytes, 24)?,
        },
    }))
}

/// Inspect the physical inbound reference of a generation receipt.
pub fn generation_receipt_target(bytes: &[u8]) -> Result<Option<ObjectRef>, GenerationStoreError> {
    Ok(decode_version(bytes)?.map(|receipt| receipt.generation_record))
}

/// Re-encode a retained receipt after its backing generation moves.
pub fn relocate_generation_receipt(
    bytes: &[u8],
    target: ObjectRef,
) -> Result<[u8; 32], GenerationStoreError> {
    let mut receipt = decode_version(bytes)?.ok_or(GenerationStoreError::CorruptRecord(
        "missing generation receipt",
    ))?;
    if receipt.generation_record.length != target.length {
        return Err(GenerationStoreError::CorruptRecord(
            "relocated generation length",
        ));
    }
    receipt.generation_record = target;
    Ok(encode_version(receipt))
}

pub fn vector_generation_id(
    profile: CompatibilityProfileId,
    archive: ArchiveVectorId,
    source_archive_version: u64,
) -> VectorGenerationId {
    let mut hash = Sha256::new();
    hash.update([
        0x43, 0x56, 0x41, 0x2d, 0x56, 0x45, 0x43, 0x54, 0x4f, 0x52, 0x2d, 0x47, 0x45, 0x4e, 0x45,
        0x52, 0x41, 0x54, 0x49, 0x4f, 0x4e, 0x2d, 0x56, 0x32, 0x00,
    ]);
    hash.update(profile.0);
    hash.update(archive.0);
    hash.update(source_archive_version.to_le_bytes());
    VectorGenerationId(hash.finalize().into())
}

pub struct VectorGenerationStore {
    generations: Vec<VectorGeneration>,
    by_id: HashMap<VectorGenerationId, usize>,
    current_by_profile: HashMap<CompatibilityProfileId, usize>,
    next_vector_version: u64,
}

impl Default for VectorGenerationStore {
    fn default() -> Self {
        Self {
            generations: Vec::new(),
            by_id: HashMap::new(),
            current_by_profile: HashMap::new(),
            next_vector_version: 1,
        }
    }
}

impl VectorGenerationStore {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn initialize(&self, container: &mut Container) -> Result<(), GenerationStoreError> {
        let format = encode_format();
        if crate::has_format(container, &format)? {
            return Err(GenerationStoreError::ConflictingFormat);
        }
        container.append(&format)?;
        Ok(())
    }

    pub fn open(
        container: &Container,
        profiles: &CompatibilityProfileStore,
        archives: &ArchiveVectorStore,
    ) -> Result<Self, GenerationStoreError> {
        let mut replay = VectorGenerationOpenState::new();
        let mut cursor = container.physical_cursor()?;
        while let Some(mut frame) = cursor.next_frame()? {
            let object = frame.object();
            let mut payload = Vec::new();
            std::io::Read::read_to_end(&mut frame, &mut payload)
                .map_err(|_| GenerationStoreError::CorruptRecord("generation frame read"))?;
            replay.ingest(object, &payload)?;
        }
        replay.finish(profiles, archives)
    }

    pub fn publish(
        &mut self,
        container: &mut Container,
        profiles: &CompatibilityProfileStore,
        archives: &ArchiveVectorStore,
        profile: CompatibilityProfileId,
        archive: ArchiveVectorId,
        source_archive_version: u64,
    ) -> Result<VectorGeneration, GenerationStoreError> {
        if profiles.get_active(profile).is_none() {
            return Err(GenerationStoreError::MissingProfile);
        }
        archives
            .info(archive)
            .ok_or(GenerationStoreError::MissingArchiveVector)?;
        if let Some(current) = self.current_generation(profile)
            && source_archive_version < current.source_archive_version
        {
            return Err(GenerationStoreError::SourceVersionRegression);
        }
        let id = vector_generation_id(profile, archive, source_archive_version);
        if let Some(index) = self.by_id.get(&id).copied() {
            return Ok(self.generations[index]);
        }
        let vector_version = self.next_vector_version;
        let record = container.append(&encode_generation(GenerationPayload {
            id,
            compatibility_profile_id: profile,
            archive_vector_id: archive,
            source_archive_version,
        }))?;
        container.append(&encode_version(GenerationVersion {
            vector_version,
            generation_record: record,
        }))?;
        let generation = VectorGeneration {
            id,
            compatibility_profile_id: profile,
            archive_vector_id: archive,
            source_archive_version,
            vector_version,
        };
        self.insert(generation)?;
        Ok(generation)
    }

    fn insert(&mut self, generation: VectorGeneration) -> Result<(), GenerationStoreError> {
        if vector_generation_id(
            generation.compatibility_profile_id,
            generation.archive_vector_id,
            generation.source_archive_version,
        ) != generation.id
        {
            return Err(GenerationStoreError::HashCollision);
        }
        if generation.vector_version != self.next_vector_version {
            return Err(GenerationStoreError::InvalidGenerationVersion);
        }
        if self.by_id.contains_key(&generation.id) {
            return Err(GenerationStoreError::HashCollision);
        }
        if let Some(current) = self.current_generation(generation.compatibility_profile_id)
            && generation.source_archive_version < current.source_archive_version
        {
            return Err(GenerationStoreError::SourceVersionRegression);
        }
        let index = self.generations.len();
        self.generations.push(generation);
        self.by_id.insert(generation.id, index);
        self.current_by_profile
            .insert(generation.compatibility_profile_id, index);
        self.next_vector_version = self
            .next_vector_version
            .checked_add(1)
            .ok_or(GenerationStoreError::VectorVersionExhausted)?;
        Ok(())
    }

    pub fn generation(&self, id: VectorGenerationId) -> Option<VectorGeneration> {
        self.by_id.get(&id).map(|index| self.generations[*index])
    }

    pub fn current_generation(&self, id: CompatibilityProfileId) -> Option<VectorGeneration> {
        self.current_by_profile
            .get(&id)
            .map(|index| self.generations[*index])
    }

    pub fn generation_at(
        &self,
        profile: CompatibilityProfileId,
        version: u64,
    ) -> Option<VectorGeneration> {
        self.generations
            .iter()
            .rev()
            .find(|generation| {
                generation.compatibility_profile_id == profile
                    && generation.vector_version <= version
            })
            .copied()
    }

    pub fn generations(&self) -> &[VectorGeneration] {
        &self.generations
    }

    pub fn vector_version(&self) -> u64 {
        self.next_vector_version.saturating_sub(1)
    }

    pub fn stats(&self) -> VectorGenerationStats {
        VectorGenerationStats {
            generations: self.generations.len(),
            active_profiles: self.current_by_profile.len(),
            vector_version: self.vector_version(),
        }
    }
}

pub struct VectorGenerationOpenState {
    pending: HashMap<u64, (ObjectRef, GenerationPayload)>,
    generations: Vec<VectorGeneration>,
    next_vector_version: u64,
    format_seen: bool,
}

impl VectorGenerationOpenState {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            generations: Vec::new(),
            next_vector_version: 1,
            format_seen: false,
        }
    }

    pub fn ingest(&mut self, chunk: ObjectRef, payload: &[u8]) -> Result<(), GenerationStoreError> {
        if decode_format(payload)? {
            if self.format_seen {
                return Err(GenerationStoreError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        if let Some(version) = decode_version(payload)? {
            if version.vector_version != self.next_vector_version
                || version.generation_record.offset >= chunk.offset
            {
                return Err(GenerationStoreError::InvalidGenerationVersion);
            }
            let (record_ref, source) = self
                .pending
                .remove(&version.generation_record.offset)
                .ok_or(GenerationStoreError::InvalidGenerationVersion)?;
            if record_ref != version.generation_record {
                return Err(GenerationStoreError::InvalidGenerationVersion);
            }
            let generation = VectorGeneration {
                id: source.id,
                compatibility_profile_id: source.compatibility_profile_id,
                archive_vector_id: source.archive_vector_id,
                source_archive_version: source.source_archive_version,
                vector_version: version.vector_version,
            };
            if vector_generation_id(
                generation.compatibility_profile_id,
                generation.archive_vector_id,
                generation.source_archive_version,
            ) != generation.id
            {
                return Err(GenerationStoreError::HashCollision);
            }
            self.generations.push(generation);
            self.next_vector_version = self
                .next_vector_version
                .checked_add(1)
                .ok_or(GenerationStoreError::VectorVersionExhausted)?;
            return Ok(());
        }
        if let Some(source) = decode_generation(payload)? {
            if vector_generation_id(
                source.compatibility_profile_id,
                source.archive_vector_id,
                source.source_archive_version,
            ) != source.id
            {
                return Err(GenerationStoreError::HashCollision);
            }
            if self.pending.insert(chunk.offset, (chunk, source)).is_some() {
                return Err(GenerationStoreError::InvalidGenerationVersion);
            }
        }
        Ok(())
    }

    pub fn finish(
        self,
        profiles: &CompatibilityProfileStore,
        archives: &ArchiveVectorStore,
    ) -> Result<VectorGenerationStore, GenerationStoreError> {
        if !self.format_seen {
            return Err(GenerationStoreError::MissingFormat);
        }
        if !self.pending.is_empty() {
            return Err(GenerationStoreError::InvalidGenerationVersion);
        }
        let mut store = VectorGenerationStore::empty();
        for generation in self.generations {
            // A retired profile cannot publish new generations, but it still
            // backs generations already present in durable history.
            if profiles.get(generation.compatibility_profile_id).is_none() {
                return Err(GenerationStoreError::MissingProfile);
            }
            archives
                .info(generation.archive_vector_id)
                .ok_or(GenerationStoreError::MissingArchiveVector)?;
            store.insert(generation)?;
        }
        Ok(store)
    }
}

impl Default for VectorGenerationOpenState {
    fn default() -> Self {
        Self::new()
    }
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, GenerationStoreError> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or(GenerationStoreError::CorruptRecord("generation integer"))?;
    Ok(u32::from_le_bytes(raw.try_into().expect("u32 width")))
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64, GenerationStoreError> {
    let raw = bytes
        .get(offset..offset + 8)
        .ok_or(GenerationStoreError::CorruptRecord("generation integer"))?;
    Ok(u64::from_le_bytes(raw.try_into().expect("u64 width")))
}
