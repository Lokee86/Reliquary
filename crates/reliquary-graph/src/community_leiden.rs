use crate::CommunityError;
use crate::graph_store::GraphStore;
use std::collections::BTreeSet;

pub(crate) fn structural_edges(
    graph: &GraphStore,
) -> Result<BTreeSet<(usize, usize)>, CommunityError> {
    let mut edges = BTreeSet::new();
    for relation in graph.active_relations() {
        let source = graph.memory_projection_node_id(relation.source)?.0 as usize;
        let target = graph.memory_projection_node_id(relation.target)?.0 as usize;
        edges.insert(if source < target {
            (source, target)
        } else {
            (target, source)
        });
    }
    Ok(edges)
}
