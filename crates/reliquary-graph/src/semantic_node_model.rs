use crate::{EntityId, MemoryId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticNodeKind {
    Memory,
    Entity,
    Observation,
}
impl SemanticNodeKind {
    pub const fn code(self) -> u8 {
        match self {
            Self::Memory => 1,
            Self::Entity => 2,
            Self::Observation => 3,
        }
    }
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Memory),
            2 => Some(Self::Entity),
            3 => Some(Self::Observation),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticNodeRef {
    pub kind: SemanticNodeKind,
    pub id: [u8; 32],
}
impl SemanticNodeRef {
    pub const fn memory(id: MemoryId) -> Self {
        Self {
            kind: SemanticNodeKind::Memory,
            id,
        }
    }
    pub const fn entity(id: EntityId) -> Self {
        Self {
            kind: SemanticNodeKind::Entity,
            id,
        }
    }
    pub const fn observation(id: [u8; 32]) -> Self {
        Self {
            kind: SemanticNodeKind::Observation,
            id,
        }
    }
    pub const fn as_memory(self) -> Option<MemoryId> {
        if matches!(self.kind, SemanticNodeKind::Memory) {
            Some(self.id)
        } else {
            None
        }
    }
    pub const fn as_entity(self) -> Option<EntityId> {
        if matches!(self.kind, SemanticNodeKind::Entity) {
            Some(self.id)
        } else {
            None
        }
    }
}
impl From<MemoryId> for SemanticNodeRef {
    fn from(value: MemoryId) -> Self {
        Self::memory(value)
    }
}
