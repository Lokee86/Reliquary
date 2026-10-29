use reliquary_graph::{GraphRelation, GraphRelationChange, GraphRelationKind, MemoryId};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

/// Source-time and identity order used to build the single canonical duplicate chain.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DuplicateTemporalKey {
    pub timestamp_ns: i64,
    pub memory_id: MemoryId,
}

impl DuplicateTemporalKey {
    pub fn new(timestamp_ns: i64, memory_id: MemoryId) -> Self {
        Self {
            timestamp_ns,
            memory_id,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct DuplicateComponent {
    ordered: BTreeMap<DuplicateTemporalKey, MemoryId>,
}

/// Incremental planner over the owner's immutable active-relation snapshot.
/// It proposes Graph edits only; the owner remains responsible for admission and writes.
#[derive(Clone, Debug, Default)]
pub struct DuplicateIndex {
    memory_graph_version: Option<u64>,
    components: Vec<Option<DuplicateComponent>>,
    member_component: HashMap<MemoryId, usize>,
}

impl DuplicateIndex {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn memory_graph_version(&self) -> Option<u64> {
        self.memory_graph_version
    }

    pub fn set_memory_graph_version(&mut self, version: u64) {
        self.memory_graph_version = Some(version);
    }

    pub fn rebuild(
        memory_graph_version: u64,
        relations: &[GraphRelation],
        keys: &HashMap<MemoryId, DuplicateTemporalKey>,
    ) -> Self {
        let adjacency = duplicate_adjacency(relations, true);
        let mut index = Self {
            memory_graph_version: Some(memory_graph_version),
            ..Self::default()
        };
        let mut visited = HashSet::new();
        for start in adjacency.keys().copied() {
            if !visited.insert(start) {
                continue;
            }
            let component_index = index.components.len();
            let mut component = DuplicateComponent::default();
            let mut stack = vec![start];
            while let Some(id) = stack.pop() {
                if let Some(key) = keys.get(&id).copied() {
                    component.ordered.insert(key, id);
                    index.member_component.insert(id, component_index);
                }
                if let Some(neighbors) = adjacency.get(&id) {
                    for neighbor in neighbors.iter().copied() {
                        if visited.insert(neighbor) {
                            stack.push(neighbor);
                        }
                    }
                }
            }
            index.components.push(Some(component));
        }
        index
    }

    pub fn plan_union(
        &mut self,
        a: MemoryId,
        a_key: DuplicateTemporalKey,
        b: MemoryId,
        b_key: DuplicateTemporalKey,
        relations: &[GraphRelation],
    ) -> Vec<GraphRelationChange> {
        let a_component = self.ensure_member(a, a_key);
        let b_component = self.ensure_member(b, b_key);
        let component = if a_component == b_component {
            a_component
        } else {
            self.merge_components(a_component, b_component)
        };
        self.reconcile_component(component, relations)
    }

    fn ensure_member(&mut self, id: MemoryId, key: DuplicateTemporalKey) -> usize {
        if let Some(component) = self.member_component.get(&id).copied() {
            return component;
        }
        let component = self.components.len();
        let mut value = DuplicateComponent::default();
        value.ordered.insert(key, id);
        self.components.push(Some(value));
        self.member_component.insert(id, component);
        component
    }

    fn merge_components(&mut self, left: usize, right: usize) -> usize {
        let (keep, remove) = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        let removed = self.components[remove]
            .take()
            .expect("duplicate component exists");
        let kept = self.components[keep]
            .as_mut()
            .expect("duplicate component exists");
        for (key, id) in removed.ordered {
            kept.ordered.insert(key, id);
            self.member_component.insert(id, keep);
        }
        keep
    }

    fn reconcile_component(
        &self,
        component: usize,
        relations: &[GraphRelation],
    ) -> Vec<GraphRelationChange> {
        let ordered = &self.components[component]
            .as_ref()
            .expect("duplicate component exists")
            .ordered;
        let members: HashSet<_> = ordered.values().copied().collect();
        let desired: HashSet<_> = ordered
            .values()
            .copied()
            .collect::<Vec<_>>()
            .windows(2)
            .map(|pair| (pair[1], pair[0]))
            .collect();
        let existing: HashSet<_> = relations
            .iter()
            .filter(|r| {
                r.active
                    && r.kind == GraphRelationKind::DuplicateOf
                    && members.contains(&r.source)
                    && members.contains(&r.target)
            })
            .map(|r| (r.source, r.target))
            .collect();
        let mut changes = Vec::new();
        for &(source, target) in existing.difference(&desired) {
            changes.push(change(source, target, false));
        }
        for &(source, target) in desired.difference(&existing) {
            changes.push(change(source, target, true));
        }
        changes.sort_by_key(|c| (c.source, c.target, c.active));
        changes
    }
}

fn change(source: MemoryId, target: MemoryId, active: bool) -> GraphRelationChange {
    GraphRelationChange {
        source,
        target,
        kind: GraphRelationKind::DuplicateOf,
        active,
    }
}

/// Returns a deterministic duplicate component from a complete relation snapshot.
pub fn duplicate_component(source: MemoryId, relations: &[GraphRelation]) -> Vec<MemoryId> {
    let adjacency = duplicate_adjacency(relations, false);
    let mut visited = HashSet::from([source]);
    let mut queue = VecDeque::from([source]);
    while let Some(current) = queue.pop_front() {
        if let Some(neighbors) = adjacency.get(&current) {
            for neighbor in neighbors.iter().copied() {
                if visited.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
    }
    let mut ids: Vec<_> = visited.into_iter().collect();
    ids.sort();
    ids
}

fn duplicate_adjacency(
    relations: &[GraphRelation],
    active_only: bool,
) -> HashMap<MemoryId, Vec<MemoryId>> {
    let mut adjacency: HashMap<MemoryId, Vec<MemoryId>> = HashMap::new();
    for relation in relations
        .iter()
        .filter(|r| r.kind == GraphRelationKind::DuplicateOf && (!active_only || r.active))
    {
        adjacency
            .entry(relation.source)
            .or_default()
            .push(relation.target);
        adjacency
            .entry(relation.target)
            .or_default()
            .push(relation.source);
    }
    adjacency
}

/// Pure representative ordering shared by current and historical policy paths.
pub fn representative_order_key(
    lifecycle_state: &str,
    source_timestamp_ns: Option<i64>,
    memory_id: MemoryId,
) -> (u8, i64, MemoryId) {
    let state_rank = match lifecycle_state {
        "canonical" => 0,
        "knowledge" => 1,
        "extracted" => 2,
        _ => 3,
    };
    (
        state_rank,
        source_timestamp_ns.unwrap_or(i64::MAX),
        memory_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relation(source: MemoryId, target: MemoryId) -> GraphRelation {
        GraphRelation {
            source,
            target,
            kind: GraphRelationKind::DuplicateOf,
            active: true,
            origin: reliquary_graph::GraphRelationOrigin::Dream,
            global_version: 1,
            graph_version: 1,
        }
    }

    #[test]
    fn duplicate_component_uses_supplied_transitive_edges_and_is_ordered() {
        let relations = [relation([2; 32], [1; 32]), relation([3; 32], [2; 32])];
        assert_eq!(
            duplicate_component([3; 32], &relations),
            vec![[1; 32], [2; 32], [3; 32]]
        );
    }

    #[test]
    fn representative_order_prefers_state_then_source_time_then_id() {
        assert!(
            representative_order_key("canonical", Some(20), [3; 32])
                < representative_order_key("knowledge", Some(1), [1; 32])
        );
        assert!(
            representative_order_key("knowledge", Some(1), [1; 32])
                < representative_order_key("knowledge", Some(1), [2; 32])
        );
        assert!(
            representative_order_key("other", None, [1; 32])
                > representative_order_key("extracted", Some(i64::MAX), [1; 32])
        );
    }
}
