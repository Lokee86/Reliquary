use crate::{
    ArchiveVectorId, CompatibilityProfileId, VectorGeneration, VectorGenerationId,
    VectorGenerationStats,
};
use reliquary_container::{Container, ContainerError, ObjectRef};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;

const FORMAT_MAGIC: &[u8; 8] = b"CVAVGFM2";
const GENERATION_MAGIC: &[u8; 8] = b"CVAVGEN2";
const VERSION_MAGIC: &[u8; 8] = b"CVAVGRC2";
const FORMAT_SCHEMA: u32 = 1;

#[derive(Debug)]
pub enum GenerationStoreError {
    Container(ContainerError),
    CorruptRecord(&'static str),
    MissingFormat,
    ConflictingFormat,
    InvalidGenerationVersion,
    SourceVersionRegression,
    VectorVersionExhausted,
    HashCollision,
}

impl fmt::Display for GenerationStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vector-generation store error: {self:?}")
    }
}
impl std::error::Error for GenerationStoreError {}
impl From<ContainerError> for GenerationStoreError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

#[derive(Clone, Copy)]
pub struct GenerationPayload {
    pub id: VectorGenerationId,
    pub compatibility_profile_id: CompatibilityProfileId,
    pub archive_vector_id: ArchiveVectorId,
    pub source_archive_version: u64,
}

#[derive(Clone, Copy)]
pub struct GenerationVersion {
    pub global_version: u64,
    pub vector_version: u64,
    pub record: ObjectRef,
}

pub fn encode_format() -> [u8; 12] {
    let mut out = [0; 12];
    out[..8].copy_from_slice(FORMAT_MAGIC);
    out[8..].copy_from_slice(&FORMAT_SCHEMA.to_le_bytes());
    out
}
pub fn decode_format(b: &[u8]) -> Result<bool, GenerationStoreError> {
    if !b.starts_with(FORMAT_MAGIC) {
        return Ok(false);
    }
    if b.len() != 12 || u32_at(b, 8)? != FORMAT_SCHEMA {
        return Err(GenerationStoreError::CorruptRecord(
            "generation format marker",
        ));
    }
    Ok(true)
}
pub fn encode_generation(p: GenerationPayload) -> [u8; 112] {
    let mut out = [0; 112];
    out[..8].copy_from_slice(GENERATION_MAGIC);
    out[8..40].copy_from_slice(&p.id.0);
    out[40..72].copy_from_slice(&p.compatibility_profile_id.0);
    out[72..104].copy_from_slice(&p.archive_vector_id.0);
    out[104..112].copy_from_slice(&p.source_archive_version.to_le_bytes());
    out
}
pub fn decode_generation(b: &[u8]) -> Result<Option<GenerationPayload>, GenerationStoreError> {
    if !b.starts_with(GENERATION_MAGIC) {
        return Ok(None);
    }
    if b.len() != 112 {
        return Err(GenerationStoreError::CorruptRecord("generation payload"));
    }
    Ok(Some(GenerationPayload {
        id: VectorGenerationId(b[8..40].try_into().unwrap()),
        compatibility_profile_id: CompatibilityProfileId(b[40..72].try_into().unwrap()),
        archive_vector_id: ArchiveVectorId(b[72..104].try_into().unwrap()),
        source_archive_version: u64_at(b, 104)?,
    }))
}
pub fn encode_version(v: GenerationVersion) -> [u8; 40] {
    let mut out = [0; 40];
    out[..8].copy_from_slice(VERSION_MAGIC);
    out[8..16].copy_from_slice(&v.global_version.to_le_bytes());
    out[16..24].copy_from_slice(&v.vector_version.to_le_bytes());
    out[24..40].copy_from_slice(&v.record.legacy_bytes());
    out
}
pub fn decode_version(b: &[u8]) -> Result<Option<GenerationVersion>, GenerationStoreError> {
    if !b.starts_with(VERSION_MAGIC) {
        return Ok(None);
    }
    if b.len() != 40 {
        return Err(GenerationStoreError::CorruptRecord("generation version"));
    }
    Ok(Some(GenerationVersion {
        global_version: u64_at(b, 8)?,
        vector_version: u64_at(b, 16)?,
        record: ObjectRef::from_legacy_bytes(b[24..40].try_into().unwrap()),
    }))
}
fn u32_at(b: &[u8], o: usize) -> Result<u32, GenerationStoreError> {
    Ok(u32::from_le_bytes(
        b.get(o..o + 4)
            .ok_or(GenerationStoreError::CorruptRecord("generation integer"))?
            .try_into()
            .unwrap(),
    ))
}
fn u64_at(b: &[u8], o: usize) -> Result<u64, GenerationStoreError> {
    Ok(u64::from_le_bytes(
        b.get(o..o + 8)
            .ok_or(GenerationStoreError::CorruptRecord("generation integer"))?
            .try_into()
            .unwrap(),
    ))
}

pub fn vector_generation_id(
    profile: CompatibilityProfileId,
    archive: ArchiveVectorId,
    source_version: u64,
) -> VectorGenerationId {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"CVA-VECTOR-GENERATION-V2\0");
    hash.update(profile.0);
    hash.update(archive.0);
    hash.update(source_version.to_le_bytes());
    VectorGenerationId(hash.finalize().into())
}

#[derive(Default)]
pub struct VectorGenerationStore {
    generations: Vec<VectorGeneration>,
    by_id: HashMap<VectorGenerationId, usize>,
    current_by_profile: HashMap<CompatibilityProfileId, usize>,
    next_vector_version: u64,
}
impl VectorGenerationStore {
    pub fn empty() -> Self {
        Self {
            next_vector_version: 1,
            ..Self::default()
        }
    }
    pub fn initialize(&self, container: &mut Container) -> Result<(), GenerationStoreError> {
        container.append(&encode_format())?;
        Ok(())
    }
    pub fn publish(
        &mut self,
        container: &mut Container,
        profile: CompatibilityProfileId,
        archive: ArchiveVectorId,
        source_version: u64,
    ) -> Result<VectorGeneration, GenerationStoreError> {
        if let Some(current) = self.current_generation(profile)
            && source_version < current.source_archive_version
        {
            return Err(GenerationStoreError::SourceVersionRegression);
        }
        let id = vector_generation_id(profile, archive, source_version);
        if let Some(index) = self.by_id.get(&id).copied() {
            return Ok(self.generations[index]);
        }
        let record = container.append(&encode_generation(GenerationPayload {
            id,
            compatibility_profile_id: profile,
            archive_vector_id: archive,
            source_archive_version: source_version,
        }))?;
        let vector_version = self.next_vector_version;
        let next = vector_version
            .checked_add(1)
            .ok_or(GenerationStoreError::VectorVersionExhausted)?;
        let global_version = container.allocate_version()?;
        container.append(&encode_version(GenerationVersion {
            global_version,
            vector_version,
            record,
        }))?;
        let generation = VectorGeneration {
            id,
            compatibility_profile_id: profile,
            archive_vector_id: archive,
            source_archive_version: source_version,
            global_version,
            vector_version,
        };
        self.insert(generation)?;
        self.next_vector_version = next;
        Ok(generation)
    }
    pub fn insert_rebuilt(
        &mut self,
        generation: VectorGeneration,
    ) -> Result<(), GenerationStoreError> {
        if generation.vector_version != self.next_vector_version {
            return Err(GenerationStoreError::InvalidGenerationVersion);
        }
        self.insert(generation)?;
        self.next_vector_version = self
            .next_vector_version
            .checked_add(1)
            .ok_or(GenerationStoreError::VectorVersionExhausted)?;
        Ok(())
    }
    pub fn generation(&self, id: VectorGenerationId) -> Option<VectorGeneration> {
        self.by_id.get(&id).map(|i| self.generations[*i])
    }
    pub fn current_generation(&self, id: CompatibilityProfileId) -> Option<VectorGeneration> {
        self.current_by_profile
            .get(&id)
            .map(|i| self.generations[*i])
    }
    pub fn generation_at(
        &self,
        id: CompatibilityProfileId,
        version: u64,
    ) -> Option<VectorGeneration> {
        self.generations
            .iter()
            .rev()
            .find(|g| g.compatibility_profile_id == id && g.vector_version <= version)
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
    fn insert(&mut self, g: VectorGeneration) -> Result<(), GenerationStoreError> {
        if vector_generation_id(
            g.compatibility_profile_id,
            g.archive_vector_id,
            g.source_archive_version,
        ) != g.id
        {
            return Err(GenerationStoreError::HashCollision);
        }
        if let Some(i) = self.by_id.get(&g.id).copied() {
            return if self.generations[i] == g {
                Ok(())
            } else {
                Err(GenerationStoreError::HashCollision)
            };
        }
        if let Some(current) = self.current_generation(g.compatibility_profile_id)
            && g.source_archive_version < current.source_archive_version
        {
            return Err(GenerationStoreError::SourceVersionRegression);
        }
        let i = self.generations.len();
        self.generations.push(g);
        self.by_id.insert(g.id, i);
        self.current_by_profile
            .insert(g.compatibility_profile_id, i);
        Ok(())
    }
}

/// Structural journal replay for generation records. Product reference checks
/// (Archive, packed vectors and profiles) deliberately remain at the root boundary.
pub struct GenerationOpenState {
    pending: HashMap<ObjectRef, GenerationPayload>,
    versioned: HashSet<ObjectRef>,
    generations: Vec<VectorGeneration>,
    next_vector_version: u64,
    format_seen: bool,
}
impl GenerationOpenState {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            versioned: HashSet::new(),
            generations: Vec::new(),
            next_vector_version: 1,
            format_seen: false,
        }
    }
    pub fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        latest_global: u64,
    ) -> Result<(), GenerationStoreError> {
        if decode_format(payload)? {
            if self.format_seen {
                return Err(GenerationStoreError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        if let Some(version) = decode_version(payload)? {
            if version.vector_version != self.next_vector_version
                || version.global_version == 0
                || version.global_version > latest_global
                || self
                    .generations
                    .last()
                    .is_some_and(|last| last.global_version >= version.global_version)
                || !version.record.precedes(chunk)
            {
                return Err(GenerationStoreError::InvalidGenerationVersion);
            }
            let source = self
                .pending
                .remove(&version.record)
                .ok_or(GenerationStoreError::InvalidGenerationVersion)?;
            if !self.versioned.insert(version.record) {
                return Err(GenerationStoreError::InvalidGenerationVersion);
            }
            if vector_generation_id(
                source.compatibility_profile_id,
                source.archive_vector_id,
                source.source_archive_version,
            ) != source.id
            {
                return Err(GenerationStoreError::HashCollision);
            }
            self.generations.push(VectorGeneration {
                id: source.id,
                compatibility_profile_id: source.compatibility_profile_id,
                archive_vector_id: source.archive_vector_id,
                source_archive_version: source.source_archive_version,
                global_version: version.global_version,
                vector_version: version.vector_version,
            });
            self.next_vector_version = self
                .next_vector_version
                .checked_add(1)
                .ok_or(GenerationStoreError::VectorVersionExhausted)?;
            return Ok(());
        }
        if let Some(generation) = decode_generation(payload)? {
            self.pending.insert(chunk, generation);
        }
        Ok(())
    }
    pub fn finish_records(self) -> Result<Vec<VectorGeneration>, GenerationStoreError> {
        if !self.format_seen {
            return Err(GenerationStoreError::MissingFormat);
        }
        Ok(self.generations)
    }

    pub fn finish(self) -> Result<VectorGenerationStore, GenerationStoreError> {
        let generations = self.finish_records()?;
        let mut store = VectorGenerationStore::empty();
        for generation in generations {
            store.insert_rebuilt(generation)?;
        }
        Ok(store)
    }
}
