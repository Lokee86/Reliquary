//! Bounded reads of active duplicate edges at current and replay cuts.
use super::*;

impl GraphStore {
    pub fn duplicate_context(
        &self,
        seeds: &[[u8; 32]],
        limit: usize,
    ) -> Result<(BTreeSet<[u8; 32]>, Vec<Relation>), GraphError> {
        self.duplicate_context_where(seeds, limit, |_| true)
    }

    pub fn duplicate_context_where(
        &self,
        seeds: &[[u8; 32]],
        limit: usize,
        eligible: impl Fn(&Relation) -> bool,
    ) -> Result<(BTreeSet<[u8; 32]>, Vec<Relation>), GraphError> {
        read_context(seeds, limit, |node| {
            self.duplicate_incident
                .get(&node)
                .into_iter()
                .flatten()
                .map(|key| (*key, &self.states[key]))
                .filter(|(_, relation)| eligible(relation))
        })
    }

    pub fn duplicate_history(&self) -> DuplicateHistory<'_> {
        DuplicateHistory {
            graph: self,
            position: 0,
            version: 0,
            incident: BTreeMap::new(),
        }
    }
}

/// A replay-local index of the duplicate edges active at the requested cut.
/// Forward reads visit each publication at most once.
pub struct DuplicateHistory<'a> {
    graph: &'a GraphStore,
    position: usize,
    version: u64,
    incident: BTreeMap<NodeRef, BTreeMap<Key, &'a Relation>>,
}

impl DuplicateHistory<'_> {
    pub fn context_at(
        &mut self,
        version: u64,
        seeds: &[[u8; 32]],
        limit: usize,
    ) -> Result<(BTreeSet<[u8; 32]>, Vec<Relation>), GraphError> {
        if version > self.graph.graph_version {
            return Err(GraphError::CorruptRecord("Graph snapshot ahead of owner"));
        }
        if !seeds.is_empty() {
            self.advance_to(version);
        }
        read_context(seeds, limit, |node| {
            self.incident
                .get(&node)
                .into_iter()
                .flatten()
                .map(|(key, relation)| (*key, *relation))
        })
    }

    fn advance_to(&mut self, version: u64) {
        if version < self.version {
            self.position = 0;
            self.incident.clear();
        }
        while let Some(relation) = self.graph.history.get(self.position) {
            if relation.graph_version > version {
                break;
            }
            self.position += 1;
            if relation.kind != RelationKind::DuplicateOf {
                continue;
            }
            let key = Key {
                source: relation.source,
                target: relation.target,
                kind: relation.kind,
            };
            for node in [relation.source, relation.target] {
                if relation.active {
                    self.incident.entry(node).or_default().insert(key, relation);
                } else if let Some(edges) = self.incident.get_mut(&node) {
                    edges.remove(&key);
                    if edges.is_empty() {
                        self.incident.remove(&node);
                    }
                }
            }
        }
        self.version = version;
    }
}

fn read_context<'a, I>(
    seeds: &[[u8; 32]],
    limit: usize,
    incident: impl Fn(NodeRef) -> I,
) -> Result<(BTreeSet<[u8; 32]>, Vec<Relation>), GraphError>
where
    I: Iterator<Item = (Key, &'a Relation)>,
{
    if seeds.len() > 17 || limit > 128 || limit < seeds.len() {
        return Err(GraphError::InvalidClosure("duplicate context limit"));
    }
    let mut ids: BTreeSet<_> = seeds.iter().copied().collect();
    let mut queue: std::collections::VecDeque<_> = ids.iter().copied().collect();
    let mut edges = BTreeMap::new();
    while let Some(id) = queue.pop_front() {
        for (key, relation) in incident(NodeRef::Memory(id)).take(257) {
            edges.insert(key, *relation);
            if edges.len() > 256 {
                return Err(GraphError::InvalidClosure("duplicate edge limit"));
            }
            for node in [relation.source, relation.target] {
                if let NodeRef::Memory(other) = node {
                    if ids.insert(other) {
                        if ids.len() > limit {
                            return Err(GraphError::InvalidClosure("duplicate component limit"));
                        }
                        queue.push_back(other);
                    }
                }
            }
        }
    }
    Ok((ids, edges.into_values().collect()))
}
