//! Product replay admission after Memory and Entity projections finish.
use reliquary_perception_store::EntityStore;
use crate::graph_owner::GraphOwner;
use crate::memory_store::MemoryStore;
use crate::{GraphError, SemanticNodeKind};
use reliquary_container::{ObjectRef, publication_envelope::PublicationEnvelope};
pub(crate) struct GraphReplay(reliquary_graph::GraphOpenState);
impl GraphReplay {
    pub(crate) fn new() -> Self {
        Self(reliquary_graph::GraphOpenState::new())
    }
    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        latest_global: u64,
    ) -> Result<(), GraphError> {
        self.0
            .ingest(chunk, payload, latest_global)
            .map_err(Into::into)
    }
    pub(crate) fn ingest_publication_envelope(
        &mut self,
        envelope: PublicationEnvelope<'_>,
        latest_global: u64,
    ) -> Result<bool, GraphError> {
        self.0
            .ingest_publication_envelope(envelope, latest_global)
            .map_err(Into::into)
    }
    pub(crate) fn finish(self, m: &MemoryStore, e: &EntityStore) -> Result<GraphOwner, GraphError> {
        let mut current_memories = Vec::new();
        let mut historical_memories = Vec::new();
        let mut current_entities = Vec::new();
        let mut historical_entities = Vec::new();
        let mut historical_principals = Vec::new();
        let nodes = self.0.semantic_nodes();
        let referenced_entities: std::collections::HashSet<_> = nodes
            .iter()
            .filter(|n| n.kind == SemanticNodeKind::Entity)
            .map(|n| crate::EntityId(n.id))
            .collect();
        let mut latest_kinds = std::collections::HashMap::new();
        if !referenced_entities.is_empty() {
            for record in e.records() {
                if referenced_entities.contains(&record.id) {
                    latest_kinds.insert(
                        record.id,
                        record.kind == crate::entity_principal::PRINCIPAL_ENTITY_KIND,
                    );
                }
            }
        }
        for node in nodes {
            match node.kind {
                SemanticNodeKind::Memory => {
                    let id = crate::MemoryId(node.id);
                    if m.contains_memory(id) {
                        current_memories.push(node.id);
                    }
                    if m.contains_historical_memory(id) {
                        historical_memories.push(node.id);
                    }
                }
                SemanticNodeKind::Entity => {
                    let id = crate::EntityId(node.id);
                    if let Ok(entity) = e.entity(id) {
                        current_entities.push((
                            node.id,
                            entity.kind == crate::entity_principal::PRINCIPAL_ENTITY_KIND,
                        ));
                    }
                    if e.contains_historical(id) {
                        historical_entities.push(node.id);
                    }
                    if latest_kinds.get(&id).copied().unwrap_or(false) {
                        historical_principals.push(node.id);
                    }
                }
                SemanticNodeKind::Observation => {}
            }
        }
        let facts = reliquary_graph::GraphAdmission::for_replay(
            current_memories,
            historical_memories,
            current_entities,
            historical_entities,
            historical_principals,
        );
        self.0
            .finish(&facts)
            .map(|inner| GraphOwner { inner })
            .map_err(Into::into)
    }
}
