//! Graph owner reconstruction against already accepted Memory and Entity state.
use super::OwnerError;
use crate::entity_store::EntityStore;
use crate::graph_rebuild::GraphOpenState;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;

/// Finish the concrete Graph projection and validate its accepted references.
/// Historical History proof and rewrite-baseline validation remain at their
/// surrounding checkpoints in the REL open sequence.
pub(crate) fn finish_reliquary_graph(
    state: GraphOpenState,
    memories: &MemoryStore,
    entities: &EntityStore,
) -> Result<GraphStore, OwnerError> {
    let mut graph = state.finish()?;
    super::validate_rebuilt_graph(&mut graph, memories, entities)?;
    Ok(graph)
}
