# ADR 0039: Unified deterministic Memory Freshness

Parent: [Architectural decisions](INDEX.md)

Implementation contract: [Unified Memory Freshness plan](../generic-staleness-implementation-plan.md). Policy summary: [Memory Freshness](../memory-staleness-plan.md).

## Status

**Accepted design — 2026-10-03.** Implementation pending. Amends ADR 0033 by withdrawing its mandatory stale-Memory Perception/Chronos audit proposal. The existing Phase 1 REL activity-clock implementation remains authoritative. No direct changes to existing Observation, Chronos, Dream relationship ownership or Ego summary-cadence contracts.

## Context

Periodic per-Memory review deadlines, separate Community and access windows and a semantic reinforcement audit proved unnecessarily complex for the actual objective: prioritizing a finite context around subjects receiving recent attention. REL accepted turns are the one canonical measure of conversation progress. Insomnia/Dream already generate the Memories, relationships, duplicate components and derived Leiden Communities needed to interpret *where* that activity is concentrated.

Freshness may decline even while a fact remains true. It must not be confused with semantic contradiction, valid time, archival or model confidence.

## Decision

- One bounded signed -100..+100 integer Freshness score per Memory; new Memories start at +100. Derived states: Fresh +1..+100, Stale -50..0, Dormant -100..-51. No separate mutable lifecycle truth.
- Creation initializes a Memory at +100 with no propagation: it initially has no connections. First accepted MemoryStore publication creates local +100 state without starting decay for newly `extracted` Memories. **Ordinary Memory-Web admission is the first successful settled Dream lifecycle promotion to nonarchived `knowledge` or `canonical`, even if no relationships are found**. A newly direct-published Memory explicitly already in an active state may admit at first publication. The Graph's physical Memory projection records relation endpoints only, so first edge is not a universal admission boundary; an edge may exist before final settlement and isolated settled Memories may lack Graph nodes. Prepared drafts, no-change replay, later reprocessing and revisions do not reset admission. Lazy decay thereafter loses one point per ten accepted owner-REL turns, preserving the sub-10-turn remainder and continuing to -100. No wall-time aging or all-Web per-message score rewriting.
- Events: qualifying direct access adds +25; only **actual committed first-cycle Dream relationships** add +50 to existing linked information, including an established Dream-resolved duplicate canonical representative. No relationship means no linkage propagation event. Scoring, event ordering and propagation are deterministic even though event-producing models are probabilistic.
- Use binary Dream graph relationships, hop distance and one fixed **originating Leiden Community**. Normal per-hop reduction is 5; **any hop outside the originating Community** costs 10, including subsequent hops inside other Communities. Do not invent generic numerical Dream link strengths or an arbitrary distance cap; stop when the next candidate is nonpositive.
- Each Memory can be reinforced **once per originating event**, using the strongest eligible graph path, never the sum of several paths. Received score increases do not generate new events; independent later real events are cumulative. A Dream duplicate-chain rewire alone is not an event.
- Build bounded concurrent propagation from the outset, with descending remaining-reinforcement frontier barriers and deterministic commits/ordering. Use idempotent owner-local event identity and crash-safe multi-node effect publication. Derive an indexed next-lifecycle threshold from score and the existing REL activity clock.
- Ego consumes graded Freshness for routine context selection without deleting Memories or violating the established full-Web-if-economical, mandatory broad audit and protected hard-keep rules. Freshness does not require Perception or Chronos verification. Summary regeneration source-progress scheduling is **separate**.

## Superseded proposal and boundaries

This supersedes per-Memory `MemoryReconsideration` registration/`next_review_at` reviews, 500-turn elapsed review gating, four-new-Memories-in-100-turn reactivation, separate access/Community decay counters and mandatory evidence-time Perception audit for relevance recovery. Generic typed source-progress comparison already implemented for future Ego summary scheduling is retained as an independent utility; optional Archive counter materialization remains an independently evaluated optimization.

Current policy is REL-local. PHY has no Archive REL turn clock and must not inherit another REL's clock, wall time or raw PHY Memory versions by accident. The F1 source-inspected contract allows a nonpositive source/recipient to pass along an otherwise positive **single originating event**; remaining event strength alone determines propagation. +25 access requires an explicit accepted context/use receipt, never an automatic low-level read or search hit. Initial Dream settlement yields at most one multi-root event per newly created Memory, based on genuine first-cycle links to pre-existing Memories, including initial committed links preserved across partial-attempt retries; no node update or raw duplicate-chain rewire may create another event.

## Consequences and verification

- An unreinforced +100 Memory reaches 0 after 1,000 accepted **post-admission** REL turns and first Dormant score -51 after 1,510.
- Duplicate-component canonical target selection must follow Dream's existing active/canonical ownership, not assume an arbitrary chain endpoint.
- Persistence must distinguish Freshness metadata/event results from semantic Memory revisions and preserve exact accepted history across reopen and physical transformations.
- Tests must cover stronger alternate paths, arbitrary cycles, originating-Community exit/re-entry, long duplicate chains, negative/positive score transitions, concurrent interleavings, retries and crash safety; measure dense Community reachability before adding optimization beyond natural positive-score stopping.
