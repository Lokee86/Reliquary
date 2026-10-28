use crate::EntityCandidate;
use reliquary_memory_core::MemoryId;
use reliquary_perception_store::Entity;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct CandidateEvidence {
    pub exact_surface: bool,
    pub normalized_surface: bool,
    pub alias_surface: bool,
    pub source_association: bool,
    pub lexical_memories: HashMap<MemoryId, f64>,
    pub graph_memories: HashSet<MemoryId>,
}

pub fn finalize_candidate(
    entity: Entity,
    evidence: CandidateEvidence,
    support_limit: usize,
) -> EntityCandidate {
    let mut lexical: Vec<_> = evidence.lexical_memories.into_iter().collect();
    lexical.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.0.cmp(&right.0.0))
    });
    let best_lexical_score = lexical.first().map(|(_, score)| *score).unwrap_or(0.0);
    let lexical_memory_hits = lexical.len();
    let mut graph: Vec<_> = evidence.graph_memories.into_iter().collect();
    graph.sort_by_key(|id| id.0);
    let graph_neighbor_hits = graph.len();

    let mut supporting_memory_ids = Vec::new();
    for (memory_id, _) in &lexical {
        push_unique(&mut supporting_memory_ids, *memory_id, support_limit);
    }
    for memory_id in graph {
        push_unique(&mut supporting_memory_ids, memory_id, support_limit);
    }

    EntityCandidate {
        entity,
        exact_surface: evidence.exact_surface,
        normalized_surface: evidence.normalized_surface,
        alias_surface: evidence.alias_surface,
        source_association: evidence.source_association,
        lexical_memory_hits,
        graph_neighbor_hits,
        best_lexical_score,
        supporting_memory_ids,
    }
}

pub fn push_unique(values: &mut Vec<MemoryId>, value: MemoryId, limit: usize) {
    if values.len() < limit && !values.contains(&value) {
        values.push(value);
    }
}

pub fn candidate_order(left: &EntityCandidate, right: &EntityCandidate) -> std::cmp::Ordering {
    let left_lanes = usize::from(left.source_association)
        + usize::from(left.lexical_memory_hits > 0)
        + usize::from(left.graph_neighbor_hits > 0);
    let right_lanes = usize::from(right.source_association)
        + usize::from(right.lexical_memory_hits > 0)
        + usize::from(right.graph_neighbor_hits > 0);
    right
        .exact_surface
        .cmp(&left.exact_surface)
        .then_with(|| right.normalized_surface.cmp(&left.normalized_surface))
        .then_with(|| right.alias_surface.cmp(&left.alias_surface))
        .then_with(|| right_lanes.cmp(&left_lanes))
        .then_with(|| right.best_lexical_score.total_cmp(&left.best_lexical_score))
        .then_with(|| right.lexical_memory_hits.cmp(&left.lexical_memory_hits))
        .then_with(|| right.graph_neighbor_hits.cmp(&left.graph_neighbor_hits))
        .then_with(|| left.entity.id.cmp(&right.entity.id))
}
