use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::thread;

use crate::{CommunityId, MemoryId};

use super::policy::{FreshnessPolicy, FreshnessPolicyError};

pub type MemoryAdjacency = BTreeMap<MemoryId, BTreeSet<MemoryId>>;
pub type CommunityMembership = BTreeMap<MemoryId, CommunityId>;

/// Conservative runtime default for independent-event workers.
pub fn default_worker_count() -> usize {
    thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .clamp(1, 4)
}

const PARALLEL_BATCH_EVENTS: usize = 16;

/// Fixed event-level resource budget; graph topology never affects scheduling.
pub(crate) fn planned_event_workers(event_count: usize, requested_workers: usize) -> usize {
    if event_count < PARALLEL_BATCH_EVENTS {
        return usize::from(event_count != 0);
    }
    let available = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    planned_event_workers_with_parallelism(event_count, requested_workers, available)
}

fn planned_event_workers_with_parallelism(
    event_count: usize,
    requested_workers: usize,
    available_parallelism: usize,
) -> usize {
    if event_count < PARALLEL_BATCH_EVENTS {
        return usize::from(event_count != 0);
    }
    requested_workers
        .max(1)
        .min(available_parallelism.max(1))
        .min(event_count)
        .min(4)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropagationEvent {
    /// Existing information roots. Each root starts its own fixed-origin search.
    pub roots: Vec<MemoryId>,
    /// Event principal (+25 access or +50 first-cycle relationship).
    pub principal: i16,
    /// A first-cycle new Memory is not a recipient or a bridge for its own event.
    pub excluded: BTreeSet<MemoryId>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PropagationStats {
    pub examined_edges: u64,
    pub candidate_expansions: u64,
    pub touched_memories: usize,
    pub max_frontier_width: usize,
    pub worker_count: usize,
    pub frontier_levels: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PropagationOutcome {
    pub candidates: BTreeMap<MemoryId, i16>,
    pub stats: PropagationStats,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropagationError {
    InvalidPrincipal,
    InvalidPolicy,
    WorkerFailure,
}

impl fmt::Display for PropagationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPrincipal => {
                f.write_str("propagation principal is not configured by policy")
            }
            Self::InvalidPolicy => f.write_str("Freshness policy is invalid"),
            Self::WorkerFailure => f.write_str("Freshness propagation worker failed"),
        }
    }
}

impl std::error::Error for PropagationError {}

/// Computes an event against the accepted, immutable graph and Community snapshot.
/// The optimization shares a strongest-path search between roots that have the
/// same originating Community. Distinct origin groups remain independent.
pub fn propagate(
    event: &PropagationEvent,
    adjacency: &MemoryAdjacency,
    memberships: &CommunityMembership,
    workers: usize,
) -> Result<BTreeMap<MemoryId, i16>, PropagationError> {
    propagate_with_policy(
        event,
        adjacency,
        memberships,
        workers,
        &FreshnessPolicy::default(),
    )
}

pub fn propagate_with_policy(
    event: &PropagationEvent,
    adjacency: &MemoryAdjacency,
    memberships: &CommunityMembership,
    workers: usize,
    policy: &FreshnessPolicy,
) -> Result<BTreeMap<MemoryId, i16>, PropagationError> {
    propagate_single::<false>(event, adjacency, memberships, workers, policy)
        .map(|outcome| outcome.candidates)
}

pub fn propagate_with_stats(
    event: &PropagationEvent,
    adjacency: &MemoryAdjacency,
    memberships: &CommunityMembership,
    workers: usize,
) -> Result<PropagationOutcome, PropagationError> {
    propagate_with_policy_and_stats(
        event,
        adjacency,
        memberships,
        workers,
        &FreshnessPolicy::default(),
    )
}

pub fn propagate_with_policy_and_stats(
    event: &PropagationEvent,
    adjacency: &MemoryAdjacency,
    memberships: &CommunityMembership,
    _workers: usize,
    policy: &FreshnessPolicy,
) -> Result<PropagationOutcome, PropagationError> {
    propagate_single::<true>(event, adjacency, memberships, _workers, policy)
}

fn validate_policy(policy: &FreshnessPolicy) -> Result<(), PropagationError> {
    policy
        .validate()
        .map_err(|_: FreshnessPolicyError| PropagationError::InvalidPolicy)
}

fn validate_event_principal(
    event: &PropagationEvent,
    policy: &FreshnessPolicy,
) -> Result<(), PropagationError> {
    if policy.is_valid_event_principal(event.principal) {
        Ok(())
    } else {
        Err(PropagationError::InvalidPrincipal)
    }
}

fn validate_batch(
    events: &[PropagationEvent],
    policy: &FreshnessPolicy,
) -> Result<(), PropagationError> {
    validate_policy(policy)?;
    for event in events {
        validate_event_principal(event, policy)?;
    }
    Ok(())
}

fn propagate_single<const TRACK_STATS: bool>(
    event: &PropagationEvent,
    adjacency: &MemoryAdjacency,
    memberships: &CommunityMembership,
    _workers: usize,
    policy: &FreshnessPolicy,
) -> Result<PropagationOutcome, PropagationError> {
    validate_policy(policy)?;
    validate_event_principal(event, policy)?;
    if event.roots.iter().all(|root| event.excluded.contains(root)) {
        // Preserve the direct single-event API's historical all-excluded stats.
        return Ok(PropagationOutcome::default());
    }
    // P0 found synchronization within one multi-root event adds overhead.
    // Independent batches may use a bounded pool; each event stays serial.
    propagate_serial_validated::<TRACK_STATS>(event, adjacency, memberships, policy)
}

/// Calculates independent events with one bounded total-worker pool over a shared pinned
/// graph snapshot. Each worker processes its assigned event serially, so the batch never
/// nests per-event worker pools. Output positions match input positions.
pub fn propagate_events_with_policy(
    events: &[PropagationEvent],
    adjacency: &MemoryAdjacency,
    memberships: &CommunityMembership,
    workers: usize,
    policy: &FreshnessPolicy,
) -> Result<Vec<PropagationOutcome>, PropagationError> {
    propagate_events_impl::<true>(events, adjacency, memberships, workers, policy)
}

/// Crate-private production path for consumers that need only candidate scores.
/// Public batch APIs continue returning fully populated statistics.
pub(crate) fn propagate_event_candidates_with_policy(
    events: &[PropagationEvent],
    adjacency: &MemoryAdjacency,
    memberships: &CommunityMembership,
    workers: usize,
    policy: &FreshnessPolicy,
) -> Result<Vec<BTreeMap<MemoryId, i16>>, PropagationError> {
    propagate_events_impl::<false>(events, adjacency, memberships, workers, policy).map(
        |outcomes| {
            outcomes
                .into_iter()
                .map(|outcome| outcome.candidates)
                .collect()
        },
    )
}

fn propagate_events_impl<const TRACK_STATS: bool>(
    events: &[PropagationEvent],
    adjacency: &MemoryAdjacency,
    memberships: &CommunityMembership,
    workers: usize,
    policy: &FreshnessPolicy,
) -> Result<Vec<PropagationOutcome>, PropagationError> {
    validate_batch(events, policy)?;
    if events.is_empty() {
        return Ok(Vec::new());
    }
    let worker_count = planned_event_workers(events.len(), workers);
    if worker_count == 1 {
        return events
            .iter()
            .map(|event| {
                propagate_serial_validated::<TRACK_STATS>(event, adjacency, memberships, policy)
            })
            .collect();
    }

    let batches = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(worker_count);
        for worker in 0..worker_count {
            handles.push(scope.spawn(move || {
                let mut results = Vec::new();
                for index in (worker..events.len()).step_by(worker_count) {
                    results.push((
                        index,
                        propagate_serial_validated::<TRACK_STATS>(
                            &events[index],
                            adjacency,
                            memberships,
                            policy,
                        ),
                    ));
                }
                results
            }));
        }
        join_event_workers(handles)
    })?;

    let mut ordered: Vec<Option<Result<PropagationOutcome, PropagationError>>> =
        std::iter::repeat_with(|| None).take(events.len()).collect();
    for batch in batches {
        for (index, result) in batch {
            ordered[index] = Some(result);
        }
    }
    ordered
        .into_iter()
        .map(|result| result.unwrap_or(Err(PropagationError::WorkerFailure)))
        .collect()
}

/// Join every scoped worker even after a panic so no implicit scope join can
/// turn a second worker failure into an escaping panic.
pub(crate) fn join_event_workers<'scope, T>(
    handles: Vec<thread::ScopedJoinHandle<'scope, T>>,
) -> Result<Vec<T>, PropagationError> {
    let mut batches = Vec::with_capacity(handles.len());
    let mut worker_failed = false;
    for handle in handles {
        match handle.join() {
            Ok(batch) => batches.push(batch),
            Err(_) => worker_failed = true,
        }
    }
    if worker_failed {
        Err(PropagationError::WorkerFailure)
    } else {
        Ok(batches)
    }
}

/// One multi-source strongest-path traversal per immutable originating Community.
/// A physical Memory's strongest score is merged only after each independent
/// origin group has finished; no accepted-use events or durable receipts collapse.
fn propagate_serial_validated<const TRACK_STATS: bool>(
    event: &PropagationEvent,
    adjacency: &MemoryAdjacency,
    memberships: &CommunityMembership,
    policy: &FreshnessPolicy,
) -> Result<PropagationOutcome, PropagationError> {
    let roots = event
        .roots
        .iter()
        .copied()
        .map(|id| (id, event.principal))
        .collect();
    Ok(propagate_frontier::<TRACK_STATS>(
        &roots,
        &event.excluded,
        adjacency,
        memberships,
        policy,
    ))
}

/// Shared numerical frontier. Callers validate their distinct event principals.
/// Only traversal exclusions remove bridges; credit eligibility is a caller concern.
pub(crate) fn propagate_frontier<const TRACK_STATS: bool>(
    roots: &BTreeMap<MemoryId, i16>,
    traversal_excluded: &BTreeSet<MemoryId>,
    adjacency: &MemoryAdjacency,
    memberships: &CommunityMembership,
    policy: &FreshnessPolicy,
) -> PropagationOutcome {
    let mut origin_roots = BTreeMap::<Option<CommunityId>, BTreeMap<MemoryId, i16>>::new();
    for (&root, &strength) in roots {
        if !traversal_excluded.contains(&root) {
            origin_roots
                .entry(memberships.get(&root).copied())
                .or_default()
                .insert(root, strength);
        }
    }
    let mut candidates = BTreeMap::<MemoryId, i16>::new();
    let mut examined_edges = 0u64;
    let mut candidate_expansions = 0u64;
    let mut frontier_levels = 0usize;
    let mut max_frontier_width = 0usize;

    for (origin, roots) in origin_roots {
        let mut frontier = BTreeMap::<i16, BTreeSet<MemoryId>>::new();
        let mut best = BTreeMap::<MemoryId, i16>::new();
        for (root, strength) in roots {
            frontier.entry(strength).or_default().insert(root);
            best.insert(root, strength);
        }

        while let Some((&strength, _)) = frontier.last_key_value() {
            if TRACK_STATS {
                frontier_levels += 1;
            }
            let nodes = frontier
                .remove(&strength)
                .expect("frontier strength exists");
            if TRACK_STATS {
                max_frontier_width = max_frontier_width.max(nodes.len());
            }
            for memory in nodes {
                if best.get(&memory).copied() != Some(strength) {
                    continue;
                }
                for neighbor in adjacency.get(&memory).into_iter().flatten() {
                    if TRACK_STATS {
                        examined_edges = examined_edges.saturating_add(1);
                    }
                    if traversal_excluded.contains(neighbor) {
                        continue;
                    }
                    let stays_in_origin =
                        origin.is_some() && memberships.get(neighbor).copied() == origin;
                    let hop_cost = if stays_in_origin {
                        policy.local_hop_cost
                    } else {
                        policy.outside_hop_cost
                    };
                    let next = strength - hop_cost;
                    if next <= 0 {
                        continue;
                    }
                    if TRACK_STATS {
                        candidate_expansions = candidate_expansions.saturating_add(1);
                    }
                    if best.get(neighbor).is_some_and(|previous| *previous >= next) {
                        continue;
                    }
                    best.insert(*neighbor, next);
                    frontier.entry(next).or_default().insert(*neighbor);
                }
            }
        }

        for (memory, strength) in best {
            candidates
                .entry(memory)
                .and_modify(|existing| *existing = (*existing).max(strength))
                .or_insert(strength);
        }
    }

    let stats = if TRACK_STATS {
        PropagationStats {
            examined_edges,
            candidate_expansions,
            touched_memories: candidates.len(),
            max_frontier_width,
            worker_count: 1,
            frontier_levels,
        }
    } else {
        PropagationStats::default()
    };
    PropagationOutcome { stats, candidates }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_id(value: u8) -> MemoryId {
        MemoryId([value; 32])
    }

    fn community_id(value: u8) -> CommunityId {
        CommunityId([value; 32])
    }

    fn connect(graph: &mut MemoryAdjacency, a: u8, b: u8) {
        graph.entry(memory_id(a)).or_default().insert(memory_id(b));
        graph.entry(memory_id(b)).or_default().insert(memory_id(a));
    }

    #[test]
    fn nondefault_owner_policy_controls_principal_and_hop_costs() {
        let policy = FreshnessPolicy {
            decay_turns_per_point: 5,
            access_principal: 30,
            linkage_principal: 60,
            local_hop_cost: 6,
            outside_hop_cost: 12,
        };
        let mut graph = MemoryAdjacency::new();
        for (a, b) in [(1, 2), (2, 3), (3, 4)] {
            connect(&mut graph, a, b);
        }
        let communities = [
            (memory_id(1), community_id(7)),
            (memory_id(2), community_id(7)),
            (memory_id(3), community_id(8)),
            (memory_id(4), community_id(8)),
        ]
        .into_iter()
        .collect();
        let event = PropagationEvent {
            roots: vec![memory_id(1)],
            principal: 30,
            excluded: BTreeSet::new(),
        };
        assert_eq!(
            propagate(&event, &graph, &communities, 3),
            Err(PropagationError::InvalidPrincipal)
        );
        let result = propagate_with_policy(&event, &graph, &communities, 3, &policy).unwrap();
        assert_eq!(result.get(&memory_id(2)), Some(&24));
        assert_eq!(result.get(&memory_id(3)), Some(&12));
        assert!(!result.contains_key(&memory_id(4)));
    }

    #[test]
    fn origin_community_cost_applies_per_destination_and_reentry() {
        let mut graph = MemoryAdjacency::new();
        for (a, b) in [(1, 2), (2, 3), (3, 4), (4, 5)] {
            connect(&mut graph, a, b);
        }
        let communities = [
            (memory_id(1), community_id(7)),
            (memory_id(2), community_id(7)),
            (memory_id(3), community_id(8)),
            (memory_id(4), community_id(8)),
            (memory_id(5), community_id(7)),
        ]
        .into_iter()
        .collect();
        let event = PropagationEvent {
            roots: vec![memory_id(1)],
            principal: 50,
            excluded: BTreeSet::new(),
        };
        let result = propagate(&event, &graph, &communities, 3).unwrap();
        assert_eq!(result.get(&memory_id(1)), Some(&50));
        assert_eq!(result.get(&memory_id(2)), Some(&45));
        assert_eq!(result.get(&memory_id(3)), Some(&35));
        assert_eq!(result.get(&memory_id(4)), Some(&25));
        assert_eq!(result.get(&memory_id(5)), Some(&20));
    }

    #[test]
    fn multiple_roots_keep_separate_origin_context_and_merge_only_by_max() {
        let mut graph = MemoryAdjacency::new();
        connect(&mut graph, 1, 3);
        connect(&mut graph, 2, 3);
        connect(&mut graph, 3, 4);
        let communities = [
            (memory_id(1), community_id(7)),
            (memory_id(2), community_id(8)),
            (memory_id(3), community_id(8)),
            (memory_id(4), community_id(7)),
        ]
        .into_iter()
        .collect();
        let event = PropagationEvent {
            roots: vec![memory_id(2), memory_id(1), memory_id(2)],
            principal: 50,
            excluded: BTreeSet::new(),
        };
        let one = propagate(&event, &graph, &communities, 1).unwrap();
        let many = propagate(&event, &graph, &communities, 8).unwrap();
        assert_eq!(one, many);
        assert_eq!(one.get(&memory_id(3)), Some(&45));
        assert_eq!(one.get(&memory_id(4)), Some(&35));
    }

    #[test]
    fn cycles_keep_best_event_path_independent_of_recipient_score() {
        let mut graph = MemoryAdjacency::new();
        for (a, b) in [(1, 2), (2, 3), (3, 4), (4, 1), (1, 3)] {
            connect(&mut graph, a, b);
        }
        let communities = (1..=4)
            .map(|node| (memory_id(node), community_id(3)))
            .collect();
        let event = PropagationEvent {
            roots: vec![memory_id(1)],
            principal: 25,
            excluded: BTreeSet::new(),
        };
        let result = propagate(&event, &graph, &communities, 2).unwrap();
        assert_eq!(result.get(&memory_id(2)), Some(&20));
        assert_eq!(result.get(&memory_id(3)), Some(&20));
        assert_eq!(result.get(&memory_id(4)), Some(&20)); // direct 1->4 strongest path
    }

    #[test]
    fn excluded_source_is_neither_reinforced_nor_used_as_bridge() {
        let mut graph = MemoryAdjacency::new();
        connect(&mut graph, 1, 9);
        connect(&mut graph, 9, 2);
        let communities = [1, 2, 9]
            .into_iter()
            .map(|node| (memory_id(node), community_id(6)))
            .collect();
        let event = PropagationEvent {
            roots: vec![memory_id(1)],
            principal: 50,
            excluded: [memory_id(9)].into_iter().collect(),
        };
        let result = propagate(&event, &graph, &communities, 1).unwrap();
        assert!(!result.contains_key(&memory_id(9)));
        assert!(!result.contains_key(&memory_id(2)));
    }

    #[test]
    fn unknown_origin_or_destination_uses_conservative_outside_cost() {
        let mut graph = MemoryAdjacency::new();
        connect(&mut graph, 1, 2);
        let event = PropagationEvent {
            roots: vec![memory_id(1)],
            principal: 25,
            excluded: BTreeSet::new(),
        };
        let result = propagate(&event, &graph, &CommunityMembership::new(), 1).unwrap();
        assert_eq!(result.get(&memory_id(2)), Some(&15));
    }

    #[test]
    fn independent_event_batch_uses_stable_event_order_and_same_results_by_worker_count() {
        let mut graph = MemoryAdjacency::new();
        for (a, b) in [(1, 3), (2, 3), (3, 4)] {
            connect(&mut graph, a, b);
        }
        let communities = [
            (memory_id(1), community_id(7)),
            (memory_id(2), community_id(8)),
            (memory_id(3), community_id(8)),
            (memory_id(4), community_id(7)),
        ]
        .into_iter()
        .collect();
        let events = [
            PropagationEvent {
                roots: vec![memory_id(1)],
                principal: 25,
                excluded: BTreeSet::new(),
            },
            PropagationEvent {
                roots: vec![memory_id(2)],
                principal: 50,
                excluded: BTreeSet::new(),
            },
        ];
        let serial = propagate_events_with_policy(
            &events,
            &graph,
            &communities,
            1,
            &FreshnessPolicy::default(),
        )
        .unwrap();
        let parallel = propagate_events_with_policy(
            &events,
            &graph,
            &communities,
            4,
            &FreshnessPolicy::default(),
        )
        .unwrap();
        assert_eq!(serial, parallel);
        assert_eq!(serial.len(), 2);
        assert_eq!(serial[0].candidates.get(&memory_id(3)), Some(&15));
        assert_eq!(serial[1].candidates.get(&memory_id(3)), Some(&45));
        assert_eq!(serial[1].stats.worker_count, 1);
    }

    #[test]
    fn dense_single_root_expansion_is_identical_across_worker_counts() {
        let mut graph = MemoryAdjacency::new();
        for a in 1..=24 {
            for b in (a + 1)..=24 {
                connect(&mut graph, a, b);
            }
        }
        let communities = (1..=24)
            .map(|node| (memory_id(node), community_id(9)))
            .collect();
        let event = PropagationEvent {
            roots: vec![memory_id(1)],
            principal: 50,
            excluded: BTreeSet::new(),
        };
        let one = propagate(&event, &graph, &communities, 1).unwrap();
        let many = propagate(&event, &graph, &communities, 5).unwrap();
        assert_eq!(one, many);
        assert_eq!(one.len(), 24);
        assert!(
            one.iter()
                .filter(|(memory, _)| **memory != memory_id(1))
                .all(|(_, score)| *score == 45)
        );
    }

    #[test]
    fn production_candidate_batch_matches_public_stats_batch_and_preserves_empty_stats() {
        let mut graph = MemoryAdjacency::new();
        connect(&mut graph, 1, 2);
        connect(&mut graph, 2, 3);
        let events = vec![
            PropagationEvent {
                roots: vec![memory_id(1)],
                principal: 25,
                excluded: BTreeSet::new(),
            },
            PropagationEvent {
                roots: vec![memory_id(3)],
                principal: 50,
                excluded: BTreeSet::new(),
            },
        ];
        let policy = FreshnessPolicy::default();
        let tracked =
            propagate_events_with_policy(&events, &graph, &CommunityMembership::new(), 2, &policy)
                .unwrap();
        let candidates = propagate_event_candidates_with_policy(
            &events,
            &graph,
            &CommunityMembership::new(),
            2,
            &policy,
        )
        .unwrap();
        assert_eq!(
            candidates,
            tracked
                .into_iter()
                .map(|outcome| outcome.candidates)
                .collect::<Vec<_>>()
        );

        let excluded = PropagationEvent {
            roots: vec![memory_id(1)],
            principal: 25,
            excluded: [memory_id(1)].into_iter().collect(),
        };
        assert_eq!(
            propagate_with_stats(&excluded, &graph, &CommunityMembership::new(), 1).unwrap(),
            PropagationOutcome::default()
        );
        let batch = propagate_events_with_policy(
            &[excluded],
            &graph,
            &CommunityMembership::new(),
            1,
            &policy,
        )
        .unwrap();
        assert_eq!(batch.len(), 1);
        assert!(batch[0].candidates.is_empty());
        assert_eq!(
            batch[0].stats,
            PropagationStats {
                worker_count: 1,
                ..PropagationStats::default()
            }
        );
    }

    #[test]
    fn fixed_planner_respects_threshold_and_resource_caps() {
        for count in [0, 1, 15, 16, 17] {
            for requested in [0usize, 1, 2, 4, 8, 64] {
                for available in [0usize, 1, 2, 4, 8] {
                    let expected = if count == 0 {
                        0
                    } else if count < 16 {
                        1
                    } else {
                        requested.max(1).min(available.max(1)).min(4)
                    };
                    assert_eq!(
                        planned_event_workers_with_parallelism(count, requested, available),
                        expected,
                        "events={count} cap={requested} hardware={available}"
                    );
                }
            }
        }
    }

    #[test]
    fn parallel_batch_preserves_candidates_order_and_empty_events() {
        let mut graph = MemoryAdjacency::new();
        connect(&mut graph, 1, 2);
        let events = (0..17)
            .map(|index| PropagationEvent {
                roots: match index % 4 {
                    0 => vec![],
                    1 => vec![memory_id(1)],
                    2 => vec![memory_id(2)],
                    _ => vec![memory_id(99)],
                },
                principal: if index % 2 == 0 { 25 } else { 50 },
                excluded: if index % 4 == 2 {
                    [memory_id(2)].into_iter().collect()
                } else {
                    BTreeSet::new()
                },
            })
            .collect::<Vec<_>>();
        let policy = FreshnessPolicy::default();
        let memberships = CommunityMembership::new();
        let serial =
            propagate_events_with_policy(&events, &graph, &memberships, 1, &policy).unwrap();
        for cap in [2, 4, 8, 64] {
            let tracked =
                propagate_events_with_policy(&events, &graph, &memberships, cap, &policy).unwrap();
            assert_eq!(tracked, serial);
            let candidates =
                propagate_event_candidates_with_policy(&events, &graph, &memberships, cap, &policy)
                    .unwrap();
            assert_eq!(
                candidates,
                serial
                    .iter()
                    .map(|outcome| outcome.candidates.clone())
                    .collect::<Vec<_>>()
            );
        }
        let mut invalid = events;
        invalid[16].principal = 26;
        assert_eq!(
            propagate_events_with_policy(&invalid, &graph, &memberships, 4, &policy),
            Err(PropagationError::InvalidPrincipal)
        );
        let invalid_policy = FreshnessPolicy {
            local_hop_cost: 0,
            ..policy
        };
        assert_eq!(
            propagate_events_with_policy(&[], &graph, &memberships, 4, &invalid_policy),
            Err(PropagationError::InvalidPolicy)
        );
    }

    #[test]
    fn worker_failure_joins_every_worker_and_returns_error_without_escaping_panic() {
        let result = thread::scope(|scope| {
            let handles = vec![
                scope.spawn(|| -> Vec<()> { panic!("first worker") }),
                scope.spawn(|| vec![()]),
                scope.spawn(|| -> Vec<()> { panic!("second worker") }),
            ];
            join_event_workers(handles)
        });
        assert_eq!(result, Err(PropagationError::WorkerFailure));
    }
}
