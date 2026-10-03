use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

use crate::{CommunityId, MemoryId};

use super::policy::{FreshnessPolicy, FreshnessPolicyError};

pub type MemoryAdjacency = BTreeMap<MemoryId, BTreeSet<MemoryId>>;
pub type CommunityMembership = BTreeMap<MemoryId, CommunityId>;

/// Conservative runtime default for independent graph-frontier workers.
pub fn default_worker_count() -> usize {
    thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .clamp(1, 4)
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SearchNode {
    root: MemoryId,
    memory: MemoryId,
    origin: Option<CommunityId>,
}

struct ExpansionJob<'a> {
    frontier: Vec<(i16, SearchNode)>,
    adjacency: &'a MemoryAdjacency,
    memberships: &'a CommunityMembership,
    excluded: &'a BTreeSet<MemoryId>,
    results: mpsc::Sender<Result<Vec<(i16, SearchNode)>, ()>>,
    local_hop_cost: i16,
    outside_hop_cost: i16,
}

enum WorkerMessage<'a> {
    Expand(ExpansionJob<'a>),
    Stop,
}

/// Evaluates one event against a pinned, undirected owner-local graph snapshot.
///
/// Each originating root carries its own immutable Community context through search.
/// Worker threads expand same-strength frontier batches and synchronize before lower
/// strengths are processed. The pool is bounded and reused across every frontier level.
/// Final results merge by maximum once per physical Memory.
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
    propagate_with_policy_and_stats(event, adjacency, memberships, workers, policy)
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
    workers: usize,
    policy: &FreshnessPolicy,
) -> Result<PropagationOutcome, PropagationError> {
    policy
        .validate()
        .map_err(|_: FreshnessPolicyError| PropagationError::InvalidPolicy)?;
    if !policy.is_valid_event_principal(event.principal) {
        return Err(PropagationError::InvalidPrincipal);
    }
    let roots: Vec<_> = event
        .roots
        .iter()
        .copied()
        .filter(|root| !event.excluded.contains(root))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if roots.is_empty() {
        return Ok(PropagationOutcome::default());
    }

    let worker_count = workers.max(1).min(32);
    if worker_count == 1 {
        return propagate_serial_validated(event, adjacency, memberships, policy);
    }
    let examined_edges = Arc::new(AtomicU64::new(0));
    let candidate_expansions = Arc::new(AtomicU64::new(0));
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(worker_count);
        let mut job_senders = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let (job_sender, job_receiver) = mpsc::channel::<WorkerMessage<'_>>();
            job_senders.push(job_sender);
            let examined_edges = Arc::clone(&examined_edges);
            let candidate_expansions = Arc::clone(&candidate_expansions);
            handles.push(scope.spawn(move || {
                loop {
                    let message = job_receiver.recv();
                    match message {
                        Ok(WorkerMessage::Expand(job)) => {
                            let ExpansionJob {
                                frontier,
                                adjacency,
                                memberships,
                                excluded,
                                results,
                                local_hop_cost,
                                outside_hop_cost,
                            } = job;
                            let expanded =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    let mut expanded = Vec::new();
                                    for (remaining, node) in frontier {
                                        for neighbor in
                                            adjacency.get(&node.memory).into_iter().flatten()
                                        {
                                            examined_edges.fetch_add(1, Ordering::Relaxed);
                                            if excluded.contains(neighbor) {
                                                continue;
                                            }
                                            let stays_in_origin = node.origin.is_some()
                                                && memberships.get(neighbor).copied()
                                                    == node.origin;
                                            let cost = if stays_in_origin {
                                                local_hop_cost
                                            } else {
                                                outside_hop_cost
                                            };
                                            let candidate = remaining - cost;
                                            if candidate > 0 {
                                                candidate_expansions
                                                    .fetch_add(1, Ordering::Relaxed);
                                                expanded.push((
                                                    candidate,
                                                    SearchNode {
                                                        root: node.root,
                                                        memory: *neighbor,
                                                        origin: node.origin,
                                                    },
                                                ));
                                            }
                                        }
                                    }
                                    expanded
                                }));
                            let failed = expanded.is_err();
                            if results.send(expanded.map_err(|_| ())).is_err() || failed {
                                break;
                            }
                        }
                        Ok(WorkerMessage::Stop) | Err(_) => break,
                    }
                }
            }));
        }

        let mut frontier = BTreeMap::<i16, BTreeSet<SearchNode>>::new();
        let mut best_by_root = BTreeMap::<(MemoryId, MemoryId), i16>::new();
        for root in roots {
            let origin = memberships.get(&root).copied();
            let node = SearchNode {
                root,
                memory: root,
                origin,
            };
            frontier.entry(event.principal).or_default().insert(node);
            best_by_root.insert((root, root), event.principal);
        }

        let (result_tx, result_rx) = mpsc::channel();
        let mut worker_result = Ok(());
        let mut frontier_levels = 0usize;
        let mut max_frontier_width = 0usize;
        while let Some((&strength, _)) = frontier.last_key_value() {
            frontier_levels += 1;
            let Some(nodes) = frontier.remove(&strength) else {
                continue;
            };
            // BTreeSet provides stable root/node order. Jobs are coarse chunks; no
            // per-node tasks are created, even for a large dense Community.
            max_frontier_width = max_frontier_width.max(nodes.len());
            let entries: Vec<_> = nodes
                .into_iter()
                .filter(|node| {
                    best_by_root.get(&(node.root, node.memory)).copied() == Some(strength)
                })
                .map(|node| (strength, node))
                .collect();
            if entries.is_empty() {
                continue;
            }
            let chunk_size = entries.len().div_ceil(worker_count);
            let job_count = entries.len().div_ceil(chunk_size);
            for (worker, chunk) in entries.chunks(chunk_size).enumerate() {
                if job_senders[worker]
                    .send(WorkerMessage::Expand(ExpansionJob {
                        frontier: chunk.to_vec(),
                        adjacency,
                        memberships,
                        excluded: &event.excluded,
                        results: result_tx.clone(),
                        local_hop_cost: policy.local_hop_cost,
                        outside_hop_cost: policy.outside_hop_cost,
                    }))
                    .is_err()
                {
                    worker_result = Err(PropagationError::WorkerFailure);
                    break;
                }
            }
            if worker_result.is_err() {
                break;
            }
            let mut expanded = Vec::new();
            for _ in 0..job_count {
                match result_rx.recv() {
                    Ok(Ok(mut output)) => expanded.append(&mut output),
                    Ok(Err(())) | Err(_) => {
                        worker_result = Err(PropagationError::WorkerFailure);
                        break;
                    }
                }
            }
            if worker_result.is_err() {
                break;
            }

            // Deterministic merge barrier: retain only the strongest candidate for
            // each (originating root, physical Memory) before advancing to lower scores.
            expanded.sort_unstable_by(|(score_a, node_a), (score_b, node_b)| {
                score_b.cmp(score_a).then_with(|| node_a.cmp(node_b))
            });
            for (candidate, node) in expanded {
                let key = (node.root, node.memory);
                if best_by_root.get(&key).is_some_and(|old| *old >= candidate) {
                    continue;
                }
                best_by_root.insert(key, candidate);
                frontier.entry(candidate).or_default().insert(node);
            }
        }

        drop(result_tx);
        for sender in job_senders {
            let _ = sender.send(WorkerMessage::Stop);
        }
        for handle in handles {
            if handle.join().is_err() {
                worker_result = Err(PropagationError::WorkerFailure);
            }
        }
        worker_result?;

        let mut event_best = BTreeMap::<MemoryId, i16>::new();
        for ((_, memory), candidate) in best_by_root {
            event_best
                .entry(memory)
                .and_modify(|best| *best = (*best).max(candidate))
                .or_insert(candidate);
        }
        let stats = PropagationStats {
            examined_edges: examined_edges.load(Ordering::Relaxed),
            candidate_expansions: candidate_expansions.load(Ordering::Relaxed),
            touched_memories: event_best.len(),
            max_frontier_width,
            worker_count,
            frontier_levels,
        };
        Ok(PropagationOutcome {
            candidates: event_best,
            stats,
        })
    })
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
    policy
        .validate()
        .map_err(|_: FreshnessPolicyError| PropagationError::InvalidPolicy)?;
    for event in events {
        if !policy.is_valid_event_principal(event.principal) {
            return Err(PropagationError::InvalidPrincipal);
        }
    }
    if events.is_empty() {
        return Ok(Vec::new());
    }
    let worker_count = workers.max(1).min(32).min(events.len());
    if worker_count == 1 {
        return events
            .iter()
            .map(|event| propagate_serial_validated(event, adjacency, memberships, policy))
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
                        propagate_serial_validated(&events[index], adjacency, memberships, policy),
                    ));
                }
                results
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().map_err(|_| PropagationError::WorkerFailure))
            .collect::<Result<Vec<_>, _>>()
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

fn propagate_serial_validated(
    event: &PropagationEvent,
    adjacency: &MemoryAdjacency,
    memberships: &CommunityMembership,
    policy: &FreshnessPolicy,
) -> Result<PropagationOutcome, PropagationError> {
    let roots: Vec<_> = event
        .roots
        .iter()
        .copied()
        .filter(|root| !event.excluded.contains(root))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut frontier = BTreeMap::<i16, BTreeSet<SearchNode>>::new();
    let mut best_by_root = BTreeMap::<(MemoryId, MemoryId), i16>::new();
    for root in roots {
        let node = SearchNode {
            root,
            memory: root,
            origin: memberships.get(&root).copied(),
        };
        frontier.entry(event.principal).or_default().insert(node);
        best_by_root.insert((root, root), event.principal);
    }

    let mut examined_edges = 0u64;
    let mut candidate_expansions = 0u64;
    let mut frontier_levels = 0usize;
    let mut max_frontier_width = 0usize;
    while let Some((&strength, _)) = frontier.last_key_value() {
        frontier_levels += 1;
        let Some(nodes) = frontier.remove(&strength) else {
            continue;
        };
        max_frontier_width = max_frontier_width.max(nodes.len());
        let mut expanded = Vec::new();
        for node in nodes {
            if best_by_root.get(&(node.root, node.memory)).copied() != Some(strength) {
                continue;
            }
            for neighbor in adjacency.get(&node.memory).into_iter().flatten() {
                examined_edges = examined_edges.saturating_add(1);
                if event.excluded.contains(neighbor) {
                    continue;
                }
                let stays_in_origin =
                    node.origin.is_some() && memberships.get(neighbor).copied() == node.origin;
                let cost = if stays_in_origin {
                    policy.local_hop_cost
                } else {
                    policy.outside_hop_cost
                };
                let candidate = strength - cost;
                if candidate > 0 {
                    candidate_expansions = candidate_expansions.saturating_add(1);
                    expanded.push((
                        candidate,
                        SearchNode {
                            root: node.root,
                            memory: *neighbor,
                            origin: node.origin,
                        },
                    ));
                }
            }
        }
        expanded.sort_unstable_by(|(score_a, node_a), (score_b, node_b)| {
            score_b.cmp(score_a).then_with(|| node_a.cmp(node_b))
        });
        for (candidate, node) in expanded {
            let key = (node.root, node.memory);
            if best_by_root.get(&key).is_some_and(|old| *old >= candidate) {
                continue;
            }
            best_by_root.insert(key, candidate);
            frontier.entry(candidate).or_default().insert(node);
        }
    }

    let mut candidates = BTreeMap::<MemoryId, i16>::new();
    for ((_, memory), candidate) in best_by_root {
        candidates
            .entry(memory)
            .and_modify(|best| *best = (*best).max(candidate))
            .or_insert(candidate);
    }
    let stats = PropagationStats {
        examined_edges,
        candidate_expansions,
        touched_memories: candidates.len(),
        max_frontier_width,
        worker_count: 1,
        frontier_levels,
    };
    Ok(PropagationOutcome { candidates, stats })
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
}
