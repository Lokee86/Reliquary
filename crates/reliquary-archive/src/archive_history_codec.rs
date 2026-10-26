use crate::{ArchiveError, ArchiveRecordVersion};

const FORMAT_MAGIC: [u8; 8] = *b"CVAAFMT2";
const RECORD_MAGIC: [u8; 8] = *b"CVAAREC1";

pub fn encode_archive_format() -> [u8; 12] {
    let mut out = [0_u8; 12];
    out[..8].copy_from_slice(&FORMAT_MAGIC);
    out[8..12].copy_from_slice(&1_u32.to_le_bytes());
    out
}

pub(crate) fn decode_archive_format(bytes: &[u8]) -> Result<bool, ArchiveError> {
    if bytes.len() < 8 || bytes[..8] != FORMAT_MAGIC {
        return Ok(false);
    }
    if bytes.len() != 12 || u32::from_le_bytes(bytes[8..12].try_into().unwrap()) != 1 {
        return Err(ArchiveError::CorruptRecord("invalid Archive format marker"));
    }
    Ok(true)
}

pub fn encode_record_version(version: ArchiveRecordVersion) -> [u8; 40] {
    let mut out = [0_u8; 40];
    out[..8].copy_from_slice(&RECORD_MAGIC);
    out[8..16].copy_from_slice(&version.global_version.to_le_bytes());
    out[16..24].copy_from_slice(&version.archive_version.to_le_bytes());
    out[24..40].copy_from_slice(&version.record.legacy_bytes());
    out
}

fn read_u64(bytes: &[u8], start: usize) -> u64 {
    u64::from_le_bytes(bytes[start..start + 8].try_into().unwrap())
}
