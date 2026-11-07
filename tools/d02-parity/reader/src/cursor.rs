use crate::{
    evidence_encoding::{hex, parse_hex, sha256},
    page_chain::{MAX_CHAIN_PAGES, PageSnapshot},
};
use reliquary_owner_session::{MAX_MEMORY_ID_CURSOR_BYTES, MemoryId};

pub(crate) const CURSOR_VERSION: &str = "d02m2";
pub(crate) const CURSOR_FAMILY: &str = "memory_ids";
pub(crate) const CURSOR_ORDER: &str = "ascending_memory_id";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CursorPosition {
    pub page_index: u64,
    pub after: MemoryId,
    pub previous_page_digest: [u8; 32],
}

pub(crate) fn encode_cursor(
    snapshot: &PageSnapshot,
    next_page_index: u64,
    after: MemoryId,
    previous_page_digest: [u8; 32],
) -> Result<String, String> {
    if next_page_index == 0 || next_page_index >= MAX_CHAIN_PAGES as u64 {
        return Err("memory_cursor_page_index_out_of_range".into());
    }
    let unsigned = format!(
        "{CURSOR_VERSION}|{}|{}|{}|{}|{}|{}|{CURSOR_FAMILY}|{CURSOR_ORDER}|{}|{}|{}",
        snapshot.kind,
        snapshot.scope,
        hex(&snapshot.owner_id),
        hex(&snapshot.file_sha256),
        hex(&snapshot.history_generation),
        snapshot.memory_version,
        next_page_index,
        hex(&after.0),
        hex(&previous_page_digest),
    );
    let token = format!("{unsigned}|{}", hex(&sha256(unsigned.as_bytes())));
    if token.len() > MAX_MEMORY_ID_CURSOR_BYTES {
        return Err("memory_cursor_exceeded_bound".into());
    }
    Ok(token)
}

pub(crate) fn decode_cursor(
    token: &str,
    snapshot: &PageSnapshot,
) -> Result<CursorPosition, String> {
    if token.len() > MAX_MEMORY_ID_CURSOR_BYTES {
        return Err("memory_cursor_exceeded_bound".into());
    }
    let fields = token.split('|').collect::<Vec<_>>();
    if fields.len() != 13
        || fields[0] != CURSOR_VERSION
        || fields[7] != CURSOR_FAMILY
        || fields[8] != CURSOR_ORDER
    {
        return Err("memory_cursor_schema_mismatch".into());
    }
    let (unsigned, checksum) = token
        .rsplit_once('|')
        .ok_or("memory_cursor_schema_mismatch")?;
    if checksum != hex(&sha256(unsigned.as_bytes())) {
        return Err("memory_cursor_checksum_mismatch".into());
    }
    let owner_id = parse_hex::<16>(fields[3]).ok_or("memory_cursor_owner_mismatch")?;
    let file_sha256 = parse_hex::<32>(fields[4]).ok_or("memory_cursor_file_mismatch")?;
    let history_generation =
        parse_hex::<32>(fields[5]).ok_or("memory_cursor_generation_mismatch")?;
    let memory_version = fields[6]
        .parse::<u64>()
        .map_err(|_| "memory_cursor_cut_mismatch")?;
    let page_index = fields[9]
        .parse::<u64>()
        .map_err(|_| "memory_cursor_page_index_invalid")?;
    let after = parse_hex::<32>(fields[10]).ok_or("memory_cursor_after_id_invalid")?;
    let previous_page_digest =
        parse_hex::<32>(fields[11]).ok_or("memory_cursor_predecessor_invalid")?;
    if fields[1] != snapshot.kind
        || fields[2] != snapshot.scope
        || owner_id != snapshot.owner_id
        || file_sha256 != snapshot.file_sha256
        || history_generation != snapshot.history_generation
        || memory_version != snapshot.memory_version
        || page_index == 0
        || page_index >= MAX_CHAIN_PAGES as u64
    {
        return Err("memory_cursor_binding_mismatch".into());
    }
    Ok(CursorPosition {
        page_index,
        after: MemoryId(after),
        previous_page_digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> PageSnapshot {
        PageSnapshot {
            kind: "REL".into(),
            scope: "project".into(),
            owner_id: [0x11; 16],
            file_sha256: [0x33; 32],
            history_generation: [0x22; 32],
            memory_version: 7,
        }
    }

    fn reseal(mut fields: Vec<String>) -> String {
        let unsigned = fields[..12].join("|");
        fields[12] = hex(&sha256(unsigned.as_bytes()));
        fields.join("|")
    }

    #[test]
    fn cursor_binds_snapshot_page_and_predecessor_but_checksum_is_not_authentication() {
        let snapshot = snapshot();
        let position = MemoryId([0x44; 32]);
        let predecessor = [0x55; 32];
        let token = encode_cursor(&snapshot, 1, position, predecessor).unwrap();
        assert_eq!(
            decode_cursor(&token, &snapshot).unwrap(),
            CursorPosition {
                page_index: 1,
                after: position,
                previous_page_digest: predecessor,
            }
        );

        for (index, replacement) in [
            (1, "PHY".into()),
            (2, "private".into()),
            (3, hex(&[0xA1; 16])),
            (4, hex(&[0xA2; 32])),
            (5, hex(&[0xA3; 32])),
            (6, "8".into()),
        ] {
            let mut fields = token.split('|').map(str::to_owned).collect::<Vec<_>>();
            fields[index] = replacement;
            let resealed = reseal(fields);
            assert_eq!(
                decode_cursor(&resealed, &snapshot),
                Err("memory_cursor_binding_mismatch".into())
            );
        }
        for (index, replacement) in [(0, "d02m3"), (7, "other_family"), (8, "descending")] {
            let mut fields = token.split('|').map(str::to_owned).collect::<Vec<_>>();
            fields[index] = replacement.into();
            assert_eq!(
                decode_cursor(&reseal(fields), &snapshot),
                Err("memory_cursor_schema_mismatch".into())
            );
        }
        let tampered_checksum = format!("{}x", &token[..token.len() - 1]);
        assert_eq!(
            decode_cursor(&tampered_checksum, &snapshot),
            Err("memory_cursor_checksum_mismatch".into())
        );

        let mut fields = token.split('|').map(str::to_owned).collect::<Vec<_>>();
        fields[9] = "2".into();
        fields[11] = hex(&[0xA4; 32]);
        let resealed = reseal(fields);
        let decoded = decode_cursor(&resealed, &snapshot).unwrap();
        assert_eq!(decoded.page_index, 2);
        assert_eq!(decoded.previous_page_digest, [0xA4; 32]);
    }
}
