use crate::rel_metadata_model::{
    MAX_REL_DEPENDENCIES, MAX_REL_DEPENDENCY_ID_BYTES, MAX_REL_TYPE_LABEL_BYTES, RelMetadata,
};

pub(crate) const REL_METADATA_MAGIC: [u8; 8] = *b"CVARELM1";

pub(crate) fn encode_rel_metadata(metadata: &RelMetadata) -> Result<Vec<u8>, String> {
    validate_rel_metadata(metadata)?;
    let mut out = Vec::new();
    out.extend_from_slice(&REL_METADATA_MAGIC);
    write_optional_string(&mut out, metadata.type_label.as_deref())?;
    out.extend_from_slice(&(metadata.dependencies.len() as u32).to_le_bytes());
    for dependency in &metadata.dependencies {
        write_string(&mut out, dependency)?;
    }
    Ok(out)
}

pub(crate) fn decode_rel_metadata(bytes: &[u8]) -> Result<Option<RelMetadata>, String> {
    if bytes.len() < REL_METADATA_MAGIC.len() || bytes[..8] != REL_METADATA_MAGIC {
        return Ok(None);
    }
    let mut cursor = 8;
    let type_label = read_optional_string(bytes, &mut cursor)?;
    let count = read_u32(bytes, &mut cursor)? as usize;
    if count > MAX_REL_DEPENDENCIES {
        return Err("REL dependency count exceeds limit".into());
    }
    let mut dependencies = Vec::with_capacity(count);
    for _ in 0..count {
        dependencies.push(read_string(bytes, &mut cursor)?);
    }
    if cursor != bytes.len() {
        return Err("REL metadata has trailing bytes".into());
    }
    let metadata = RelMetadata {
        type_label,
        dependencies,
    };
    validate_rel_metadata(&metadata)?;
    Ok(Some(metadata))
}

pub(crate) fn validate_rel_metadata(metadata: &RelMetadata) -> Result<(), String> {
    if let Some(label) = metadata.type_label.as_deref() {
        if label.trim().is_empty() || label.len() > MAX_REL_TYPE_LABEL_BYTES {
            return Err("REL type label is empty or exceeds limit".into());
        }
    }
    if metadata.dependencies.len() > MAX_REL_DEPENDENCIES {
        return Err("REL dependency count exceeds limit".into());
    }
    let mut previous: Option<&str> = None;
    for dependency in &metadata.dependencies {
        if dependency.trim().is_empty() || dependency.len() > MAX_REL_DEPENDENCY_ID_BYTES {
            return Err("REL dependency owner ID is empty or exceeds limit".into());
        }
        if previous.is_some_and(|value| value >= dependency.as_str()) {
            return Err("REL dependencies must be unique and sorted".into());
        }
        previous = Some(dependency);
    }
    Ok(())
}

fn write_optional_string(out: &mut Vec<u8>, value: Option<&str>) -> Result<(), String> {
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

fn read_optional_string(bytes: &[u8], cursor: &mut usize) -> Result<Option<String>, String> {
    let flag = *bytes
        .get(*cursor)
        .ok_or_else(|| "REL metadata is truncated".to_string())?;
    *cursor += 1;
    match flag {
        0 => Ok(None),
        1 => read_string(bytes, cursor).map(Some),
        _ => Err("REL metadata optional-string flag is invalid".into()),
    }
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), String> {
    let len = u32::try_from(value.len()).map_err(|_| "REL metadata string is too large")?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, String> {
    let len = read_u32(bytes, cursor)? as usize;
    let end = cursor
        .checked_add(len)
        .ok_or_else(|| "REL metadata string length overflow".to_string())?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or_else(|| "REL metadata string is truncated".to_string())?;
    *cursor = end;
    String::from_utf8(raw.to_vec()).map_err(|_| "REL metadata string is not UTF-8".into())
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, String> {
    let end = cursor
        .checked_add(4)
        .ok_or_else(|| "REL metadata length overflow".to_string())?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or_else(|| "REL metadata is truncated".to_string())?;
    *cursor = end;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}
