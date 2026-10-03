# Unified Memory Freshness and retained source-progress infrastructure — implementation plan

Parent: [Documentation index](INDEX.md). Companion policy: [Memory Freshness contract](memory-staleness-plan.md). Existing foundation: [REL activity clock Phase 1](rel-activity-clock-implementation-plan.md).

**Status (2026-10-03):** The prior per-Memory generic-review/Perception-revalidation design is **superseded**. This is the normative implementation sequence for deterministic Memory Freshness. Phase 1's accepted-REL-turn counter exists. Prior steps 2.1/2.2 (optional Archive counter materialization) may proceed independently if still justified; step 2.3's pure typed-clock evaluator exists with incomplete integration verification. **Old steps 2.4–2.7 have been withdrawn and must not be implemented as previously written.** Freshness itself is not implemented by this document. Recheck the working tree and concurrent branches before starting any phase: unrelated source and documentation modifications are present.

## Purpose

Implement one deterministic graph-based Memory Freshness system that controls context priority and lifecycle state using the existing owner-REL accepted-turn clock, access events and newly settled Memory links. Explicitly retire the former per-Memory periodic semantic-audit architecture without throwing away independently useful clock/checkpoint work or Ego summary scheduling.

## Overview

The design has one bounded signed score per REL Memory, deterministic lazy decay, fixed principal-based graph-hop reductions, originating-Community penalties, Dream-owned duplicate-chain reinforcement, event-scoped strongest-path aggregation and bounded concurrent traversal. A sparse due-boundary index avoids rewriting the full Web on every turn. Recovery retains event identity and atomic multi-Memory effects. PHY needs its own separately justified source; Perception/Chronos do not gate Freshness.

## 1. Architectural decision and migration from the old plan

Memory staleness is no longer a periodic model assessment, a per-Memory `next_review_at` schedule, a rolling Community density counter, or a Perception-certified support window. It is **one bounded signed Freshness counter per Memory**, driven by accepted activity and events on the *existing* Memory graph. Inputs can originate in probabilistic Insomnia/Dream decisions, but scoring, propagation, decay, lifecycle boundaries, scheduling and replay are deterministic.

Freshness is a measure of **contextual relevance/attention**, not an assertion that a proposition is true, still valid in the world, or semantically verified. This machinery is intentionally also a context-control mechanism: even a factually correct, unaccessed Memory becomes lower priority. It neither archives, deletes, supersedes nor rewrites a Memory body. Dream continues to own relationships, duplicate canonicalization and their independently decided lifecycle; Chronos and Perception retain their own semantic responsibilities but are **not prerequisites for Freshness transitions**. Ego reads Freshness as a selection/ranking signal while retaining its independent coverage and hard-keep obligations.

The following old obligations are **removed for Memories**:

- `MemoryReconsideration` registrations, per-Memory review deadlines, periodic stale-audit scheduling and “successful no-reinforcement review postpones deadline.”
- Mandatory Perception evidence verification or Chronos event-time windows before Freshness increases, as well as the obsolete 100-turn Community count/revival rule and separate access/Community activity counters.
- A “500 REL turns since last reinforcement” state boundary; the configured score decay, not a fixed review age, now determines state.
- Any requirement to construct numerical Dream relationship strengths, invent a new recency graph, trigger model calls to advance Freshness, or scan all Memories per REL turn.

**Retained but separate:** the REL clock and its optional recovery/checkpoint work, `ProcessingEpochStore` cooldowns/readiness, and source-progress math useful for REL/PHY Ego *summary regeneration*. A generic durable summary review store may still be justified by Ego or future Observation requirements, but it is **not** the Memory Freshness owner, and its absent implementation must not block Freshness. Do not quietly revive the withdrawn generic all-target schedule under new names.

## 2. Exact Freshness mechanics

### 2.1 Counter and lifecycle

For each owner-local Memory, creation initializes the score at `+100` but **creates no Freshness propagation event**: new Memories initially have no graph connections. Its decay clock starts only at its authoritative **Memory-Web admission** boundary, not earlier during extraction/drafting. **F1 identified two distinct boundaries:** first accepted owner-MemoryStore publication creates local +100 state without propagation, while successful initial Dream lifecycle promotion to nonarchived `knowledge`/`canonical` admits ordinary extracted Memories for decay (including isolated Memories with no Graph edge). A direct first Memory publication already explicitly in such an active state can admit immediately. Graph projection node creation requires relations and is not a universal admission signal. A new duplicate archived on Dream settlement reinforces its existing peer but is not enrolled as an ordinary active Web Memory. Retries, revisions and reprocessing never reanchor admission. Legal scores are integers in `[-100,+100]`. The current state is **derived**, not an independently mutable second authority:

| Current score | Derived state |
| --- | --- |
| `+1..+100` | Fresh |
| `-50..0` | Stale |
| `-100..-51` | Dormant |

Transitions occur in either direction whenever the materialized/current score crosses these boundaries. `0` is already Stale; `-50` is still Stale; `-51` is the first Dormant score. A Dormant Memory continues to decay down to `-100`; reinforcement must earn its way back through the same numerical boundaries. Being Dormant does not prevent direct access, graph reinforcement or explicit retrieval. Score transitions never change Dream's `lifecycle_state`, `archived` or `superseded_by` fields.

### 2.2 Only one clock; exact lazy decay

REL activity is the existing authoritative Archive position for each *first-accepted* logical `user`/`assistant` turn (both Archive acceptance paths). A different REL's turns and elapsed wall time have no effect. There is no second continuously incrementing Freshness clock.

**Tentative default:** starting from **Memory-Web admission**, lose 1 score point per 10 subsequent accepted owner REL turns, down to `-100`. Set `accounted_turn` to the authoritative admission REL activity position, and never charge time spent outside the Web. Store a score materialized at `accounted_turn` (the last decay-step boundary) rather than advancing an independent periodic timer:

```text
steps = floor((current_owner_rel_turn - accounted_turn) / 10)
current_score = max(-100, stored_score - steps)
new_accounted_turn = accounted_turn + 10 * steps
```

On event application, first settle decay through the event's authoritative activity position, then add its reinforcement and clamp to `+100`. Preserve the sub-10-turn remainder in `accounted_turn` even when an event occurs; resetting that remainder on access would manufacture extra lifetime. At saturation `-100`, reanchor safely when materializing so the score can react correctly to a later reinforcement and no historical decay debt can be applied twice. All arithmetic and positions are checked integers.

An unreinforced `+100` Memory reaches `0` 1,000 REL turns **after Web admission** and reaches `-51` 1,510 turns **after Web admission**; these supersede the old proposed 500-turn reconsideration number. These are *REL activity*, not calendar-day estimates. Use the Memory owner's REL clock for REL Memories. **PHY does not own a REL turn stream**: a PHY-specific Freshness source must be explicitly designed/authorized instead of silently treating PHY Memory-version changes or wall time as equivalent. This first rollout targets REL Memories; no false global user-wide turn counter.

For a Memory materialized after turn `t`, compute the next zero or `-51` crossing from the stored score and retained remainder. Maintain a derived, owner-local ordered next-boundary index for due transitions. It may be rebuilt on reopen from stored entries; do not persist redundant state labels as truth. A turn advances Archive only, not all Memory records; due processing visits only crossed thresholds. A lazy score read must reflect the correct state even if the threshold dispatcher has not run yet.

### 2.3 Originating events and numerical defaults

| Event | Local effect | Propagation decrement per hop | Outside originating Community |
| --- | ---: | ---: | ---: |
| Qualifying direct Memory access | `+25` to accessed Memory | `-5` (20% of event principal) | `-10` per outside hop |
| New Memory creation | Initialize only that Memory to `+100`; no connections exist yet | **No event and no propagation** | Not applicable |
| Memory-Web admission | Start that Memory's decay at the admission REL turn | **No propagation merely from admission** | Not applicable |
| Actual first-cycle link established by Dream | `+50` to the linked existing Memory, then graph propagation from existing connected information | `-5` (10% of 50) per onward hop | `-10` per outside hop |
| First-cycle new duplicate recognized by Dream | Reinforce its existing active/canonical representative through the `+50` new-linkage event | Same new-linkage rules | Same outside rules |

Reductions are **fixed amounts derived from the principal event**, *not* multiplying the remaining score by 80%/90% at each hop. Examples: an access event can offer `+25,+20,+15,+10,+5` along successive wholly local positions; a new linkage offers `+50,+45,+40...` from the newly linked existing Memory onward. Traversal ends when the next candidate increase is `<=0`. The `+50` arises **only after a real Dream relationship has been established**, reinforcing the linked existing Memory; it is **not** a second `+50` added to the new Memory's initial `+100`. A newly created unlinked Memory does not initiate a graph traversal.

One accepted originating event is identified and applied **once**; receiving any positive propagated score never creates another event. A genuine later access or a genuinely new **established first-cycle relationship** is a new event. Creation and Web admission never originate propagation: the new Memory has no connected graph to traverse at creation. Historical revision, duplicate retry, link rewiring, already-existing graph edges or a processor retry do not fabricate new originating activity. First-cycle linkage events start at **existing Memories that Dream has actually connected**, not at the unlinked new Memory; direct access raises its own source by `+25` and is still locally effective even if that Memory remains Dormant. **F1 propagation contract:** the event's remaining positive reinforcement, not any recipient's current stored score, controls traversal. A Dormant or still-negative intermediate Memory can relay the **same originating event** as long as its remaining propagation amount stays positive; the recipient's score update does not originate a new event. Even if direct +25 access leaves its source below zero, its +25 event still traverses eligible edges. Never gate propagation on a Memory's post-increase score.

### 2.4 Distance and originating-Community locality

Use existing active **Memory-to-Memory** Graph relations as binary connections. For relevance propagation, project them into a deduplicated **undirected adjacency** regardless of the relation's semantic direction; this does not change Dream's directional meaning. The current `GraphDirection` API exposes only `Incoming`/`Outgoing`, so construct this read-only adjacency from a version-pinned `active_relations()` view (or add a narrow equivalent later), **not** a nonexistent `Both` direction or repeated global scans per worker. Exclude Entity/Principal association edges. Dream classifies relationship kinds but does **not** persist generic numerical connection strengths; no speculative edge weights. Link **distance** and Community membership are the propagation inputs.

The **originating Leiden Community stays fixed for each event root's path search**; a multi-root first-link event may have distinct root Communities, but all candidate paths still merge into one maximum per physical Memory for that single event. A hop inside its root's originating Community subtracts the normal 5 points. A hop entering or continuing **outside the originating Community** subtracts **10 points**; being within some *other* Community never resets the penalty. The per-hop cost is based on the destination relative to the fixed originating Community. If a path re-enters the originating Community, subsequent in-Community hops again use 5 points. Example: `+50 -> +45` locally, then `+35 -> +25 -> +15 -> +5` outside, then stop if the next cost exhausts the principal. This replaces the earlier incorrect “only the boundary-crossing edge costs double” interpretation.

Use the current *available, validated* Leiden snapshot as a derived membership index. An ordinary Freshness event does not trigger Leiden recomputation. Handle newly created nodes not yet included in that snapshot explicitly: root on the established existing linked Memory's Community where possible, otherwise treat uncertain paths conservatively with the outside penalty until current membership exists. A duplicate component should resolve its representative's Community; never claim a stale snapshot exactly represents Community membership at every historic graph revision. Snapshot rebuild does not itself emit Freshness events or retroactively replay existing ones.

**Single-event idempotence:** a Memory receives at most **one** reinforcement per event. If several graph paths reach it, select the **highest remaining candidate** for that event and apply it once, not their sum. Do not permit cycles, duplicate links, multiple initial linkage paths or duplicate components to inflate reinforcement. Equal candidate ties resolve by stable Memory ID for reproducibility. Do not impose an extra artificial hop cap: positive reinforcement naturally bounds the path; a single Community can still have substantial branching, so measure actual edge visits.

### 2.5 Duplicate-chain semantics

Reuse Dream's existing `DuplicateOf` component machinery. Dream currently merges duplicate components, orders members by source chronology and Memory ID, and rewires a linear duplicate chain. Its lifecycle logic selects canonical/active state; Freshness **must not** implement a parallel duplicate classifier or derive “canonical” merely from which end of a chain is newest.

When a first-cycle Memory is recognized as a duplicate and Dream settles that classification, the **existing active canonical representative** is the reinforcement target; raw physical chain rewires and aliases to that same existing component must not generate more than one +50 root. Begin propagation at its representative through ordinary eligible existing graph neighbors; do **not** invent zero-cost traversal through every physical duplicate-chain link or duplicate-path bonus unless F3 establishes a separately approved rule. This avoids making chain length an extra source-event count. The new duplicate retains its own initial score subject to Dream's independent archival decision; archived duplicates do not become extra ordinary Ego context candidates simply because their numeric score is high.

Represent a genuinely established **first-cycle linkage** as one logical event even if Dream settles multiple links or reorders its duplicate component. No creation event exists to double-count. If Dream has not yet settled links, **do not schedule speculative propagation or create a pending propagation event**; rely on the existing durable Dream work/settlement path and emit Freshness reinforcement only for actual committed relationships. A terminal no-link outcome creates no reinforcement. A chain merge or intermediary insertion among existing Memories must **not** generate freshness without an independent new Memory or access event. Define the canonical-target and component-alias lookup against current Dream owner APIs and regression-test existing owner precedence. Duplicate components spanning snapshots must use the representative's validated membership or the conservative unknown-Community rule, not accidentally lower the outside penalty.

### 2.6 Deterministic parallel propagation

Concurrency is required **from the initial implementation**, not an afterthought. Use a bounded worker pool to parallelize neighbor expansion and independent originating-event calculations; avoid a task or lock per visited edge/node. Commit each event's effects deterministically.

For an individual event, run a **highest-remaining-reinforcement frontier**. The reductions are positive integers; process descending remaining-score buckets with a **barrier between levels**. Workers may expand same-level nodes concurrently, accumulate candidate maxima for neighboring nodes, and merge candidates by `max`; only finalize a node when no higher bucket remains. This guarantees the strongest path wins and once-per-event reinforcement regardless of scheduling. Do not finalize a weakly reached node while another worker may still discover a stronger path. Use one visited/best-candidate structure scoped to the event, not an all-graph per-node reset. Expanded/candidate candidates at `<=0` are discarded. Stable ordering is required only at commit and exact tie points, not throughout independent parallel reads.

Multiple independent events may compute concurrently against a **version-pinned graph/Community snapshot**. Apply committed effects in one documented canonical event order: REL acceptance/source position, then stable unique event ID. When event effects conflict or an intervening graph version changes a pending event's valid topology, revalidate/recompute against that event's pinned or deliberately rebased snapshot; never use scheduler completion order as semantic event order. Merge true independent increases from separate events (subject to score cap and settled decay), while counting overlapping paths of the **same** event only once. Apply updates with per-owner coherent publication/CAS or an equivalent deterministic batch commit. Model concurrency must not put concurrent workers inside mutable Container writes.

### 2.7 Persistence, exactly-once logical application and recovery

Create one owner-local, versioned **Freshness metadata/event-result format**, distinct from `MemoryRecord`'s immutable body/semantic revision and the old proposed `ReviewScheduleStore`. Each touched Memory needs at most its score-at-accounted-position and clock anchor in the current projection; persist enough ordered accepted event identity/result history or a committed batch + checkpoint to reconstruct it after a crash. Do not create a new version of every Memory simply because REL activity advanced. Avoid quadratic historical per-node snapshots and an unbounded in-memory all-event set: use existing source identity/receipts and a compact processed-event watermark plus explicit in-flight identities where a watermark alone is insufficient.

An event that touches multiple nodes must not become half-applied on reopen. Stage all results and an explicit accepted batch/commit marker (or reuse an existing atomic version-linked grouped publication with the same guarantee); replay only accepted complete batches. A crash between creation, provisional first-cycle Graph publications and first durable Dream settlement must neither admit still-extracted Memories prematurely nor lose links published by an earlier partial attempt. Recover the first active Dream settlement's admission decay anchor (or explicitly direct-published active state), then derive the first-cycle event from the entire committed initial relationship set, even if a retry reports `NoChange`. There is no speculative creation-propagation event. A failed or retried write must be idempotent. No separate global distributed task system.

Preserve owner identity and correct REL activity mapping across open, repack, reclamation, principal backfill, migration and reconciliation. Identical history copies preserve scores, anchors, events and next-boundary derivation. For a *reordered* destination history, derive destination event positions and **replay/rebase** from verifiable source identities and available graph state; never copy numerically equal activity positions that denote different histories, and never silently grant `+100` to old Memories as a recovery shortcut. If some causally necessary provenance is missing, report a bounded explicit incompatibility/rebuild requirement rather than invent semantics. Existing RELs without Freshness state must acquire an **explicit one-time initialization policy** (for example, a controlled baseline at current activity position or replay of complete supported events); do not silently rewrite them on read-only open or pretend a rebase was a historical record. PHY handling remains deferred pending a real owner-local activity source.

### 2.8 Context selection and lifecycle ownership

Expose an owner-local `freshness(memory_id, current_rel_turn)` read that lazily decays without mutating the Memory's semantic revision; expose derived Fresh/Stale/Dormant state and a bounded, due-only lifecycle transition drain for consumers that need notifications. Reads of historic/versioned cuts must specify both the appropriate graph/event state and clock cut; never present a latest-only score as an exact earlier snapshot.

Ego's working-Web selector can use the score as a graded recency/relevance input. An entire small Web that fits economically still participates; inactive/dormant Memories normally fall out of **routine priority**, not persistence, ordinary retrieval, mandatory broad audit or independent policy-protected keep lanes. Do not convert a score into a semantic truth assertion or overwrite Chronos valid-time. Perception's optional future evidentiary findings may have an explicit separately specified input mechanism, but Perception is not the Freshness gate or required periodic auditor. New unrelated Memories outside this REL do not decay this REL. Existing processor readiness/Episode barriers still determine when Insomnia/Dream/Ego run, entirely independently of Freshness.

## 3. Historical calibration and measurement constraints

Measured authoritative Insomnia completions (distinct *new* Memory records, not `memory_version`):

| Fixture | Accepted turns | Completed Insomnia episodes | First-cycle REL Memories | New Memories per 100 REL turns |
| --- | ---: | ---: | ---: | ---: |
| User ChatGPT first 14 days | 5,857 | 374 | 1,087 | 18.6 |
| User ChatGPT first 28 days | 16,465 | 1,211 | 2,816 | 17.1 |
| Ellis synthetic | 2,972 | 128 | 678 | 22.8 |

The historical **14-day stored** Leiden snapshot covers 1,084 of those 1,087 new Memories in 12 final-snapshot Communities. Retrospective 100-turn windows, sampled every 25 turns, had zero *new* Memories in 56% of Community windows; at least four in 14.5%. These are diagnostic context, **not** a proposal to restore the discarded four-in-100 reactivation gate. The stored 28-day Community snapshots cover only the early graph; Ellis has no corresponding current snapshot. Do **not** extrapolate current-Community percentages from those histories, invent verified weighted Dream links, or treat these fixtures as representative population research.

Before freezing parameters, reconstruct actual **Web admission anchors** and committed first-cycle Dream linkage/direct-access events where reliably available (not hypothetical creation-propagation events), and analyze real Dream node/edge reachability and duplicate-component size, measure propagation candidate/edge counts and touched Memories per event at p50/p95/p99, cross-Community crossings, parallel speedup versus deterministic serial reference, and score/state distribution over turn history. Inspect 14-day, 28-day and Ellis where graph/snapshot coverage is actually sufficient. Keep defaults configurable rather than adding extra percentage heuristics without measurements.

## 4. Implementation order — new contract

**Phase 1 — Existing accepted REL activity:** Already implemented; preserve its authoritative user/assistant accounting, per-turn positions, historical Archive-version cuts and preservation tests. This code is a prerequisite, not a competing Freshness tracker.

**Retained optional foundation 2.1–2.2 — Archive counter snapshots:** Continue in the existing stream only if their recovery/checkpoint value is still justified; maintain its existing checksum/owner/prefix proof and honest reopen performance claims. Snapshot writes are *not* Freshness events or deadlines.

**Retained utility 2.3 — Typed clocks:** Existing `src/staleness/` pure `Fixed`/`ProportionalCapped` evaluator remains available for Ego summary cadence and other independently justified generic reviews. The recorded isolated tests passed, while full Cargo/integration verification remains incomplete. Do not repurpose `ReviewInterval` as the per-Memory Freshness score or claim it implements this design.

**WITHDRAWN previous 2.4–2.7:** The proposed common REL/PHY `ReviewScheduleStore`, `MemoryReconsideration` registration, mandatory audit completion and its migration gates are no longer steps in Memory Freshness. If an Ego-only durable summary scheduler is later justified, specify and implement it separately under the [Ego plan](ego-web-synthesis-plan.md), with its original successful-summary/covered-cut safeguards; do not force Memories onto that store.

### F1 — Source-inspected contract and independent reference tests — complete as specification

Source review establishes the exact implementation seams below; `tests/freshness_f1_reference.rs` supplies **standalone reference/golden tests**, not proof of production integration. The Graph-inspection correction is reflected here: extracted publication and later active first-Dream-settlement admission are separate. F2–F5 will compare their implementation to that reference. No production Freshness writer, new codec or host consumption callback has been introduced by F1.

**Creation versus Web admission — verified source seams.** `src/runtime_inference/insomnia/processor/application.rs::commit_application` stages drafts and durably publishes genuinely new `MemoryStore` records through grouped `InsomniaCompletion`, while `existing` entries are old IDs/retries. `src/memory/memory_store.rs::publish_with_source_ref_and_temporal_inference` is the direct creation path; its `changed=true` also covers later revisions, so first creation must be detected from absent identity/first revision, **not** the boolean alone. Creation initializes +100 without propagation or decay. `semantic_graph/graph_store.rs` and `graph_store_read.rs` confirm the Graph's Memory projection includes only relation endpoints: first edge can precede Dream settlement, and successfully settled isolated Memories need not have any Graph-projection node. Thus neither first MemoryStore record nor first Graph node is the universal admission signal.

**Ordinary Web admission:** `runtime_inference/dream_lifecycle.rs::reconcile_dream_lifecycle` transitions newly `extracted` Memories to active `knowledge` or `canonical` on a successful initial Dream pass even with **zero** relationships. First *successfully durable* initial Dream settlement in either of those states is admission. Anchor decay at that owner's `Archive::rel_turn_count()` at accepted settlement, not the older Episode source cut, initial storage time, provisional edge, vector readiness or wall time. A genuinely new **direct-published** Memory already explicitly in `knowledge` or `canonical` can admit at first accepted publication; directly published `extracted` Memories still wait for Dream settlement. A new duplicate archived during Dream settlement reinforces existing canonical information but is not itself admitted as an ordinary active Web Memory. Never reset admission on later Dream reconsideration or a Memory revision. F2/F5 must persist the first admission identity/clock at the owner boundary, not conflate it with the immutable body or semantic revision.

**Dream first-link event — actual committed new connections only.** `src/runtime_inference/dream_owner_publisher.rs::publish_dream_pair` returns `DreamPublicationOutcome::Published(Vec<GraphRelation>)`, `NoChange` or `Withheld`. For duplicate relationships, `dream_owner_duplicate.rs` may publish *several* directed chain rewires; these raw edge mutations are **not distinct Freshness events**. Both the direct `dream_processor.rs`/`dream_processor_frontier.rs` methods and long-lived `runtime_host_dream_owner.rs::commit_project` publish pairs and reconcile lifecycle; **only the long-lived runtime additionally persists a Dream processed-epoch marker today**. F3 must collect actual new first-cycle relationships on **all** these paths, not attach only to one runtime worker. F3 must unify first-settlement/admission identity across **both** execution paths; the runtime's processed epoch and prepass `extracted` state help, but neither alone is a universal idempotency key. If an initial attempt published edges before crashing, a retry may report `NoChange` for those committed edges; the eventual first settled pass must recover all genuine first-cycle links across that attempt/recovery window. Periodic reprocessing, rewires, user graph edits and pre-existing edges are not first-cycle event sources.

Use **one first-cycle linkage event per newly created Memory's settled initial Dream pass** (even if Dream archives that new Memory as a duplicate), containing an initially deduplicated set of connected *pre-existing* Memory roots (distinct existing component representatives when duplicate links resolve). Each root receives a +50 candidate in that **same event**, and downstream overlapping paths use the maximum, not addition. Do not credit a same-batch peer that was also freshly initialized +100 merely because it is adjacent; do not credit the new source Memory through a return path. For duplicate recognition, resolve the pre-existing active/canonical representative through Dream's authority *after* settlement; do not treat each physical `DuplicateOf` rewire as an event. An entirely `Withheld` or genuinely zero-link first pass produces no Freshness event. A retry's pair-level `NoChange` is not proof that the complete initial pass had no committed links: include any genuine first-cycle edges from its recoverable partial-attempt history. Require producer identity and graph-version proof at commit. Proposed stable identity: (REL owner UUID, new Memory ID/first published revision, **first** Dream-processing instance); retain one idempotent receipt for the complete root set so a retry cannot issue new +50 events. A later independently created Memory has its own first-cycle event even when its text duplicates an older one, including if its own lifecycle is archived on settlement. F5 resolves any cross-record crash gap using existing Dream's recovery cursor plus atomic Freshness event publication, not a speculative event at creation.

**Qualifying access — consumption, not a read operation.** The current `Cva::memory` is called extensively by Dream, retrieval-index construction, search hydration, lifecycle reconciliation and tests; `retrieve_memories` enumerates candidates, while `runtime_host_memory_search.rs` hydrates even items subsequently truncated from the visible search list. Therefore **none** of these reads/search hits automatically generates +25. The intended producer is an explicit owner-scoped **accepted use receipt** from a higher-level context-assembly/delivery boundary *after* the Memory content is actually included in an accepted prompt/context or deliberately opened for use by the user. It supplies the Memory ID, owner, accepted REL activity position and stable delivery/use identity. A cancelled, merely proposed, deduplicated or discarded context is not access; repeated appearances of one Memory in the **same** accepted delivery count once. Separate real deliveries can reinforce independently. No general-purpose auto-observer on `MemoryStore` or internal Graph traversal. The host currently exposes retrieval/search but not a universal accepted-context-consumption receipt; F3 must add this narrow explicit seam when a real consumer exists, without falsifying access events in the meantime.

**Traversal — event amount independent of node score.** Use active owner-local Memory-to-Memory edges, deduplicated and treated as undirected for context relevance. Pin graph and validated Leiden membership for each event; use the *originating existing root's* Community as the fixed reference, with known in-origin destination costing five and other/unknown destination costing ten. Multiple first-cycle roots in different Communities are explored in separate per-root origin contexts but aggregated by max **once per event per physical Memory**. The current defined policy charges five again on a hop re-entering the original Community (destination-based cost), rather than transferring origin to whatever other Community the path crossed. A propagation amount of zero/negative stops; **a recipient's negative Freshness score never blocks event traversal**. `GraphDirection` currently has only `Incoming`/`Outgoing`: materialize a single read-only bidirectional view rather than repeatedly traversing the whole Graph. Keep sorted IDs for deterministic tie behavior. Duplicate-component aliasing beyond canonical root selection is a separate F3 testable optimization; do not assume a zero-cost walk through physical duplicate chains without explicit agreement.

**Cross-phase acceptance:** F1 golden reference cases cover unlinked creation, admission at nonzero turn, unrelated clock progress before admission, first-link publication versus retries/rewires, access consumption versus internal reads, remainder-preserving decay, +100/-100 saturation, +25/+50 propagation, multiple roots/paths, a Dormant intermediate, Community departure/return and deterministic path maxima. F2 builds the real owner-local score/anchor API; F3 binds both Dream commit paths and the eventual true access consumer; F4 differential-tests the concurrently evaluated graph against this reference; F5 validates crash/reconcile/repack and clock mapping. The now-superseded per-Memory generic review scheduler and mandatory Perception stale-Memory audit are **not** implementation dependencies.

### F2 — Owner-local Freshness state and pure lazy-decay engine

- Implement signed score, **explicit admission REL-turn anchor**, accounted-turn remainder, checked decay, derived state and event idempotency metadata separate from semantic Memory revisions; creation alone never starts decay before Web admission.
- Implement read-only current/historical policy with explicit clock cuts; build rebuildable per-owner due-transition index and test boundary under backlog without eager per-turn updates.
- Implement explicit legacy enrollment; preserve existing opening semantics and do not mutate source files on ordinary open. Establish a small focused direct owner-local API, not a new public orchestration framework.

### F3 — Dream/Insomnia/access event adapters and duplicate folding

- At grouped/direct first Memory publication initialize `+100` without decay or propagation for `extracted` Memories. Admit at first successful settled nonarchived Dream `knowledge`/`canonical` state (including zero-link), or at first direct publication already in an explicitly active state. On **both** Dream execution paths, emit one multi-root `+50` event only for genuine first-cycle committed links to pre-existing Memories; recover provisional edges from an earlier failed attempt and emit no event for an unlinked settlement. Add the narrow accepted-consumption receipt before enabling +25 access; internal reads/search results are never implicit access.
- Integrate Dream's actual duplicate component/canonical-owner decisions, including arbitrary chain length, merging/intermediate insertions, existing canonical reinforcement and no rewiring-only events.
- Use one owner-validated event identity/provenance path; test retries, independently new duplicates, archived entries and graph/snapshot mismatch.

### F4 — Concurrent deterministic propagation

- Implement bounded worker-pool, descending score-frontier expansion and deterministic maxima; pin graph and originating Community per event.
- Compare against a deliberately simple serial reference across cycles, long/branchy paths, complete/dense Communities, overlapping events, cross-Community paths and duplicate folding. Verify independent worker counts/schedules produce byte-for-byte equivalent final score projections.
- Ensure graph reads/work scheduling do not monopolize Container mutation locks. Benchmark real event workloads before adding caches, distance cutoffs or precomputed all-pairs reachability.

### F5 — Atomic event publication and restart recovery

- Persist events/effects or equivalent replayable atomic batches with expected owner/version checks, crash markers and compact checkpoints as needed.
- Integrate deferred Dream settlement, producer retries and idempotent backfill. Reopen validates no partial event and exactly one application per committed originating identity.
- Add copy/repack/reclamation/fresh-migration/append-only and divergent reconciliation preservation tests against source logical history. Do not confuse snapshot history, score anchors and unrelated global Memory versions.

### F6 — Lifecycle deadline index and Ego integration

- Support due-only boundary notifications and cheap lazy score reads. Integrate Ego's graded score priority without excluding mandatory full-Web audit/keep lanes when the Web is small.
- Test Dormant retrieval, access reinvigoration over repeated events, direct positive-root event semantics, independently archived/superseded Memories, and behavior with Perception disabled.
- Preserve summary regeneration clocks/queues as a **different** Ego concern.

### F7 — Calibration, documentation and closeout

- Run pure/serial/parallel differential tests, Archive and Dream regression suites, REL open/replay and storage-transform gates, Ego selection fixtures and long-running fixture measurements.
- Measure p50/p95/p99 touched nodes, examined edges, maximum frontier, cost/event and end-to-end Insomnia/Dream latency; record bounded-worker speedup and contention rather than asserting parallelism is always faster.
- Run format checks, available Cargo test gates and repository documentation validators. Record pre-existing worktree/build issues and concurrency collisions; do not claim green integration without completed verification. Update `architecture.md`, `storage-format.md`, `api.md` and `current-limitations.md` **only when corresponding code actually ships**.

## 5. Acceptance matrix

| Scenario | Required outcome |
| --- | --- |
| Empty/new REL, 10-turn remainder, sparse events | Exact score and conserved decay remainder |
| +100 unreinforced Memory admitted to Web | Stale 1,000 turns after admission; Dormant 1,510 after admission; saturates at -100 |
| Access at current score -100 | A real accepted-consumption receipt adds +25 locally; Memory remains Dormant but positive propagation strength can still cross its active graph edges |
| Creation, Web admission, actual Dream linkage | First accepted extracted Memory publication initializes +100 without decay/propagation; first successful active Dream settlement anchors decay even if isolated; direct first publication already active may admit immediately; genuine first-cycle links generate one multi-root +50 event for pre-existing connected Memories |
| Same event reaches node through 2+ paths | Highest positive candidate applies **once**, not a sum |
| Several independent accepted events | Each event contributes exactly once in canonical order |
| Local hops vs 1st/2nd hop outside origin | -5 vs -10 per hop, including hops wholly inside *other* Communities |
| Re-enter origin after outside traversal | Normal reduction when destination is inside original Community |
| New duplicate vs existing chain rewiring | New occurrence reinforces resolved canonical; pure rewire generates no event |
| Source retry, worker retry or crash before committed batch | No double credit and no half-applied graph-wide scores |
| Worker count 1 vs N; arbitrary task interleaving | Equal candidate winners and identical final projection |
| State transitions `+1 -> 0`, `-50 -> -51` and reverse | Correct Fresh/Stale/Dormant; no semantic Memory archival |
| No events for many REL turns | O(1) lazy score read plus bounded due-transition drain, no all-Web per-turn writes |
| Community snapshot is stale or new root lacks membership | Documented conservative rule, no automatic full Leiden rebuild |
| Large/dense active graph | Correct algorithm and measured traversal cost, no arbitrary extra hop cap |
| Legacy reopen; full/partial physical history transformations | Explicit enrollment or correct replay/rebase; no invented score/activity history |
| Ego with Perception absent, Web small/large | Graded priority while preserving full-Web-if-economical and hard-keep rules |
| PHY has no REL activity stream | No accidental use of unrelated REL, wall time or raw PHY Memory versions for this REL policy |

## Related docs

- [Memory Freshness contract](memory-staleness-plan.md) — concise normative behavior and separation from semantic truth.
- [REL activity clock](rel-activity-clock-implementation-plan.md) — existing canonical turn semantics; no new clock.
- [Ego Memory-Web synthesis](ego-web-synthesis-plan.md) — separate summary refresh scheduling and use of Freshness as graded context priority.
- [Perception subsystem](perception-subsystem-plan.md) — separate optional proposition/evidence reasoning; **old mandatory stale-Memory audit description is superseded**.
- [Chronos subsystem](chronos-subsystem-plan.md) — semantic temporal meaning, not a Freshness activity timer.
- [Dream implementation](dream-implementation-plan.md) — authoritative relationship and duplicate decisions.

## Notes

**Non-goals:** a universal wall-time freshness metric; treating score as truth or confidence; a new probabilistic Freshness pass; manual edge-weight inference; running Dream/Leiden on every turn; a broad worker scheduler replacement; reinstating separate rolling Community/access counters; reviving per-Memory `next_review_at` records; triggering unrelated model, embedding or summary operations from score changes.