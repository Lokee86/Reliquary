//! Score one historical import from a frozen owner graph and event batch.

use crate::{
    CommunityMembership, FreshnessPolicy, FreshnessRecord, MemoryAdjacency,
    historical_planner::MemoryAvailability,
    historical_propagation::{HistoricalPropagationEvent, propagate_historical_events},
    historical_scorer::{HistoricalScoreEvent, score_historical_records},
};
use reliquary_memory_core::MemoryId;
use std::collections::{BTreeMap, BTreeSet};

pub struct HistoricalImportGraph<'a> {
    pub adjacency: &'a MemoryAdjacency,
    pub memberships: &'a CommunityMembership,
    pub authorized: &'a BTreeSet<MemoryId>,
    pub availability: &'a BTreeMap<MemoryId, MemoryAvailability>,
}

/// Merge propagated import credit with the retained live contributions before
/// chronological scoring. An event is expanded once, regardless of root count.
pub fn score_historical_import(
    initial: &BTreeMap<MemoryId, FreshnessRecord>,
    events: &[([u8; 32], HistoricalPropagationEvent)],
    retained: &[HistoricalScoreEvent],
    graph: HistoricalImportGraph<'_>,
    cut: u64,
    policy: &FreshnessPolicy,
) -> Result<BTreeMap<MemoryId, FreshnessRecord>, String> {
    let mut identities = BTreeSet::new();
    for (id, event) in events {
        if event.position > cut || !identities.insert(*id) {
            return Err("historical import event exceeds the cut or repeats an identity".into());
        }
    }
    let batch: Vec<_> = events.iter().map(|(_, event)| event.clone()).collect();
    let outcomes = propagate_historical_events(
        &batch,
        graph.adjacency,
        graph.memberships,
        graph.authorized,
        graph.availability,
        1,
        policy,
    )
    .map_err(|error| error.to_string())?;
    let mut contributions = retained.to_vec();
    for ((event_id, event), outcome) in events.iter().zip(outcomes) {
        contributions.extend(outcome.candidates.into_iter().map(|(recipient, amount)| {
            HistoricalScoreEvent {
                recipient,
                position: event.position,
                event_id: *event_id,
                amount,
            }
        }));
    }
    if cut == 0 && initial.is_empty() && contributions.is_empty() {
        return Ok(BTreeMap::new());
    }
    score_historical_records(initial, &contributions, cut, policy)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
