use crate::{MemoryBody, MemoryBodyId, MemoryError, MemoryId};

pub(crate) const ROUTING_MARKER: &[u8; 8] = b"RELMRTE2";
const MAX_MENTIONS: usize = 64;
const MAX_TEXT_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryRoutingField {
    Title,
    Content,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRoutingMention {
    pub field: MemoryRoutingField,
    pub start_byte: u32,
    pub end_byte: u32,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRoutingMetadata {
    pub memory_id: MemoryId,
    pub body_id: MemoryBodyId,
    pub mentions: Vec<MemoryRoutingMention>,
}

impl MemoryRoutingMention {
    fn validate(&self) -> Result<(), MemoryError> {
        if self.start_byte >= self.end_byte
            || self.text.is_empty()
            || self.text.len() > MAX_TEXT_BYTES
        {
            return Err(MemoryError::Corrupt("invalid Memory routing mention"));
        }
        Ok(())
    }
}

impl MemoryRoutingMetadata {
    pub(crate) fn validate_body(&self, body: &MemoryBody) -> Result<(), MemoryError> {
        for mention in &self.mentions {
            let text = match mention.field {
                MemoryRoutingField::Title => &body.title,
                MemoryRoutingField::Content => &body.content,
            };
            if text.get(mention.start_byte as usize..mention.end_byte as usize)
                != Some(mention.text.as_str())
            {
                return Err(MemoryError::Corrupt(
                    "Memory routing span differs from body",
                ));
            }
        }
        Ok(())
    }
}

/// Encode exact body-bound routing metadata without changing the Memory clock.
pub fn encode_routing_metadata(value: &MemoryRoutingMetadata) -> Result<Vec<u8>, MemoryError> {
    if value.mentions.len() > MAX_MENTIONS {
        return Err(MemoryError::FieldTooLarge);
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(ROUTING_MARKER);
    bytes.extend_from_slice(&value.memory_id.0);
    bytes.extend_from_slice(&value.body_id.0);
    bytes.extend_from_slice(&(value.mentions.len() as u32).to_le_bytes());
    for mention in &value.mentions {
        mention.validate()?;
        bytes.push(match mention.field {
            MemoryRoutingField::Title => 1,
            MemoryRoutingField::Content => 2,
        });
        bytes.extend_from_slice(&mention.start_byte.to_le_bytes());
        bytes.extend_from_slice(&mention.end_byte.to_le_bytes());
        bytes.extend_from_slice(&(mention.text.len() as u32).to_le_bytes());
        bytes.extend_from_slice(mention.text.as_bytes());
    }
    Ok(bytes)
}

pub fn decode_routing_metadata(bytes: &[u8]) -> Result<MemoryRoutingMetadata, MemoryError> {
    if !bytes.starts_with(ROUTING_MARKER) {
        return Err(MemoryError::Corrupt("invalid Memory routing marker"));
    }
    let mut data = &bytes[8..];
    let memory_id = MemoryId(take(&mut data, 32)?.try_into().unwrap());
    let body_id = MemoryBodyId(take(&mut data, 32)?.try_into().unwrap());
    let count = number(&mut data)? as usize;
    if count > MAX_MENTIONS {
        return Err(MemoryError::FieldTooLarge);
    }
    let mut mentions = Vec::with_capacity(count);
    for _ in 0..count {
        let field = match take(&mut data, 1)?[0] {
            1 => MemoryRoutingField::Title,
            2 => MemoryRoutingField::Content,
            _ => return Err(MemoryError::Corrupt("invalid Memory routing field")),
        };
        let start_byte = number(&mut data)?;
        let end_byte = number(&mut data)?;
        let length = number(&mut data)? as usize;
        if length > MAX_TEXT_BYTES {
            return Err(MemoryError::FieldTooLarge);
        }
        let text = std::str::from_utf8(take(&mut data, length)?)
            .map_err(|_| MemoryError::Corrupt("invalid Memory routing UTF-8"))?
            .to_owned();
        let mention = MemoryRoutingMention {
            field,
            start_byte,
            end_byte,
            text,
        };
        mention.validate()?;
        mentions.push(mention);
    }
    if !data.is_empty() {
        return Err(MemoryError::Corrupt("trailing Memory routing bytes"));
    }
    Ok(MemoryRoutingMetadata {
        memory_id,
        body_id,
        mentions,
    })
}

fn take<'a>(data: &mut &'a [u8], count: usize) -> Result<&'a [u8], MemoryError> {
    let (value, rest) = data
        .split_at_checked(count)
        .ok_or(MemoryError::Corrupt("truncated Memory routing metadata"))?;
    *data = rest;
    Ok(value)
}

fn number(data: &mut &[u8]) -> Result<u32, MemoryError> {
    Ok(u32::from_le_bytes(take(data, 4)?.try_into().unwrap()))
}
