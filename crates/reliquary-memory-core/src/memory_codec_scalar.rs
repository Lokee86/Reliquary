use crate::{EpisodeId, MemoryError, SourceEpisodeId};

pub(crate) fn read_i64(bytes: &[u8], cursor: &mut usize) -> Result<i64, MemoryError> {
    let end = cursor.checked_add(8).ok_or(MemoryError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(MemoryError::CorruptRecord("truncated i64"))?;
    *cursor = end;
    Ok(i64::from_le_bytes(raw.try_into().unwrap()))
}

pub(crate) fn write_optional_i64(out: &mut Vec<u8>, value: Option<i64>) {
    match value {
        Some(value) => {
            out.push(1);
            out.extend_from_slice(&value.to_le_bytes());
        }
        None => out.push(0),
    }
}

pub(crate) fn read_optional_i64(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Option<i64>, MemoryError> {
    let flag = *bytes
        .get(*cursor)
        .ok_or(MemoryError::CorruptRecord("missing optional i64 flag"))?;
    *cursor += 1;
    match flag {
        0 => Ok(None),
        1 => read_i64(bytes, cursor).map(Some),
        _ => Err(MemoryError::CorruptRecord("invalid optional i64 flag")),
    }
}

pub(crate) fn write_optional_episode(out: &mut Vec<u8>, value: Option<EpisodeId>) {
    match value {
        Some(id) => {
            out.push(1);
            out.extend_from_slice(&id.0);
        }
        None => out.push(0),
    }
}

pub(crate) fn read_optional_episode(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Option<EpisodeId>, MemoryError> {
    let flag = *bytes.get(*cursor).ok_or(MemoryError::CorruptRecord(
        "missing optional episode id flag",
    ))?;
    *cursor += 1;
    match flag {
        0 => Ok(None),
        1 => {
            let end = cursor.checked_add(32).ok_or(MemoryError::FieldTooLarge)?;
            let raw = bytes
                .get(*cursor..end)
                .ok_or(MemoryError::CorruptRecord("truncated optional episode id"))?;
            *cursor = end;
            Ok(Some(SourceEpisodeId(raw.try_into().unwrap())))
        }
        _ => Err(MemoryError::CorruptRecord(
            "invalid optional episode id flag",
        )),
    }
}
