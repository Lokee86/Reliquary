# Freshness semantic policy module — 2026-10-11

Parent: [Documentation index](INDEX.md).

## Purpose

Implement the graph-independent numerical and candidate-reduction core frozen by the v1 semantic Freshness policy.

## Overview

**Date:** 2026-10-11. **Status:** Steps 5–6 complete; live F8 remains disabled pending candidate search, durability, wiring and release gates. The architecture refresh exception is recorded below.

## Related docs

- [Live semantic Freshness calibration and v1 policy](freshness-semantic-calibration-2026-10-11.md)
- [Live semantic policy/API contract](freshness-live-semantic-policy-contract.md)
- [Implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)

## Scope

Step 5 implements the graph-independent semantic policy frozen in the v1 calibration record. The module is `src/freshness/semantic_policy.rs` and is exposed through `freshness::semantic_policy`.

It owns:

- versioned policy constants and canonical descriptor/digest;
- inclusive cosine thresholding and the bounded linear bonus curve;
- deterministic binary64 cosine calculation and input validation;
- source/target vector profile and generation compatibility checks;
- strongest-match-per-Memory reduction, deterministic tie ordering, candidate limiting, and local effect calculation.

It does not own Archive ingestion, vector storage, lifecycle eligibility, Freshness durability, or graph propagation. Those remain explicit seams for later steps.

## Frozen behavior

The default policy is version 1, numerical revision 1, eligibility revision 1, threshold `0.75`, bonus cap `10`, and candidate limit `64`. A cosine at the threshold receives bonus `1`; values below the threshold receive `0`. The curve is monotone and capped at `10`.

Usable candidates must have finite cosine in `[-1, 1]`, compatible profile evidence, a nonzero generation, and no self-source exclusion. Invalid rows are dropped before dedupe and do not consume the candidate limit. A target keeps its strongest cosine; exact ties use fragment-id bytes and then vector-binding bytes. Ranking occurs by descending cosine and then `MemoryId` bytes before the limit is applied. Returned effects are sorted by `MemoryId` bytes.

The canonical descriptor is:

`{"bonus_cap":10,"candidate_limit":64,"curve":"inclusive-linear-v1","eligibility_revision":1,"numerical_revision":1,"threshold":0.75,"version":1}`

Its domain-separated SHA-256 digest (ASCII `reliquary:semantic-freshness-policy`, zero byte, descriptor) is `5886dc23e386664b3dbef03a116dbfab5567ad2998be0e21e07c4895ab1d261f`.

## Verification

Focused unit tests cover threshold boundaries, monotonicity, cap behavior, strongest-match and tie reduction, candidate ranking and limiting, incompatible profile/generation evidence, invalid inputs, cosine validation, and descriptor stability.

Step 6 first repaired Step 5's uncompilable candidate comparator, incorrect 0.9 bonus expectation, missing policy digest domain separation, threshold rendering precision, and cosine normalization order to match the frozen contract. The earlier Step 5 completion claim had no passing Rust evidence; these repairs now have passing verification.

Verification on 2026-10-11: `cargo fmt --all -- --check`, `cargo check --locked -j 2`, and full `cargo test --locked -j 2` passed (1,100 library tests and all integration/doc-test targets). Focused commands passed 38 Freshness tests and five accepted-source tests. Both documentation-policy modes and `git diff --check` passed. Builds used the isolated `reliquary-r3-verification` target with dev/test debug information disabled. Existing warnings and intentional dead-code warnings for the unwired producer remain.

`python scripts/check_architecture.py --refresh` passed the 28-fixture/112-API coverage guard, then made no further progress after Lexicon reported Python analysis. It was cancelled after approximately three minutes; Arcana/Pitlord enforcement did not complete. This is a verification exception, not a passing architecture gate. No CLI gate was run because the source seam is crate-private and unwired, with no CLI interface or behavior change.

## Documentation impact

- Inspected: repository instructions, architecture, invariants, storage format, maintainer map, current limitations, roadmap, frozen semantic policy/calibration/plan and accepted-turn/vector seams.
- Updated: this implementation record, architecture source-preparation boundary, maintainer routing and Step 6 plan marker.
- Not affected: public API/CLI, storage format, current limitations and roadmap activation; live F8 remains disabled.
- Compliance check: full and changed-from-origin/main documentation policy passed.
- Known gaps: architecture enforcement refresh did not complete; later F8 gates remain unimplemented.

## Architecture impact

- Standards changed: none.
- Ownership/boundaries: Archive owns accepted identity/activity and fragments; vector stores own retained evidence; the facade coordinates a read-only proof; Freshness retains scoring and receipt authority. No ownership moves.
- Pitlord enforcement: coverage guard passed; Lexicon refresh was cancelled before Arcana/Pitlord checks.
- Other verification: root formatting, check and full tests passed, including the five producer fixtures.
- Known gaps: candidate search, receipt durability and live wiring belong to Steps 7–9; full architecture enforcement remains to be rerun.

## Accepted-turn source producer (Step 6)

The internal read-only seam is `Cva::prepare_accepted_semantic_source` in `src/facade/cva_semantic_source.rs`. Call it after Archive fragment and vector-generation publication, not inside synchronous turn ingestion. It takes the exact immutable accepted Node and an explicitly selected retained generation ID. It resolves the REL UUID and original accepted activity position through Archive, rejecting source drift and non-user/non-assistant nodes.

Readiness is proved against the selected generation's Archive cut. All source-containing fragments at that cut must have retained vector bindings; no selected generation, a generation predating the source, absent fragments, or incomplete mapping returns retryable `Pending`. The selected cut stays frozen even when newer fragments/profiles are published. A complete result containing only mixed-turn or invalid vectors is explicitly `Ready` with empty usable evidence and exclusion counts, not pending.

Only a fragment whose start and end are this exact node and whose resolved turn count is one can supply a genuine vector. Mixed-turn embeddings cannot attribute attention to this turn and are excluded with coverage. Providers may materialize turn-only fragments through the existing Archive fragment API and build compatible vectors through the existing generation API; this seam does neither inference nor storage mutation. Retrieval results, summaries, inserted context and Echo/tool streams are not provider inputs.

The returned proof binds owner UUID, immutable Node/content, accepted REL position, full base and semantic policies, profile/generation, sorted fragment/row evidence, and a domain-separated source digest. Stable event identity remains owner + exact conversation/node strings; the codec/event-ID implementation is Step 8. No receipt or effect is applied here, and live activation remains disabled. Step 7 owns target eligibility and complete self-source provenance/cohort exclusion, including asynchronous Memory publication.

Tests exercise pending versus completed-empty, immutable-source conflict and role rejection, exact turn-only evidence, mixed-turn exclusion, generation-cut completeness, stable retained evidence after newer activity/generation, and retry/reopen without mutations.

## Bounded candidate search and target eligibility (Step 7)

`Cva::search_eligible_semantic_targets` is the crate-private candidate adapter. It reads only the current owner REL's Memory vector bindings for the accepted source generation profile, then filters before the usable candidate limit: missing vectors, archived/superseded or unsettled lifecycle states, missing or future Freshness admission, and the complete caller-supplied newly-created source cohort are excluded. Content and grounding provenance matching the exact accepted conversation/node are also excluded. Foreign REL and PHY candidates cannot enter because the adapter only enumerates this Cva's owner-local MemoryStore.

The adapter reports scanned, eligible and excluded counts and returns no partial result on vector-store errors. For each eligible target it evaluates every selected genuine source vector with the frozen numerical cosine routine; `plan_local_effects` retains the strongest match per Memory, ranks by descending cosine and MemoryId bytes, applies the inclusive threshold and candidate limit, then returns MemoryId-sorted local effects. Search remains read-only and does not apply scores, traverse Graph/Community, or publish receipts. The complete self-source cohort is an explicit input so asynchronous Memory publication must be resolved by the coordinator before search is accepted.

Six focused tests cover the filter-before-limit rule, future admission, lifecycle exclusion, source cohort/provenance exclusion, strongest-match dedupe, generation compatibility, pending/empty readiness and read-only reopen stability.

## Notes

This module provides a deterministic pure-policy seam for later accepted-turn production and bounded search. It does not activate semantic reinforcement or change existing Freshness scoring principals.
