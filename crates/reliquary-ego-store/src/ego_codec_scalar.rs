use crate::{EgoAnchorPriority, EgoError, MAX_EGO_ANCHOR_CHARS};

pub fn validate_text(kind: &str, text: &str) -> Result<(), EgoError> {
    if text.trim().is_empty() {
        return Err(invalid(format!("{kind} text is empty")));
    }
    Ok(())
}

pub fn validate_anchor_text(text: &str) -> Result<(), EgoError> {
    validate_text("anchor", text)?;
    let chars = text.chars().count();
    if chars > MAX_EGO_ANCHOR_CHARS {
        return Err(invalid(format!(
            "anchor text exceeds {MAX_EGO_ANCHOR_CHARS} characters"
        )));
    }
    Ok(())
}

pub(super) fn priority_byte(priority: EgoAnchorPriority) -> u8 {
    match priority {
        EgoAnchorPriority::High => 1,
        EgoAnchorPriority::Normal => 2,
        EgoAnchorPriority::Low => 3,
    }
}

pub(super) fn priority_from_byte(byte: u8) -> Result<EgoAnchorPriority, EgoError> {
    match byte {
        1 => Ok(EgoAnchorPriority::High),
        2 => Ok(EgoAnchorPriority::Normal),
        3 => Ok(EgoAnchorPriority::Low),
        _ => Err(invalid("anchor priority is invalid")),
    }
}

pub(super) fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, EgoError> {
    Ok(u64::from_le_bytes(read_array::<8>(bytes, cursor)?))
}

pub(super) fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), EgoError> {
    let len = u32::try_from(value.len()).map_err(|_| invalid("string is too large"))?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

pub(super) fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, EgoError> {
    let len = u32::from_le_bytes(read_array::<4>(bytes, cursor)?) as usize;
    let end = cursor
        .checked_add(len)
        .ok_or_else(|| invalid("string length overflow"))?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or_else(|| invalid("string is truncated"))?;
    *cursor = end;
    String::from_utf8(raw.to_vec()).map_err(|_| invalid("string is not UTF-8"))
}

pub(super) fn read_byte(bytes: &[u8], cursor: &mut usize) -> Result<u8, EgoError> {
    let value = *bytes
        .get(*cursor)
        .ok_or_else(|| invalid("record is truncated"))?;
    *cursor += 1;
    Ok(value)
}

pub(super) fn read_array<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], EgoError> {
    let end = cursor
        .checked_add(N)
        .ok_or_else(|| invalid("record length overflow"))?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or_else(|| invalid("record is truncated"))?;
    *cursor = end;
    Ok(raw.try_into().unwrap())
}

pub(super) fn invalid(message: impl Into<String>) -> EgoError {
    EgoError::InvalidRecord(message.into())
}
