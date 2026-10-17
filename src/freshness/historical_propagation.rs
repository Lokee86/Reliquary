//! Import-only propagation over a pinned, explicitly authorized owner graph.
//! Temporal and self/cohort exclusions affect credit, never bridge expansion.
use super::historical_planner::MemoryAvailability;
use super::propagation::{join_event_workers, planned_event_workers, propagate_frontier};
use super::semantic_policy::{SEMANTIC_BONUS_CAP, SEMANTIC_CANDIDATE_LIMIT};
use super::{
    CommunityMembership, FreshnessPolicy, MemoryAdjacency, PropagationError, PropagationOutcome,
};
use crate::MemoryId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoricalPropagationKind {
    Semantic,
    FirstCycleDream,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalPropagationEvent {
    pub kind: HistoricalPropagationKind,
    pub position: u64,
    /// One strongest bonus per root for the entire originating source event.
    pub roots: BTreeMap<MemoryId, i16>,
    pub credit_excluded: BTreeSet<MemoryId>,
}

impl HistoricalPropagationEvent {
    /// Preserve the entire Step 16 source turn as one propagation event.
    pub fn from_semantic(
        event: &super::historical_semantic::HistoricalSemanticEvent,
    ) -> Result<Self, HistoricalPropagationError> {
        let policy = super::semantic_policy::SemanticPolicy::default();
        if policy
            .digest()
            .map_err(|_| HistoricalPropagationError::InvalidEvent)?
            != event.semantic_policy_digest
        {
            return Err(HistoricalPropagationError::InvalidEvent);
        }
        let mut roots = BTreeMap::<MemoryId, i16>::new();
        for effect in &event.effects {
            if !(1..=SEMANTIC_BONUS_CAP).contains(&effect.bonus) {
                return Err(HistoricalPropagationError::InvalidEvent);
            }
            roots
                .entry(effect.memory_id)
                .and_modify(|bonus| *bonus = (*bonus).max(effect.bonus))
                .or_insert(effect.bonus);
        }
        if roots.len() > SEMANTIC_CANDIDATE_LIMIT || event.source_position == 0 {
            return Err(HistoricalPropagationError::InvalidEvent);
        }
        Ok(Self {
            kind: HistoricalPropagationKind::Semantic,
            position: event.source_position,
            roots,
            credit_excluded: event.credit_excluded.clone(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoricalPropagationError {
    UnsupportedPolicy,
    InvalidEvent,
    UnauthorizedRoot(MemoryId),
    Propagation(PropagationError),
}
impl std::fmt::Display for HistoricalPropagationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "historical propagation: {self:?}")
    }
}
impl std::error::Error for HistoricalPropagationError {}

/// Pure planning API: authorization must include all allowed bridges, including
/// future and lifecycle-ineligible Memories. Missing availability suppresses credit.
/// The caller pins owner/graph/Community proofs; this API persists no events.
pub fn propagate_historical_events(
    events: &[HistoricalPropagationEvent],
    adjacency: &MemoryAdjacency,
    memberships: &CommunityMembership,
    authorized: &BTreeSet<MemoryId>,
    availability: &BTreeMap<MemoryId, MemoryAvailability>,
    workers: usize,
    policy: &FreshnessPolicy,
) -> Result<Vec<PropagationOutcome>, HistoricalPropagationError> {
    if *policy != FreshnessPolicy::default_v1() {
        return Err(HistoricalPropagationError::UnsupportedPolicy);
    }
    for event in events {
        if event.position == 0
            || (event.kind == HistoricalPropagationKind::Semantic
                && event.roots.len() > SEMANTIC_CANDIDATE_LIMIT)
            || event.roots.values().any(|&bonus| match event.kind {
                HistoricalPropagationKind::Semantic => !(1..=SEMANTIC_BONUS_CAP).contains(&bonus),
                HistoricalPropagationKind::FirstCycleDream => bonus != policy.linkage_principal,
            })
        {
            return Err(HistoricalPropagationError::InvalidEvent);
        }
        for root in event.roots.keys() {
            if !authorized.contains(root) {
                return Err(HistoricalPropagationError::UnauthorizedRoot(*root));
            }
        }
    }
    // Build the authorized induced graph once for the batch, without temporal pruning.
    let graph: MemoryAdjacency = adjacency
        .iter()
        .filter(|(id, _)| authorized.contains(id))
        .map(|(&id, neighbors)| (id, neighbors.intersection(authorized).copied().collect()))
        .collect();
    let calculate = |event: &HistoricalPropagationEvent| {
        let mut outcome =
            propagate_frontier::<true>(&event.roots, &BTreeSet::new(), &graph, memberships, policy);
        outcome.candidates.retain(|id, _| !event.credit_excluded.contains(id) && matches!(
            availability.get(id), Some(MemoryAvailability::Supported { position }) if *position > 0 && *position <= event.position
        ));
        outcome.stats.touched_memories = outcome.candidates.len();
        outcome
    };
    let count = planned_event_workers(events.len(), workers);
    if count <= 1 {
        return Ok(events.iter().map(calculate).collect());
    }
    let batches = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(count);
        for worker in 0..count {
            let calculate = &calculate;
            handles.push(scope.spawn(move || {
                (worker..events.len())
                    .step_by(count)
                    .map(|index| (index, calculate(&events[index])))
                    .collect::<Vec<_>>()
            }));
        }
        join_event_workers(handles)
    })
    .map_err(HistoricalPropagationError::Propagation)?;
    let mut ordered = vec![PropagationOutcome::default(); events.len()];
    for batch in batches {
        for (index, outcome) in batch {
            ordered[index] = outcome;
        }
    }
    Ok(ordered)
}

#[cfg(test)]
#[path = "historical_propagation_tests.rs"]
mod tests;
