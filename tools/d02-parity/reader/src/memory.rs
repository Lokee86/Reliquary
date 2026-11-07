use reliquary_owner_session::{
    CurrentOwnerObservation, MAX_MEMORY_ID_CURSOR_BYTES, MAX_MEMORY_ID_PAGE_COUNT,
    MAX_MEMORY_ID_PAGE_PAYLOAD_BYTES, MemoryId, ReadOnlyCurrentOwnerObserver,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CURSOR_VERSION: &str = "d02m1";
const CURSOR_FAMILY: &str = "memory_ids";
const CURSOR_ORDER: &str = "ascending_memory_id";
const MAX_MEMORY_PAGE_JSON_BYTES: usize = 10_240;

pub(crate) fn observe_memory_id_page(
    observer: &mut ReadOnlyCurrentOwnerObserver,
    cursor: Option<&str>,
) -> Result<Value, String> {
    let snapshot = observer.observation().clone();
    let after = cursor
        .map(|token| decode_cursor(token, &snapshot))
        .transpose()?;
    let page = observer.memory_ids_page(after, MAX_MEMORY_ID_PAGE_COUNT)?;
    let next_cursor = if page.ids.len() == MAX_MEMORY_ID_PAGE_COUNT {
        Some(encode_cursor(
            &snapshot,
            page.ids.last().copied().ok_or("empty_full_memory_page")?,
        )?)
    } else {
        None
    };
    let ids = page.ids.iter().map(|id| hex(&id.0)).collect::<Vec<_>>();
    let mut value = json!({
        "family": CURSOR_FAMILY,
        "status": "INCOMPLETE",
        "coverage": "one_current_memory_identity_page_only",
        "memory_version": page.memory_version.to_string(),
        "ids": ids,
        "returned_count": page.ids.len().to_string(),
        "raw_payload_bytes": page.payload_bytes.to_string(),
        "maximum_items": MAX_MEMORY_ID_PAGE_COUNT.to_string(),
        "maximum_raw_payload_bytes": MAX_MEMORY_ID_PAGE_PAYLOAD_BYTES.to_string(),
        "cursor_maximum_bytes": MAX_MEMORY_ID_CURSOR_BYTES.to_string(),
        "next_cursor": next_cursor,
        "revision_history": "INCOMPLETE",
        "bodies": "INCOMPLETE",
        "lifecycle_and_provenance": "INCOMPLETE"
    });
    value["serialized_page_bytes"] = json!("0");
    for _ in 0..3 {
        let encoded = serde_json::to_vec(&value).map_err(|_| "memory_page_json_encode_failed")?;
        value["serialized_page_bytes"] = json!(encoded.len().to_string());
    }
    let encoded = serde_json::to_vec(&value).map_err(|_| "memory_page_json_encode_failed")?;
    let reported_len = encoded.len().to_string();
    if encoded.len() > MAX_MEMORY_PAGE_JSON_BYTES
        || value["serialized_page_bytes"].as_str() != Some(reported_len.as_str())
    {
        return Err("memory_page_json_exceeded_bound".into());
    }
    Ok(value)
}

fn encode_cursor(snapshot: &CurrentOwnerObservation, after: MemoryId) -> Result<String, String> {
    let unsigned = format!(
        "{CURSOR_VERSION}|{}|{}|{}|{}|{CURSOR_FAMILY}|{CURSOR_ORDER}|{}",
        hex(&snapshot.owner_id),
        hex(&snapshot.file_sha256_before),
        hex(&snapshot.history_generation),
        snapshot.memory_version,
        hex(&after.0),
    );
    let token = format!("{unsigned}|{}", hex(&Sha256::digest(unsigned.as_bytes())));
    if token.len() > MAX_MEMORY_ID_CURSOR_BYTES {
        return Err("memory_cursor_exceeded_bound".into());
    }
    Ok(token)
}

fn decode_cursor(token: &str, snapshot: &CurrentOwnerObservation) -> Result<MemoryId, String> {
    if token.len() > MAX_MEMORY_ID_CURSOR_BYTES {
        return Err("memory_cursor_exceeded_bound".into());
    }
    let fields = token.split('|').collect::<Vec<_>>();
    if fields.len() != 9
        || fields[0] != CURSOR_VERSION
        || fields[5] != CURSOR_FAMILY
        || fields[6] != CURSOR_ORDER
    {
        return Err("memory_cursor_schema_mismatch".into());
    }
    let (unsigned, checksum) = token
        .rsplit_once('|')
        .ok_or("memory_cursor_schema_mismatch")?;
    if checksum != hex(&Sha256::digest(unsigned.as_bytes())) {
        return Err("memory_cursor_checksum_mismatch".into());
    }
    let owner_id = parse_hex::<16>(fields[1]).ok_or("memory_cursor_owner_mismatch")?;
    let file_sha256 = parse_hex::<32>(fields[2]).ok_or("memory_cursor_file_mismatch")?;
    let history_generation =
        parse_hex::<32>(fields[3]).ok_or("memory_cursor_generation_mismatch")?;
    let memory_version = fields[4]
        .parse::<u64>()
        .map_err(|_| "memory_cursor_cut_mismatch")?;
    let after = parse_hex::<32>(fields[7]).ok_or("memory_cursor_after_id_invalid")?;
    if owner_id != snapshot.owner_id
        || file_sha256 != snapshot.file_sha256_before
        || history_generation != snapshot.history_generation
        || memory_version != snapshot.memory_version
    {
        return Err("memory_cursor_binding_mismatch".into());
    }
    Ok(MemoryId(after))
}

fn parse_hex<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N.checked_mul(2)? {
        return None;
    }
    let mut bytes = [0_u8; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let start = index.checked_mul(2)?;
        *byte = u8::from_str_radix(value.get(start..start + 2)?, 16).ok()?;
    }
    Some(bytes)
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_owner_session::{CurrentOwnerKind, OwnerScope};

    #[test]
    fn memory_cursor_rejects_each_context_binding_change() {
        let snapshot = CurrentOwnerObservation {
            kind: CurrentOwnerKind::Rel,
            owner_id: [0x11; 16],
            scope: OwnerScope::Project,
            latest_allocated_global_version: 9,
            transaction_time_ns_at_latest_allocated_version: Some(90),
            memory_version: 7,
            history_generation: [0x22; 32],
            file_sha256_before: [0x33; 32],
        };
        let position = MemoryId([0x44; 32]);
        let token = encode_cursor(&snapshot, position).unwrap();
        assert_eq!(decode_cursor(&token, &snapshot), Ok(position));

        for (index, replacement) in [
            (1, hex(&[0xA1; 16])),
            (2, hex(&[0xA2; 32])),
            (3, hex(&[0xA3; 32])),
            (4, "8".into()),
        ] {
            let mut fields = token.split('|').map(str::to_owned).collect::<Vec<_>>();
            fields[index] = replacement;
            let tampered = reseal(fields);
            assert_eq!(
                decode_cursor(&tampered, &snapshot),
                Err("memory_cursor_binding_mismatch".into())
            );
        }

        for (index, replacement) in [(0, "d02m2"), (5, "other_family"), (6, "descending")] {
            let mut fields = token.split('|').map(str::to_owned).collect::<Vec<_>>();
            fields[index] = replacement.into();
            let tampered = reseal(fields);
            assert_eq!(
                decode_cursor(&tampered, &snapshot),
                Err("memory_cursor_schema_mismatch".into())
            );
        }

        let tampered_checksum = format!("{}x", &token[..token.len() - 1]);
        assert_eq!(
            decode_cursor(&tampered_checksum, &snapshot),
            Err("memory_cursor_checksum_mismatch".into())
        );
    }

    fn reseal(mut fields: Vec<String>) -> String {
        let unsigned = fields[..8].join("|");
        fields[8] = hex(&Sha256::digest(unsigned.as_bytes()));
        fields.join("|")
    }
}
