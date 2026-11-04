use crate::{ContainerError, grouped_clock, publication_envelope};
use std::collections::{BTreeMap, BTreeSet};

const V1: &[u8; 8] = b"RELVERS1";
const V2: &[u8; 8] = b"RELVERS2";

pub(crate) fn encode(version: u64, time: Option<i64>) -> Vec<u8> {
    match time {
        Some(time) => {
            let mut out = Vec::with_capacity(24);
            out.extend_from_slice(V2);
            out.extend_from_slice(&version.to_le_bytes());
            out.extend_from_slice(&time.to_le_bytes());
            out
        }
        None => {
            let mut out = Vec::with_capacity(16);
            out.extend_from_slice(V1);
            out.extend_from_slice(&version.to_le_bytes());
            out
        }
    }
}

pub(crate) fn observe_range(next: &mut u64, start: u64, count: u64) -> Result<(), ContainerError> {
    if count == 0 {
        return Ok(());
    }
    if start != *next {
        return Err(ContainerError::InvalidVersionRecord);
    }
    *next = start
        .checked_add(count)
        .ok_or(ContainerError::VersionExhausted)?;
    Ok(())
}

pub(crate) fn observe_payload(
    next: &mut u64,
    times: &mut BTreeMap<u64, i64>,
    reservations: &mut BTreeSet<u64>,
    bytes: &[u8],
) -> Result<(), ContainerError> {
    if bytes.len() >= 8 && (&bytes[..8] == V1 || &bytes[..8] == V2) {
        let expected = if &bytes[..8] == V1 { 16 } else { 24 };
        if bytes.len() != expected {
            return Err(ContainerError::InvalidVersionRecord);
        }
        let version = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        observe_range(next, version, 1)?;
        if expected == 24 {
            times.insert(
                version,
                i64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            );
        }
        reservations.insert(version);
        return Ok(());
    }
    if let Some((version, clock)) = publication_envelope::decode_clock(bytes)? {
        match clock {
            publication_envelope::Clock::Allocate(time) => {
                observe_range(next, version, 1)?;
                if let Some(time) = time {
                    times.insert(version, time);
                }
            }
            publication_envelope::Clock::BindReserved => {
                if !reservations.remove(&version) {
                    return Err(ContainerError::InvalidVersionRecord);
                }
            }
        }
        return Ok(());
    }
    if let Some((start, count, time)) = grouped_clock::insomnia_range(bytes)? {
        observe_range(next, start, u64::from(count))?;
        if let Some(time) = time {
            observe_time_range(times, start, u64::from(count), time)?;
        }
    }
    if let Some((start, count, time)) = grouped_clock::external_range(bytes)? {
        observe_range(next, start, u64::from(count))?;
        observe_time_range(times, start, u64::from(count), time)?;
    }
    Ok(())
}

fn observe_time_range(
    times: &mut BTreeMap<u64, i64>,
    start: u64,
    count: u64,
    time: i64,
) -> Result<(), ContainerError> {
    for offset in 0..count {
        let version = start
            .checked_add(offset)
            .ok_or(ContainerError::VersionExhausted)?;
        if let Some(old) = times.insert(version, time) {
            if old != time {
                return Err(ContainerError::InvalidVersionRecord);
            }
        }
    }
    Ok(())
}
