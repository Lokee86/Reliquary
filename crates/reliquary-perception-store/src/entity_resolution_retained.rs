//! Concrete inert evidence from the frozen V1 reader's unrecognized CVAERS02 family.
//! This is literal retention, never a decoder for or activation of old decisions.
use crate::ResolutionError;
use sha2::{Digest, Sha256};
pub const SOURCE_MAGIC: &[u8; 8] = b"CVAERS02";
pub const MAGIC: &[u8; 8] = b"CVAERL01";
const MAX_BYTES: usize = 8 * 1024 * 1024;
pub fn is_source(bytes: &[u8]) -> bool {
    bytes.starts_with(SOURCE_MAGIC)
}
pub fn is_current(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}
pub fn encode(original: &[u8]) -> Result<Vec<u8>, ResolutionError> {
    if !is_source(original) || original.len() > MAX_BYTES {
        return Err(ResolutionError::CorruptRecord(
            "retained Entity resolution source",
        ));
    }
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&(original.len() as u32).to_le_bytes());
    bytes.extend_from_slice(original);
    let hash = Sha256::digest(&bytes);
    bytes.extend_from_slice(&hash);
    Ok(bytes)
}
pub fn decode(bytes: &[u8]) -> Result<Option<&[u8]>, ResolutionError> {
    if !is_current(bytes) {
        if bytes.starts_with(b"CVAERL") {
            return Err(ResolutionError::CorruptRecord(
                "retained Entity resolution version",
            ));
        }
        return Ok(None);
    }
    let error = || ResolutionError::CorruptRecord("retained Entity resolution evidence");
    if bytes.len() < 52 {
        return Err(error());
    }
    let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if len > MAX_BYTES || len.checked_add(44) != Some(bytes.len()) {
        return Err(error());
    }
    let end = 12 + len;
    if !is_source(&bytes[12..end]) || Sha256::digest(&bytes[..end]).as_slice() != &bytes[end..] {
        return Err(error());
    }
    Ok(Some(&bytes[12..end]))
}
