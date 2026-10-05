use crate::{ArchiveError, ConversationMetadata, ConversationPolicy};

pub(crate) const CONVERSATION_METADATA_MAGIC: [u8; 8] = *b"CVACONV3";
pub(crate) const ACTIVE_CONVERSATION_METADATA_MAGIC: [u8; 8] = *b"CVACONV2";
pub(crate) const LEGACY_CONVERSATION_METADATA_MAGIC: [u8; 8] = *b"CVACONV1";

pub(crate) fn encode_conversation_metadata(
    metadata: &ConversationMetadata,
) -> Result<Vec<u8>, ArchiveError> {
    if metadata.policy_revision == 0 {
        return Err(ArchiveError::CorruptRecord(
            "zero conversation policy revision",
        ));
    }
    let mut out = Vec::with_capacity(74 + metadata.conversation_id.len());
    out.extend_from_slice(&CONVERSATION_METADATA_MAGIC);
    write_string(&mut out, &metadata.conversation_id)?;
    write_string(&mut out, metadata.title.as_deref().unwrap_or(""))?;
    out.push(u8::from(metadata.active));
    out.push(match metadata.policy {
        ConversationPolicy::SequentialChronological => 1,
        ConversationPolicy::VisibleAutoBranch => 2,
    });
    out.extend_from_slice(&metadata.policy_revision.to_le_bytes());
    Ok(out)
}

pub(crate) fn decode_conversation_metadata(
    bytes: &[u8],
) -> Result<ConversationMetadata, ArchiveError> {
    let magic = bytes.get(..8).ok_or(ArchiveError::CorruptRecord(
        "invalid conversation metadata record",
    ))?;
    let has_policy = magic == CONVERSATION_METADATA_MAGIC;
    let has_active = has_policy || magic == ACTIVE_CONVERSATION_METADATA_MAGIC;
    if !has_active && magic != LEGACY_CONVERSATION_METADATA_MAGIC {
        return Err(ArchiveError::CorruptRecord(
            "invalid conversation metadata record",
        ));
    }
    let mut cursor = 8;
    let conversation_id = read_string(bytes, &mut cursor)?;
    let title = read_string(bytes, &mut cursor)?;
    let active = if has_active {
        match read_byte(bytes, &mut cursor)? {
            0 => false,
            1 => true,
            _ => {
                return Err(ArchiveError::CorruptRecord(
                    "invalid conversation active flag",
                ));
            }
        }
    } else {
        false
    };
    let (policy, policy_revision) = if has_policy {
        let policy = match read_byte(bytes, &mut cursor)? {
            1 => ConversationPolicy::SequentialChronological,
            2 => ConversationPolicy::VisibleAutoBranch,
            _ => return Err(ArchiveError::CorruptRecord("invalid conversation policy")),
        };
        let end = cursor + 8;
        let raw = bytes.get(cursor..end).ok_or(ArchiveError::CorruptRecord(
            "missing conversation policy revision",
        ))?;
        cursor = end;
        let revision = u64::from_le_bytes(raw.try_into().unwrap());
        if revision == 0 {
            return Err(ArchiveError::CorruptRecord(
                "zero conversation policy revision",
            ));
        }
        (policy, revision)
    } else {
        (ConversationPolicy::default(), 1)
    };
    if cursor != bytes.len() {
        return Err(ArchiveError::CorruptRecord(
            "conversation metadata trailing bytes",
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
