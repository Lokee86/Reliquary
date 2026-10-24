//! Native read-only Freshness grading for agentic retrieval.
use crate::agentic_memory_retrieval::RankedAgenticMemory;
use crate::freshness_storage::FreshnessStore;
use crate::{AgenticMemoryError, FreshnessRecord, MemoryRef};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn grade_agentic_memories(
    freshness: &FreshnessStore,
    owner_id: Option<String>,
    activity_head: u64,
    ids: &[crate::MemoryId],
    as_of_turn: u64,
) -> Result<Vec<RankedAgenticMemory>, AgenticMemoryError> {
    let owner_id = owner_id
        .ok_or_else(|| AgenticMemoryError::Retrieval("REL lacks durable owner identity".into()))?;
    if as_of_turn > activity_head {
        return Err(AgenticMemoryError::Retrieval(
            "Freshness error: Ego Freshness cut is beyond the REL activity head".into(),
        ));
    }

    let policy = freshness.policy();
    let mut grades = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for memory_id in ids.iter().copied().filter(|id| seen.insert(*id)) {
        let record = if as_of_turn == activity_head {
            freshness.current_rewrite_record(memory_id, as_of_turn)
        } else {
            freshness
                .record_at(memory_id, as_of_turn)
                .map_err(|error| retrieval_freshness_error(error))?
        };
        let Some(record) = record else {
            continue;
        };
        grades.insert(
            memory_id,
            score_at(record, as_of_turn, &policy)
                .map_err(|error| retrieval_freshness_error(error))?,
        );
    }

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

fn score_at(
    record: FreshnessRecord,
    turn: u64,
    policy: &crate::freshness::FreshnessPolicy,
) -> Result<i16, crate::freshness::FreshnessError> {
    // The former public owner read computed both fields for every enrolled record.
    // Retain the state calculation so both halves of that read preserve failures.
    let score = record.score_at_with_policy(turn, policy)?;
    record.state_at_with_policy(turn, policy)?;
    Ok(score)
}

fn retrieval_freshness_error(error: impl std::fmt::Display) -> AgenticMemoryError {
    AgenticMemoryError::Retrieval(format!("Freshness error: {error}"))
}
