use crate::interaction_stream_store::ArchivePersistenceError;
use crate::{InteractionRole, InteractionStreamRecord, InteractionStreamStatus};

#[cfg(test)]
#[path = "interaction_stream_compact_tests.rs"]
mod compact_tests;

const COMPACT_MAGIC: &[u8; 8] = b"CVAISTR2";

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum CompactRecord {
    Begin(InteractionStreamRecord),
    Append {
        message_id: String,
        base_len: u32,
        suffix: String,
    },
    Interrupted {
        message_id: String,
        content_len: u32,
    },
}

pub(crate) fn is_compact(bytes: &[u8]) -> bool {
    bytes.starts_with(COMPACT_MAGIC)
}

pub(crate) fn encode_compact(record: &CompactRecord) -> Result<Vec<u8>, ArchivePersistenceError> {
    let mut out = COMPACT_MAGIC.to_vec();
    match record {
        CompactRecord::Begin(record) => {
            if record.message_id.is_empty()
                || record.session_id.is_empty()
                || !record.content.is_empty()
                || record.status != InteractionStreamStatus::Streaming
            {
                return Err(corrupt("invalid interaction stream Begin"));
            }
            out.push(0);
            write_string(&mut out, &record.message_id)?;
            write_string(&mut out, &record.session_id)?;
            match &record.parent_message_id {
                None => out.push(0),
                Some(parent) => {
                    out.push(1);
                    write_string(&mut out, parent)?;
                }
            }
            out.push(match record.role {
                InteractionRole::User => 0,
                InteractionRole::Agent => 1,
            });
            out.extend_from_slice(&record.timestamp_ns.to_le_bytes());
        }
        CompactRecord::Append {
            message_id,
            base_len,
            suffix,
        } => {
            if message_id.is_empty() || suffix.is_empty() {
                return Err(corrupt("invalid interaction stream Append"));
            }
            base_len
                .checked_add(
                    u32::try_from(suffix.len())
                        .map_err(|_| corrupt("interaction stream suffix exceeds u32 length"))?,
                )
                .ok_or_else(|| corrupt("interaction stream content exceeds u32 length"))?;
            out.push(1);
            write_string(&mut out, message_id)?;
            out.extend_from_slice(&base_len.to_le_bytes());
            write_string(&mut out, suffix)?;
        }
        CompactRecord::Interrupted {
            message_id,
            content_len,
        } => {
            if message_id.is_empty() {
                return Err(corrupt("invalid interaction stream Interrupted"));
            }
            out.push(2);
            write_string(&mut out, message_id)?;
            out.extend_from_slice(&content_len.to_le_bytes());
        }
    }
    Ok(out)
}

pub(crate) fn decode_compact(
    bytes: &[u8],
) -> Result<Option<CompactRecord>, ArchivePersistenceError> {
    if !is_compact(bytes) {
        if bytes.starts_with(b"CVAISTR") {
            return Err(corrupt("unknown or truncated interaction stream version"));
        }
        return Ok(None);
    }
    let mut cursor = 8;
    let record = match read_byte(bytes, &mut cursor)? {
        0 => {
            let message_id = read_string(bytes, &mut cursor)?;
            let session_id = read_string(bytes, &mut cursor)?;
            let parent_message_id = match read_byte(bytes, &mut cursor)? {
                0 => None,
                1 => Some(read_string(bytes, &mut cursor)?),
                _ => return Err(corrupt("invalid interaction stream parent flag")),
            };
            let role = match read_byte(bytes, &mut cursor)? {
                0 => InteractionRole::User,
                1 => InteractionRole::Agent,
                _ => return Err(corrupt("invalid interaction stream role")),
            };
            let timestamp_ns = read_i64(bytes, &mut cursor)?;
            CompactRecord::Begin(InteractionStreamRecord {
                message_id,
                session_id,
                parent_message_id,
                role,
                timestamp_ns,
                content: String::new(),
                status: InteractionStreamStatus::Streaming,
            })
        }
        1 => CompactRecord::Append {
            message_id: read_string(bytes, &mut cursor)?,
            base_len: read_u32(bytes, &mut cursor)?,
            suffix: read_string(bytes, &mut cursor)?,
        },
        2 => CompactRecord::Interrupted {
            message_id: read_string(bytes, &mut cursor)?,
            content_len: read_u32(bytes, &mut cursor)?,
        },
        _ => return Err(corrupt("invalid interaction stream kind")),
    };
    if cursor != bytes.len() {
        return Err(corrupt("interaction stream trailing bytes"));
    }
    match &record {
        CompactRecord::Begin(begin)
            if begin.message_id.is_empty() || begin.session_id.is_empty() =>
        {
            return Err(corrupt("invalid interaction stream Begin"));
        }
        CompactRecord::Append {
            message_id,
            base_len,
            suffix,
        } => {
            if message_id.is_empty()
                || suffix.is_empty()
                || base_len
                    .checked_add(
                        u32::try_from(suffix.len())
                            .map_err(|_| corrupt("interaction stream suffix exceeds u32 length"))?,
                    )
                    .is_none()
            {
                return Err(corrupt("invalid interaction stream Append"));
            }
        }
        CompactRecord::Interrupted { message_id, .. } if message_id.is_empty() => {
            return Err(corrupt("invalid interaction stream Interrupted"));
        }
        _ => {}
    }
    Ok(Some(record))
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, ArchivePersistenceError> {
    let end = cursor
        .checked_add(4)
        .ok_or_else(|| corrupt("interaction stream position overflow"))?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or_else(|| corrupt("interaction stream truncated position"))?;
    *cursor = end;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), ArchivePersistenceError> {
    let len = u32::try_from(value.len())
        .map_err(|_| corrupt("interaction stream field exceeds u32 length"))?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, ArchivePersistenceError> {
    let length_end = cursor
        .checked_add(4)
        .ok_or_else(|| corrupt("interaction stream length overflow"))?;
    let raw = bytes
        .get(*cursor..length_end)
        .ok_or_else(|| corrupt("interaction stream missing string length"))?;
    let len = u32::from_le_bytes(raw.try_into().unwrap()) as usize;
    *cursor = length_end;
    let end = cursor
        .checked_add(len)
        .ok_or_else(|| corrupt("interaction stream string overflow"))?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or_else(|| corrupt("interaction stream truncated string"))?;
    *cursor = end;
    String::from_utf8(raw.to_vec())
        .map_err(|_| corrupt("interaction stream contains invalid UTF-8"))
}

fn read_byte(bytes: &[u8], cursor: &mut usize) -> Result<u8, ArchivePersistenceError> {
    let value = *bytes
        .get(*cursor)
        .ok_or_else(|| corrupt("interaction stream truncated byte"))?;
    *cursor += 1;
    Ok(value)
}

fn read_i64(bytes: &[u8], cursor: &mut usize) -> Result<i64, ArchivePersistenceError> {
    let end = cursor
        .checked_add(8)
        .ok_or_else(|| corrupt("interaction stream timestamp overflow"))?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or_else(|| corrupt("interaction stream truncated timestamp"))?;
    *cursor = end;
    Ok(i64::from_le_bytes(raw.try_into().unwrap()))
}

fn corrupt(message: impl Into<String>) -> ArchivePersistenceError {
    ArchivePersistenceError::InteractionStream(message.into())
}
