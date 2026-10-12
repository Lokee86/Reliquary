# Historical import propagation adapter verification — 2026-10-12

Parent: [Documentation index](INDEX.md).

## Purpose

Record Step 17 / H3 implementation of the import-only propagation adapter.

## Overview

`src/freshness/historical_propagation.rs` coordinates pure planning against an explicitly authorized, pinned owner-local graph. `HistoricalPropagationEvent::from_semantic` converts the whole Step 16 source turn into one event, verifies the shipped F8 policy digest, and takes the strongest duplicate root bonus. Semantic roots are bounded to 64 with bonuses 1..10; historical first-cycle Dream roots require +50. Only the frozen base Freshness default-v1 policy is supported. The future Dream producer must supply its first-cycle proof; this numerical adapter does not invent proof or events.

The existing deterministic frontier in `src/freshness/propagation.rs` now accepts crate-private per-root strengths. Ordinary live entry points retain their existing principal validation and traversal exclusions. Import traversal builds the authorized induced graph once per batch and never prunes future, unsupported, lifecycle-ineligible, self or cohort nodes for temporal/credit reasons. After traversal it awards only Supported recipients with positive availability <= event position, excluding the explicit source/cohort credit set. Missing availability gives no credit. Authorization removes inaccessible nodes from traversal and rejects unauthorized roots. Isolated authorized roots remain valid even without Graph adjacency.

Each root retains its originating Community across the entire path, including outside hops and reentry. Same-origin searches share the strongest frontier; origins merge by maximum. Roots, paths and fragments do not sum within an originating source event. Independent events retain input order and use the existing bounded event-worker budget with no nested pools. Returned traversal statistics describe the full authorized traversal; touched_memories counts credited recipients.

This is an in-memory planning API, with no new storage, scoring publication, live semantic propagation, permanent event queue or inference. Caller responsibilities remain pinning owner/graph/Community evidence, root provenance/eligibility, event identity and whole-turn grouping. Later scoring/checkpoint steps preserve stable event identities and apply scores.

## Verification

Fixtures exercise variable bonuses and max aggregation, isolated roots, cycles, fixed-origin Community costs and reentry, unknown membership, unauthorized bridges/roots, future and unsupported bridges, self/cohort credit exclusions, one-vs-many workers over 32 events, Step 16 duplicate-root handoff/policy mismatch, unsupported base policies and unchanged ordinary principal/excluded-bridge behavior.

With v1 semantic bonuses <=10 and local hop cost 5, positive credit can reach at most one edge; a two-edge semantic bridge necessarily has zero remaining strength. The future-node bridge oracle uses a +50 historical Dream event. Step 19 must preserve this arithmetic limit rather than increase semantic bonuses or weaken hop costs to fabricate a positive two-hop semantic oracle.

## Documentation impact

- Inspected: architecture, invariants, storage, maintainer map, limitations, roadmap, frozen historical manifest policy, source spec and linear sequence.
- Updated: API, architecture, limitations, maintainer routing, development verification, linear sequence and this record.
- Not affected: durable formats, live F8 locality, ordinary +25/+50 principals and PHY scope.
- Compliance check: final results recorded below.
- Known documentation gaps: historical Dream production, scoring, checkpoint and coordinator remain Steps 18–25.

## Architecture impact

- Standards added or changed: none; existing authorization and owner boundaries are preserved.
- Ownership or boundary impact: pure Freshness import adapter reuses the shared numerical frontier; Archive and Graph remain authorities for pinned source/topology evidence.
- Pitlord enforcement impact: no policy changes; refreshed enforcement is run with the Step 12 compatible tool pair.
- Other verification impact: shared frontier/live regressions and import-only fixtures.
- Known architectural gaps: adapter inputs are planner evidence supplied by the caller, not an authorization service or accepted checkpoint; later owners must revalidate pinned proofs. No PHY or cross-owner propagation.

## Related docs

- [Historical manifest policy](freshness-historical-manifest-policy.md)
- [Step 16 semantic planning verification](freshness-historical-semantic-event-planner-verification-2026-10-12.md)
- [Linear implementation sequence](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)

## Notes

Historical reconstruction remains incomplete until the later scorer, atomic checkpoint and coordinator gates pass. This step neither enables it by default nor changes routine live semantic reinforcement.

## Final results

- `cargo fmt --check` and `cargo check --locked -j 2`: passed.
- `cargo test --locked -j 2`: passed 1,126 library tests (one existing ignored measurement), all integration targets and doc tests. This run preceded the final two budget/recipient-breadth fixtures and the equivalent replacement of copied limits with canonical policy constants.
- Final `cargo test --locked --lib propagation -j 2`: 19 passed, including all seven new import fixtures and the shared live frontier regressions.
- CLI formatting, locked compile and locked tests: passed; 19 CLI tests.
- Refreshed architecture gate with compatible Lexicon/Arcana: passed all six rules, with 65,294 source nodes and zero extracted relationships. This limits static dependency evidence; behavioral fixtures cover the bridge/authorization semantics.
- Structural and committed change-impact documentation checks and `git diff --check`: passed.

A focused rerun briefly failed with Windows LNK1104 because the full-suite executable was still running. The final focused rerun completed successfully after that process exited. No test failure remains. Existing unused-code and MSVC runtime-link warnings remain unrelated to this step. Local verification logs are under the worktree's ignored `target/step17-*.log` paths.
