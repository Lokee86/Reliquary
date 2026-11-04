use crate::ContainerError;

const MAGIC: &[u8; 8] = b"RELPUB01";
const PREFIX: &[u8] = b"RELPUB";

#[derive(Clone, Copy)]
pub(crate) enum Clock {
    Allocate(Option<i64>),
    BindReserved,
}

/// A validated physical publication and its opaque owner-defined body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationEnvelope<'a> {
    pub version: u64,
    pub transaction_time_ns: Option<i64>,
    pub bound_reservation: bool,
    pub body: &'a [u8],
}

/// Decode an envelope without interpreting its owner-specific payload.
/// Non-envelope frames return `None`; malformed envelope families fail closed.
pub fn decode_publication(bytes: &[u8]) -> Result<Option<PublicationEnvelope<'_>>, ContainerError> {
    let Some((version, clock)) = decode_clock(bytes)? else {
        return Ok(None);
    };
    let (transaction_time_ns, bound_reservation, body_at) = match clock {
        Clock::Allocate(None) => (None, false, 17),
        Clock::Allocate(Some(time)) => (Some(time), false, 25),
        Clock::BindReserved => (None, true, 17),
    };
    Ok(Some(PublicationEnvelope {
        version,
        transaction_time_ns,
        bound_reservation,
        body: &bytes[body_at..],
    }))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_envelopes_preserve_physical_coordinates_and_opaque_body() {
        let body = b"OWNER001payload";
        let timed = encode(7, Some(-12), body).unwrap();
        assert_eq!(
            decode_publication(&timed).unwrap(),
            Some(PublicationEnvelope {
                version: 7,
                transaction_time_ns: Some(-12),
                bound_reservation: false,
                body
            })
        );
        let reserved = encode_bound(3, body).unwrap();
        assert_eq!(
            decode_publication(&reserved).unwrap(),
            Some(PublicationEnvelope {
                version: 3,
                transaction_time_ns: None,
                bound_reservation: true,
                body
            })
        );
        assert!(decode_publication(b"OTHER001payload").unwrap().is_none());
        for len in 6..timed.len() {
            if len < 33 {
                assert!(decode_publication(&timed[..len]).is_err());
            }
        }
    }
}
