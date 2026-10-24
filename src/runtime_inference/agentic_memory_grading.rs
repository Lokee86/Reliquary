//! Native read-only Freshness grading for agentic retrieval.
use crate::agentic_memory_retrieval::RankedAgenticMemory;
use crate::ego_grade_read::EgoGradeRead;
use crate::{AgenticMemoryError, MemoryRef};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn grade_agentic_memories(
    freshness: &EgoGradeRead<'_>,
    owner_id: Option<String>,
    activity_head: u64,
    ids: &[crate::MemoryId],
    as_of_turn: u64,
) -> Result<Vec<RankedAgenticMemory>, AgenticMemoryError> {
    let owner_id = owner_id
        .ok_or_else(|| AgenticMemoryError::Retrieval("REL lacks durable owner identity".into()))?;
    let grades = freshness
        .grades(ids, as_of_turn, activity_head)
        .map_err(|error| AgenticMemoryError::Retrieval(format!("Freshness error: {error}")))?
        .into_iter()
        .map(|(id, score, _)| (id, score))
        .collect::<BTreeMap<_, _>>();

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
