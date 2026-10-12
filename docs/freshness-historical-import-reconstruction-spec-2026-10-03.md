# Reliquary Freshness — provenance-timed historical import reconstruction

**Date:** 2026-10-03
**Status:** Accepted design; Step 13/H1 policy is frozen in the [manifest and timeline contract](freshness-historical-manifest-policy.md). Historical reconstruction is not implemented or verified.
**Scope:** Optional, more computationally intensive one-time mode for freshly generated complete imported owner-REL corpora. Ordinary import and routine live semantic reinforcement do not inherit reconstruction costs or behavior.
**Governing documents:** [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md), [F1–F8 plan](generic-staleness-implementation-plan.md), [completion plan](freshness-completion-execution-plan-2026-10-03.md), [REL activity clock](rel-activity-clock-implementation-plan.md).

## Purpose

Define the optional one-time historical reconstruction path for freshly generated imported REL corpora so Freshness can approximate prior semantic activity and first-cycle Dream reinforcement without fabricating consumed-use events, rebuilding historical graph versions, or changing ordinary live Freshness semantics.

## Overview

The [frozen v1 manifest/timeline policy](freshness-historical-manifest-policy.md) selects the initial supported rules where this broader design lists alternatives; future conversions or fallback policies require a new approved version. The reconstruction generates the corpus and settled graph once, derives a provenance-timed event stream against that fixed authorized graph, replays eligible contributions chronologically with the existing Freshness score mathematics, and atomically replaces provisional import scoring with one owner-local checkpoint. It is intentionally more expensive than normal import and remains REL-local until separate PHY clock semantics are approved.

## 1. Decision

Import the complete Archive and generate Memories, embeddings, and a settled Dream graph **once**. Reconstruct approximate historical Freshness using source provenance timestamps and that **fixed completed graph**; do not repeatedly switch between model-driven subsystems, reconstruct versioned historical graph topology, or run Insomnia/Dream inference again. Replay selected reconstruction events chronologically with the established Freshness lazy-scoring primitives. Freshness mathematics is exact for the chosen reconstructed events; their historical eligibility, Dream timing, graph connectivity and Community context are explicitly approximate.

**Strict policy separation:**
- Normal live semantic activity reinforcement remains **local and non-propagating**. A semantic match is not delivered/consumed use.
- Historical import semantic activity **does propagate** through the completed graph under a versioned **import-only** event policy, using a similarity-derived semantic principal (never implicitly +25 access or +50 Dream).
- Reconstructed first-cycle Dream reinforcement propagates under established +50 linkage semantics. Do not infer later historical Dream reprocessing.
- **Future/ineligible Memories never block propagation traversal.** Traverse the entire *authorized* completed graph, including future Memories, then filter award recipients by provenance eligibility. A future Memory can bridge a path to an eligible older Memory but receives no premature credit.
- A source Memory cannot originate an event before its reconstructed effective availability timestamp. Event-specific self, birth-cohort and duplicate restrictions suppress inappropriate **credit**, without incorrectly removing traversal bridges.
- Reuse existing per-hop -5 local/-10 outside-originating-Community behavior, strongest path per originating event, deterministic bounded concurrency and a fixed validated current Community snapshot.

**Non-goals:** arbitrary import score baselines, historical +25 consumed-use invention, a new durable reinforcement ledger, a second permanent clock, staged fragment-by-fragment inference, reconstructing hypothetical Dream runs, a periodic semantic audit, silent modification of already-enrolled live Freshness, or assigning a REL clock to PHY without a separately approved owner policy.

## 2. Ownership and inputs

Archive owns accepted owner-REL turn identity, original timestamps and first-accepted activity positions. Insomnia/Memory provenance owns original source/authority/grounding turn references and Episode origin. Dream owns final relationships, canonical duplicate decisions, birth cohorts and any first-cycle proofs actually produced by this new corpus. Graph and Community provide a version-pinned completed, authorized snapshot. Freshness exclusively owns numerical scoring, event ordering, replay/recovery and the authoritative score projection. The import coordinator only orchestrates the post-generation barrier, inputs and one-time publication.

Freeze a deterministic **reconstruction manifest** before event calculation: owner/REL and stable import identity; canonical unique eligible source-turn IDs, source timestamps, chronological order and accepted activity cut; Memory IDs, resolved source references, availability positions, birth cohorts and lifecycle; fixed graph/Community fingerprint; embedding profile/generation and coverage; import/semantic/Freshness policy versions and event-order rules. Hash the manifest and pin it through final commit. A changed input requires a deliberate restart or rebasing, not silent reuse.

Only genuinely accepted user/assistant logical turns contribute clock positions and transcript semantic evidence. Deduplicate branch-shared turns by source conversation and node identity, exclude synthetic retrieval or summary echoes, preserve ancestry and reject invalid cross-owner source bindings.

### 2.1 Time, eligibility and activity positions

Build a canonical transcript order from provenance timestamps, with deterministic ancestry-respecting tie breaks and stable conversation/node IDs. Do not sort a child ahead of a parent with contradictory timestamps: reject or require an explicit documented repair rule. The same timestamp is eligible under the stated at-or-before restriction, subject to source/self and birth-cohort exclusions and source-evidence availability. Map each first-accepted eligible transcript turn to position 1..N. Map a reinforcement event to the historical turn it comes from. Timestamps select eligible Memories; **owner-REL accepted turn positions**, not elapsed wall time, compute decay.

Important implementation constraint: existing Archive activity positions encode **original acceptance order**, not an arbitrary timestamp sort. **Preferred initial implementation:** insert historical transcript turns in the canonical order in one pass, without staged model execution, making actual Archive activity positions match the intended replay order. If an already-ingested corpus was accepted out of timestamp order, NEVER mislabel synthetic historical positions as authoritative persisted REL positions. Either provide a separately oracle-verified one-time conversion of the synthetic chronological score/remainder to the present owner-REL cut with explicit provenance/reconciliation policy, or report this corpus unsupported for historical mode. First release may require matching order. After successful reconstruction, ordinary real owner-REL turns extend the authoritative existing clock; there is no competing runtime clock.

### 2.2 Effective Memory availability

Resolve original source/authority/grounding turns and the completion of the Episode that generated the Memory. Use the **latest necessary evidence/episode boundary** as the approximate time the Memory could have been created and entered the active Web. Prefer verified source IDs mapped to the replay timeline over treating a current mutable timestamp as historical birth. Direct already-active publications use proven initial publication/source evidence. Preserve isolated active Memories (they admit and decay even without graph edges); honor archived/duplicate and never-admitted exclusions. Treat a grouped Episode's new Memories as a stable cohort. For equal timestamps, require all necessary source evidence to be present under deterministic ordered positions before admission.

Each eligible active reconstructed Memory starts at the existing **+100**; decay begins at its effective historical Web-admission position. Do not charge decay while evidence for its existence was unavailable. Creation itself produces no propagation event. Report unavailable/contradictory provenance and apply an explicitly versioned conservative fallback or skip the unsupported candidate; never silently invent early birth or assign an arbitrary score baseline.

The initial implementation is REL-local. PHY Memories sourced from RELs require a separately approved authorization, owner and clock mapping; no source REL may silently decay a PHY or different owner's Memories.

## 3. Reconstructed events: complete graph, recipient-only time gate

Event-generation output is transient: stable origin event ID, source turn identity, event kind, historical position, source timestamp, semantic/Dream policy proof and per-recipient strongest positive contribution. Generate independent matches and propagation concurrently against the same immutable authorized graph and Community snapshot. For large corpora, use bounded event queues/external sorted runs. Discard temporary event data after the authoritative checkpoint is accepted; no permanent historical reinforcement ledger.

**Universal temporal rule:** a Memory can receive reinforcement only if its resolved effective availability is at or before the event's historical position/time. This rule is applied at **recipient credit**, never in neighbor expansion. Future Memories remain traversable. Authorization is NOT approximate: no unauthorized REL/PHY crossing.

### 3.1 Semantic history (import-only propagation)

For each historical accepted turn, take embeddings of genuinely ingested fragments. Reuse actually retained, compatible embeddings if present. If fragments lack stored vectors, offer a separately disclosed **optional bounded embedding-only pass**, without any new Insomnia/Dream model generation. If declined or unsupported, skip only that unavailable semantic input and report coverage; do not pretend all fragment embeddings were saved.

Query eligible, owner-authorized Memory vectors with compatible embedding profile/generation via a bounded index, without allowing future vectors to crowd out eligible historical candidates. The shipped F8 v1 policy fixes the inclusive cosine threshold **0.75**, +1..+10 curve and top-64 eligible direct roots. The [historical manifest policy](freshness-historical-manifest-policy.md) reuses those constants and pins recipient-only import propagation separately; no new REL/PHY inference is required to select them. For each (source turn, target Memory), retain the strongest qualifying fragment similarity and only one direct root bonus. Exclude source-self matches and unsupported lifecycle/profile matches.

One **semantic originating event per accepted source turn** can have multiple eligible roots with different semantic bonuses. Evaluate each root with the existing local/outside per-hop costs relative to that root's fixed originating Community, then take the **maximum path/root candidate for each recipient for the whole turn**. Several fragments, roots or traversal paths must NOT sum multiple semantic awards to the same recipient from one turn. A direct root receives its local candidate even if isolated/no graph projection node exists. A recipient ineligible in that historical turn receives nothing, but can still bridge a route to another eligible recipient. A valid match in ordinary live ingestion continues to reinforce **only its target**.

The current propagation implementation accepts only policy-configured +25/+50 event principals. Implement a narrow, versioned **import-only variable-root principal adapter** reusing its deterministic traversal; do not weaken general principal validation or allow live semantic propagation accidentally.

### 3.2 Approximate first-cycle Dream history

Generate at most **one historical first-cycle linkage event per qualifying newly generated Memory**, anchored at the originating Memory's effective provenance/availability position. Prefer actual first-cycle roots/birth proofs produced during this new corpus's Dream execution when they accurately identify initial links; their real bulk-import execution timestamp does NOT become the historical event time. Otherwise, derive approximate first-cycle roots from the **completed settled Dream graph**: direct Dream-origin connections to older eligible active/canonical Memories, resolving duplicates through existing Dream representative authority. No historical edge timestamps, graph versions or original Dream execution traces are required.

A verified initial no-link outcome produces no fabricated +50 event. Final-only Dream adjacency may sometimes reflect a relationship established later than the original first pass; when evidence does not distinguish this, its approximate historical credit is deliberate. Do not infer a +50 event from arbitrary existing-to-existing edges, pure duplicate-chain rewiring, non-Dream relationships, or hypothetical subsequent Dream passes.

One source Memory with many qualified roots produces **one +50 multi-root event**, never one event per edge. Preserve existing duplicate canonical-target selection and cohort/self credit exclusions. During historical reconstruction, traverse all authorized nodes, including future or otherwise credit-excluded nodes as intermediates, but never award own-event credit to the originating Memory or its same-birth cohort, and never credit a future recipient. Propagation continues to merge paths by highest contribution per event.

### 3.3 No fictional consumed use

Archive conversation content is relevance evidence, not proof a Memory was delivered and consumed. Never synthesize historical +25 use events without a real accepted receipt. Independently recorded real use after reconstruction follows the unchanged normal +25 propagation lane.

## 4. Sparse chronological numerical replay

For each newly imported Memory independently:
1. Create a working FreshnessRecord at **+100**. Admit it at the reconstructed historical active-Web position when eligible under its lifecycle; never backdate unsupported evidence.
2. Collect eligible positive contributions from reconstructed semantic and Dream events. Dedupe maximum strength **within** each event, not across genuinely independent events. Sort each recipient's contributions by **(historical accepted position, stable event ID)** with an explicit deterministic tie policy for same-position semantic/Dream events.
3. For each contribution, use the existing **reinforce_with_policy** (internally settling **settle_with_policy** first) at that historical position. Negative -100 and positive +100 caps are enforced **at every event boundary**. Preserve the sub-ten-turn decay remainder and saturation handling exactly as normal. Do not use an uncapped accumulator followed by one global decay and final clamp.
4. Do not iterate through every historical turn for every Memory: process sparse reinforcement boundaries only. Memories with no events retain initial score and admission anchor. After a Memory's last historical event, **leave later decay lazy**; normal current-cut reads and future live events settle it.
5. Parallelize different Memories, never scramble chronological ordering within one Memory. Worker count and completion order must not change persisted bytes.

Example oracle: an active +100 Memory at position 0, unreinforced until position 3,000, has settled to -100; +50 reinforcement at position 3,000 yields -50, with no unbounded negative decay debt. An earlier +50 may produce a different current score after subsequent decay. This is why chronological event boundaries are required despite the approximate graph.

The result matches the normal Freshness score engine *exactly for the selected reconstructed admissions and event stream*, not necessarily unknowable historic graph/Community/Dream timing.

## 5. Durability: replace, never double-credit

Bulk generation may already have published +100 birth records and actual first-cycle Dream Freshness effects at the bulk-import acceptance cut. Historical reconstruction must **replace that provisional import score projection**, never add retrospective contributions on top of it. Preserve authoritative Memory bodies, Dream decisions, first-cycle proof journals, archive identities and graph versions.

Add the smallest narrowly scoped **atomic import-reconstruction checkpoint in the existing owner-local Freshness durability/replay contract**, not a separate ledger:
- Freeze owner, stable import identity, manifest/hash, input Archive cut, graph/Community/policy versions and the exact import cohort. Validate a no-unhandled-live-events barrier; refuse/recompute if the frozen input changed.
- Build the complete per-Memory resulting score/admitted/accounted projection, versioned source-cut/manifest metadata and whatever compact replay proof the current store requires, *off to the side*. Atomically publish one accepted, full-cohort checkpoint with the existing owner commit/batch discipline. No partially reconstructed scores visible.
- The accepted checkpoint **supersedes the provisional import birth/Dream scoring effects** for that cohort at its logical cut; ordinary subsequent legitimate accepted events remain outside the superseded interval. Replay/open/repack/reconcile must not reapply the provisional effects after restoring the checkpoint. The Dream journal and its first-cycle evidence are not deleted or falsely rewritten.
- Rebuild the sparse derived due-boundary index from the authoritative checkpoint. Resume normal operation using the existing owner-REL clock; no permanent import event queue or shadow freshness score.
- The same stable import identity, identical manifest and policy yields the same checkpoint on retry/reopen. A conflicting manifest/policy for that identity requires explicit migration or rejection. Regenerating an *already-enrolled* Archive restores existing authoritative Freshness; it does not constitute a fresh import deserving a second reconstruction.
- Identical copies/repack/reclaim must preserve checkpoint identity and result. Divergent reconciliation must rebase using verified source IDs and actual accepted-turn meaning, recomputing where necessary; never blindly transplant numerical anchors across reordered histories.

If the current Freshness owner cannot yet express a superseding checkpoint atomically, implement that narrow versioned extension and verify it **before** activating historical mode. Do not implement this feature as a second owner or uncontrolled rewriting of historical event records.

## 6. Implementation phases and hard gates

**H1 — policy, fixtures and seam.** Freeze the precise transcript tie/ancestry policy, provenance availability function, import semantic bonus/cap, variable-root multi-match event identity and maximum aggregation, Dream proof versus completed-graph fallback, credit-only eligibility, Community snapshot fallback and same-turn event order. Keep normal live semantic scoring non-propagating. Use deliberately constructed fixtures and retained data, not another expensive model-generated REL/PHY baseline.

**H2 — historical timeline and eligibility planner.** Build deduplicated source-turn ordering, verify compatibility with actual first-accepted Archive activity positions, resolve original source/authority/grounding references and Episode completion, establish per-Memory historical Web-admission positions and birth cohorts, and emit explicit coverage/errors. Hard tests: two conversations/tied timestamps, ancestry, shared branch node, non-turn exclusion, late grounding evidence, isolated active Memory, bad/missing provenance, owner isolation, nonchronological Archive acceptance (explicitly rejected until a proven conversion exists).

**H3 — import-only propagation adapter.** Pin complete graph and validated/fallback Community snapshot once. Reuse bounded deterministic strongest-path frontier with variable semantic root bonuses, maximum **per entire semantic turn**, and distinct Dream multi-root +50 events. Filter historical eligibility **at credit only**: future node as traversal bridge and ineligible bridge not credited. No relaxation of authorization, normal event-principal validation, or live semantic locality. Tests cover future bridge, cycles, multiple equal/unequal roots, duplicate aliases, self/cohort credit exclusions, disconnected direct root, uncertain Community, and workers 1 versus N.

**H4 — event producers.** Semantic matching against genuinely ingested and compatible fragments, with optional separately budgeted missing-embedding pass; first-cycle Dream approximations against actual new-corpus proofs when present, otherwise completed direct Dream links to older canonical targets. Dedupe source turn/target, source/own-Episode exclusions, verified no-link, repeat rewire suppression, profile rotation and hash-stable deterministic event IDs. No synthetic +25 use.

**H5 — sparse chronological scorer.** Group and, if necessary, spill event contributions per Memory; order canonically; reuse FreshnessRecord created/admit/reinforce_with_policy/settle_with_policy and preserve remainder. Tests compare identical chosen streams to the existing serial live mathematical oracle at every boundary; verify -100/+100 mid-history saturation, huge decay gaps, same-turn independent events, dormant recovery, lazy tail and byte-identical parallel output.

**H6 — import checkpoint/transaction.** Build and atomically accept owner-local superseding checkpoint, manifest barrier, retry/idempotence proof, recovery prefixes and due-index rebuild. Verify original provisional Dream scoring cannot double-count and that a genuine post-checkpoint +25 use remains valid. One owner of Freshness journaling/reconcile files at a time; never overlap competing edits in the same worktree.

**H7 — physical transformations and integration.** Reopen, copy, repack, reclamation, regenerated Archive, divergent merge/reorder, missing embeddings, ongoing accepted turns and current-cut readability. Verify nonchronological preimports are rejected or converted only after an explicit oracle passes. PHY integration requires independent approved owner/clock semantics; REL-only pass never implies PHY readiness.

**H8 — acceptance and measurement.** Execute focused fixtures, core regression and existing F8 live-locality tests, full repository verification and docs gates. Report CPU, temporary memory/spill, index cost, vector coverage, edge traversals, checkpoint/reopen cost, matched candidate distribution and resulting Fresh/Stale/Dormant distribution using **already retained** corpora if available. No new full REL/PHY generation is required to select an arbitrary import baseline. Record actual commands/results in a dated verification document; this design does not claim implementation.

### Mandatory executable oracles

| Fixture | Required result |
| --- | --- |
| +100 at turn 0, +50 after 3,000 turns | Decay saturates at -100 before event, then score -50; no decay debt |
| Same total reinforcement at early versus late turns | Correct distinct final scores after chronological decay |
| Reinforcement at +100 | Intermediate positive cap; discarded excess cannot be carried forward |
| No event after birth | Lazy remaining decay matches ordinary current-cut evaluation |
| Multiple semantic fragments/roots in one historical turn | One maximum semantic contribution per recipient for the whole turn |
| Same qualifying semantic activity during normal live ingestion | Matched memory only, **no** propagation |
| Completed-graph future Memory bridges two older Memories | Traversal succeeds; eligible older recipient credited; future bridge not credited |
| First-cycle source has multiple older Dream neighbors | One +50 originating multi-root event; no self/cohort credit |
| Known initial no-link and pure rewires | No invented linkage event |
| Duplicate canonical representative | Existing Dream representative authority, not arbitrary duplicate-chain endpoint |
| Previously provisional first-cycle import Freshness | Superseded checkpoint prevents double award |
| Interrupted, repeated or regenerated import | No partial publication, duplicate credit or mutation of already-enrolled Freshness |
| Mismatched source timestamp order versus actual Archive accepted order | Explicit incompatibility until a verified cut conversion is available |
| Unauthorized owner/PHY traversal | Never crosses authorization or invents a shared PHY clock |

## 7. Documentation and handoff

This design **supersedes the earlier F8 one-time uncapped/bootstrap accumulator and no-retroactive-timing proposal**, and narrows the former unconditional semantic non-propagation wording to **normal live semantic reinforcement**. The core R1–R7 release remains separate and unchanged. Do not claim an accepted historical plan implies actual imported-history support; keep all current API/storage docs truthful until H1–H8 acceptance.

Execution handoff: inspect current source and Git state before starting, then implement H1–H8 with separate passing test evidence. Run only one mutating owner-journal/reconcile edit batch at once; parallelize independent fixtures and benchmarking only when their file scopes do not collide.

## Related docs

- [Unified Memory Freshness implementation plan](generic-staleness-implementation-plan.md)
- [Freshness completion execution specification](freshness-completion-execution-plan-2026-10-03.md)
- [Memory Freshness policy](memory-staleness-plan.md)
- [ADR 0039 — unified deterministic Memory Freshness](decisions/0039-unified-deterministic-memory-freshness.md)

## Notes

This document is an accepted design specification only. No H1–H8 implementation, migration activation, or PHY Freshness behavior is implied by its presence on `main`.
