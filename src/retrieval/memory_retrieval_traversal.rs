use crate::{CommunityId, GraphRelation, MemoryId};
use std::collections::{HashMap, HashSet};

pub(crate) fn traverse(
    relations: &[GraphRelation],
    memberships: &HashMap<MemoryId, CommunityId>,
    searchable: &HashSet<MemoryId>,
    seeds: &[MemoryId],
    budget: usize,
    max_depth: usize,
    community_within_depth: bool,
) -> Vec<MemoryId> {
    let lower_relations: Vec<reliquary_graph::GraphRelation> =
        relations.iter().copied().map(Into::into).collect();
    let lower_memberships = memberships
        .iter()
        .map(|(memory_id, community_id)| (memory_id.0, *community_id))
        .collect();
    let lower_searchable = searchable.iter().map(|memory_id| memory_id.0).collect();
    let lower_seeds: Vec<_> = seeds.iter().map(|memory_id| memory_id.0).collect();

    reliquary_retrieval::graph_traversal::traverse(
        &lower_relations,
        &lower_memberships,
        &lower_searchable,
        &lower_seeds,
        budget,
        max_depth,
        community_within_depth,
    )
    .into_iter()
    .map(MemoryId)
    .collect()
}
