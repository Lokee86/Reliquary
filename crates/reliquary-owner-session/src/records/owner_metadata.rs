//! Retained REL type and dependency metadata, independent of its physical scope.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedOwnerMetadata {
    pub type_label: Option<String>,
    pub dependencies: Vec<String>,
}

pub fn decode_retained_owner_metadata(
    payload: &[u8],
) -> Result<Option<RetainedOwnerMetadata>, String> {
    if !payload.starts_with(b"RELRELM") {
        return Ok(None);
    }
    let mut bytes = payload
        .strip_prefix(b"RELRELM1")
        .ok_or("unsupported retained REL metadata carrier")?;
    let type_label = match take(&mut bytes, 1)?[0] {
        0 => None,
        1 => Some(string(&mut bytes, 256)?),
        _ => return Err("invalid retained REL type flag".into()),
    };
    let count = number(&mut bytes)?;
    if count > 4096 {
        return Err("too many retained REL dependencies".into());
    }
    let mut dependencies = Vec::<String>::with_capacity(count);
    for _ in 0..count {
        let next = string(&mut bytes, 128)?;
        if dependencies
            .last()
            .is_some_and(|previous| previous >= &next)
        {
            return Err("retained REL dependencies are not sorted and unique".into());
        }
        dependencies.push(next);
    }
    if !bytes.is_empty() {
        return Err("trailing retained REL metadata".into());
    }
    Ok(Some(RetainedOwnerMetadata {
        type_label,
        dependencies,
    }))
}

fn string(bytes: &mut &[u8], maximum: usize) -> Result<String, String> {
    let len = number(bytes)?;
    if len > maximum {
        return Err("retained REL metadata field exceeds limit".into());
    }
    let value = std::str::from_utf8(take(bytes, len)?)
        .map_err(|_| "invalid retained REL metadata UTF-8")?;
    if value.trim().is_empty() {
        return Err("empty retained REL metadata field".into());
    }
    Ok(value.to_owned())
}

fn number(bytes: &mut &[u8]) -> Result<usize, String> {
    Ok(u32::from_le_bytes(take(bytes, 4)?.try_into().unwrap()) as usize)
}

fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Result<&'a [u8], String> {
    let (value, rest) = bytes
        .split_at_checked(len)
        .ok_or("truncated retained REL metadata")?;
    *bytes = rest;
    Ok(value)
}
