//! Migration-only compact records inside the exact legacy completion envelope.
//! The original variant remains explicit because it owns clock observability.
use super::*;

fn original(bytes: &[u8], header_only: bool) -> Result<Vec<u8>, &'static str> {
    let version = *bytes.get(8).ok_or("short compact legacy completion")?;
    if !(1..=6).contains(&version) {
        return Err("unknown compact legacy completion variant");
    }
    let mut out = b"CVAINSC0".to_vec();
    out[7] = b'0' + version;
    let end = if header_only {
        bytes.len().min(85)
    } else {
        bytes.len()
    };
    out.extend_from_slice(&bytes[9..end]);
    Ok(out)
}

pub(super) fn decode_range(bytes: &[u8]) -> Result<Option<(u64, u32, Option<i64>)>, &'static str> {
    decode_embedded_version_range(&original(bytes, true)?)
}

pub(super) fn decode(bytes: &[u8]) -> Result<InsomniaCompletion, &'static str> {
    let original = original(bytes, false)?;
    match original[7] {
        b'1' => decode_completion_v1(&original, true),
        b'2' => decode_completion_v2(&original, true),
        b'3' => decode_completion_v3(&original, true),
        b'4' => decode_completion_current(&original, false, false, true),
        b'5' => decode_completion_current(&original, true, false, true),
        b'6' => decode_completion_current(&original, true, true, true),
        _ => unreachable!(),
    }
}

pub(crate) fn encode_legacy_compact(
    bytes: &[u8],
    completion: &InsomniaCompletion,
) -> Result<Vec<u8>, &'static str> {
    let version = *bytes.get(7).ok_or("short legacy completion")?;
    if !(b'1'..=b'6').contains(&version) {
        return Err("expected legacy completion");
    }
    decode_completion(bytes)?.ok_or("missing legacy completion")?;
    // The validated decoded values preserve exact string and body lengths.
    // Parse only the prefix to locate the record list, retaining every other
    // original byte (including absent timestamps and legacy proof trailers).
    let mut cursor = match version {
        b'1' => 64,
        b'2' | b'3' => 76,
        _ => 84,
    };
    read_string(bytes, &mut cursor)?;
    read_string(bytes, &mut cursor)?;
    read_memory_ids(bytes, &mut cursor)?;
    if version >= b'3' {
        read_memory_refs(bytes, &mut cursor)?;
    }
    if version >= b'2' {
        let count = read_u32(bytes, &mut cursor)? as usize;
        for _ in 0..count {
            cursor = cursor.checked_add(32).ok_or("body offset overflow")?;
            let len = read_u32(bytes, &mut cursor)? as usize;
            cursor = cursor.checked_add(len).ok_or("body offset overflow")?;
            if cursor > bytes.len() {
                return Err("truncated completion body");
            }
        }
    }
    let records_start = cursor;
    let records = read_records(bytes, &mut cursor)?;
    if records != completion.records {
        return Err("completion migration record mismatch");
    }
    let mut out = b"CVAINSC8".to_vec();
    out.push(version - b'0');
    out.extend_from_slice(&bytes[8..records_start]);
    out.extend_from_slice(
        &(u32::try_from(records.len()).map_err(|_| "too many records")?).to_le_bytes(),
    );
    for record in records {
        let creation = crate::memory_compact_contract::MemoryCreation::new(record)
            .map_err(|_| "invalid migrated creation")?;
        let encoded = crate::memory_compact_codec::encode_creation(&creation)
            .map_err(|_| "invalid migrated creation")?;
        out.extend_from_slice(
            &(u32::try_from(encoded.len()).map_err(|_| "record too large")?).to_le_bytes(),
        );
        out.extend_from_slice(&encoded);
    }
    out.extend_from_slice(&bytes[cursor..]);
    Ok(out)
}
