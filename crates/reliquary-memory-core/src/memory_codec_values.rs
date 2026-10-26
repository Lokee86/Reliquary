//! Memory-owned scalar and provenance value encodings shared by record carriers.
use crate::{MemoryError, MemoryId, MemorySourceRef};

pub(crate) fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), MemoryError> {
    let len = u32::try_from(value.len()).map_err(|_| MemoryError::FieldTooLarge)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

pub(crate) fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, MemoryError> {
    let end = cursor.checked_add(4).ok_or(MemoryError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(MemoryError::CorruptRecord("missing memory string length"))?;
    let len = u32::from_le_bytes(raw.try_into().unwrap()) as usize;
    *cursor = end;
    let end = cursor.checked_add(len).ok_or(MemoryError::FieldTooLarge)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(MemoryError::CorruptRecord("truncated memory string"))?;
    *cursor = end;
    String::from_utf8(value.to_vec()).map_err(|_| MemoryError::InvalidUtf8)
}

pub(crate) fn write_optional_string(
    out: &mut Vec<u8>,
    value: Option<&str>,
) -> Result<(), MemoryError> {
    match value {
        Some(value) => {
            out.push(1);
            write_string(out, value)
        }
        None => {
            out.push(0);
            Ok(())
        }
    }
}

pub(crate) fn read_optional_string(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Option<String>, MemoryError> {
    let flag = *bytes
        .get(*cursor)
        .ok_or(MemoryError::CorruptRecord("missing optional string flag"))?;
    *cursor += 1;
    match flag {
        0 => Ok(None),
        1 => read_string(bytes, cursor).map(Some),
        _ => Err(MemoryError::CorruptRecord("invalid optional string flag")),
    }
}

pub(crate) fn write_optional_source_ref(
    out: &mut Vec<u8>,
    value: Option<&MemorySourceRef>,
) -> Result<(), MemoryError> {
    let Some(value) = value else {
        out.push(0);
        return Ok(());
    };
    if value.owner_id.trim().is_empty()
        || value.source_node_id.trim().is_empty()
        || value
            .principal_id
            .as_deref()
            .is_some_and(|principal_id| principal_id.trim().is_empty())
    {
        return Err(MemoryError::InvalidProvenance);
    }
    validate_optional_pair(
        value.content_source_conversation_id.as_deref(),
        value.content_source_node_id.as_deref(),
    )?;
    validate_optional_pair(
        value.grounding_source_conversation_id.as_deref(),
        value.grounding_source_node_id.as_deref(),
    )?;
    out.push(1);
    write_string(out, &value.owner_id)?;
    write_optional_string(out, value.principal_id.as_deref())?;
    out.extend_from_slice(&value.source_episode_id.0);
    write_string(out, &value.source_node_id)?;
    write_optional_string(out, value.content_source_conversation_id.as_deref())?;
    write_optional_string(out, value.content_source_node_id.as_deref())?;
    write_optional_string(out, value.grounding_source_conversation_id.as_deref())?;
    write_optional_string(out, value.grounding_source_node_id.as_deref())?;
    Ok(())
}

pub(crate) fn read_optional_source_ref(
    bytes: &[u8],
    cursor: &mut usize,
    has_principal_id: bool,
) -> Result<Option<MemorySourceRef>, MemoryError> {
    let flag = *bytes.get(*cursor).ok_or(MemoryError::CorruptRecord(
        "missing optional source ref flag",
    ))?;
    *cursor += 1;
    match flag {
        0 => Ok(None),
        1 => {
            let owner_id = read_string(bytes, cursor)?;
            let principal_id = if has_principal_id {
                read_optional_string(bytes, cursor)?
            } else {
                None
            };
            let episode_end = cursor.checked_add(32).ok_or(MemoryError::FieldTooLarge)?;
            let episode_raw = bytes
                .get(*cursor..episode_end)
                .ok_or(MemoryError::CorruptRecord(
                    "truncated source ref episode id",
                ))?;
            *cursor = episode_end;
            let source_node_id = read_string(bytes, cursor)?;
            let content_source_conversation_id = read_optional_string(bytes, cursor)?;
            let content_source_node_id = read_optional_string(bytes, cursor)?;
            let grounding_source_conversation_id = read_optional_string(bytes, cursor)?;
            let grounding_source_node_id = read_optional_string(bytes, cursor)?;
            if owner_id.trim().is_empty()
                || source_node_id.trim().is_empty()
                || principal_id
                    .as_deref()
                    .is_some_and(|principal_id| principal_id.trim().is_empty())
            {
                return Err(MemoryError::CorruptRecord("invalid source ref"));
            }
            validate_optional_pair(
                content_source_conversation_id.as_deref(),
                content_source_node_id.as_deref(),
            )?;
            validate_optional_pair(
                grounding_source_conversation_id.as_deref(),
                grounding_source_node_id.as_deref(),
            )?;
            Ok(Some(MemorySourceRef {
                owner_id,
                principal_id,
                source_episode_id: crate::SourceEpisodeId(episode_raw.try_into().unwrap()),
                source_node_id,
                content_source_conversation_id,
                content_source_node_id,
                grounding_source_conversation_id,
                grounding_source_node_id,
            }))
        }
        _ => Err(MemoryError::CorruptRecord(
            "invalid optional source ref flag",
        )),
    }
}

fn validate_optional_pair(left: Option<&str>, right: Option<&str>) -> Result<(), MemoryError> {
    match (left, right) {
        (None, None) => Ok(()),
        (Some(left), Some(right)) if !left.trim().is_empty() && !right.trim().is_empty() => Ok(()),
        _ => Err(MemoryError::InvalidProvenance),
    }
}

pub(crate) fn write_optional_id(out: &mut Vec<u8>, value: Option<MemoryId>) {
    match value {
        Some(id) => {
            out.push(1);
            out.extend_from_slice(&id.0);
        }
        None => out.push(0),
    }
}

pub(crate) fn read_optional_id(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Option<MemoryId>, MemoryError> {
    let flag = *bytes.get(*cursor).ok_or(MemoryError::CorruptRecord(
        "missing optional memory id flag",
    ))?;
    *cursor += 1;
    match flag {
        0 => Ok(None),
        1 => {
            let end = cursor.checked_add(32).ok_or(MemoryError::FieldTooLarge)?;
            let raw = bytes
                .get(*cursor..end)
                .ok_or(MemoryError::CorruptRecord("truncated optional memory id"))?;
            *cursor = end;
            Ok(Some(MemoryId(raw.try_into().unwrap())))
        }
        _ => Err(MemoryError::CorruptRecord(
            "invalid optional memory id flag",
        )),
    }
}
