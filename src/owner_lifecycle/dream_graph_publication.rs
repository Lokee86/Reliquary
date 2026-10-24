use crate::graph_store::GraphStore;
use crate::memory_store::{MemoryReadView, MemoryStore};
use crate::{Container, GraphError, GraphRelation, GraphRelationChange};

/// One already-mounted physical publication boundary; no semantic callback.
pub(crate) struct DreamGraphPublication<'a> {
    container: &'a mut Container,
    memories: &'a MemoryStore,
    graph: &'a mut GraphStore,
}

impl<'a> DreamGraphPublication<'a> {
    pub(crate) fn new(
        container: &'a mut Container,
        memories: &'a MemoryStore,
        graph: &'a mut GraphStore,
    ) -> Self {
        Self {
            container,
            memories,
            graph,
        }
    }
    pub(crate) fn memories(&self) -> MemoryReadView<'_> {
        MemoryReadView::new(self.memories, self.container)
    }
    pub(crate) fn graph(&self) -> &GraphStore {
        self.graph
    }
    pub(crate) fn publish(
        &mut self,
        changes: &[GraphRelationChange],
        expected: u64,
    ) -> Result<Vec<GraphRelation>, GraphError> {
        self.graph.set_relations(self.container, changes, expected)
    }
}
