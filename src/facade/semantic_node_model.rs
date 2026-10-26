use crate::{EntityId, MemoryId};
pub use reliquary_graph::SemanticNodeKind;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticNodeRef {
    pub kind: SemanticNodeKind,
    pub id: [u8; 32],
}

impl SemanticNodeRef {
    pub const fn memory(id: MemoryId) -> Self {
        Self {
            kind: SemanticNodeKind::Memory,
            id: id.0,
        }
    }

    pub const fn entity(id: EntityId) -> Self {
        Self {
            kind: SemanticNodeKind::Entity,
            id: id.0,
        }
    }

    pub const fn observation(id: [u8; 32]) -> Self {
        Self {
            kind: SemanticNodeKind::Observation,
            id,
        }
    }

    pub const fn as_memory(self) -> Option<MemoryId> {
        match self.kind {
            SemanticNodeKind::Memory => Some(MemoryId(self.id)),
            SemanticNodeKind::Entity | SemanticNodeKind::Observation => None,
        }
    }

    pub const fn as_entity(self) -> Option<EntityId> {
        match self.kind {
            SemanticNodeKind::Entity => Some(EntityId(self.id)),
            SemanticNodeKind::Memory | SemanticNodeKind::Observation => None,
        }
    }
}

impl From<MemoryId> for SemanticNodeRef {
    fn from(value: MemoryId) -> Self {
        Self::memory(value)
    }
}

impl From<EntityId> for SemanticNodeRef {
    fn from(value: EntityId) -> Self {
        Self::entity(value)
    }
}

impl From<SemanticNodeRef> for reliquary_graph::SemanticNodeRef {
    fn from(value: SemanticNodeRef) -> Self {
        Self {
            kind: value.kind,
            id: value.id,
        }
    }
}
impl From<reliquary_graph::SemanticNodeRef> for SemanticNodeRef {
    fn from(value: reliquary_graph::SemanticNodeRef) -> Self {
        Self {
            kind: value.kind,
            id: value.id,
        }
    }
}
