//! F1 standalone, deliberately simple mathematical oracle for later differential tests.
//! These tests freeze policy, not an integrated Freshness implementation.
//! Run without dependency compilation: rustc --edition=2024 --test tests/freshness_f1_reference.rs -o target/freshness_f1_reference.exe

use std::collections::{BTreeMap, BTreeSet};

type Id = u8;
type Community = u8;

#[derive(Clone, Debug)]
struct ReferenceScore {
    score: i16,
    /// First successfully settled active Dream lifecycle (or directly published
    /// already-active first state); None means not yet admitted to the Web.
    admission: Option<u64>,
    /// Last fully applied decay-step boundary, initially equal to admission.
    accounted: Option<u64>,
}

impl ReferenceScore {
    fn created() -> Self {
        Self {
            score: 100,
            admission: None,
            accounted: None,
        }
    }

    fn admit(&mut self, accepted_rel_turn: u64) -> bool {
        if self.admission.is_some() {
            return false; // duplicate publication/revision must not restart decay
        }
        self.admission = Some(accepted_rel_turn);
        self.accounted = Some(accepted_rel_turn);
        true
    }

    fn settle(&mut self, accepted_rel_turn: u64) {
        let last = self
            .accounted
            .expect("cannot decay a draft before Web admission");
        assert!(accepted_rel_turn >= last, "REL clock must be monotone");
        let steps = (accepted_rel_turn - last) / 10;
        self.score =
            (i64::from(self.score) - i64::try_from(steps).unwrap_or(i64::MAX)).max(-100) as i16;
        self.accounted = Some(last + steps * 10); // preserve remainder across reinforcement
    }

    fn reinforce(&mut self, amount: i16, accepted_rel_turn: u64) {
        self.settle(accepted_rel_turn);
        self.score = (self.score + amount).min(100);
    }

    fn state(&self) -> &'static str {
        match self.score {
            1..=100 => "Fresh",
            -50..=0 => "Stale",
            -100..=-51 => "Dormant",
            _ => panic!("score outside contract"),
        }
    }
}

/// First creation alone does not start decay for extracted Memories. The
/// first successfully settled Dream lifecycle result does, even without
/// edges, unless Dream archives the new duplicate. Direct first publication
/// already explicitly active may admit immediately.
fn qualifies_for_first_admission(
    directly_created_now: bool,
    first_dream_settled: bool,
    state: &str,
    archived: bool,
) -> bool {
    !archived
        && matches!(state, "knowledge" | "canonical")
        && (directly_created_now || first_dream_settled)
}

#[test]
fn extracted_creation_and_provisional_graph_edges_do_not_start_decay() {
    let mut memory = ReferenceScore::created(); // first accepted extracted publication at 120
    assert!(!qualifies_for_first_admission(
        true,
        false,
        "extracted",
        false
    ));
    assert_eq!(memory.admission, None);
    // Dream may persist an edge before its first initial lifecycle settlement.
    assert!(!qualifies_for_first_admission(
        false,
        false,
        "extracted",
        false
    ));
    assert_eq!(memory.score, 100);
    // A successful, initially isolated Dream pass still admits a knowledge Memory.
    assert!(qualifies_for_first_admission(
        false,
        true,
        "knowledge",
        false
    ));
    assert!(memory.admit(340));
    assert!(!memory.admit(350)); // retry / later Dream run never reanchors
    memory.settle(350);
    assert_eq!(memory.score, 99);
}

#[test]
fn canonical_or_knowledge_first_settlement_admits_even_without_any_links() {
    for state in ["knowledge", "canonical"] {
        assert!(qualifies_for_first_admission(false, true, state, false));
    }
    assert!(qualifies_for_first_admission(
        true,
        false,
        "knowledge",
        false
    ));
    assert!(qualifies_for_first_admission(
        true,
        false,
        "canonical",
        false
    ));
    assert!(!qualifies_for_first_admission(
        false,
        false,
        "knowledge",
        false
    ));
    // An initial duplicate can still issue its link event at Dream settlement,
    // but its archived newborn copy does not join the active Web for decay.
    assert!(!qualifies_for_first_admission(
        false, true, "archived", true
    ));
}

/// Read-only event-scoped oracle. A root is an existing Memory, not the new source.
/// Every root has its own fixed originating Community, all roots form ONE event.
/// Edge direction is irrelevant to contextual relevance; graph has already been
/// projected into active, deduplicated, Memory-to-Memory undirected adjacency.
fn propagate(
    adjacency: &BTreeMap<Id, BTreeSet<Id>>,
    memberships: &BTreeMap<Id, Community>,
    roots: &[Id],
    principal: i16,
    excluded_new_source: Option<Id>,
) -> BTreeMap<Id, i16> {
    let mut event_best = BTreeMap::new();
    for &root in roots {
        assert_ne!(Some(root), excluded_new_source);
        let origin_community = memberships.get(&root).copied();
        let mut root_best = BTreeMap::<Id, i16>::new();
        let mut frontier = vec![(root, principal)];
        while !frontier.is_empty() {
            // An intentionally serial O(N^2) oracle. F4 must compare a concurrent
            // implementation against these results, not duplicate this scheduler.
            frontier.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)));
            let (node, remaining) = frontier.pop().unwrap();
            if remaining <= 0 || Some(node) == excluded_new_source {
                continue;
            }
            if root_best.get(&node).is_some_and(|old| *old >= remaining) {
                continue;
            }
            root_best.insert(node, remaining);
            for &neighbor in adjacency.get(&node).into_iter().flatten() {
                if Some(neighbor) == excluded_new_source {
                    continue;
                }
                let in_origin = origin_community.is_some()
                    && memberships.get(&neighbor).copied() == origin_community;
                let next = remaining - if in_origin { 5 } else { 10 };
                if next > 0 {
                    frontier.push((neighbor, next));
                }
            }
        }
        for (id, candidate) in root_best {
            event_best
                .entry(id)
                .and_modify(|best: &mut i16| *best = (*best).max(candidate))
                .or_insert(candidate);
        }
    }
    event_best
}

fn connect(graph: &mut BTreeMap<Id, BTreeSet<Id>>, a: Id, b: Id) {
    graph.entry(a).or_default().insert(b);
    graph.entry(b).or_default().insert(a);
}

/// Pure F1 event selection: a Dream first pass can issue at most one event
/// containing only genuine published links to Memories present before the
/// new source was created. Graph rewires and withheld/no-op results are absent.
fn first_cycle_roots(
    source_was_newly_created: bool,
    source_was_previously_dream_processed: bool,
    preexisting: &BTreeSet<Id>,
    committed_first_pass_links: &[Id],
) -> Option<Vec<Id>> {
    if !source_was_newly_created || source_was_previously_dream_processed {
        return None;
    }
    let roots: BTreeSet<_> = committed_first_pass_links
        .iter()
        .copied()
        .filter(|id| preexisting.contains(id))
        .collect();
    (!roots.is_empty()).then(|| roots.into_iter().collect())
}

#[test]
fn draft_and_creation_do_not_propagate_or_decay_before_admission() {
    let mut memory = ReferenceScore::created();
    assert_eq!(memory.score, 100);
    assert_eq!(memory.admission, None);
    assert_eq!(first_cycle_roots(true, false, &BTreeSet::new(), &[]), None);
    assert!(memory.admit(825));
    assert_eq!(memory.score, 100);
    memory.settle(834);
    assert_eq!(memory.score, 100);
    assert!(!memory.admit(834)); // no re-admission on replay/revision
    memory.settle(835);
    assert_eq!(memory.score, 99);
}

#[test]
fn admission_anchor_is_dream_settlement_not_episode_origin_or_extraction() {
    // The Episode and Insomnia's first accepted extracted-Memory publication may
    // precede Dream's first settled nonarchived knowledge/canonical admission.
    // No decay debt is charged before the successfully settled Web boundary.
    let mut memory = ReferenceScore::created();
    memory.admit(800);
    memory.settle(1799);
    assert_eq!(memory.score, 1);
    memory.settle(1800);
    assert_eq!(memory.score, 0);
    assert_eq!(memory.state(), "Stale");
    memory.settle(2310);
    assert_eq!(memory.score, -51);
    assert_eq!(memory.state(), "Dormant");
    memory.settle(9999);
    assert_eq!(memory.score, -100);
    memory.settle(10099);
    assert_eq!(memory.score, -100);
}

#[test]
fn event_at_partial_decay_step_preserves_remainder_and_clamps() {
    let mut memory = ReferenceScore::created();
    memory.admit(3);
    memory.score = -60;
    memory.reinforce(25, 12);
    assert_eq!(memory.score, -35);
    assert_eq!(memory.accounted, Some(3));
    memory.settle(13);
    assert_eq!(memory.score, -36);
    assert_eq!(memory.accounted, Some(13));
    memory.reinforce(50, 13);
    assert_eq!(memory.score, 14);
    memory.reinforce(100, 13);
    assert_eq!(memory.score, 100);
    memory.score = -100;
    memory.reinforce(25, 13);
    assert_eq!(memory.score, -75);
    assert_eq!(memory.state(), "Dormant");
}

#[test]
fn score_boundaries_are_exact() {
    let mut score = ReferenceScore::created();
    for (value, state) in [
        (100, "Fresh"),
        (1, "Fresh"),
        (0, "Stale"),
        (-50, "Stale"),
        (-51, "Dormant"),
        (-100, "Dormant"),
    ] {
        score.score = value;
        assert_eq!(score.state(), state, "score {value}");
    }
}

#[test]
fn only_real_first_pass_dream_links_can_generate_roots() {
    let prior: BTreeSet<_> = [1, 2, 4].into_iter().collect();
    assert_eq!(first_cycle_roots(true, false, &prior, &[]), None);
    assert_eq!(
        first_cycle_roots(true, false, &prior, &[4, 2, 2, 9]),
        Some(vec![2, 4])
    );
    assert_eq!(first_cycle_roots(true, true, &prior, &[2]), None);
    assert_eq!(first_cycle_roots(false, false, &prior, &[1]), None);
    // A new same-batch peer 9 is not an older Memory even if Dream links it.
    assert_eq!(first_cycle_roots(true, false, &prior, &[9]), None);
}

#[test]
fn direct_access_propagates_even_when_source_remains_dormant() {
    let mut graph = BTreeMap::new();
    connect(&mut graph, 1, 2);
    let members = [(1, 7), (2, 7)].into_iter().collect();
    let mut first = ReferenceScore::created();
    first.admit(0);
    first.score = -100;
    first.reinforce(25, 0);
    assert_eq!(first.score, -75); // still Dormant, but event is +25
    let best = propagate(&graph, &members, &[1], 25, None);
    assert_eq!(best.get(&1), Some(&25));
    assert_eq!(best.get(&2), Some(&20));
}

#[test]
fn event_radius_stops_naturally_and_intermediate_score_is_irrelevant() {
    let mut graph = BTreeMap::new();
    let members: BTreeMap<_, _> = (0..=11).map(|id| (id, 1)).collect();
    for id in 0..11 {
        connect(&mut graph, id, id + 1);
    }
    let access = propagate(&graph, &members, &[0], 25, None);
    assert_eq!(access.get(&4), Some(&5));
    assert_eq!(access.get(&5), None);
    let link = propagate(&graph, &members, &[0], 50, None);
    assert_eq!(link.get(&9), Some(&5));
    assert_eq!(link.get(&10), None);
}

#[test]
fn other_communities_pay_double_until_reentry_to_original() {
    let mut graph = BTreeMap::new();
    for edge in [(1, 2), (2, 3), (3, 4), (4, 5)] {
        connect(&mut graph, edge.0, edge.1);
    }
    let members = [(1, 7), (2, 7), (3, 8), (4, 8), (5, 7)]
        .into_iter()
        .collect();
    let best = propagate(&graph, &members, &[1], 50, None);
    assert_eq!(best.get(&1), Some(&50));
    assert_eq!(best.get(&2), Some(&45));
    assert_eq!(best.get(&3), Some(&35));
    assert_eq!(best.get(&4), Some(&25));
    assert_eq!(best.get(&5), Some(&20));
}

#[test]
fn unknown_community_takes_conservative_outside_cost() {
    let mut graph = BTreeMap::new();
    connect(&mut graph, 1, 2);
    let members = [(1, 7)].into_iter().collect();
    let best = propagate(&graph, &members, &[1], 25, None);
    assert_eq!(best.get(&2), Some(&15));
}

#[test]
fn one_event_takes_strongest_path_once_across_roots_and_cycles() {
    let mut graph = BTreeMap::new();
    for (a, b) in [(1, 3), (2, 3), (1, 2), (3, 4), (4, 1)] {
        connect(&mut graph, a, b);
    }
    let members = (1..=4).map(|id| (id, 6)).collect();
    let best = propagate(&graph, &members, &[1, 2], 50, Some(9));
    assert_eq!(best.get(&1), Some(&50));
    assert_eq!(best.get(&2), Some(&50));
    assert_eq!(best.get(&3), Some(&45)); // never 45 + 45
    assert_eq!(best.get(&4), Some(&45)); // shortest branch, not 40
    assert_eq!(best.len(), 4);
}

#[test]
fn newly_created_source_is_not_reinforced_or_used_as_bridge_in_its_link_event() {
    let mut graph = BTreeMap::new();
    connect(&mut graph, 1, 9);
    connect(&mut graph, 9, 2);
    let members = [(1, 6), (2, 6), (9, 6)].into_iter().collect();
    let best = propagate(&graph, &members, &[1], 50, Some(9));
    assert_eq!(best.get(&9), None);
    assert_eq!(best.get(&2), None); // no traversal through +100 newborn
}

#[test]
fn separate_genuine_events_may_both_reinforce_the_same_memory() {
    let mut memory = ReferenceScore::created();
    memory.admit(0);
    memory.score = -100;
    memory.reinforce(25, 0);
    memory.reinforce(50, 0);
    assert_eq!(memory.score, -25);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadDisposition {
    InternalRead,
    SearchCandidate,
    PreparedButDiscarded,
    AcceptedContext(u64),
    ExplicitUserOpen(u64),
}

/// Only caller-acknowledged actual consumption qualifies; a Memory listed
/// twice within one accepted use must not yield two independent +25 events.
fn qualifying_uses(reads: &[(Id, ReadDisposition)]) -> BTreeSet<(u64, Id)> {
    reads
        .iter()
        .filter_map(|&(id, disposition)| match disposition {
            ReadDisposition::AcceptedContext(delivery)
            | ReadDisposition::ExplicitUserOpen(delivery) => Some((delivery, id)),
            _ => None,
        })
        .collect()
}

#[test]
fn low_level_reads_and_abandoned_context_do_not_fabricate_access_events() {
    let receipts = qualifying_uses(&[
        (1, ReadDisposition::InternalRead),
        (1, ReadDisposition::SearchCandidate),
        (1, ReadDisposition::PreparedButDiscarded),
        (1, ReadDisposition::AcceptedContext(82)),
        (1, ReadDisposition::AcceptedContext(82)),
        (1, ReadDisposition::ExplicitUserOpen(83)),
        (2, ReadDisposition::AcceptedContext(82)),
    ]);
    assert_eq!(receipts, [(82, 1), (82, 2), (83, 1)].into_iter().collect());
}

#[test]
fn distinct_root_communities_still_merge_to_one_strongest_event_contribution() {
    let mut graph = BTreeMap::new();
    connect(&mut graph, 1, 3);
    connect(&mut graph, 2, 3);
    let memberships = [(1, 7), (2, 8), (3, 8)].into_iter().collect();
    let best = propagate(&graph, &memberships, &[1, 2], 50, Some(9));
    assert_eq!(best.get(&3), Some(&45)); // in root 2's Community
    assert_eq!(best.get(&1), Some(&50));
    assert_eq!(best.get(&2), Some(&50));
}

/// Dream resolves physical duplicate-chain aliases through its existing
/// canonical/component authority. The reference counts logical roots, not
/// rewires or chain length; actual resolution is tested by F3 integration.
fn logical_roots(raw_roots: &[Id], resolved_canonical: &BTreeMap<Id, Id>) -> Vec<Id> {
    raw_roots
        .iter()
        .map(|id| resolved_canonical.get(id).copied().unwrap_or(*id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[test]
fn duplicate_chain_rewiring_does_not_multiply_reinforcement_roots() {
    let chain_aliases = [(4, 1), (3, 1), (2, 1)].into_iter().collect();
    let roots = logical_roots(&[4, 3, 2, 1, 4], &chain_aliases);
    assert_eq!(roots, vec![1]);
}
