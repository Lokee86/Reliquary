use super::ImportError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const MAGIC: &[u8; 8] = b"CVAILEG1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LegacyImportEpochOrigin {
    LegacyCvafrs03,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LegacyImportEpochStatus {
    CompletedInitial,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyImportEpoch {
    pub origin: LegacyImportEpochOrigin,
    pub status: LegacyImportEpochStatus,
    pub owner_uuid: [u8; 16],
    pub import_id: [u8; 32],
    pub source_settlement_digest: String,
    pub source_prefix_len: u64,
    pub source_prefix_digest: String,
}

pub(crate) fn encode(frame: &LegacyImportEpoch) -> Result<Vec<u8>, ImportError> {
    let body = serde_json::to_vec(frame)?;
    let len =
        u32::try_from(body.len()).map_err(|_| ImportError::Epoch("legacy frame too large"))?;
    let mut bytes = MAGIC.to_vec();
    bytes.extend(len.to_le_bytes());
    bytes.extend(body);
    let checksum = Sha256::digest(&bytes);
    bytes.extend(checksum);
    Ok(bytes)
}

pub(crate) fn decode(payload: &[u8]) -> Result<Option<LegacyImportEpoch>, ImportError> {
    if payload.get(..8) != Some(MAGIC.as_slice()) {
        return Ok(None);
    }
    if payload.len() < 44 || payload.len() > 16 * 1024 {
        return Err(ImportError::Epoch("truncated legacy epoch frame"));
    }
    let end = payload.len() - 32;
    if Sha256::digest(&payload[..end]).as_slice() != &payload[end..] {
        return Err(ImportError::Epoch("legacy epoch frame checksum mismatch"));
    }
    let len = u32::from_le_bytes(payload[8..12].try_into().unwrap()) as usize;
    if end != 12 + len {
        return Err(ImportError::Epoch("invalid legacy epoch frame length"));
    }
    let frame: LegacyImportEpoch = serde_json::from_slice(&payload[12..end])?;
    if frame.source_settlement_digest.len() != 64
        || frame.source_prefix_digest.len() != 64
        || !frame
            .source_settlement_digest
            .bytes()
            .chain(frame.source_prefix_digest.bytes())
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(ImportError::Epoch("invalid legacy epoch digest"));
    }
    Ok(Some(frame))
}
