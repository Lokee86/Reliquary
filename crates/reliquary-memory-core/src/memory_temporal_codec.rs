use crate::memory_temporal_codec_kind::{
    from_tag as indication_kind_from_tag, tag as indication_kind_tag,
};
use crate::{
    MemoryBodyId, MemoryError, MemoryTemporalInference, TemporalInference,
    TemporalInferenceResolution,
};

pub(crate) fn write_optional(
    out: &mut Vec<u8>,
    value: Option<&MemoryTemporalInference>,
) -> Result<(), MemoryError> {
    let Some(value) = value else {
        out.push(0);
        return Ok(());
    };
    if value.inference.resolutions.is_empty() || value.inference.resolutions.len() > 32 {
        return Err(MemoryError::InvalidField("temporal inference"));
    }
    out.push(1);
    out.extend_from_slice(&value.body_id.0);
    match value.source_time_ns {
        Some(timestamp) => {
            out.push(1);
            out.extend_from_slice(&timestamp.to_le_bytes());
        }
        None => out.push(0),
    }
    write_string(out, &value.inference.model)?;
    write_string(out, &value.inference.contract_version)?;
    out.push(value.inference.resolutions.len() as u8);
    for resolution in &value.inference.resolutions {
        let start = u32::try_from(resolution.start_byte).map_err(|_| MemoryError::FieldTooLarge)?;
        let end = u32::try_from(resolution.end_byte).map_err(|_| MemoryError::FieldTooLarge)?;
        if start >= end
            || resolution.evidence.is_empty()
            || resolution.canonical_expression.is_empty()
        {
            return Err(MemoryError::InvalidField("temporal inference resolution"));
        }
        out.extend_from_slice(&start.to_le_bytes());
        out.extend_from_slice(&end.to_le_bytes());
        out.push(indication_kind_tag(resolution.kind));
        write_string(out, &resolution.evidence)?;
        write_string(out, &resolution.canonical_expression)?;
    }
    Ok(())
}

pub(crate) fn read_optional(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Option<MemoryTemporalInference>, MemoryError> {
    let flag = read_u8(bytes, cursor)?;
    if flag == 0 {
        return Ok(None);
    }
    if flag != 1 {
        return Err(MemoryError::CorruptRecord(
            "invalid temporal inference flag",
        ));
    }
    let body_end = cursor.checked_add(32).ok_or(MemoryError::FieldTooLarge)?;
    let body = bytes
        .get(*cursor..body_end)
        .ok_or(MemoryError::CorruptRecord(
            "truncated temporal inference body id",
        ))?;
    *cursor = body_end;
    let source_time_ns = match read_u8(bytes, cursor)? {
        0 => None,
        1 => {
            let end = cursor.checked_add(8).ok_or(MemoryError::FieldTooLarge)?;
            let raw = bytes.get(*cursor..end).ok_or(MemoryError::CorruptRecord(
                "truncated temporal inference source time",
            ))?;
            *cursor = end;
            Some(i64::from_le_bytes(raw.try_into().unwrap()))
        }
        _ => {
            return Err(MemoryError::CorruptRecord(
                "invalid temporal source time flag",
            ));
        }
    };
    let model = read_string(bytes, cursor)?;
    let contract_version = read_string(bytes, cursor)?;
    let count = read_u8(bytes, cursor)? as usize;
    if count == 0 || count > 32 || model.trim().is_empty() || contract_version.trim().is_empty() {
        return Err(MemoryError::CorruptRecord(
            "invalid temporal inference header",
        ));
    }
    let mut resolutions = Vec::with_capacity(count);
    for _ in 0..count {
        let start_byte = read_u32(bytes, cursor)? as usize;
        let end_byte = read_u32(bytes, cursor)? as usize;
        let kind = indication_kind_from_tag(read_u8(bytes, cursor)?)?;
        let evidence = read_string(bytes, cursor)?;
        let canonical_expression = read_string(bytes, cursor)?;
        if start_byte >= end_byte || evidence.is_empty() || canonical_expression.is_empty() {
            return Err(MemoryError::CorruptRecord(
                "invalid temporal inference resolution",
            ));
        }
        resolutions.push(TemporalInferenceResolution {
            start_byte,
            end_byte,
            kind,
            evidence,
            canonical_expression,
        });
    }
    Ok(Some(MemoryTemporalInference {
        body_id: MemoryBodyId(body.try_into().unwrap()),
        source_time_ns,
        inference: TemporalInference {
            model,
            contract_version,
            resolutions,
        },
    }))
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), MemoryError> {
    let len = u32::try_from(value.len()).map_err(|_| MemoryError::FieldTooLarge)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, MemoryError> {
    let len = read_u32(bytes, cursor)? as usize;
    let end = cursor.checked_add(len).ok_or(MemoryError::FieldTooLarge)?;
    let value = bytes.get(*cursor..end).ok_or(MemoryError::CorruptRecord(
        "truncated temporal inference string",
    ))?;
    *cursor = end;
    String::from_utf8(value.to_vec()).map_err(|_| MemoryError::InvalidUtf8)
}

fn read_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, MemoryError> {
    let value = *bytes
        .get(*cursor)
        .ok_or(MemoryError::CorruptRecord("truncated temporal inference"))?;
    *cursor += 1;
    Ok(value)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, MemoryError> {
    let end = cursor.checked_add(4).ok_or(MemoryError::FieldTooLarge)?;
    let raw = bytes.get(*cursor..end).ok_or(MemoryError::CorruptRecord(
        "truncated temporal inference integer",
    ))?;
    *cursor = end;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}
