//! Rebuild derived Freshness lifecycle work from one durable REL snapshot.

use reliquary_freshness::{FreshnessDueDispatcher, FreshnessPolicy};
use reliquary_owner_session::RelRetrievalSnapshot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreshnessTransition {
    pub boundary_turn: u64,
    pub memory_id: reliquary_owner_session::model::MemoryId,
    pub from_state: FreshnessLifecycleState,
    pub to_state: FreshnessLifecycleState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreshnessLifecycleState {
    Fresh,
    Stale,
    Dormant,
}

fn lifecycle(state: reliquary_freshness::FreshnessState) -> FreshnessLifecycleState {
    match state {
        reliquary_freshness::FreshnessState::Fresh => FreshnessLifecycleState::Fresh,
        reliquary_freshness::FreshnessState::Stale => FreshnessLifecycleState::Stale,
        reliquary_freshness::FreshnessState::Dormant => FreshnessLifecycleState::Dormant,
    }
}

/// Reconstruct and drain a bounded set of lifecycle crossings. The returned
/// notices are derived, at-least-once work; durable score records remain the
/// sole authority and make restart reconstruction deterministic.
pub fn due_freshness_transitions(
    snapshot: &RelRetrievalSnapshot,
    policy: &FreshnessPolicy,
    limit: usize,
) -> Result<Vec<FreshnessTransition>, String> {
    if limit == 0 {
        return Err("Freshness due limit must be nonzero".into());
    }
    let records = snapshot.memories.iter().filter_map(|memory| {
        memory
            .freshness
            .map(|record| (memory.revision.record.id, record))
    });
    let mut dispatcher = FreshnessDueDispatcher::default();
    dispatcher
        .rebuild(records, snapshot.rel_turn, policy)
        .map_err(|error| format!("Freshness due rebuild failed: {error:?}"))?;
    Ok(dispatcher
        .drain_due(snapshot.rel_turn, limit)
        .into_iter()
        .map(|due| FreshnessTransition {
            boundary_turn: due.boundary_turn,
            memory_id: due.memory,
            from_state: lifecycle(due.from_state),
            to_state: lifecycle(due.to_state),
        })
        .collect())
}
