use crate::{GraphRelation, GraphRelationChange, MemoryId};
use reliquary_dream::DuplicateIndex as ServiceIndex;
pub(crate) use reliquary_dream::DuplicateTemporalKey;
use std::collections::HashMap;

/// Root adapter for the pure Dream duplicate planner. Owner authorization and Graph writes stay here.
#[derive(Clone, Debug, Default)]
pub(crate) struct DuplicateIndex(ServiceIndex);

impl DuplicateIndex {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn memory_graph_version(&self) -> Option<u64> {
        self.0.memory_graph_version()
    }

    pub(crate) fn set_memory_graph_version(&mut self, version: u64) {
        self.0.set_memory_graph_version(version);
    }

    pub(crate) fn rebuild(
        version: u64,
        relations: &[GraphRelation],
        keys: &HashMap<MemoryId, DuplicateTemporalKey>,
    ) -> Self {
        let relations = relations
            .iter()
            .copied()
            .map(service_relation)
            .collect::<Vec<_>>();
        let keys = keys.iter().map(|(id, key)| (id.0, *key)).collect();
        Self(ServiceIndex::rebuild(version, &relations, &keys))
    }

    pub(crate) fn plan_union(
        &mut self,
        a: MemoryId,
        a_key: DuplicateTemporalKey,
        b: MemoryId,
        b_key: DuplicateTemporalKey,
        relations: &[GraphRelation],
    ) -> Vec<GraphRelationChange> {
        let relations = relations
            .iter()
            .copied()
            .map(service_relation)
            .collect::<Vec<_>>();
        self.0
            .plan_union(a.0, a_key, b.0, b_key, &relations)
            .into_iter()
            .map(|change| GraphRelationChange {
                source: MemoryId(change.source),
                target: MemoryId(change.target),
                kind: change.kind,
                active: change.active,
            })
            .collect()
    }
}

fn service_relation(relation: GraphRelation) -> reliquary_graph::GraphRelation {
    reliquary_graph::GraphRelation {
        source: relation.source.0,
        target: relation.target.0,
        kind: relation.kind,
        active: relation.active,
        origin: relation.origin,
        global_version: relation.global_version,
        graph_version: relation.graph_version,
    }
}
