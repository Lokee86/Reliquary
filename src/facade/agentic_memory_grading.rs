//! Public-owner composition for the existing agentic retrieval freshness read.
use crate::agentic_memory_retrieval::RankedAgenticMemory;
use crate::{AgenticMemoryError, MemoryRef};
use std::collections::BTreeSet;

pub(crate) fn grade_agentic_memories(
    owner: &crate::Cva,
    ids: &[crate::MemoryId],
    as_of_turn: u64,
) -> Result<Vec<RankedAgenticMemory>, AgenticMemoryError> {
    let owner_id = owner
        .owner_id()
        .ok_or_else(|| AgenticMemoryError::Retrieval("REL lacks durable owner identity".into()))?;
    let grades = owner
        .ego_memory_freshness(ids, as_of_turn)
        .map_err(|error| AgenticMemoryError::Retrieval(error.to_string()))?
        .into_iter()
        .map(|grade| (grade.memory_id, grade.score))
        .collect::<std::collections::BTreeMap<_, _>>();
    Ok(ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|memory_id| RankedAgenticMemory {
            memory: MemoryRef {
                owner_id: owner_id.clone(),
                memory_id,
            },
            freshness: grades.get(&memory_id).copied().unwrap_or(0),
        })
        .collect())
}
