//! Pure exact relation-set topology shared by current and historical authority reads.
use crate::{GraphRelation, GraphRelationKind, MemoryId};
use std::collections::{HashSet, VecDeque};
pub(crate) fn duplicate_component(
    source_id: MemoryId,
    relations: &[GraphRelation],
) -> Vec<MemoryId> {
    let mut visited = HashSet::from([source_id]);
    let mut queue = VecDeque::from([source_id]);
    while let Some(current) = queue.pop_front() {
        for relation in relations.iter().filter(|relation| {
            relation.kind == GraphRelationKind::DuplicateOf
                && (relation.source == current || relation.target == current)
        }) {
            let neighbor = if relation.source == current {
                relation.target
            } else {
                relation.source
            };
            if visited.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    visited.into_iter().collect()
}

pub(crate) fn unique_superseder(relations: &[GraphRelation], target: MemoryId) -> Option<MemoryId> {
    let mut sources: Vec<_> = relations
        .iter()
        .filter(|relation| {
            relation.kind == GraphRelationKind::Supersedes && relation.target == target
        })
        .map(|relation| relation.source)
        .collect();
    sources.sort_by_key(|id| id.0);
    sources.dedup();
    match sources.as_slice() {
        [source] => Some(*source),
        _ => None,
    }
}
