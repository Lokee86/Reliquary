use crate::{ArchiveError, ContentId, FileId, IngestedTurn, Node, StoredFile};

pub(crate) const LEGACY_INGESTED_TURN_MAGIC: [u8; 8] = *b"CVATURN1";
pub(crate) const INGESTED_TURN_MAGIC: [u8; 8] = *b"CVATURN2";

pub(crate) fn encode_ingested_turn(turn: &IngestedTurn) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::with_capacity(128 + turn.attachments.len() * 96);
    out.extend_from_slice(&INGESTED_TURN_MAGIC);
    encode_ingested_turn_fields(&mut out, turn)?;
    Ok(out)
}

/// Encode current ingested-turn fields without the standalone record magic.
pub(crate) fn encode_ingested_turn_fields(
    out: &mut Vec<u8>,
    turn: &IngestedTurn,
) -> Result<(), ArchiveError> {
    let count = u32::try_from(turn.attachments.len()).map_err(|_| ArchiveError::FieldTooLarge)?;
    let mut fields = Vec::with_capacity(120 + turn.attachments.len() * 96);
    fields.extend_from_slice(&turn.node.timestamp_ns.to_le_bytes());
    fields.extend_from_slice(&turn.node.content_id.0);
    write_string(&mut fields, &turn.node.id)?;
    write_string(&mut fields, &turn.node.conversation_id)?;
    write_string(&mut fields, turn.node.parent_id.as_deref().unwrap_or(""))?;
    write_string(&mut fields, &turn.node.role)?;
    write_string(&mut fields, turn.node.principal_id.as_deref().unwrap_or(""))?;
    fields.extend_from_slice(&count.to_le_bytes());
    for file in &turn.attachments {
        fields.extend_from_slice(&file.id.0);
        fields.extend_from_slice(&file.content_id.0);
        fields.extend_from_slice(&file.byte_length.to_le_bytes());
        write_string(&mut fields, &file.filename)?;
        write_string(&mut fields, file.mime_type.as_deref().unwrap_or(""))?;
    }
    out.extend_from_slice(&fields);
    Ok(())
}

pub(crate) fn decode_ingested_turn(bytes: &[u8]) -> Result<IngestedTurn, ArchiveError> {
    if bytes.len() < 52 {
        return Err(ArchiveError::CorruptRecord("short ingested turn"));
    }
    let mut cursor = 8;
    let turn = decode_ingested_turn_fields(bytes, &mut cursor, bytes[..8] == INGESTED_TURN_MAGIC)?;
    if cursor != bytes.len() {
        return Err(ArchiveError::CorruptRecord("ingested turn trailing bytes"));
    }
    Ok(turn)
}

/// Decode caller-owned fields, advancing `cursor`; `has_principal` selects the
/// V2 principal field while preserving V1's `None` default.
pub(crate) fn decode_ingested_turn_fields(
    bytes: &[u8],
    cursor: &mut usize,
    has_principal: bool,
) -> Result<IngestedTurn, ArchiveError> {
    let mut offset = *cursor;
    let timestamp_ns = read_i64(bytes, &mut offset)?;
    let content_id = ContentId(read_fixed::<32>(
        bytes,
        &mut offset,
        "truncated ingested content ID",
    )?);
    let id = read_string(bytes, &mut offset)?;
    let conversation_id = read_string(bytes, &mut offset)?;
    let parent = read_string(bytes, &mut offset)?;
    let role = read_string(bytes, &mut offset)?;
    let principal_id = if has_principal {
        let principal = read_string(bytes, &mut offset)?;
        (!principal.is_empty()).then_some(principal)
    } else {
        None
    };
    let count = read_u32(bytes, &mut offset)? as usize;
    let mut attachments = Vec::with_capacity(count);
    for _ in 0..count {
        let fixed_end = offset.checked_add(72).ok_or(ArchiveError::FieldTooLarge)?;
        let fixed = bytes
            .get(offset..fixed_end)
            .ok_or(ArchiveError::CorruptRecord("truncated ingested attachment"))?;
        let file_id = FileId(fixed[0..32].try_into().unwrap());
        let file_content_id = ContentId(fixed[32..64].try_into().unwrap());
        let byte_length = u64::from_le_bytes(fixed[64..72].try_into().unwrap());
        offset = fixed_end;
        let filename = read_string(bytes, &mut offset)?;
        let mime_type = read_string(bytes, &mut offset)?;
        attachments.push(StoredFile {
            id: file_id,
            content_id: file_content_id,
            filename,
            mime_type: (!mime_type.is_empty()).then_some(mime_type),
            byte_length,
        });
    }
    *cursor = offset;
    Ok(IngestedTurn {
        node: Node {
            id,
            conversation_id,
            parent_id: (!parent.is_empty()).then_some(parent),
            role,
            principal_id,
            timestamp_ns,
            content_id,
        },
        attachments,
    })
}

fn read_i64(bytes: &[u8], cursor: &mut usize) -> Result<i64, ArchiveError> {
    let end = cursor.checked_add(8).ok_or(ArchiveError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(ArchiveError::CorruptRecord("short ingested turn"))?;
    *cursor = end;
    Ok(i64::from_le_bytes(raw.try_into().unwrap()))
}

fn read_fixed<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
    error: &'static str,
) -> Result<[u8; N], ArchiveError> {
    let end = cursor.checked_add(N).ok_or(ArchiveError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(ArchiveError::CorruptRecord(error))?;
    *cursor = end;
    Ok(raw.try_into().unwrap())
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), ArchiveError> {
    let len = u32::try_from(value.len()).map_err(|_| ArchiveError::FieldTooLarge)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, ArchiveError> {
    let end = cursor.checked_add(4).ok_or(ArchiveError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(ArchiveError::CorruptRecord("missing u32"))?;
    *cursor = end;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, ArchiveError> {
    let len = read_u32(bytes, cursor)? as usize;
    let end = cursor.checked_add(len).ok_or(ArchiveError::FieldTooLarge)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(ArchiveError::CorruptRecord("truncated string"))?;
    *cursor = end;
    String::from_utf8(value.to_vec()).map_err(|_| ArchiveError::InvalidUtf8)
}
