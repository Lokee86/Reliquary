//! Current CVAINSC8 compact evidence with an explicit field-presence layout.
//! Layout determines clock observability; records are always compact creations.
use super::*;
fn layout(bytes: &[u8]) -> Result<u8, &'static str> {
    let value = *bytes.get(8).ok_or("short retained compact completion")?;
    if !(1..=6).contains(&value) {
        return Err("unknown retained compact completion layout");
    }
    Ok(value)
}
pub(super) fn decode_range(bytes: &[u8]) -> Result<Option<(u64, u32, Option<i64>)>, &'static str> {
    let shape = layout(bytes)?;
    if shape == 1 {
        return Ok(None);
    }
    let end = if shape >= 4 { 85 } else { 77 };
    if bytes.len() < end {
        return Err("short retained compact completion range");
    }
    let start = u64::from_le_bytes(bytes[65..73].try_into().unwrap());
    let count = u32::from_le_bytes(bytes[73..77].try_into().unwrap());
    if count > 0 && start == 0 {
        return Err("invalid completion global version range");
    }
    let time = if shape >= 4 {
        Some(i64::from_le_bytes(bytes[77..85].try_into().unwrap()))
    } else {
        None
    };
    Ok(Some((start, count, time)))
}
pub(super) fn decode(bytes: &[u8]) -> Result<InsomniaCompletion, &'static str> {
    decode_with_body_spans(bytes).map(|(completion, _)| completion)
}

pub(super) fn decode_with_body_spans(
    bytes: &[u8],
) -> Result<(InsomniaCompletion, Vec<MemoryGroupedBodySpan>), &'static str> {
    let shape = layout(bytes)?;
    // Common field readers use a fixed eight-byte prefix. The neutral prefix
    // carries no format identity and never dispatches through an old decoder.
    let mut fields = vec![0; 8];
    fields.extend_from_slice(&bytes[9..]);
    match shape {
        1 => decode_implicit_range_compact_with_body_spans(&fields, 1),
        2 => decode_local_compact_with_body_spans(&fields, 1),
        3 => decode_external_compact_with_body_spans(&fields, 1),
        4 => decode_completion_current_with_body_spans(&fields, false, false, 1),
        5 => decode_completion_current_with_body_spans(&fields, true, false, 1),
        6 => decode_completion_current_with_body_spans(&fields, true, true, 1),
        _ => unreachable!(),
    }
}
