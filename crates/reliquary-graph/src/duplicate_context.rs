//! Bounded reads from the active duplicate edge index.
use super::*;
impl GraphStore {
    pub fn duplicate_context(
        &self,
        seeds: &[[u8; 32]],
        limit: usize,
    ) -> Result<(BTreeSet<[u8; 32]>, Vec<Relation>), GraphError> {
        if seeds.len() > 17 || limit > 128 || limit < seeds.len() {
            return Err(GraphError::InvalidClosure("duplicate context limit"));
        }
        let mut ids: BTreeSet<_> = seeds.iter().copied().collect();
        let mut queue: std::collections::VecDeque<_> = ids.iter().copied().collect();
        let mut edges = BTreeMap::new();
        while let Some(id) = queue.pop_front() {
            if let Some(keys) = self.duplicate_incident.get(&NodeRef::Memory(id)) {
                for key in keys.iter().take(257) {
                    let relation = self.states[key];
                    edges.insert(*key, relation);
                    if edges.len() > 256 {
                        return Err(GraphError::InvalidClosure("duplicate edge limit"));
                    }
                    for node in [relation.source, relation.target] {
                        if let NodeRef::Memory(other) = node {
                            if ids.insert(other) {
                                if ids.len() > limit {
                                    return Err(GraphError::InvalidClosure(
                                        "duplicate component limit",
                                    ));
                                }
                                queue.push_back(other);
                            }
                        }
                    }
                }
            }
        }
        Ok((ids, edges.into_values().collect()))
    }
}
