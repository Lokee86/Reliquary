use crate::memory_routing_model::{
    MAX_MEMORY_ENTITY_MENTIONS, MemoryEntityMention, MemoryRoutingMetadata, MemoryTextField,
};
use crate::{MemoryBodyId, MemoryError, MemoryId};

const ROUTING_MAGIC_V2: [u8; 8] = *b"CVAMRTE2";

pub fn encode(value: &MemoryRoutingMetadata) -> Result<Vec<u8>, MemoryError> {
    let mention_count =
        u32::try_from(value.entity_mentions.len()).map_err(|_| MemoryError::FieldTooLarge)?;
    let mut out = Vec::with_capacity(96);
    out.extend_from_slice(&ROUTING_MAGIC_V2);
    out.extend_from_slice(&value.memory_id.0);
    out.extend_from_slice(&value.body_id.0);
    out.extend_from_slice(&mention_count.to_le_bytes());
    write_mentions(&mut out, &value.entity_mentions)?;
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<Option<MemoryRoutingMetadata>, MemoryError> {
    if bytes.len() < 8 {
        return Ok(None);
    }
    if bytes[..8] == ROUTING_MAGIC_V2 {
        return decode_v2(bytes).map(Some);
    }
    Ok(None)
}

fn decode_v2(bytes: &[u8]) -> Result<MemoryRoutingMetadata, MemoryError> {
    if bytes.len() < 76 {
        return Err(MemoryError::CorruptRecord("short Memory routing metadata"));
    }
    let memory_id = MemoryId(bytes[8..40].try_into().unwrap());
    let body_id = MemoryBodyId(bytes[40..72].try_into().unwrap());
    let mut cursor = 72;
    let entity_mentions = read_mentions(bytes, &mut cursor)?;
    if cursor != bytes.len() {
        return Err(MemoryError::CorruptRecord(
            "Memory routing metadata trailing bytes",
        ));
    }
    Ok(MemoryRoutingMetadata {
        memory_id,
        body_id,
        entity_mentions,
    })
}

fn write_mentions(out: &mut Vec<u8>, mentions: &[MemoryEntityMention]) -> Result<(), MemoryError> {
    for mention in mentions {
        out.push(mention.field.tag());
        out.extend_from_slice(&mention.start_byte.to_le_bytes());
        out.extend_from_slice(&mention.end_byte.to_le_bytes());
        write_string(out, &mention.text)?;
    }
    Ok(())
}

fn read_mentions(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Vec<MemoryEntityMention>, MemoryError> {
    let mention_count = read_u32(bytes, cursor)? as usize;
    if mention_count > MAX_MEMORY_ENTITY_MENTIONS {
        return Err(MemoryError::CorruptRecord(
            "too many Memory entity mentions",
        ));
    }
    let mut mentions = Vec::with_capacity(mention_count);
    for _ in 0..mention_count {
        let field = MemoryTextField::from_tag(read_u8(bytes, cursor)?)
            .ok_or(MemoryError::CorruptRecord("invalid Memory mention field"))?;
        let start_byte = read_u32(bytes, cursor)?;
        let end_byte = read_u32(bytes, cursor)?;
        let text = read_string(bytes, cursor)?;
        mentions.push(MemoryEntityMention {
            field,
            start_byte,
            end_byte,
            text,
        });
    }
    Ok(mentions)
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), MemoryError> {
    let len = u32::try_from(value.len()).map_err(|_| MemoryError::FieldTooLarge)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, MemoryError> {
    let len = read_u32(bytes, cursor)? as usize;
    let end = cursor.checked_add(len).ok_or(MemoryError::FieldTooLarge)?;
    let raw = bytes.get(*cursor..end).ok_or(MemoryError::CorruptRecord(
        "truncated Memory routing string",
    ))?;
    *cursor = end;
    String::from_utf8(raw.to_vec()).map_err(|_| MemoryError::InvalidUtf8)
}

fn read_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, MemoryError> {
    let value = *bytes
        .get(*cursor)
        .ok_or(MemoryError::CorruptRecord("truncated Memory routing byte"))?;
    *cursor += 1;
    Ok(value)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, MemoryError> {
    let end = cursor.checked_add(4).ok_or(MemoryError::FieldTooLarge)?;
    let raw = bytes.get(*cursor..end).ok_or(MemoryError::CorruptRecord(
        "truncated Memory routing integer",
    ))?;
    *cursor = end;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}
