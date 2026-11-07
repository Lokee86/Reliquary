use crate::ArchiveError;
use reliquary_container::{PublicationEnvelope, decode_publication};

pub const MARKER: &[u8; 8] = b"RELAPB01";
const PREFIX: &[u8; 6] = b"RELAPB";

pub(crate) enum Body<'a> {
    Applying(&'a [u8]),
    Repeat(u64),
}

pub(crate) struct Publication<'a> {
    pub envelope: PublicationEnvelope<'a>,
    pub archive_version: u64,
    pub body: Body<'a>,
}

pub(crate) fn encode(archive_version: u64, body: Body<'_>) -> Vec<u8> {
    let mut bytes = MARKER.to_vec();
    bytes.extend_from_slice(&archive_version.to_le_bytes());
    match body {
        Body::Applying(record) => {
            bytes.push(0);
            bytes.extend_from_slice(record);
        }
        Body::Repeat(first_applying) => {
            bytes.push(1);
            bytes.extend_from_slice(&first_applying.to_le_bytes());
        }
    }
    bytes
}

pub(crate) fn in_frame(bytes: &[u8]) -> Result<Option<Publication<'_>>, ArchiveError> {
    if bytes.starts_with(PREFIX) {
        return Err(ArchiveError::CorruptRecord(
            "versioned Archive record has no publication envelope",
        ));
    }
    let Some(envelope) = decode_publication(bytes)? else {
        return Ok(None);
    };
    Ok(
        decode_body(envelope.body)?.map(|(archive_version, body)| Publication {
            envelope,
            archive_version,
            body,
        }),
    )
}

pub(crate) fn decode_body(bytes: &[u8]) -> Result<Option<(u64, Body<'_>)>, ArchiveError> {
    if !bytes.starts_with(PREFIX) {
        return Ok(None);
    }
    let invalid = ArchiveError::CorruptRecord("invalid versioned Archive publication");
    if !bytes.starts_with(MARKER) || bytes.len() < 25 {
        return Err(invalid);
    }
    let archive_version = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let data = &bytes[17..];
    if archive_version == 0 {
        return Err(invalid);
    }
    let body = match bytes[16] {
        0 if !data.starts_with(PREFIX) && !data.starts_with(b"RELPUB") => Body::Applying(data),
        1 if data.len() == 8 => {
            let origin = u64::from_le_bytes(data.try_into().unwrap());
            if origin == 0 || origin >= archive_version {
                return Err(invalid);
            }
            Body::Repeat(origin)
        }
        _ => return Err(invalid),
    };
    Ok(Some((archive_version, body)))
}
