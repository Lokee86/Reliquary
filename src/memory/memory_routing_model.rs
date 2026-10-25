use crate::{MemoryBodyId, MemoryId};

pub use crate::memory_text_field_contract::MemoryTextField;

pub const MAX_MEMORY_ENTITY_MENTIONS: usize = 64;
pub const MAX_MEMORY_ROUTING_TEXT_BYTES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryEntityMention {
    pub field: MemoryTextField,
    pub start_byte: u32,
    pub end_byte: u32,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRoutingMetadata {
    pub memory_id: MemoryId,
    pub body_id: MemoryBodyId,
    pub entity_mentions: Vec<MemoryEntityMention>,
}
