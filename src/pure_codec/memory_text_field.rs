/// Stable value contract for the two Memory text fields used by owner codecs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemoryTextField {
    Title,
    Content,
}

impl MemoryTextField {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::Title => 1,
            Self::Content => 2,
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Title),
            2 => Some(Self::Content),
            _ => None,
        }
    }
}

/// Immutable identity shared by Memory and Entity resolution codecs.
///
/// The generic owner identifier keeps this contract independent of either
/// domain model while preserving the exact byte coordinates in persisted keys.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MemoryEntityMentionKey<T> {
    pub memory_id: T,
    pub field: MemoryTextField,
    pub start_byte: u32,
    pub end_byte: u32,
}

impl<T> MemoryEntityMentionKey<T> {
    pub fn from_parts(
        memory_id: T,
        field: MemoryTextField,
        start_byte: u32,
        end_byte: u32,
    ) -> Self {
        Self {
            memory_id,
            field,
            start_byte,
            end_byte,
        }
    }
}
