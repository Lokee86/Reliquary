use crate::{
    CompatibilityProbeReference, CompatibilityProfile, CompatibilityProfileError,
    CompatibilityProfileId, EmbeddingMode, VectorNormalization,
};

const FORMAT_MAGIC: &[u8; 8] = b"CVACPFM1";
const FORMAT_SCHEMA: u32 = 1;
const PROFILE_MAGIC: &[u8; 8] = b"CVACPRO1";
const RETIREMENT_MAGIC: &[u8; 8] = b"CVACPRT1";
const FIXED_LEN: usize = 60;
const REFERENCE_HEADER_LEN: usize = 8;

pub(crate) fn encode_format() -> [u8; 12] {
    let mut out = [0_u8; 12];
    out[..8].copy_from_slice(FORMAT_MAGIC);
    out[8..].copy_from_slice(&FORMAT_SCHEMA.to_le_bytes());
    out
}

pub(crate) fn encode_retirement(id: CompatibilityProfileId) -> [u8; 40] {
    let mut out = [0_u8; 40];
    out[..8].copy_from_slice(RETIREMENT_MAGIC);
    out[8..].copy_from_slice(&id.0);
    out
}

pub(crate) fn decode_retirement(
    bytes: &[u8],
) -> Result<Option<CompatibilityProfileId>, CompatibilityProfileError> {
    if !bytes.starts_with(RETIREMENT_MAGIC) {
        return Ok(None);
    }
    if bytes.len() != 40 {
        return Err(CompatibilityProfileError::CorruptRecord(
            "profile retirement",
        ));
    }
    Ok(Some(CompatibilityProfileId(
        bytes[8..40].try_into().expect("profile id"),
    )))
}

pub(crate) fn decode_format(bytes: &[u8]) -> Result<bool, CompatibilityProfileError> {
    if !bytes.starts_with(FORMAT_MAGIC) {
        return Ok(false);
    }
    if bytes.len() != 12 || read_u32(bytes, 8)? != FORMAT_SCHEMA {
        return Err(CompatibilityProfileError::CorruptRecord(
            "profile format marker",
        ));
    }
    Ok(true)
}

pub(crate) fn encode_profile(
    profile: &CompatibilityProfile,
) -> Result<Vec<u8>, CompatibilityProfileError> {
    let vector_bytes = profile
        .references
        .iter()
        .try_fold(0_usize, |total, reference| {
            total.checked_add(reference.vector.len().checked_mul(4)?)
        })
        .ok_or(CompatibilityProfileError::SizeOverflow)?;
    let headers = profile
        .references
        .len()
        .checked_mul(REFERENCE_HEADER_LEN)
        .ok_or(CompatibilityProfileError::SizeOverflow)?;
    let mut out = Vec::with_capacity(FIXED_LEN + headers + vector_bytes);
    out.extend_from_slice(PROFILE_MAGIC);
    out.extend_from_slice(&profile.id.0);
    out.extend_from_slice(&profile.dimensions.to_le_bytes());
    out.push(profile.normalization.tag());
    out.extend_from_slice(&[0; 3]);
    out.extend_from_slice(&profile.probe_suite_version.to_le_bytes());
    out.extend_from_slice(&profile.compatibility_policy_version.to_le_bytes());
    let count = u32::try_from(profile.references.len())
        .map_err(|_| CompatibilityProfileError::SizeOverflow)?;
    out.extend_from_slice(&count.to_le_bytes());
    for reference in &profile.references {
        out.push(reference.mode.tag());
        out.extend_from_slice(&[0; 3]);
        let len = u32::try_from(reference.vector.len())
            .map_err(|_| CompatibilityProfileError::SizeOverflow)?;
        out.extend_from_slice(&len.to_le_bytes());
        for value in &reference.vector {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(out)
}

pub(crate) fn decode_profile(
    bytes: &[u8],
) -> Result<Option<CompatibilityProfile>, CompatibilityProfileError> {
    if !bytes.starts_with(PROFILE_MAGIC) {
        return Ok(None);
    }
    if bytes.len() < FIXED_LEN || bytes[45..48] != [0, 0, 0] {
        return Err(CompatibilityProfileError::CorruptRecord("profile header"));
    }
    let id = CompatibilityProfileId(bytes[8..40].try_into().expect("profile id"));
    let dimensions = read_u32(bytes, 40)?;
    let normalization = VectorNormalization::from_tag(bytes[44]).ok_or(
        CompatibilityProfileError::CorruptRecord("profile normalization"),
    )?;
    let probe_suite_version = read_u32(bytes, 48)?;
    let compatibility_policy_version = read_u32(bytes, 52)?;
    let reference_count = usize::try_from(read_u32(bytes, 56)?)
        .map_err(|_| CompatibilityProfileError::SizeOverflow)?;
    let mut offset = FIXED_LEN;
    let mut references = Vec::with_capacity(reference_count);
    for _ in 0..reference_count {
        let header = bytes
            .get(offset..offset + REFERENCE_HEADER_LEN)
            .ok_or(CompatibilityProfileError::CorruptRecord("reference header"))?;
        if header[1..4] != [0, 0, 0] {
            return Err(CompatibilityProfileError::CorruptRecord(
                "reference reserved bytes",
            ));
        }
        let mode = EmbeddingMode::from_tag(header[0])
            .ok_or(CompatibilityProfileError::CorruptRecord("reference mode"))?;
        let len = usize::try_from(u32::from_le_bytes(header[4..8].try_into().unwrap()))
            .map_err(|_| CompatibilityProfileError::SizeOverflow)?;
        offset = offset
            .checked_add(REFERENCE_HEADER_LEN)
            .ok_or(CompatibilityProfileError::SizeOverflow)?;
        let byte_len = len
            .checked_mul(4)
            .ok_or(CompatibilityProfileError::SizeOverflow)?;
        let end = offset
            .checked_add(byte_len)
            .ok_or(CompatibilityProfileError::SizeOverflow)?;
        let raw = bytes
            .get(offset..end)
            .ok_or(CompatibilityProfileError::CorruptRecord("reference vector"))?;
        let vector = raw
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect();
        references.push(CompatibilityProbeReference { mode, vector });
        offset = end;
    }
    if offset != bytes.len() {
        return Err(CompatibilityProfileError::CorruptRecord(
            "profile trailing bytes",
        ));
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

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, CompatibilityProfileError> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or(CompatibilityProfileError::CorruptRecord("profile integer"))?;
    Ok(u32::from_le_bytes(raw.try_into().expect("u32 width")))
}
