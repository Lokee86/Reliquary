# Memory Freshness

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Define the implemented deterministic Memory relevance engine, including score semantics, REL activity decay, accepted-use and Dream events, semantic reinforcement, replay/recovery, and explicit historical reconstruction.

## Overview

Freshness is an owner-local relevance/attention signal. It is not truth, validity, semantic confidence, archival state, or deletion authority.

Each REL Memory has one bounded signed score. The score changes through deterministic REL activity decay and explicitly authorized events over existing Memory graph topology. Perception and Chronos do not gate ordinary Freshness transitions.

## Score and state

| Parameter | Current contract |
| --- | ---: |
| Score range | `-100..+100` |
| New Memory publication | initialize `+100`; no propagation |
| Fresh | `+1..+100` |
| Stale | `-50..0` |
| Dormant | `-100..-51` |
| Natural decay | `-1` per 10 subsequent accepted owner-REL turns |
| Accepted Memory use | `+25` principal |
| First-cycle Dream linkage | `+50` principal toward existing connected information |
| Same-origin-Community hop cost | 5 |
| Outside-origin-Community hop cost | 10 |
| Event multiplicity | one update per Memory per event; strongest path wins |

Creation and Memory-Web admission do not themselves propagate.

## Admission and decay

First accepted Memory publication creates local +100 state. For ordinary extracted Memories, decay begins at the first successfully settled Dream lifecycle promotion to active `knowledge`/`canonical`, including zero-link settlement. A direct first publication already explicitly active may admit immediately.

The only REL Freshness clock is the Archive's accepted-turn activity position. Wall time, other RELs, Memory revisions, and model calls are not decay clocks.

Lazy decay preserves sub-ten-turn remainder:

```text
steps = floor((current_turn - accounted_turn) / 10)
score = max(-100, stored_score - steps)
accounted_turn += 10 * steps
```

The derived boundary index schedules score-state crossings without rewriting every Memory each turn.

## Accepted-use events

A qualifying accepted use is a confirmed delivery or deliberate consumption of Memory content. Raw reads, search hits, candidate metadata, graph inspection, internal hydration, and truncated results do not reinforce.

The durable event is published through the accepted-use receipt boundary and is idempotent under retry.

## Dream linkage events

A genuine first-cycle Dream relationship may reinforce existing connected information with a +50 principal. A new duplicate reinforces its existing active/canonical representative under the same event semantics.

Retries, later graph rewiring, already-existing links, component-chain changes, and Memory creation do not fabricate linkage events.

## Propagation

Freshness projects active Memory-to-Memory Graph relations into a read-only deduplicated undirected adjacency. Graph semantic direction is unchanged.

Propagation uses the event's remaining strength, not recipient score. A negative/Dormant intermediate can relay the same event while remaining strength is positive.

The event root's originating Community remains the locality reference for the full path. Leaving it increases hop cost to 10; entering another Community does not reset locality. Re-entering the original Community restores the ordinary cost.

Multiple paths to the same Memory collapse to the highest candidate. Cycles cannot stack reinforcement.

## Concurrency and persistence

Propagation is bounded and deterministic. Parallel expansion may evaluate same-strength work concurrently, but commit semantics are independent of scheduler completion order.

Freshness persists event identity and complete effects under the owner/container durability boundary. Crash/reopen and exact retry cannot partially or multiply apply an event.

## Live semantic reinforcement

Accepted source fragments may locally reinforce already-existing compatible Memories when semantic similarity satisfies the frozen v1 policy. Current v1 uses cosine threshold `>= 0.75`, a monotone +1..+10 local effect curve, and a top-64 candidate bound.

Normal live semantic reinforcement is local and non-propagating. The source Memory cohort/provenance is excluded from self-reinforcement. Re-embedding cannot manufacture a second award for the same semantic event.

The exact policy, identity, and calibration details remain in the live semantic contract/calibration documents.

## Historical reconstruction

Historical Freshness is an explicit offline/exclusive REL operation, not default live behavior. It reconstructs provenance-timed semantic/Dream effects over a fixed completed graph and applies chronological lazy scoring under a versioned policy.

Historical import semantic reinforcement is the deliberate exception that may propagate. Future/ineligible nodes may serve as authorized traversal bridges but do not receive credit before eligibility.

The current supported release is explicit opt-in REL reconstruction. PHY and mounted historical management remain outside that release.

## Owner boundaries

### Store ownership

`reliquary-freshness-store` owns score and policy values, fixed-byte owner
identities, durable Freshness records, accepted receipts, semantic/import
settlements, rewrite baselines, and replay. It validates supplied effects and
persists/replays owner facts; graph traversal, recipient selection, semantic
matching, historical event construction, and cross-owner authorization remain
in higher-level services and facades. Container owns physical frames and write
admission.

Source ownership and product integration are complete. The separate CVAFRS07
recovery and authoritative six-file corpus gate remain unverified; see the
[extraction specification](../freshness-store-crate-extraction-spec-2026-10-26.md)
and [implementation evidence](../freshness-store-crate-extraction-evidence-2026-10-26.md).

Freshness does not:

- mutate Memory bodies or Dream lifecycle;
- create Graph relationships;
- decide duplicates;
- infer accepted use from transcript text;
- use Chronos as its activity clock;
- make Perception truth/relevance judgments; or
- archive/delete Memories merely because the score is low.

PHY does not inherit the REL activity clock. Any PHY Freshness policy requires an independently justified owner-local activity source.

## Performance

The retained implementation uses bounded grouped propagation, conservative batch scheduling, bounded receipt residency/history structures, and measured hot-path optimizations. Performance work did not introduce a graph cache or change v1 relevance semantics where measurements did not justify it. The extraction currently omits the lower-store trace rows `freshness.payload.shape` and `freshness.history.scan`; root-stage traces remain available, but those two lower-stage measurements are not emitted by the new crate.

## Code Map

- `src/freshness/**`
- `crates/reliquary-freshness-store/src/`
- `src/freshness_due.rs`
- `src/freshness_dream_journal.rs`
- `src/runtime_inference/freshness_access.rs`
- `src/facade/*freshness*`

## Related docs

- [REL activity clock](rel-activity-clock.md)
- [Dream](dream.md)
- [Dream Store extraction boundary](../dream-store-crate-extraction-spec-2026-10-27.md)
- [Agentic Memory retrieval](agentic-memory-retrieval.md)
- [Live semantic Freshness contract](../freshness-live-semantic-policy-contract.md)
- [Historical manifest/timeline policy](../freshness-historical-manifest-policy.md)
- [Historical production disposition](../freshness-historical-production-disposition-2026-10-13.md)
- [Performance verification](../freshness-performance-verification-2026-10-03.md)
- [ADR 0039](../decisions/0039-unified-deterministic-memory-freshness.md)
- [Behavioral contracts](../behavioral-contracts.md)

## Notes

The former generic-staleness, Memory-staleness, completion, performance, and historical execution plans are retired. Verification records remain as evidence; this document and the specialized current contracts own behavior.
