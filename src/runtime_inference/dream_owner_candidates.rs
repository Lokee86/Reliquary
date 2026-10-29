use crate::chronos::temporal_matches;
use crate::dream_candidate_ranking::{ScoredCandidate, rank_and_select};
use crate::dream_owner_vectors::load_memory_vectors;
use crate::dream_pair_history::DreamPairStore;
use crate::dream_temporal::analyze_memory_temporal;
use crate::graph_owner::GraphOwner;
use crate::memory_store::MemoryStore;
use crate::memory_vector_store::MemoryVectorStore;
use crate::{
    CompatibilityProfileId, DreamCandidateConfig, DreamCandidateError, DreamCandidateSet,
    DreamMemoryContext, GraphRelation, Memory, MemoryBodyId, MemoryId,
};
use reliquary_container::Container;
use reliquary_dream::lexical_score;
use reliquary_vector_store::PackedVectorStore;

pub(crate) fn dream_memory_context<R>(
    container: &mut Container,
    memories: &MemoryStore,
    graph: &GraphOwner,
    memory_id: MemoryId,
    source_time: &R,
) -> Result<DreamMemoryContext, DreamCandidateError>
where
    R: Fn(&Memory) -> Option<i64>,
{
    let memory = memories.memory(container, memory_id)?;
    let body_id = memories.current_body_id(memory_id)?;
    let relations = graph.active_relations();
    Ok(build_context(memory, body_id, &relations, source_time))
}

pub(crate) fn dream_candidates<R>(
    container: &mut Container,
    memories: &MemoryStore,
    graph: &GraphOwner,
    dream_pairs: &DreamPairStore,
    memory_vectors: &MemoryVectorStore,
    packed_vectors: &PackedVectorStore,
    compatibility_profile_id: CompatibilityProfileId,
    source_id: MemoryId,
    config: DreamCandidateConfig,
    source_time: &R,
) -> Result<DreamCandidateSet, DreamCandidateError>
where
    R: Fn(&Memory) -> Option<i64>,
{
    reliquary_dream::validate_candidate_config(config)
        .map_err(|_| DreamCandidateError::InvalidConfig)?;
    let source = memories.memory(container, source_id)?;
    let source_body = memories.current_body_id(source_id)?;
    let vectors = load_memory_vectors(
        container,
        memories,
        memory_vectors,
        packed_vectors,
        compatibility_profile_id,
    )?;
    let source_vector = vectors
        .get(&source_body)
        .ok_or(DreamCandidateError::MissingSourceVector)?;
    let relations = graph.active_relations();
    let source_context = build_context(source, source_body, &relations, source_time);

    let mut scored = Vec::new();
    for id in memories.current_ids() {
        if id == source_id
            || dream_pairs.contains(source_id, id)
            || relations.iter().any(|relation| {
                (relation.source == source_id && relation.target == id)
                    || (relation.source == id && relation.target == source_id)
            })
        {
            continue;
        }
        let memory = memories.memory(container, id)?;
        if memory.archived {
            continue;
        }
        let body_id = memories.current_body_id(id)?;
        let semantic_score = vectors
            .get(&body_id)
            .map(|candidate| {
                reliquary_dream::candidate_cosine(source_vector, candidate)
                    .map_err(DreamCandidateError::CorruptVector)
            })
            .transpose()?;
        let lexical_score = lexical_score(&source_context.memory, &memory);
        let context = build_context(memory, body_id, &relations, source_time);
        let (temporal_matches, temporal_score) =
            temporal_matches(&source_context.temporal, &context.temporal);
        scored.push(ScoredCandidate {
            context,
            semantic_score,
            lexical_score,
            semantic_rank: None,
            prior_rank: None,
            lexical_rank: None,
            temporal_score,
            temporal_rank: None,
            temporal_matches,
            fused_score: 0.0,
        });
    }
    Ok(DreamCandidateSet {
        candidates: rank_and_select(&source_context, scored, config),
        source: source_context,
    })
}

fn build_context<R>(
    memory: Memory,
    body_id: MemoryBodyId,
    relations: &[GraphRelation],
    source_time: &R,
) -> DreamMemoryContext
where
    R: Fn(&Memory) -> Option<i64>,
{
    let source_timestamp_ns = source_time(&memory);
    let graph_relations = relations
        .iter()
        .copied()
        .filter(|relation| relation.source == memory.id || relation.target == memory.id)
        .collect();
    let temporal = analyze_memory_temporal(&memory, body_id, source_timestamp_ns);
    DreamMemoryContext {
        memory,
        body_id,
        source_timestamp_ns,
        graph_relations,
        temporal,
    }
}
