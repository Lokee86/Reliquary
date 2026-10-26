//! Typed product representations; durable state belongs to reliquary-graph.
use crate::{EntityId, MemoryId, SemanticNodeRef};
use arcana::NodeId;
pub use reliquary_graph::{
    GraphDirection, GraphRelationKind, GraphRelationOrigin, GraphStats, SemanticGraphRelationKind,
};

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
pub(crate) struct GraphNodeRecord {
    pub semantic_node: SemanticNodeRef,
    pub node_id: NodeId,
}

impl From<GraphRelationChange> for reliquary_graph::GraphRelationChange {
    fn from(v: GraphRelationChange) -> Self {
        Self {
            source: v.source.0,
            target: v.target.0,
            kind: v.kind,
            active: v.active,
        }
    }
}

impl From<reliquary_graph::GraphRelationChange> for GraphRelationChange {
    fn from(v: reliquary_graph::GraphRelationChange) -> Self {
        Self {
            source: MemoryId(v.source),
            target: MemoryId(v.target),
            kind: v.kind,
            active: v.active,
        }
    }
}

impl From<GraphRelation> for reliquary_graph::GraphRelation {
    fn from(v: GraphRelation) -> Self {
        Self {
            source: v.source.0,
            target: v.target.0,
            kind: v.kind,
            active: v.active,
            origin: v.origin,
            global_version: v.global_version,
            graph_version: v.graph_version,
        }
    }
}

impl From<reliquary_graph::GraphRelation> for GraphRelation {
    fn from(v: reliquary_graph::GraphRelation) -> Self {
        Self {
            source: MemoryId(v.source),
            target: MemoryId(v.target),
            kind: v.kind,
            active: v.active,
            origin: v.origin,
            global_version: v.global_version,
            graph_version: v.graph_version,
        }
    }
}

impl From<SemanticGraphRelationChange> for reliquary_graph::SemanticGraphRelationChange {
    fn from(v: SemanticGraphRelationChange) -> Self {
        Self {
            source: v.source.into(),
            target: v.target.into(),
            kind: v.kind,
            active: v.active,
        }
    }
}

impl From<reliquary_graph::SemanticGraphRelationChange> for SemanticGraphRelationChange {
    fn from(v: reliquary_graph::SemanticGraphRelationChange) -> Self {
        Self {
            source: v.source.into(),
            target: v.target.into(),
            kind: v.kind,
            active: v.active,
        }
    }
}

impl From<SemanticGraphRelation> for reliquary_graph::SemanticGraphRelation {
    fn from(v: SemanticGraphRelation) -> Self {
        Self {
            source: v.source.into(),
            target: v.target.into(),
            kind: v.kind,
            active: v.active,
            origin: v.origin,
            global_version: v.global_version,
            graph_version: v.graph_version,
        }
    }
}

impl From<reliquary_graph::SemanticGraphRelation> for SemanticGraphRelation {
    fn from(v: reliquary_graph::SemanticGraphRelation) -> Self {
        Self {
            source: v.source.into(),
            target: v.target.into(),
            kind: v.kind,
            active: v.active,
            origin: v.origin,
            global_version: v.global_version,
            graph_version: v.graph_version,
        }
    }
}

impl From<GraphNeighbor> for reliquary_graph::GraphNeighbor {
    fn from(v: GraphNeighbor) -> Self {
        Self {
            memory_id: v.memory_id.0,
            kind: v.kind,
        }
    }
}

impl From<reliquary_graph::GraphNeighbor> for GraphNeighbor {
    fn from(v: reliquary_graph::GraphNeighbor) -> Self {
        Self {
            memory_id: MemoryId(v.memory_id),
            kind: v.kind,
        }
    }
}

impl From<SemanticGraphNeighbor> for reliquary_graph::SemanticGraphNeighbor {
    fn from(v: SemanticGraphNeighbor) -> Self {
        Self {
            node: v.node.into(),
            kind: v.kind,
        }
    }
}

impl From<reliquary_graph::SemanticGraphNeighbor> for SemanticGraphNeighbor {
    fn from(v: reliquary_graph::SemanticGraphNeighbor) -> Self {
        Self {
            node: v.node.into(),
            kind: v.kind,
        }
    }
}

impl From<MemoryGraphPath> for reliquary_graph::MemoryGraphPath {
    fn from(v: MemoryGraphPath) -> Self {
        Self {
            memories: v.memories.into_iter().map(|id| id.0).collect(),
            relations: v.relations,
        }
    }
}

impl From<reliquary_graph::MemoryGraphPath> for MemoryGraphPath {
    fn from(v: reliquary_graph::MemoryGraphPath) -> Self {
        Self {
            memories: v.memories.into_iter().map(MemoryId).collect(),
            relations: v.relations,
        }
    }
}

impl From<GraphNodeRecord> for reliquary_graph::GraphNodeRecord {
    fn from(v: GraphNodeRecord) -> Self {
        Self {
            semantic_node: v.semantic_node.into(),
            node_id: v.node_id,
        }
    }
}

impl From<reliquary_graph::GraphNodeRecord> for GraphNodeRecord {
    fn from(v: reliquary_graph::GraphNodeRecord) -> Self {
        Self {
            semantic_node: v.semantic_node.into(),
            node_id: v.node_id,
        }
    }
}
