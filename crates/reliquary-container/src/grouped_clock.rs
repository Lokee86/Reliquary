//! Bounded physical clock recognition for established grouped record prefixes.
//! Semantic record validation remains with each owner.
use crate::ContainerError;
use sha2::{Digest, Sha256};

pub(crate) fn insomnia_range(
    bytes: &[u8],
) -> Result<Option<(u64, u32, Option<i64>)>, ContainerError> {
    let (start_at, count_at, time_at) = if bytes.starts_with(b"RELINSC8") {
        let shape = *bytes.get(8).ok_or(ContainerError::InvalidVersionRecord)?;
        if !(1..=6).contains(&shape) {
            return Err(ContainerError::InvalidVersionRecord);
        }
        if shape == 1 {
            return Ok(None);
        }
        (65, 73, if shape >= 4 { Some(77) } else { None })
    } else if bytes.starts_with(b"RELINSC7") {
        (64, 72, Some(76))
    } else {
        return Ok(None);
    };
    let end = time_at.map_or(count_at + 4, |at| at + 8);
    if bytes.len() < end {
        return Err(ContainerError::InvalidVersionRecord);
    }
    let start = u64::from_le_bytes(bytes[start_at..start_at + 8].try_into().unwrap());
    let count = u32::from_le_bytes(bytes[count_at..count_at + 4].try_into().unwrap());
    if count > 0 && start == 0 {
        return Err(ContainerError::InvalidVersionRecord);
    }
    Ok(Some((
        start,
        count,
        time_at.map(|at| i64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())),
    )))
}

pub(crate) fn external_range(bytes: &[u8]) -> Result<Option<(u64, u32, i64)>, ContainerError> {
    if !bytes.starts_with(b"RELXMEM2") {
        return Ok(None);
    }
    const MAX_BODY: usize = 8 * 1024 * 1024;
    if bytes.len() < 68 || bytes.len() > MAX_BODY + 68 {
        return Err(ContainerError::InvalidVersionRecord);
    }
    let len = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    if usize::try_from(len).ok().and_then(|n| n.checked_add(68)) != Some(bytes.len())
        || Sha256::digest(&bytes[..bytes.len() - 32]).as_slice() != &bytes[bytes.len() - 32..]
    {
        return Err(ContainerError::InvalidVersionRecord);
    }
    let count = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    if count > 128 {
        return Err(ContainerError::InvalidVersionRecord);
    }
    Ok(Some((
        u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        count,
        i64::from_le_bytes(bytes[28..36].try_into().unwrap()),
    )))
}
