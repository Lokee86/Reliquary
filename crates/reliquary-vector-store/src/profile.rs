use crate::model::{
    CompatibilityProbeReference, CompatibilityProfile, CompatibilityProfileId, EmbeddingMode,
    VectorNormalization,
};
use reliquary_container::{Container, ContainerError, ObjectRef};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fmt;

const FORMAT_MAGIC: &[u8; 8] = b"RELCPFM1";
const FORMAT_SCHEMA: u32 = 1;
const PROFILE_MAGIC: &[u8; 8] = b"RELCPRO1";
const RETIREMENT_MAGIC: &[u8; 8] = b"RELCPRT1";
const FIXED_LEN: usize = 60;
const REFERENCE_HEADER_LEN: usize = 8;
const PROBE_SUITE_VERSION: u32 = 1;
const POLICY_VERSION_MIN: u32 = 2;
const POLICY_VERSION_MAX: u32 = 3;

#[derive(Debug)]
pub enum ProfileStoreError {
    Container(ContainerError),
    CorruptRecord(&'static str),
    MissingFormat,
    ConflictingFormat,
    MissingProfile,
    InvalidProfile(&'static str),
    HashCollision,
    SizeOverflow,
}

impl fmt::Display for ProfileStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "compatibility-profile store error: {self:?}")
    }
}
impl std::error::Error for ProfileStoreError {}
impl From<ContainerError> for ProfileStoreError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompatibilityProfileStats {
    pub profiles: usize,
    pub active_profiles: usize,
    pub retired_profiles: usize,
}

pub fn encode_format() -> [u8; 12] {
    let mut out = [0_u8; 12];
    out[..8].copy_from_slice(FORMAT_MAGIC);
    out[8..].copy_from_slice(&FORMAT_SCHEMA.to_le_bytes());
    out
}

pub fn encode_retirement(id: CompatibilityProfileId) -> [u8; 40] {
    let mut out = [0_u8; 40];
    out[..8].copy_from_slice(RETIREMENT_MAGIC);
    out[8..].copy_from_slice(&id.0);
    out
}

pub fn encode_profile(profile: &CompatibilityProfile) -> Result<Vec<u8>, ProfileStoreError> {
    let vector_bytes = profile
        .references
        .iter()
        .try_fold(0_usize, |total, reference| {
            total.checked_add(reference.vector.len().checked_mul(4)?)
        })
        .ok_or(ProfileStoreError::SizeOverflow)?;
    let headers = profile
        .references
        .len()
        .checked_mul(REFERENCE_HEADER_LEN)
        .ok_or(ProfileStoreError::SizeOverflow)?;
    let mut out = Vec::with_capacity(FIXED_LEN + headers + vector_bytes);
    out.extend_from_slice(PROFILE_MAGIC);
    out.extend_from_slice(&profile.id.0);
    out.extend_from_slice(&profile.dimensions.to_le_bytes());
    out.push(profile.normalization.tag());
    out.extend_from_slice(&[0; 3]);
    out.extend_from_slice(&profile.probe_suite_version.to_le_bytes());
    out.extend_from_slice(&profile.compatibility_policy_version.to_le_bytes());
    let count =
        u32::try_from(profile.references.len()).map_err(|_| ProfileStoreError::SizeOverflow)?;
    out.extend_from_slice(&count.to_le_bytes());
    for reference in &profile.references {
        out.push(reference.mode.tag());
        out.extend_from_slice(&[0; 3]);
        let len =
            u32::try_from(reference.vector.len()).map_err(|_| ProfileStoreError::SizeOverflow)?;
        out.extend_from_slice(&len.to_le_bytes());
        for value in &reference.vector {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(out)
}

pub fn decode_profile(bytes: &[u8]) -> Result<Option<CompatibilityProfile>, ProfileStoreError> {
    if !bytes.starts_with(PROFILE_MAGIC) {
        return Ok(None);
    }
    if bytes.len() < FIXED_LEN || bytes[45..48] != [0, 0, 0] {
        return Err(ProfileStoreError::CorruptRecord("profile header"));
    }
    let id = CompatibilityProfileId(bytes[8..40].try_into().expect("profile id"));
    let dimensions = read_u32(bytes, 40)?;
    let normalization = VectorNormalization::from_tag(bytes[44])
        .ok_or(ProfileStoreError::CorruptRecord("profile normalization"))?;
    let probe_suite_version = read_u32(bytes, 48)?;
    let compatibility_policy_version = read_u32(bytes, 52)?;
    let reference_count =
        usize::try_from(read_u32(bytes, 56)?).map_err(|_| ProfileStoreError::SizeOverflow)?;
    if reference_count > bytes.len().saturating_sub(FIXED_LEN) / REFERENCE_HEADER_LEN {
        return Err(ProfileStoreError::CorruptRecord("reference count"));
    }
    let mut offset = FIXED_LEN;
    let mut references = Vec::with_capacity(reference_count);
    for _ in 0..reference_count {
        let header = bytes
            .get(offset..offset + REFERENCE_HEADER_LEN)
            .ok_or(ProfileStoreError::CorruptRecord("reference header"))?;
        if header[1..4] != [0, 0, 0] {
            return Err(ProfileStoreError::CorruptRecord("reference reserved bytes"));
        }
        let mode = EmbeddingMode::from_tag(header[0])
            .ok_or(ProfileStoreError::CorruptRecord("reference mode"))?;
        let len = usize::try_from(u32::from_le_bytes(header[4..8].try_into().unwrap()))
            .map_err(|_| ProfileStoreError::SizeOverflow)?;
        offset = offset
            .checked_add(REFERENCE_HEADER_LEN)
            .ok_or(ProfileStoreError::SizeOverflow)?;
        let byte_len = len.checked_mul(4).ok_or(ProfileStoreError::SizeOverflow)?;
        let end = offset
            .checked_add(byte_len)
            .ok_or(ProfileStoreError::SizeOverflow)?;
        let raw = bytes
            .get(offset..end)
            .ok_or(ProfileStoreError::CorruptRecord("reference vector"))?;
        let vector = raw
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect();
        references.push(CompatibilityProbeReference { mode, vector });
        offset = end;
    }
    if offset != bytes.len() {
        return Err(ProfileStoreError::CorruptRecord("profile trailing bytes"));
    }
    Ok(Some(CompatibilityProfile {
        id,
        dimensions,
        normalization,
        probe_suite_version,
        compatibility_policy_version,
        references,
    }))
}

pub fn decode_retirement(
    bytes: &[u8],
) -> Result<Option<CompatibilityProfileId>, ProfileStoreError> {
    if !bytes.starts_with(RETIREMENT_MAGIC) {
        return Ok(None);
    }
    if bytes.len() != 40 {
        return Err(ProfileStoreError::CorruptRecord("profile retirement"));
    }
    Ok(Some(CompatibilityProfileId(
        bytes[8..40].try_into().expect("profile id"),
    )))
}

pub fn decode_format(bytes: &[u8]) -> Result<bool, ProfileStoreError> {
    if !bytes.starts_with(FORMAT_MAGIC) {
        return Ok(false);
    }
    if bytes.len() != 12 || read_u32(bytes, 8)? != FORMAT_SCHEMA {
        return Err(ProfileStoreError::CorruptRecord("profile format marker"));
    }
    Ok(true)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ProfileStoreError> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or(ProfileStoreError::CorruptRecord("profile integer"))?;
    Ok(u32::from_le_bytes(raw.try_into().expect("u32 width")))
}

pub fn compatibility_profile_id(profile: &CompatibilityProfile) -> CompatibilityProfileId {
    let mut hash = Sha256::new();
    hash.update([
        0x43, 0x56, 0x41, 0x2d, 0x43, 0x4f, 0x4d, 0x50, 0x41, 0x54, 0x49, 0x42, 0x49, 0x4c, 0x49,
        0x54, 0x59, 0x2d, 0x50, 0x52, 0x4f, 0x46, 0x49, 0x4c, 0x45, 0x2d, 0x56, 0x31, 0x00,
    ]);
    hash.update(profile.dimensions.to_le_bytes());
    hash.update([profile.normalization.tag()]);
    hash.update(profile.probe_suite_version.to_le_bytes());
    hash.update(profile.compatibility_policy_version.to_le_bytes());
    for reference in &profile.references {
        hash.update([reference.mode.tag()]);
        for value in &reference.vector {
            hash.update(value.to_le_bytes());
        }
    }
    CompatibilityProfileId(hash.finalize().into())
}

fn validate_profile_shape(profile: &CompatibilityProfile) -> Result<(), ProfileStoreError> {
    if profile.dimensions == 0
        || profile.probe_suite_version != PROBE_SUITE_VERSION
        || !(POLICY_VERSION_MIN..=POLICY_VERSION_MAX)
            .contains(&profile.compatibility_policy_version)
        || profile.references.len() != 4
    {
        return Err(ProfileStoreError::InvalidProfile("profile contract"));
    }
    let modes = [
        EmbeddingMode::Query,
        EmbeddingMode::Query,
        EmbeddingMode::Document,
        EmbeddingMode::Document,
    ];
    let dimensions =
        usize::try_from(profile.dimensions).map_err(|_| ProfileStoreError::SizeOverflow)?;
    if profile
        .references
        .iter()
        .zip(modes)
        .any(|(reference, expected)| reference.mode != expected)
    {
        return Err(ProfileStoreError::InvalidProfile("probe order"));
    }
    for reference in &profile.references {
        if reference.vector.len() != dimensions {
            return Err(ProfileStoreError::InvalidProfile(
                "invalid embedding vector",
            ));
        }
        let mut norm = 0.0_f64;
        for value in &reference.vector {
            if !value.is_finite() {
                return Err(ProfileStoreError::InvalidProfile(
                    "invalid embedding vector",
                ));
            }
            let value = *value as f64;
            norm += value * value;
        }
        if profile.normalization == VectorNormalization::L2 && (norm.sqrt() - 1.0).abs() > 1e-4 {
            return Err(ProfileStoreError::InvalidProfile(
                "embedding not normalized",
            ));
        }
    }
    Ok(())
}

pub fn validate_profile(profile: &CompatibilityProfile) -> Result<(), ProfileStoreError> {
    validate_profile_shape(profile)?;
    if profile.id != compatibility_profile_id(profile) {
        return Err(ProfileStoreError::HashCollision);
    }
    Ok(())
}

#[derive(Default)]
pub struct CompatibilityProfileStore {
    profiles: HashMap<CompatibilityProfileId, CompatibilityProfile>,
    retired: HashSet<CompatibilityProfileId>,
}

impl CompatibilityProfileStore {
    pub fn open(container: &Container) -> Result<Self, ProfileStoreError> {
        let mut state = CompatibilityProfileOpenState::new();
        let mut cursor = container.physical_cursor()?;
        while let Some(mut frame) = cursor.next_frame()? {
            let object = frame.object();
            let mut payload = Vec::new();
            std::io::Read::read_to_end(&mut frame, &mut payload)
                .map_err(|_| ProfileStoreError::CorruptRecord("profile frame read"))?;
            state.ingest(object, &payload)?;
        }
        state.finish()
    }

    pub fn initialize(&self, container: &mut Container) -> Result<(), ProfileStoreError> {
        let format = encode_format();
        if crate::has_format(container, &format)? {
            return Err(ProfileStoreError::ConflictingFormat);
        }
        container.append(&format)?;
        Ok(())
    }
    pub fn put(
        &mut self,
        container: &mut Container,
        profile: CompatibilityProfile,
    ) -> Result<CompatibilityProfileId, ProfileStoreError> {
        validate_profile_shape(&profile)?;
        if profile.id != compatibility_profile_id(&profile) {
            return Err(ProfileStoreError::InvalidProfile("profile identity"));
        }
        if let Some(existing) = self.profiles.get(&profile.id) {
            return if existing == &profile {
                Ok(profile.id)
            } else {
                Err(ProfileStoreError::HashCollision)
            };
        }
        container.append(&encode_profile(&profile)?)?;
        let id = profile.id;
        self.profiles.insert(id, profile);
        Ok(id)
    }
    pub fn insert_rebuilt(
        &mut self,
        profile: CompatibilityProfile,
    ) -> Result<(), ProfileStoreError> {
        validate_profile_shape(&profile)?;
        if profile.id != compatibility_profile_id(&profile) {
            return Err(ProfileStoreError::HashCollision);
        }
        if let Some(existing) = self.profiles.get(&profile.id) {
            return if existing == &profile {
                Ok(())
            } else {
                Err(ProfileStoreError::HashCollision)
            };
        }
        self.profiles.insert(profile.id, profile);
        Ok(())
    }
    pub fn get(&self, id: CompatibilityProfileId) -> Option<&CompatibilityProfile> {
        self.profiles.get(&id)
    }
    pub fn iter(&self) -> impl Iterator<Item = &CompatibilityProfile> {
        self.profiles.values()
    }
    pub fn get_active(&self, id: CompatibilityProfileId) -> Option<&CompatibilityProfile> {
        (!self.retired.contains(&id))
            .then(|| self.profiles.get(&id))
            .flatten()
    }
    pub fn retire(
        &mut self,
        container: &mut Container,
        id: CompatibilityProfileId,
    ) -> Result<bool, ProfileStoreError> {
        if !self.profiles.contains_key(&id) {
            return Err(ProfileStoreError::MissingProfile);
        }
        if self.retired.contains(&id) {
            return Ok(false);
        }
        container.append(&encode_retirement(id))?;
        self.retired.insert(id);
        Ok(true)
    }
    pub fn retire_rebuilt(&mut self, id: CompatibilityProfileId) -> Result<(), ProfileStoreError> {
        if !self.profiles.contains_key(&id) {
            return Err(ProfileStoreError::MissingProfile);
        }
        self.retired.insert(id);
        Ok(())
    }
    pub fn is_retired(&self, id: CompatibilityProfileId) -> bool {
        self.retired.contains(&id)
    }
    pub fn profiles(&self) -> Vec<CompatibilityProfile> {
        let mut profiles: Vec<_> = self.profiles.values().cloned().collect();
        profiles.sort_by_key(|profile| profile.id.0);
        profiles
    }
    pub fn stats(&self) -> CompatibilityProfileStats {
        CompatibilityProfileStats {
            profiles: self.profiles.len(),
            active_profiles: self.profiles.len() - self.retired.len(),
            retired_profiles: self.retired.len(),
        }
    }
}

pub struct CompatibilityProfileOpenState {
    store: CompatibilityProfileStore,
    format_seen: bool,
}
impl CompatibilityProfileOpenState {
    pub fn new() -> Self {
        Self {
            store: CompatibilityProfileStore::default(),
            format_seen: false,
        }
    }
    pub fn ingest(&mut self, _chunk: ObjectRef, payload: &[u8]) -> Result<(), ProfileStoreError> {
        if decode_format(payload)? {
            if self.format_seen {
                return Err(ProfileStoreError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        if let Some(profile) = decode_profile(payload)? {
            self.store.insert_rebuilt(profile)?;
            return Ok(());
        }
        if let Some(id) = decode_retirement(payload)? {
            self.store.retire_rebuilt(id)?;
        }
        Ok(())
    }
    pub fn finish(self) -> Result<CompatibilityProfileStore, ProfileStoreError> {
        if !self.format_seen {
            return Err(ProfileStoreError::MissingFormat);
        }
        Ok(self.store)
    }
}

impl Default for CompatibilityProfileOpenState {
    fn default() -> Self {
        Self::new()
    }
}
