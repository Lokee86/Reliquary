# Memory Freshness — deterministic graph-based lifecycle contract

Parent: [Documentation index](INDEX.md). Canonical detailed specification and implementation order: [Unified Freshness implementation plan](generic-staleness-implementation-plan.md). Source authority: [REL accepted-turn activity clock](rel-activity-clock-implementation-plan.md).

**Status:** Newly agreed design, not yet implemented. This document replaces the prior “activity staleness and provenance-qualified Memory audit” policy. Its old periodic per-Memory review cutoff, Perception/Chronos qualification gate and distinct rolling Community/access counters are **superseded**. The REL activity counter is already implemented; ongoing optional Archive checkpoint and typed-clock work is separate.

## Purpose

Define the agreed deterministic REL Memory relevance score, its numerical lifecycle boundaries and its relationship to existing owner-specific semantic processing. This supersedes the previous generic Memory-reconsideration deadline and mandatory Perception-audit policy.

## Overview

Each new Memory starts at +100 on a -100..+100 scale; **creation emits no propagation because the new Memory has no connections**. Decay begins once the Memory has entered the Memory Web, at its recorded admission REL turn, losing one point per ten subsequent accepted owner-REL turns. Direct accesses and genuinely newly established Dream links introduce bounded propagation events across the existing Dream graph; the originating Community remains the reference for reduced cross-Community reinforcement. All scoring is deterministic and lazy, including lifecycle transitions at zero and below -50. Perception and Chronos retain independent semantic responsibilities; Ego can use the score for contextual selection.

## Goal and separation of concerns

Memory Freshness measures **contextual relevance** inside each REL Memory Web. It is not an independent model assessment of factual truth, nor proof that a Memory's underlying real-world claim remains valid. It keeps old information from monopolizing Ego context unless continued access, newly created connected Memories or duplicate recurrence demonstrate attention. Freshness is deterministic; Dream link/duplicate classifications and qualifying user/agent accesses may originate in probabilistic processes. Insomnia Memory creation only initializes the local score; actual Web admission starts decay; creation itself is not a reinforcement input.

Dream owns graph topology, duplicate components, canonical lifecycle and separate archive/supersession decisions. Freshness consumes committed graph decisions without adding link strengths or controlling Dream. Perception may eventually provide optional evidence through a separately defined event, but is **not required to refresh** Memories and does not conduct a mandatory stale-Memory audit. Chronos governs semantic/event time where needed by those other systems; it is **not** the Freshness clock. The processing coordinator's Episode barriers and processor cooldowns remain independent.

## Score and derived state

| Parameter | Initial policy |
| --- | ---: |
| Score limits | `-100..+100`, inclusive |
| New Memory creation | Initialize `+100`; **no propagation** |
| Memory-Web admission | Begin the decay clock from the authoritative admission REL turn; admission alone does not propagate |
| Fresh | `+1..+100` |
| Stale | `-50..0` |
| Dormant | `-100..-51` |
| Natural decay | `-1` per 10 first-accepted REL user/assistant turns |
| Qualifying direct access | `+25` |
| Actual first-cycle Dream linkage | `+50` to existing connected information, once per committed first-cycle relationship event; **no link means no event** |
| Within-origin-Community hop | `-5` from remaining event strength |
| Hop outside originating Community | `-10` from remaining event strength |
| Further event propagation | Stop if next increase is `<=0` |
| Reinforcement multiplicity | Once per Memory per originating event; strongest path wins |

These values are **initial configuration**, subject to measured tuning. The local and outside deductions happen per hop, as *constant units based on the event principal*, not multiplicative percentages. The originating Community remains the reference even after the path enters a second or third Community. Within another Community still means **outside the original Community** and costs double; returning to the original Community permits ordinary hop deductions again. Example from a `+50` linkage: `+50 -> +45` (same origin), then `+35 -> +25 -> +15 -> +5` outside.

A newly created Memory is `+100` on creation and initially has no connections, so nothing propagates. **The identified ordinary Web-admission boundary is the first successful, durably settled Dream lifecycle promotion of an extracted Memory to nonarchived `knowledge` or `canonical`.** First accepted Insomnia/MemoryStore publication only initializes +100: this Memory is searchable, but its decay clock is still inactive. It begins decaying on first settled Dream processing even if there are **no** links; a direct first Memory publication already explicitly in active `knowledge`/`canonical` may admit immediately. The Graph's physical Memory projection registers relation endpoints, not every stored Memory, so a first edge may precede settlement and an isolated settled Memory may lack a Graph-projection node. Neither is the general admission signal. A new duplicate archived by Dream can reinforce an older canonical Memory without being admitted itself. A prepared draft, later retry or revision is not admission. Only after Dream actually creates a first-cycle relationship can connected existing information receive `+50` and propagate it further. The `+50` is **not** another bonus added to the new Memory. A repeated accepted new duplicate must reinforce the **existing active canonical representative** identified through Dream's established ordered duplicate-component machinery; duplicate-chain physical length is not a measure of information distance. Mere chain reorganization cannot generate a Freshness event. A subsequently generated independent duplicate is another real originating event.

**Single-event rule:** candidate increases received at graph nodes during propagation are score updates, *never fresh originating events*. If a node is reached through multiple paths, take the highest candidate and apply it once, not the sum. Separate qualifying access and actual new-linkage events remain cumulative; Memory creation alone is not an event. Looping or reentering a node does not manufacture extra reinforcement.

## Decay and processing

The sole REL Freshness clock is the authoritative REL accepted-turn position; no wall-time aging, other-REL activity clock, independent access tick or all-Web per-message loop. Store score plus the last **fully accounted decay-step position**. On a score read or event, compute `floor((current_turn - accounted_turn) / 10)`, subtract that many points, clamp to `-100`, and advance the stored accounting position by exactly the number of completed decay steps. **Preserve leftover turns** when an access or new linkage occurs. For an unreinforced `+100` Memory, zero occurs 1,000 accepted REL turns **after Web admission**, and the first Dormant score `-51` at 1,510 subsequent turns.

Use an owner-local, derived ordered index of future score-crossing positions to dispatch due lifecycle transitions without scanning or rewriting every Memory for each incoming turn. Lazy score reads must always produce the correct state even if a due-dispatch job has yet to run. Dormant Memories continue down to `-100` and are still retrievable and eligible for positive reinforcement; they are not archived or deleted by this counter. A direct `+25` applied to `-100` gives `-75`, which remains Dormant; no automatic full-score reset.

No numerical link weights are required: Dream currently provides typed relationships, not general strengths. A propagation event traverses the existing graph with binary edges, positive remaining event strength, fixed Community membership context and strongest-path aggregation. Newly created nodes absent from the current derived Leiden snapshot use a conservative documented membership fallback, without initiating a full Leiden rebuild for each event.

## Concurrency and event authority

Start with a bounded worker pool; a dense Community could place many nodes within the natural positive-reinforcement radius. Parallel workers evaluate neighbor candidates, and descending-strength frontier barriers finalize a Memory only when no stronger path can still arrive. An event has one scoped best-candidate map. Graph and Community snapshots are pinned for the event; commits apply events in canonical REL activity/event-ID order, independent of worker completion order. Several **independent** events may legitimately reinforce the same Memory and must all count; paths of the **same** event cannot stack.

Persist event identities and complete effect batches under the existing owner/Container durability boundary so retries and crashes do not duplicate or partially apply a propagation. Dream's initial lifecycle reconciliation distinguishes extracted-to-active admission from later reprocessing; the long-lived host also persists a processed epoch, whereas direct DreamProcessor calls do not currently do so. Initial `Published` links must be grouped into **one event per newly created Memory's first settled Dream pass**, with at most +50 for each pre-existing linked Memory and maximum, not summation, downstream. A retry after partial Graph publication may see `NoChange` for already-committed initial links, so F3/F5 must preserve or reconstruct the complete first-cycle link set before event publication. A duplicate's canonical representative is selected through Dream and raw chain rewires are not separate events. Creation precedes ordinary Web admission. Some provisional initial Graph links may precede successful Dream settlement; **do not queue a hypothetical propagation event at creation**. At first accepted Dream settlement, anchor decay for an active new Memory and issue reinforcement only for genuine initial relationships to existing Memories, using durable initial-pass identity and recoverable link history. A completed zero-link Dream pass admits the active Memory but generates no reinforcement. Later rewiring never retroactively creates additional first-cycle events.

The only ordinary Freshness effect of direct access is its specified local bonus and one corresponding graph event; do not introduce new semantic model calls. A directly accessed negative-scoring Memory receives its local `+25` regardless of whether the result crosses a lifecycle boundary. **Event propagation depends on remaining event strength, not the current score of its source or recipients.** A +25 accepted direct access may propagate even if the accessed Memory stays Dormant; a negative recipient may relay that same diminishing event without originating a second event. A raw `MemoryStore` read, internal search hit, Dream candidate read or truncated search result is *not* access: emit a +25 event only from explicit, accepted consumption of Memory content in an actual delivered context or deliberate user use, with a stable owner/use identity to deduplicate retries and repeated appearances in one delivery. The current runtime lacks a universal accepted-context receipt, so F3 adds a narrow producer seam instead of guessing from reads.

## Ego/context and owner boundaries

Ego may use the score to rank/shape the routine working Memory Web. Freshness does **not** authorize removal of historical Memory records, global exclusion from search, elision of mandatory broad audit, bypass of explicit protected keep lanes, or a substitute for actual contradiction/supersession handling. When the whole Web fits economically in one batch, include it; don't impose artificial pruning merely because some scores are low. If scale requires selection, Freshness is a graded input alongside established coverage, graph routing and hard keeps, not an automatic deletion filter.

REL and PHY are distinct owners. The REL accepted-turn clock cannot simply be shared with a PHY or transferred from one project's activity into another. This design initially covers REL-local Memories. PHY scoring requires an independently justified owner-local source and explicit separate policy, not assumed wall time or indiscriminate Memory-version ticks.

## Verification and calibration

The authoritative source history and grouped Insomnia completion format distinguish **new Memories** from later semantic revisions. Measured first-cycle REL generation:

| Fixture | Accepted REL turns | Completed episodes | New REL Memories | Rate per 100 turns |
| --- | ---: | ---: | ---: | ---: |
| User first 14 days | 5,857 | 374 | 1,087 | 18.6 |
| User first 28 days | 16,465 | 1,211 | 2,816 | 17.1 |
| Ellis | 2,972 | 128 | 678 | 22.8 |

In the user's available historic **14-day stored Leiden snapshot** (12 Communities, 1,084 of 1,087 newly created Memories mapped), 56% of retrospective overlapping 100-turn Community windows have no new Memory and 14.5% have at least four. The snapshot is retrospective and **does not** identify the contemporaneous community at each earlier turn. The 28-day stored snapshot is not current to its full graph; Ellis lacks a stored snapshot. These measurements justify testing locality and event frequency, **not** bringing back a rolling Community count threshold.

The existing source path was inspected for F1: grouped/direct first Memory publication creates local +100 state, while the first successfully settled Dream lifecycle admission to `knowledge`/`canonical` starts decay for ordinary extracted Memories, including isolated ones. Direct first publication already explicitly active can admit immediately. Dream has distinct `Published`, `NoChange` and `Withheld` outcomes, and host retrieval hydrates Memories for internal search before final selection. The [F1 reference tests](../tests/freshness_f1_reference.rs) encode the pure mathematical contract without claiming production integration. Implementation and acceptance tests belong to [the canonical detailed plan](generic-staleness-implementation-plan.md). Required cases include creation-with-no-propagation, authoritative Web-admission decay anchors, isolated Web Memories, actual Dream linkage events only, score and remainder boundaries, one event reaching one node via multiple paths, cycles, Community exits and reentries, duplicate-component reinforcement, first-cycle-only triggering, failed and retried event settlement, deterministic concurrent traversal, crash recovery, sparse due-transition indexing and a Perception-disabled Ego path.

**Rejected legacy policy:** The former target/source-cut `MemoryReconsideration` schedule, 500-turn age threshold, mandatory stale-Memory Perception evidence verification, Chronos-gated reinforcement, separate Community/access counters and four-in-100 reactivation rule are not alternative current designs. Generic `Fixed`/`ProportionalCapped` clock math remains potentially useful for **Ego summary refresh scheduling**, which is separate from per-Memory Freshness.

## Related docs

- [Unified Freshness implementation plan](generic-staleness-implementation-plan.md)
- [REL activity clock](rel-activity-clock-implementation-plan.md)
- [Ego Memory-Web summary plan](ego-web-synthesis-plan.md)
- [Dream design](dream-implementation-plan.md)
- [Perception subsystem plan](perception-subsystem-plan.md)
- [Chronos subsystem plan](chronos-subsystem-plan.md)
- [Roadmap](roadmap.md)

## Notes

This document describes an accepted design, not shipped implementation. The detailed plan's F1 verification gate will freeze edge cases such as propagation through nonpositive intermediate recipients before production integration; initial numerical settings remain calibratable.