use crate::{
    ConversationCompaction, ConversationCompactionError, MAX_CONVERSATION_COMPACTION_SUMMARY_BYTES,
};

pub(crate) const COMPACTION_MAGIC: [u8; 8] = *b"CVACMP1\0";
pub(crate) const RECORD_HEADER_LEN: usize = 40;
const STATE_FREE: u8 = 0;
const STATE_LIVE: u8 = 1;
const NONE_LEN: u32 = u32::MAX;

#[derive(Clone, Debug)]
pub enum DecodedCompactionChunk {
    Free,
    Live {
        record: ConversationCompaction,
        supersedes_through_message_id: Option<String>,
    },
}

pub fn is_compaction_payload(payload: &[u8]) -> bool {
    payload.len() >= 8 && payload[..8] == COMPACTION_MAGIC
}

pub fn free_prefix() -> [u8; 9] {
    let mut out = [0_u8; 9];
    out[..8].copy_from_slice(&COMPACTION_MAGIC);
    out[8] = STATE_FREE;
    out
}

pub fn encode_live(
    record: &ConversationCompaction,
    supersedes: Option<&str>,
) -> Result<Vec<u8>, ConversationCompactionError> {
    validate_text(&record.conversation_id, "conversation ID is required")?;
    validate_text(&record.through_message_id, "through-message ID is required")?;
    if record.summary.trim().is_empty() {
        return Err(ConversationCompactionError::InvalidRecord(
            "summary is required",
        ));
    }
    if record.summary.len() > MAX_CONVERSATION_COMPACTION_SUMMARY_BYTES {
        return Err(ConversationCompactionError::RecordTooLarge);
    }
    let conversation = record.conversation_id.as_bytes();
    let through = record.through_message_id.as_bytes();
    let summary = record.summary.as_bytes();
    let supersedes = supersedes.map(str::as_bytes);
    let conversation_len = u32_len(conversation.len())?;
    let through_len = u32_len(through.len())?;
    let summary_len = u32_len(summary.len())?;
    let supersedes_len = supersedes
        .map(|value| u32_len(value.len()))
        .transpose()?
        .unwrap_or(NONE_LEN);
    let total = RECORD_HEADER_LEN
        .checked_add(conversation.len())
        .and_then(|value| value.checked_add(through.len()))
        .and_then(|value| value.checked_add(supersedes.map_or(0, <[u8]>::len)))
        .and_then(|value| value.checked_add(summary.len()))
        .ok_or(ConversationCompactionError::RecordTooLarge)?;
    let mut out = vec![0_u8; total];
    out[..8].copy_from_slice(&COMPACTION_MAGIC);
    out[8] = STATE_LIVE;
    out[16..24].copy_from_slice(&record.generation.to_le_bytes());
    out[24..28].copy_from_slice(&conversation_len.to_le_bytes());
    out[28..32].copy_from_slice(&through_len.to_le_bytes());
    out[32..36].copy_from_slice(&supersedes_len.to_le_bytes());
    out[36..40].copy_from_slice(&summary_len.to_le_bytes());
    let mut cursor = RECORD_HEADER_LEN;
    put(&mut out, &mut cursor, conversation);
    put(&mut out, &mut cursor, through);
    if let Some(value) = supersedes {
        put(&mut out, &mut cursor, value);
    }
    put(&mut out, &mut cursor, summary);
    Ok(out)
}

pub fn decode(
    payload: &[u8],
) -> Result<Option<DecodedCompactionChunk>, ConversationCompactionError> {
    if !is_compaction_payload(payload) {
        return Ok(None);
    }
    if payload.len() < 9 {
        return Err(ConversationCompactionError::InvalidRecord(
            "truncated header",
        ));
    }
    if payload[8] == STATE_FREE {
        return Ok(Some(DecodedCompactionChunk::Free));
    }
    if payload[8] != STATE_LIVE || payload.len() < RECORD_HEADER_LEN {
        return Err(ConversationCompactionError::InvalidRecord("invalid state"));
    }
    let generation = u64::from_le_bytes(payload[16..24].try_into().unwrap());
    let conversation_len = usize_len(payload, 24)?;
    let through_len = usize_len(payload, 28)?;
    let supersedes_raw = u32::from_le_bytes(payload[32..36].try_into().unwrap());
    let supersedes_len = (supersedes_raw != NONE_LEN).then_some(supersedes_raw as usize);
    let summary_len = usize_len(payload, 36)?;
    let mut cursor = RECORD_HEADER_LEN;
    let conversation_id = text(payload, &mut cursor, conversation_len)?;
    let through_message_id = text(payload, &mut cursor, through_len)?;
    let supersedes_through_message_id = supersedes_len
        .map(|len| text(payload, &mut cursor, len))
        .transpose()?;
    let summary = text(payload, &mut cursor, summary_len)?;
    Ok(Some(DecodedCompactionChunk::Live {
        record: ConversationCompaction {
            conversation_id,
            through_message_id,
            summary,
            generation,
        },
        supersedes_through_message_id,
    }))
}

fn validate_text(value: &str, message: &'static str) -> Result<(), ConversationCompactionError> {
    if value.trim().is_empty() {
        Err(ConversationCompactionError::InvalidRecord(message))
    } else {
        Ok(())
    }
}

fn u32_len(value: usize) -> Result<u32, ConversationCompactionError> {
    u32::try_from(value).map_err(|_| ConversationCompactionError::RecordTooLarge)
}

fn usize_len(payload: &[u8], offset: usize) -> Result<usize, ConversationCompactionError> {
    Ok(u32::from_le_bytes(payload[offset..offset + 4].try_into().unwrap()) as usize)
}

fn put(out: &mut [u8], cursor: &mut usize, value: &[u8]) {
    out[*cursor..*cursor + value.len()].copy_from_slice(value);
    *cursor += value.len();
}

fn text(
    payload: &[u8],
    cursor: &mut usize,
    len: usize,
) -> Result<String, ConversationCompactionError> {
    let end = cursor
        .checked_add(len)
        .filter(|end| *end <= payload.len())
        .ok_or(ConversationCompactionError::InvalidRecord(
            "truncated payload",
        ))?;
    let value = std::str::from_utf8(&payload[*cursor..end])
        .map_err(|_| ConversationCompactionError::InvalidRecord("invalid UTF-8"))?
        .to_owned();
    *cursor = end;
    Ok(value)
}
