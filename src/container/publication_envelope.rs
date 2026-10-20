//! Physical publication framing; concrete owners interpret the opaque body.
use crate::ContainerError;

const MAGIC: &[u8; 8] = b"CVAPUB01";
const PREFIX: &[u8] = b"CVAPUB";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PublicationClock {
    Allocate {
        transaction_time_ns: Option<i64>,
    },
    /// Bind at this physical slot to an earlier standalone reservation.
    /// Its timestamp belongs to that reservation, not to this header.
    BindReserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PublicationEnvelope<'a> {
    pub global_version: u64,
    pub clock: PublicationClock,
    pub body: &'a [u8],
}

pub(crate) fn encode(envelope: PublicationEnvelope<'_>) -> Result<Vec<u8>, ContainerError> {
    validate_body(envelope.global_version, envelope.body)?;
    let (tag, timestamp) = match envelope.clock {
        PublicationClock::Allocate {
            transaction_time_ns: None,
        } => (0, None),
        PublicationClock::Allocate {
            transaction_time_ns: Some(time),
        } => (1, Some(time)),
        PublicationClock::BindReserved => (2, None),
    };
    let header_len: usize = if timestamp.is_some() { 25 } else { 17 };
    let capacity = header_len
        .checked_add(envelope.body.len())
        .ok_or(ContainerError::ChunkTooLarge)?;
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&envelope.global_version.to_le_bytes());
    out.push(tag);
    if let Some(time) = timestamp {
        out.extend_from_slice(&time.to_le_bytes());
    }
    out.extend_from_slice(envelope.body);
    Ok(out)
}

pub(crate) fn decode(payload: &[u8]) -> Result<Option<PublicationEnvelope<'_>>, ContainerError> {
    if !payload.starts_with(PREFIX) {
        return Ok(None);
    }
    if !payload.starts_with(MAGIC) || payload.len() < 17 {
        return Err(ContainerError::InvalidVersionRecord);
    }
    let global_version = u64::from_le_bytes(payload[8..16].try_into().unwrap());
    let (clock, body_at) = match payload[16] {
        0 => (
            PublicationClock::Allocate {
                transaction_time_ns: None,
            },
            17,
        ),
        1 if payload.len() >= 25 => (
            PublicationClock::Allocate {
                transaction_time_ns: Some(i64::from_le_bytes(payload[17..25].try_into().unwrap())),
            },
            25,
        ),
        2 => (PublicationClock::BindReserved, 17),
        _ => return Err(ContainerError::InvalidVersionRecord),
    };
    let body = &payload[body_at..];
    validate_body(global_version, body)?;
    Ok(Some(PublicationEnvelope {
        global_version,
        clock,
        body,
    }))
}

fn validate_body(global_version: u64, body: &[u8]) -> Result<(), ContainerError> {
    if global_version == 0 || body.len() < 8 || body.starts_with(PREFIX) {
        return Err(ContainerError::InvalidVersionRecord);
    }
    Ok(())
}

#[cfg(test)]
#[path = "publication_envelope_tests.rs"]
mod tests;
