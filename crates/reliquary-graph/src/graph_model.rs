use crate::{EntityId, MemoryId, SemanticNodeRef};
use arcana::{EdgeKind, NodeId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GraphRelationKind {
    Topical,
    Factual,
    Causal,
    Recurrent,
    References,
    DuplicateOf,
    Supersedes,
    StructuralParent,
}

impl GraphRelationKind {
    pub const fn code(self) -> u16 {
        match self {
            Self::Topical => 1,
            Self::Factual => 2,
            Self::Causal => 3,
            Self::Recurrent => 4,
            Self::References => 5,
            Self::DuplicateOf => 6,
            Self::Supersedes => 7,
            Self::StructuralParent => 8,
        }
    }

    pub const fn from_code(code: u16) -> Option<Self> {
        Some(match code {
            1 => Self::Topical,
            2 => Self::Factual,
            3 => Self::Causal,
            4 => Self::Recurrent,
            5 => Self::References,
            6 => Self::DuplicateOf,
            7 => Self::Supersedes,
            8 => Self::StructuralParent,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticGraphRelationKind {
    Memory(GraphRelationKind),
    EntityAssociation,
    PrincipalAssociation,
}

impl SemanticGraphRelationKind {
    pub const ENTITY_ASSOCIATION_CODE: u16 = 100;
    pub const PRINCIPAL_ASSOCIATION_CODE: u16 = 101;

    pub const fn code(self) -> u16 {
        match self {
            Self::Memory(kind) => kind.code(),
            Self::EntityAssociation => Self::ENTITY_ASSOCIATION_CODE,
            Self::PrincipalAssociation => Self::PRINCIPAL_ASSOCIATION_CODE,
        }
    }

    pub const fn from_code(code: u16) -> Option<Self> {
        if let Some(kind) = GraphRelationKind::from_code(code) {
            return Some(Self::Memory(kind));
        }
        match code {
            Self::ENTITY_ASSOCIATION_CODE => Some(Self::EntityAssociation),
            Self::PRINCIPAL_ASSOCIATION_CODE => Some(Self::PrincipalAssociation),
            _ => None,
        }
    }

    pub(crate) const fn edge_kind(self) -> EdgeKind {
        EdgeKind(self.code())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GraphRelationOrigin {
    Dream,
    User,
    Perception,
}

impl GraphRelationOrigin {
    pub const fn code(self) -> u8 {
        match self {
            Self::Dream => 1,
            Self::User => 2,
            Self::Perception => 3,
        }
    }

    pub const fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            1 => Self::Dream,
            2 => Self::User,
            3 => Self::Perception,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphDirection {
    Outgoing,
    Incoming,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GraphRelationChange {
    pub source: MemoryId,
    pub target: MemoryId,
    pub kind: GraphRelationKind,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SemanticGraphRelationChange {
    pub source: SemanticNodeRef,
    pub target: SemanticNodeRef,
    pub kind: SemanticGraphRelationKind,
    pub active: bool,
}

impl SemanticGraphRelationChange {
    pub const fn entity_association(
        memory_id: MemoryId,
        entity_id: EntityId,
        active: bool,
    ) -> Self {
        Self {
            source: SemanticNodeRef::memory(memory_id),
            target: SemanticNodeRef::entity(entity_id),
            kind: SemanticGraphRelationKind::EntityAssociation,
            active,
        }
    }

    pub const fn principal_association(
        memory_id: MemoryId,
        entity_id: EntityId,
        active: bool,
    ) -> Self {
        Self {
            source: SemanticNodeRef::memory(memory_id),
            target: SemanticNodeRef::entity(entity_id),
            kind: SemanticGraphRelationKind::PrincipalAssociation,
            active,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphRelation {
    pub source: MemoryId,
    pub target: MemoryId,
    pub kind: GraphRelationKind,
    pub active: bool,
    pub origin: GraphRelationOrigin,
    pub global_version: u64,
    pub graph_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticGraphRelation {
    pub source: SemanticNodeRef,
    pub target: SemanticNodeRef,
    pub kind: SemanticGraphRelationKind,
    pub active: bool,
    pub origin: GraphRelationOrigin,
    pub global_version: u64,
    pub graph_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphNeighbor {
    pub memory_id: MemoryId,
    pub kind: GraphRelationKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticGraphNeighbor {
    pub node: SemanticNodeRef,
    pub kind: SemanticGraphRelationKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryGraphPath {
    pub memories: Vec<MemoryId>,
    pub relations: Vec<GraphRelationKind>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphStats {
    pub nodes: usize,
    pub memory_nodes: usize,
    pub active_relations: usize,
    pub memory_active_relations: usize,
    pub relation_mutations: usize,
    pub graph_version: u64,
    pub memory_graph_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphNodeRecord {
    pub semantic_node: SemanticNodeRef,
    pub node_id: NodeId,
}
