use crate::chronos::temporal_matches;
use crate::dream_candidate_ranking::{
    ScoredCandidate, lexical_score, rank_lanes, select_candidates,
};
use crate::dream_pair_history::DreamPairStore;
use crate::dream_temporal_policy::analyze_memory_temporal;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryReadView;
use crate::memory_vector_reader::MemoryVectorReader;
use crate::{
    CompatibilityProfileId, DreamCandidateConfig, DreamCandidateError, DreamCandidateSet,
    DreamMemoryContext, GraphRelation, MAX_DREAM_CANDIDATE_LIMIT, Memory, MemoryBodyId, MemoryId,
};
use std::collections::HashMap;

pub(crate) struct DreamMemoryContextWork {
    pub(crate) memory: Memory,
    pub(crate) body_id: MemoryBodyId,
    relations: Vec<GraphRelation>,
}

pub(crate) fn prepare_dream_memory_context(
    memories: &MemoryReadView<'_>,
    graph: &GraphStore,
    memory_id: MemoryId,
) -> Result<DreamMemoryContextWork, DreamCandidateError> {
    let memory = memories.memory(memory_id)?;
    let body_id = memories.current_body_id(memory_id)?;
    let relations = graph
        .active_relations()
        .into_iter()
        .filter(|relation| relation.source == memory_id || relation.target == memory_id)
        .collect();
    Ok(DreamMemoryContextWork {
        memory,
        body_id,
        relations,
    })
}

pub(crate) fn finish_dream_memory_context(
    work: DreamMemoryContextWork,
    source_timestamp_ns: Option<i64>,
) -> DreamMemoryContext {
    build_context(
        work.memory,
        work.body_id,
        &work.relations,
        source_timestamp_ns,
    )
}

pub(crate) struct DreamCandidateWork {
    source: DreamMemoryContextWork,
    candidates: Vec<DreamCandidateWorkItem>,
    config: DreamCandidateConfig,
}

struct DreamCandidateWorkItem {
    context: DreamMemoryContextWork,
    semantic_score: Option<f64>,
    lexical_score: f64,
}

impl DreamCandidateWork {
    pub(crate) fn source_time_memories(&self) -> impl Iterator<Item = &Memory> {
        std::iter::once(&self.source.memory).chain(
            self.candidates
                .iter()
                .map(|candidate| &candidate.context.memory),
        )
    }
}

pub(crate) fn prepare_dream_candidates(
    memories: &MemoryReadView<'_>,
    graph: &GraphStore,
    dream_pairs: &DreamPairStore,
    memory_vectors: &MemoryVectorReader<'_>,
    compatibility_profile_id: CompatibilityProfileId,
    source_id: MemoryId,
    config: DreamCandidateConfig,
) -> Result<DreamCandidateWork, DreamCandidateError> {
    validate_config(config)?;
    let source = memories.memory(source_id)?;
    let source_body = memories.current_body_id(source_id)?;
    let vectors = memory_vectors.load(memories, compatibility_profile_id)?;
    let source_vector = vectors
        .get(&source_body)
        .ok_or(DreamCandidateError::MissingSourceVector)?;
    let relations = graph.active_relations();
    let source_context = DreamMemoryContextWork {
        relations: relations
            .iter()
            .copied()
            .filter(|relation| relation.source == source_id || relation.target == source_id)
            .collect(),
        memory: source,
        body_id: source_body,
    };

    let mut candidates = Vec::new();
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
        let memory = memories.memory(id)?;
        if memory.archived {
            continue;
        }
        let body_id = memories.current_body_id(id)?;
        let semantic_score = vectors
            .get(&body_id)
            .map(|candidate| cosine(source_vector, candidate))
            .transpose()?;
        let lexical_score = lexical_score(&source_context.memory, &memory);
        let context = DreamMemoryContextWork {
            relations: relations
                .iter()
                .copied()
                .filter(|relation| relation.source == id || relation.target == id)
                .collect(),
            memory,
            body_id,
        };
        candidates.push(DreamCandidateWorkItem {
            context,
            semantic_score,
            lexical_score,
        });
    }

    Ok(DreamCandidateWork {
        source: source_context,
        candidates,
        config,
    })
}

pub(crate) fn finish_dream_candidates(
    work: DreamCandidateWork,
    source_times: &HashMap<MemoryId, Option<i64>>,
) -> DreamCandidateSet {
    let source_id = work.source.memory.id;
    let source_context = build_context(
        work.source.memory,
        work.source.body_id,
        &work.source.relations,
        source_times.get(&source_id).copied().flatten(),
    );
    let mut scored = work
        .candidates
        .into_iter()
        .map(|candidate| {
            let candidate_id = candidate.context.memory.id;
            let context = build_context(
                candidate.context.memory,
                candidate.context.body_id,
                &candidate.context.relations,
                source_times.get(&candidate_id).copied().flatten(),
            );
            let (temporal_matches, temporal_score) =
                temporal_matches(&source_context.temporal, &context.temporal);
            ScoredCandidate {
                context,
                semantic_score: candidate.semantic_score,
                lexical_score: candidate.lexical_score,
                semantic_rank: None,
                prior_rank: None,
                lexical_rank: None,
                temporal_score,
                temporal_rank: None,
                temporal_matches,
                fused_score: 0.0,
            }
        })
        .collect::<Vec<_>>();
    rank_lanes(&source_context, &mut scored, work.config);
    DreamCandidateSet {
        source: source_context,
        candidates: select_candidates(scored, work.config),
    }
}

fn build_context(
    memory: Memory,
    body_id: MemoryBodyId,
    relations: &[GraphRelation],
    source_timestamp_ns: Option<i64>,
) -> DreamMemoryContext {
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
