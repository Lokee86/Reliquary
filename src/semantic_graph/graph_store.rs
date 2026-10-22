use crate::graph_codec::{GraphNodePayload, encode_format, encode_node};
use crate::{
    Container, GraphError, GraphNodeRecord, MemoryId, SemanticGraphRelation,
    SemanticGraphRelationKind, SemanticNodeRef,
};
use arcana::storage::InMemoryGraph;
use arcana::{GraphDataset, NodeId};
use std::collections::{BTreeSet, HashMap, HashSet};

#[cfg(test)]
#[path = "graph_publication_overflow_tests.rs"]
mod publication_overflow_tests;

#[path = "graph_store_compact.rs"]
mod compact;
#[path = "graph_store_read.rs"]
mod read;
#[path = "graph_rewrite_baseline.rs"]
mod rewrite_baseline;
#[path = "graph_store_validation.rs"]
mod validation;
#[path = "graph_store_write.rs"]
mod write;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct RelationKey {
    source: SemanticNodeRef,
    target: SemanticNodeRef,
    kind: SemanticGraphRelationKind,
}

pub(crate) struct GraphStore {
    rewrite_baseline_memories: Vec<MemoryId>,
    rewrite_baseline: Option<([u8; 16], crate::HistoryGenerationId, [u8; 32])>,
    nodes: Vec<GraphNodeRecord>,
    node_by_semantic: HashMap<SemanticNodeRef, NodeId>,
    states: HashMap<RelationKey, SemanticGraphRelation>,
    // Derived visibility only; canonical relation mutations remain unchanged.
    excluded_nodes: HashSet<SemanticNodeRef>,
    relation_mutations: usize,
    transaction_global_versions: Vec<u64>,
    topology: InMemoryGraph,
    memory_topology: InMemoryGraph,
    memory_relation_nodes: HashSet<MemoryId>,
    memory_nodes: Vec<MemoryId>,
    memory_node_by_id: HashMap<MemoryId, NodeId>,
    entities_by_memory: HashMap<MemoryId, BTreeSet<crate::EntityId>>,
    memories_by_entity: HashMap<crate::EntityId, HashSet<MemoryId>>,
    principals_by_memory: HashMap<MemoryId, BTreeSet<crate::EntityId>>,
    memories_by_principal: HashMap<crate::EntityId, HashSet<MemoryId>>,
    next_graph_version: u64,
    memory_graph_version: u64,
    format_initialized: bool,
}

impl GraphStore {
    pub(crate) fn validate_memory_change_shape(
        change: crate::GraphRelationChange,
        origin: crate::GraphRelationOrigin,
    ) -> Result<(), GraphError> {
        validation::validate_memory_change(change, origin)
    }

    pub(crate) fn validate_semantic_change_shape(
        change: crate::SemanticGraphRelationChange,
        origin: crate::GraphRelationOrigin,
    ) -> Result<(), GraphError> {
        validation::validate_change(change, origin)
    }

    pub(crate) fn empty() -> Self {
        Self {
            rewrite_baseline_memories: Vec::new(),
            rewrite_baseline: None,
            nodes: Vec::new(),
            node_by_semantic: HashMap::new(),
            states: HashMap::new(),
            excluded_nodes: HashSet::new(),
            relation_mutations: 0,
            transaction_global_versions: Vec::new(),
            topology: empty_topology(),
            memory_topology: empty_topology(),
            memory_relation_nodes: HashSet::new(),
            memory_nodes: Vec::new(),
            memory_node_by_id: HashMap::new(),
            entities_by_memory: HashMap::new(),
            memories_by_entity: HashMap::new(),
            principals_by_memory: HashMap::new(),
            memories_by_principal: HashMap::new(),
            next_graph_version: 1,
            memory_graph_version: 0,
            format_initialized: false,
        }
    }

    pub(crate) fn initialize(&mut self, container: &mut Container) -> Result<(), GraphError> {
        self.ensure_format(container)
    }

    pub(crate) fn mark_format_initialized(&mut self) {
        self.format_initialized = true;
    }

    pub(crate) fn insert_node_rebuilt(&mut self, node: GraphNodeRecord) -> Result<(), GraphError> {
        let expected = u32::try_from(self.nodes.len()).map_err(|_| GraphError::NodeIdExhausted)?;
        if node.node_id != NodeId(expected)
            || self.node_by_semantic.contains_key(&node.semantic_node)
        {
            return Err(GraphError::InvalidNodeMapping);
        }
        self.node_by_semantic
            .insert(node.semantic_node, node.node_id);
        self.nodes.push(node);
        Ok(())
    }

    pub(crate) fn insert_transaction_rebuilt(
        &mut self,
        relations: &[SemanticGraphRelation],
    ) -> Result<(), GraphError> {
        if relations.is_empty()
            || relations
                .iter()
                .any(|relation| relation.graph_version != self.next_graph_version)
        {
            return Err(GraphError::InvalidGraphVersion);
        }
        let next_graph_version = self
            .next_graph_version
            .checked_add(1)
            .ok_or(GraphError::GraphVersionExhausted)?;
        for relation in relations {
            validation::validate_relation_contract(
                relation.source,
                relation.target,
                relation.kind,
                relation.origin,
            )?;
            if !self.node_by_semantic.contains_key(&relation.source)
                || !self.node_by_semantic.contains_key(&relation.target)
            {
                return Err(GraphError::InvalidNodeMapping);
            }
        }
        self.insert_transaction(relations)?;
        self.next_graph_version = next_graph_version;
        Ok(())
    }

    pub(crate) fn finish_rebuild(&mut self) -> Result<(), GraphError> {
        for node in &self.nodes {
            if node.semantic_node.kind == crate::SemanticNodeKind::Observation {
                return Err(GraphError::UnsupportedSemanticNode(node.semantic_node));
            }
            if self.node_by_semantic.get(&node.semantic_node) != Some(&node.node_id) {
                return Err(GraphError::InvalidNodeMapping);
            }
        }
        for relation in self.states.values().filter(|relation| relation.active) {
            validation::validate_stored_change(*relation)?;
        }
        self.excluded_nodes.clear();
        self.rebuild_topologies()
    }

    pub(crate) fn exclude_from_current(&mut self, node: SemanticNodeRef) -> Result<(), GraphError> {
        if self.excluded_nodes.insert(node) {
            self.rebuild_topologies()?;
        }
        Ok(())
    }

    pub(crate) fn exclude_from_current_many(
        &mut self,
        nodes: &[SemanticNodeRef],
    ) -> Result<(), GraphError> {
        let mut changed = false;
        for node in nodes {
            changed |= self.excluded_nodes.insert(*node);
        }
        if changed {
            self.rebuild_topologies()?;
        }
        Ok(())
    }

    fn is_current_relation(&self, relation: &SemanticGraphRelation) -> bool {
        relation.active
            && !self.excluded_nodes.contains(&relation.source)
            && !self.excluded_nodes.contains(&relation.target)
    }

    pub(super) fn ensure_format(&mut self, container: &mut Container) -> Result<(), GraphError> {
        if !self.format_initialized {
            container.append(&encode_format())?;
            self.format_initialized = true;
        }
        Ok(())
    }

    pub(super) fn ensure_semantic_node(
        &mut self,
        container: &mut Container,
        semantic_node: SemanticNodeRef,
    ) -> Result<NodeId, GraphError> {
        if let Some(node_id) = self.node_by_semantic.get(&semantic_node).copied() {
            return Ok(node_id);
        }
        let raw = u32::try_from(self.nodes.len()).map_err(|_| GraphError::NodeIdExhausted)?;
        let node_id = NodeId(raw);
        container.append(&encode_node(GraphNodePayload {
            semantic_node,
            node_id,
        }))?;
        self.node_by_semantic.insert(semantic_node, node_id);
        self.nodes.push(GraphNodeRecord {
            semantic_node,
            node_id,
        });
        Ok(node_id)
    }

    pub(super) fn insert_transaction(
        &mut self,
        relations: &[SemanticGraphRelation],
    ) -> Result<(), GraphError> {
        let first = relations.first().ok_or(GraphError::InvalidGraphVersion)?;
        if relations.iter().any(|relation| {
            relation.global_version != first.global_version
                || relation.graph_version != first.graph_version
        }) || self
            .transaction_global_versions
            .last()
            .is_some_and(|previous| *previous >= first.global_version)
        {
            return Err(GraphError::InvalidGraphVersion);
        }

        let mut keys = std::collections::HashSet::with_capacity(relations.len());
        for relation in relations {
            let key = RelationKey {
                source: relation.source,
                target: relation.target,
                kind: relation.kind,
            };
            if !keys.insert(key) {
                return Err(GraphError::DuplicateRelationChange);
            }
        }

        for relation in relations {
            let key = RelationKey {
                source: relation.source,
                target: relation.target,
                kind: relation.kind,
            };
            self.states.insert(key, *relation);
            if matches!(relation.kind, SemanticGraphRelationKind::Memory(_)) {
                self.memory_relation_nodes
                    .insert(relation.source.as_memory().expect("Memory relation source"));
                self.memory_relation_nodes
                    .insert(relation.target.as_memory().expect("Memory relation target"));
            }
        }
        self.relation_mutations += relations.len();
        if relations
            .iter()
            .any(|relation| matches!(relation.kind, SemanticGraphRelationKind::Memory(_)))
        {
            self.memory_graph_version = first.graph_version;
        }
        self.transaction_global_versions.push(first.global_version);
        Ok(())
    }
}

fn empty_topology() -> InMemoryGraph {
    InMemoryGraph::new(&GraphDataset {
        node_count: 0,
        edges: Vec::new(),
    })
    .expect("empty topology is valid")
}
