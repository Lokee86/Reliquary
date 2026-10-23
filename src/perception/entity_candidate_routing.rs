use crate::lexical_contract::lexical_terms;
use crate::{
    Cva, EntityCandidateError, GraphDirection, GraphError, Memory, MemoryEntityMention, MemoryId,
    MemoryTextField, Phylactery,
};
use std::collections::HashSet;

pub(super) trait CandidateOwner {
    fn graph_neighbors_for_candidate(
        &self,
        memory_id: MemoryId,
        direction: GraphDirection,
    ) -> Result<Vec<crate::GraphNeighbor>, GraphError>;
}

impl CandidateOwner for Cva {
    fn graph_neighbors_for_candidate(
        &self,
        memory_id: MemoryId,
        direction: GraphDirection,
    ) -> Result<Vec<crate::GraphNeighbor>, GraphError> {
        self.graph_neighbors(memory_id, direction)
    }
}

impl CandidateOwner for Phylactery {
    fn graph_neighbors_for_candidate(
        &self,
        memory_id: MemoryId,
        direction: GraphDirection,
    ) -> Result<Vec<crate::GraphNeighbor>, GraphError> {
        self.graph_neighbors(memory_id, direction)
    }
}

pub(super) fn graph_neighbor_ids(
    owner: &impl CandidateOwner,
    memory_id: MemoryId,
    limit: usize,
) -> Result<Vec<MemoryId>, EntityCandidateError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for direction in [GraphDirection::Outgoing, GraphDirection::Incoming] {
        match owner.graph_neighbors_for_candidate(memory_id, direction) {
            Ok(neighbors) => ids.extend(neighbors.into_iter().map(|neighbor| neighbor.memory_id)),
            Err(GraphError::MissingNode(id)) if id == memory_id => {}
            Err(error) => return Err(error.into()),
        }
    }
    ids.sort_by_key(|id| id.0);
    ids.dedup();
    ids.truncate(limit);
    Ok(ids)
}

pub(super) fn context_query(memory: &Memory, mention: &MemoryEntityMention) -> String {
    let field = match mention.field {
        MemoryTextField::Title => memory.title.as_str(),
        MemoryTextField::Content => memory.content.as_str(),
    };
    let start = usize::try_from(mention.start_byte).unwrap_or(usize::MAX);
    let end = usize::try_from(mention.end_byte).unwrap_or(usize::MAX);
    let local = local_context(field, start, end);

    let mut terms = Vec::new();
    let mut seen = HashSet::new();
    append_terms(&mention.text, &mut terms, &mut seen);
    if mention.field == MemoryTextField::Content {
        append_terms(&memory.title, &mut terms, &mut seen);
    }
    append_terms(&local, &mut terms, &mut seen);
    terms.truncate(crate::MAX_ENTITY_CANDIDATE_CONTEXT_TERMS);
    terms.join(" ")
}

pub(super) fn memory_contains_surface(memory: &Memory, surface: &str) -> bool {
    let needle = surface.trim().to_lowercase();
    if needle.is_empty() {
        return false;
    }
    memory.title.to_lowercase().contains(&needle) || memory.content.to_lowercase().contains(&needle)
}

fn append_terms(value: &str, terms: &mut Vec<String>, seen: &mut HashSet<String>) {
    for term in lexical_terms(value) {
        if seen.insert(term.clone()) {
            terms.push(term);
        }
    }
}

fn local_context(value: &str, start: usize, end: usize) -> String {
    let (Some(before), Some(after)) = (value.get(..start), value.get(end..)) else {
        return String::new();
    };
    let mut prefix: Vec<_> = before.chars().rev().take(96).collect();
    prefix.reverse();
    let suffix: String = after.chars().take(96).collect();
    format!("{} {}", prefix.into_iter().collect::<String>(), suffix)
}
