use crate::{ArchiveError, ConversationMetadata, ConversationPolicy};

pub(crate) const CONVERSATION_METADATA_MAGIC: [u8; 8] = *b"CVACONV3";

pub(crate) fn encode_conversation_metadata(
    metadata: &ConversationMetadata,
) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::with_capacity(74 + metadata.conversation_id.len());
    out.extend_from_slice(&CONVERSATION_METADATA_MAGIC);
    encode_conversation_metadata_fields(&mut out, metadata)?;
    Ok(out)
}

/// Encode the current standalone metadata fields without the record magic.
pub(crate) fn encode_conversation_metadata_fields(
    out: &mut Vec<u8>,
    metadata: &ConversationMetadata,
) -> Result<(), ArchiveError> {
    if metadata.policy_revision == 0 {
        return Err(ArchiveError::CorruptRecord(
            "zero conversation policy revision",
        ));
    }
    let mut fields = Vec::with_capacity(74 + metadata.conversation_id.len());
    write_string(&mut fields, &metadata.conversation_id)?;
    write_string(&mut fields, metadata.title.as_deref().unwrap_or(""))?;
    fields.push(u8::from(metadata.active));
    fields.push(match metadata.policy {
        ConversationPolicy::SequentialChronological => 1,
        ConversationPolicy::VisibleAutoBranch => 2,
    });
    fields.extend_from_slice(&metadata.policy_revision.to_le_bytes());
    out.extend_from_slice(&fields);
    Ok(())
}

pub(crate) fn decode_conversation_metadata(
    bytes: &[u8],
) -> Result<ConversationMetadata, ArchiveError> {
    if !bytes.starts_with(&CONVERSATION_METADATA_MAGIC) {
        return Err(ArchiveError::CorruptRecord("invalid conversation metadata"));
    }
    let mut cursor = 8;
    let metadata = decode_conversation_metadata_fields(bytes, &mut cursor)?;
    if cursor != bytes.len() {
        return Err(ArchiveError::CorruptRecord(
            "conversation metadata trailing bytes",
        ));
    }
    Ok(metadata)
}

/// Decode current V3 metadata fields from `bytes`, advancing the caller cursor.
/// The standalone wrapper owns magic validation and trailing-byte checks.
pub(crate) fn decode_conversation_metadata_fields(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<ConversationMetadata, ArchiveError> {
    let conversation_id = read_string(bytes, cursor)?;
    let title = read_string(bytes, cursor)?;
    let active = match read_byte(bytes, cursor)? {
        0 => false,
        1 => true,
        _ => {
            return Err(ArchiveError::CorruptRecord(
                "invalid conversation active flag",
            ));
        }
    };
    let policy = match read_byte(bytes, cursor)? {
        1 => ConversationPolicy::SequentialChronological,
        2 => ConversationPolicy::VisibleAutoBranch,
        _ => return Err(ArchiveError::CorruptRecord("invalid conversation policy")),
    };
    let end = cursor.checked_add(8).ok_or(ArchiveError::FieldTooLarge)?;
    let raw = bytes.get(*cursor..end).ok_or(ArchiveError::CorruptRecord(
        "missing conversation policy revision",
    ))?;
    *cursor = end;
    let policy_revision = u64::from_le_bytes(raw.try_into().unwrap());
    if policy_revision == 0 {
        return Err(ArchiveError::CorruptRecord(
            "zero conversation policy revision",
        ));
    }
    Ok(ConversationMetadata {
        conversation_id,
        title: (!title.is_empty()).then_some(title),
        active,
        policy,
        policy_revision,
    })
}

fn read_byte(bytes: &[u8], cursor: &mut usize) -> Result<u8, ArchiveError> {
    let value = *bytes.get(*cursor).ok_or(ArchiveError::CorruptRecord(
        "truncated conversation metadata",
    ))?;
    *cursor += 1;
    Ok(value)
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), ArchiveError> {
    let len = u32::try_from(value.len()).map_err(|_| ArchiveError::FieldTooLarge)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, ArchiveError> {
    let length_end = cursor.checked_add(4).ok_or(ArchiveError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..length_end)
        .ok_or(ArchiveError::CorruptRecord(
            "missing conversation metadata string length",
        ))?;
    let len = u32::from_le_bytes(raw.try_into().unwrap()) as usize;
    *cursor = length_end;
    let end = cursor.checked_add(len).ok_or(ArchiveError::FieldTooLarge)?;
    let value = bytes.get(*cursor..end).ok_or(ArchiveError::CorruptRecord(
        "truncated conversation metadata string",
    ))?;
    *cursor = end;
    String::from_utf8(value.to_vec()).map_err(|_| ArchiveError::InvalidUtf8)
}
