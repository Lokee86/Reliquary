use crate::ContainerError;

const MAGIC: &[u8; 8] = b"RELPUB01";
const PREFIX: &[u8] = b"RELPUB";

#[derive(Clone, Copy)]
pub(crate) enum Clock {
    Allocate(Option<i64>),
    BindReserved,
}

pub(crate) fn encode(
    version: u64,
    transaction_time_ns: Option<i64>,
    body: &[u8],
) -> Result<Vec<u8>, ContainerError> {
    if version == 0 || body.len() < 8 || body.starts_with(PREFIX) {
        return Err(ContainerError::InvalidVersionRecord);
    }
    let (tag, time) = match transaction_time_ns {
        Some(t) => (1, Some(t)),
        None => (0, None),
    };
    let mut out = Vec::with_capacity(17 + usize::from(time.is_some()) * 8 + body.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&version.to_le_bytes());
    out.push(tag);
    if let Some(t) = time {
        out.extend_from_slice(&t.to_le_bytes());
    }
    out.extend_from_slice(body);
    Ok(out)
}

pub(crate) fn encode_bound(version: u64, body: &[u8]) -> Result<Vec<u8>, ContainerError> {
    if version == 0 || body.len() < 8 || body.starts_with(PREFIX) {
        return Err(ContainerError::InvalidVersionRecord);
    }
    let mut out = Vec::with_capacity(17 + body.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&version.to_le_bytes());
    out.push(2);
    out.extend_from_slice(body);
    Ok(out)
}

pub(crate) fn decode_clock(bytes: &[u8]) -> Result<Option<(u64, Clock)>, ContainerError> {
    if !bytes.starts_with(PREFIX) {
        return Ok(None);
    }
    if !bytes.starts_with(MAGIC) || bytes.len() < 17 {
        return Err(ContainerError::InvalidVersionRecord);
    }
    let version = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let (clock, body_at) = match bytes[16] {
        0 if bytes.len() >= 25 => (Clock::Allocate(None), 17),
        1 if bytes.len() >= 33 => (
            Clock::Allocate(Some(i64::from_le_bytes(bytes[17..25].try_into().unwrap()))),
            25,
        ),
        2 => (Clock::BindReserved, 17),
        _ => return Err(ContainerError::InvalidVersionRecord),
    };
    if version == 0 || bytes.len() < body_at + 8 || bytes[body_at..].starts_with(PREFIX) {
        return Err(ContainerError::InvalidVersionRecord);
    }
    Ok(Some((version, clock)))
}
