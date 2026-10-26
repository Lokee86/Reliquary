use crate::{ArchiveVectorId, CompatibilityProfileId, VectorGenerationError, VectorGenerationId};
use reliquary_container::ObjectRef;

const FORMAT_MAGIC: &[u8; 8] = b"CVAVGFM2";
const FORMAT_SCHEMA: u32 = 1;
const GENERATION_MAGIC: &[u8; 8] = b"CVAVGEN2";
const VERSION_MAGIC: &[u8; 8] = b"CVAVGRC2";

#[derive(Clone, Copy)]
pub(crate) struct GenerationPayload {
    pub id: VectorGenerationId,
    pub compatibility_profile_id: CompatibilityProfileId,
    pub archive_vector_id: ArchiveVectorId,
    pub source_archive_version: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct GenerationVersion {
    pub global_version: u64,
    pub vector_version: u64,
    pub record: ObjectRef,
}

pub(crate) fn encode_format() -> [u8; 12] {
    let mut out = [0_u8; 12];
    out[..8].copy_from_slice(FORMAT_MAGIC);
    out[8..].copy_from_slice(&FORMAT_SCHEMA.to_le_bytes());
    out
}

pub(crate) fn decode_format(bytes: &[u8]) -> Result<bool, VectorGenerationError> {
    if !bytes.starts_with(FORMAT_MAGIC) {
        return Ok(false);
    }
    if bytes.len() != 12 || read_u32(bytes, 8)? != FORMAT_SCHEMA {
        return Err(VectorGenerationError::CorruptRecord(
            "generation format marker",
        ));
    }
    Ok(true)
}

pub(crate) fn encode_generation(payload: GenerationPayload) -> [u8; 112] {
    let mut out = [0_u8; 112];
    out[..8].copy_from_slice(GENERATION_MAGIC);
    out[8..40].copy_from_slice(&payload.id.0);
    out[40..72].copy_from_slice(&payload.compatibility_profile_id.0);
    out[72..104].copy_from_slice(&payload.archive_vector_id.0);
    out[104..112].copy_from_slice(&payload.source_archive_version.to_le_bytes());
    out
}

pub(crate) fn decode_generation(
    bytes: &[u8],
) -> Result<Option<GenerationPayload>, VectorGenerationError> {
    if !bytes.starts_with(GENERATION_MAGIC) {
        return Ok(None);
    }
    if bytes.len() != 112 {
        return Err(VectorGenerationError::CorruptRecord("generation payload"));
    }
    Ok(Some(GenerationPayload {
        id: VectorGenerationId(bytes[8..40].try_into().expect("generation id")),
        compatibility_profile_id: CompatibilityProfileId(
            bytes[40..72].try_into().expect("profile id"),
        ),
        archive_vector_id: ArchiveVectorId(bytes[72..104].try_into().expect("archive vector id")),
        source_archive_version: read_u64(bytes, 104)?,
    }))
}

pub(crate) fn encode_version(version: GenerationVersion) -> [u8; 40] {
    let mut out = [0_u8; 40];
    out[..8].copy_from_slice(VERSION_MAGIC);
    out[8..16].copy_from_slice(&version.global_version.to_le_bytes());
    out[16..24].copy_from_slice(&version.vector_version.to_le_bytes());
    out[24..40].copy_from_slice(&version.record.legacy_bytes());
    out
}

pub(crate) fn decode_version(
    bytes: &[u8],
) -> Result<Option<GenerationVersion>, VectorGenerationError> {
    if !bytes.starts_with(VERSION_MAGIC) {
        return Ok(None);
    }
    if bytes.len() != 40 {
        return Err(VectorGenerationError::CorruptRecord("generation version"));
    }
    Ok(Some(GenerationVersion {
        global_version: read_u64(bytes, 8)?,
        vector_version: read_u64(bytes, 16)?,
        record: ObjectRef::from_legacy_bytes(bytes[24..40].try_into().unwrap()),
    }))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, VectorGenerationError> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or(VectorGenerationError::CorruptRecord("generation integer"))?;
    Ok(u32::from_le_bytes(raw.try_into().expect("u32 width")))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, VectorGenerationError> {
    let raw = bytes
        .get(offset..offset + 8)
        .ok_or(VectorGenerationError::CorruptRecord("generation integer"))?;
    Ok(u64::from_le_bytes(raw.try_into().expect("u64 width")))
}
