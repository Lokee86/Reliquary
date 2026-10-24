use crate::entity_candidate_model::{
    EntityCandidateConfig, EntityCandidateError, MAX_ENTITY_ADMISSION_SURFACE_MEMORIES,
    MAX_ENTITY_CANDIDATE_GRAPH_NEIGHBORS, MAX_ENTITY_CANDIDATE_LEXICAL_MEMORIES,
    MAX_ENTITY_CANDIDATE_SUPPORT_MEMORIES, MAX_ENTITY_CANDIDATE_SURFACE_MATCHES,
};
use crate::entity_candidate_rank::{
    CandidateEvidence, candidate_order, finalize_candidate, push_unique,
};
use crate::entity_candidate_routing::{context_query, graph_neighbor_ids, memory_contains_surface};
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::lexical_search::MemoryLexicalReadView;
use crate::{
    Entity, EntityCandidateSet, EntityId, Memory, MemoryEntityMention, MemoryEntityMentionKey,
    MemoryId, MemoryRoutingMetadata,
};
use std::collections::HashMap;

pub(crate) struct EntityCandidateContext<'a> {
    pub(crate) memories: MemoryLexicalReadView<'a>,
    pub(crate) entities: &'a EntityStore,
    pub(crate) graph: &'a GraphStore,
}

impl EntityCandidateContext<'_> {
    fn metadata(&self, memory_id: MemoryId) -> Option<&MemoryRoutingMetadata> {
        self.memories.routing_metadata(memory_id)
    }

    pub(crate) fn memory(&mut self, memory_id: MemoryId) -> Result<Memory, crate::MemoryError> {
        self.memories.memory(memory_id)
    }

    fn entities_for_surface(&self, surface: &str, limit: usize) -> Vec<Entity> {
        self.entities.candidates_for_surface(surface, limit)
    }

    fn entities_for_normalized_surface(&self, surface: &str, limit: usize) -> Vec<Entity> {
        self.entities
            .candidates_for_normalized_surface(surface, limit)
    }

    fn entities_for_alias_surface(&self, surface: &str, limit: usize) -> Vec<Entity> {
        self.entities.candidates_for_alias_surface(surface, limit)
    }

    fn associations_for_memory(&self, memory_id: MemoryId) -> Vec<EntityId> {
        self.graph.entity_associations_for_memory(memory_id)
    }

    fn lexical_memories(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::lexical_index::MemoryLexicalHit>, crate::MemoryError> {
        self.memories.candidates(query, limit)
    }

    fn entity(&self, entity_id: EntityId) -> Result<Entity, crate::EntityError> {
        self.entities.entity(entity_id)
    }
}

pub(crate) fn entity_candidates_for_mention(
    context: &mut EntityCandidateContext<'_>,
    key: MemoryEntityMentionKey,
    config: EntityCandidateConfig,
) -> Result<EntityCandidateSet, EntityCandidateError> {
    validate_config(config)?;
    let mention = mention_for_key(context.metadata(key.memory_id), key)?;
    let source = context.memory(key.memory_id)?;
    let context_query = context_query(&source, &mention);

    let mut evidence = HashMap::<EntityId, CandidateEvidence>::new();
    for entity in context.entities_for_surface(&mention.text, MAX_ENTITY_CANDIDATE_SURFACE_MATCHES)
    {
        let value = evidence.entry(entity.id).or_default();
        value.exact_surface = true;
        value.normalized_surface = true;
    }
    for entity in
        context.entities_for_normalized_surface(&mention.text, MAX_ENTITY_CANDIDATE_SURFACE_MATCHES)
    {
        evidence.entry(entity.id).or_default().normalized_surface = true;
    }
    for entity in
        context.entities_for_alias_surface(&mention.text, MAX_ENTITY_CANDIDATE_SURFACE_MATCHES)
    {
        evidence.entry(entity.id).or_default().alias_surface = true;
    }
    for entity_id in context.associations_for_memory(key.memory_id) {
        evidence.entry(entity_id).or_default().source_association = true;
    }

    let lexical_hits = if config.lexical_memory_limit == 0 || context_query.is_empty() {
        Vec::new()
    } else {
        let mut hits = context.lexical_memories(
            &context_query,
            config.lexical_memory_limit.saturating_add(1),
        )?;
        hits.retain(|hit| hit.memory_id != key.memory_id);
        hits.truncate(config.lexical_memory_limit);
        hits
    };
    for hit in &lexical_hits {
        for entity_id in context.associations_for_memory(hit.memory_id) {
            evidence
                .entry(entity_id)
                .or_default()
                .lexical_memories
                .entry(hit.memory_id)
                .and_modify(|score| *score = score.max(hit.score))
                .or_insert(hit.score);
        }
    }

    let graph_neighbors =
        graph_neighbor_ids(context.graph, key.memory_id, config.graph_neighbor_limit)?;
    for memory_id in &graph_neighbors {
        for entity_id in context.associations_for_memory(*memory_id) {
            evidence
                .entry(entity_id)
                .or_default()
                .graph_memories
                .insert(*memory_id);
        }
    }

    let mut candidates = evidence
        .into_iter()
        .map(|(entity_id, evidence)| {
            Ok(finalize_candidate(
                context.entity(entity_id)?,
                evidence,
                config.support_memory_limit,
            ))
        })
        .collect::<Result<Vec<_>, EntityCandidateError>>()?;
    candidates.sort_by(candidate_order);
    candidates.truncate(config.max_candidates);

    let mut admission_hits = context.lexical_memories(
        &mention.text,
        MAX_ENTITY_ADMISSION_SURFACE_MEMORIES.saturating_add(1),
    )?;
    admission_hits.retain(|hit| hit.memory_id != key.memory_id);
    admission_hits.truncate(MAX_ENTITY_ADMISSION_SURFACE_MEMORIES);

    let mut admission_context_memory_ids = Vec::new();
    for hit in &admission_hits {
        let memory = context.memory(hit.memory_id)?;
        if memory_contains_surface(&memory, &mention.text) {
            push_unique(
                &mut admission_context_memory_ids,
                hit.memory_id,
                crate::MAX_ENTITY_ADMISSION_CONTEXT_MEMORIES,
            );
        }
    }
    for memory_id in &graph_neighbors {
        if admission_context_memory_ids.len() >= crate::MAX_ENTITY_ADMISSION_CONTEXT_MEMORIES {
            break;
        }
        let memory = context.memory(*memory_id)?;
        if memory_contains_surface(&memory, &mention.text) {
            push_unique(
                &mut admission_context_memory_ids,
                *memory_id,
                crate::MAX_ENTITY_ADMISSION_CONTEXT_MEMORIES,
            );
        }
    }

    Ok(EntityCandidateSet {
        key,
        mention,
        candidates,
        admission_context_memory_ids,
        lexical_memories_examined: lexical_hits.len(),
        graph_neighbors_examined: graph_neighbors.len(),
    })
}

fn validate_config(config: EntityCandidateConfig) -> Result<(), EntityCandidateError> {
    if !(1..=crate::MAX_ENTITY_RESOLUTION_CANDIDATES).contains(&config.max_candidates)
        || config.lexical_memory_limit > MAX_ENTITY_CANDIDATE_LEXICAL_MEMORIES
        || config.graph_neighbor_limit > MAX_ENTITY_CANDIDATE_GRAPH_NEIGHBORS
        || config.support_memory_limit > MAX_ENTITY_CANDIDATE_SUPPORT_MEMORIES
    {
        return Err(EntityCandidateError::InvalidConfig);
    }
    Ok(())
}

fn mention_for_key(
    metadata: Option<&crate::MemoryRoutingMetadata>,
    key: MemoryEntityMentionKey,
) -> Result<MemoryEntityMention, EntityCandidateError> {
    metadata
        .and_then(|metadata| {
            metadata.entity_mentions.iter().find(|mention| {
                mention.field == key.field
                    && mention.start_byte == key.start_byte
                    && mention.end_byte == key.end_byte
            })
        })
        .cloned()
        .ok_or(EntityCandidateError::MissingMention(key))
}
