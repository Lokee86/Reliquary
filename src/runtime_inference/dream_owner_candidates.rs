use crate::chronos::temporal_matches;
use crate::dream_candidate_ranking::{
    ScoredCandidate, lexical_score, rank_lanes, select_candidates,
};
use crate::dream_owner_vectors::load_memory_vectors;
use crate::dream_pair_history::DreamPairStore;
use crate::dream_temporal::analyze_memory_temporal;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::memory_vector_store::MemoryVectorStore;
use crate::packed_vector_store::PackedVectorStore;
use crate::{
    CompatibilityProfileId, Container, DreamCandidateConfig, DreamCandidateError,
    DreamCandidateSet, DreamMemoryContext, GraphRelation, MAX_DREAM_CANDIDATE_LIMIT, Memory,
    MemoryBodyId, MemoryId,
};

pub(crate) fn dream_memory_context<R>(
    container: &mut Container,
    memories: &MemoryStore,
    graph: &GraphStore,
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
    graph: &GraphStore,
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
    validate_config(config)?;
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
            .map(|candidate| cosine(source_vector, candidate))
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
    rank_lanes(&source_context, &mut scored, config);
    Ok(DreamCandidateSet {
        source: source_context,
        candidates: select_candidates(scored, config),
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

fn validate_config(config: DreamCandidateConfig) -> Result<(), DreamCandidateError> {
    if config.limit == 0
        || config.limit > MAX_DREAM_CANDIDATE_LIMIT
        || config.semantic_limit > MAX_DREAM_CANDIDATE_LIMIT
        || config.lexical_limit > MAX_DREAM_CANDIDATE_LIMIT
        || config.temporal_limit > MAX_DREAM_CANDIDATE_LIMIT
        || config.prior_semantic_quota > config.limit
    {
        return Err(DreamCandidateError::InvalidConfig);
    }
    Ok(())
}

fn cosine(left: &[f32], right: &[f32]) -> Result<f64, DreamCandidateError> {
    if left.len() != right.len() || left.is_empty() {
        return Err(DreamCandidateError::CorruptVector("vector dimensions"));
    }
    let mut dot = 0.0_f64;
    let mut left_norm = 0.0_f64;
    let mut right_norm = 0.0_f64;
    for (left, right) in left.iter().zip(right) {
        dot += *left as f64 * *right as f64;
        left_norm += (*left as f64).powi(2);
        right_norm += (*right as f64).powi(2);
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        return Err(DreamCandidateError::CorruptVector("zero vector"));
    }
    Ok(dot / (left_norm.sqrt() * right_norm.sqrt()))
}
