use super::{ImportComplete, ImportError, ImportStart};

const START_MAGIC: &[u8; 8] = b"CVAIEPS1";
const COMPLETE_MAGIC: &[u8; 8] = b"CVAIEPC1";

pub(crate) enum ImportEpochRecord {
    Start(ImportStart),
    Complete(ImportComplete),
}

pub(crate) fn encode_start(value: &ImportStart) -> Result<Vec<u8>, ImportError> {
    encode(START_MAGIC, value)
}

pub(crate) fn encode_complete(value: &ImportComplete) -> Result<Vec<u8>, ImportError> {
    encode(COMPLETE_MAGIC, value)
}

fn encode<T: serde::Serialize>(magic: &[u8; 8], value: &T) -> Result<Vec<u8>, ImportError> {
    let body = serde_json::to_vec(value)?;
    let mut bytes = Vec::with_capacity(12 + body.len());
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&body);
    Ok(bytes)
}

pub(crate) fn decode(payload: &[u8]) -> Result<Option<ImportEpochRecord>, ImportError> {
    let kind = match payload.get(..8) {
        Some(magic) if magic == START_MAGIC => 0,
        Some(magic) if magic == COMPLETE_MAGIC => 1,
        _ => return Ok(None),
    };
    let length = payload
        .get(8..12)
        .ok_or(ImportError::Epoch("truncated import epoch record"))?;
    let length = u32::from_le_bytes(length.try_into().unwrap()) as usize;
    let body = payload
        .get(12..)
        .filter(|body| body.len() == length)
        .ok_or(ImportError::Epoch("invalid import epoch record length"))?;
    Ok(Some(if kind == 0 {
        ImportEpochRecord::Start(serde_json::from_slice(body)?)
    } else {
        ImportEpochRecord::Complete(serde_json::from_slice(body)?)
    }))
}
