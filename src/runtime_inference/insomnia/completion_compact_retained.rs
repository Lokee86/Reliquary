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
pub(super) fn decode(bytes: &[u8]) -> Result<InsomniaCompletion, &'static str> {
    let shape = layout(bytes)?;
    // Common field readers use a fixed eight-byte prefix. The neutral prefix
    // carries no format identity and never dispatches through an old decoder.
    let mut fields = vec![0; 8];
    fields.extend_from_slice(&bytes[9..]);
    match shape {
        1 => decode_implicit_range_compact(&fields),
        2 => decode_local_compact(&fields),
        3 => decode_external_compact(&fields),
        4 => decode_completion_current(&fields, false, false),
        5 => decode_completion_current(&fields, true, false),
        6 => decode_completion_current(&fields, true, true),
        _ => unreachable!(),
    }
}
