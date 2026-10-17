# Historical first-cycle Dream producer verification — 2026-10-12

Parent: [Documentation index](INDEX.md).

## Purpose

Record Step 18 / H4 of the [linear implementation sequence](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md).

## Overview

`src/freshness/historical_dream.rs` produces at most one +50 multi-root import event per qualifying source Memory at its verified cohort birth position. Completed first-cycle root proof takes precedence over completed graph adjacency. An explicit empty proof records VerifiedNoLink and never falls back; a settled marker without retained proof records Unsupported, not invented empty evidence. Nonempty actual proof with no historically eligible roots still does not fall back.

Without actual proof, only explicitly completed direct Dream-origin links supply approximate roots. Direct-publication evidence must distinguish links from duplicate-chain rewires: Graph origin alone does not establish this. User/Perception links, incomplete publications and pure rewires do not create events. The producer indexes qualified links and cohort membership once, rather than rescanning the entire graph/population for every source.

Root aliases fold to representatives supplied by Dream's existing authority. Only credit-eligible canonical targets with strictly earlier historical availability qualify; same-position targets, source/self and birth-cohort members are excluded. The event carries the complete source/cohort credit-exclusion set for Step 17's recipient-only propagation. A final archived/credit-excluded source can originate an actual first-cycle event only with explicit verified duplicate-origin proof and supported historical birth provenance; it never receives its own credit and never acquires fallback graph history. Unknown/foreign declared IDs, conflicting cohorts and unflattened representative mappings reject.

Event IDs implement the frozen H1 `reliquary:historical-dream:v1` typed-map encoding over owner UUID, import ID and originating MemoryId. Roots, positions, proof digests and bonuses do not change identity. The actual proof digest travels with the plan for later manifest/conflict checks. An independent Python standard-library encoding produces the golden `4934d474875b806cf6577542b76a697b616c4126f66e10e404e91e04556801bc` for fixture owner `[1;16]`, import `[2;32]`, source `[1;32]`.

## Owner evidence adapters

`Cva::historical_completed_initial_dream_passes()` reconstructs completed source/roots from existing validated Dream journal bytes through a disposable scan. It requires a durable REL identity and modern accepted graph-cut proof. Normal resident journals still drop settled pass bodies; no new durable record, resident history cache or schema is added. Legacy/transformed settled-only markers provide no completed root proof. The coordinator must distinguish a settled-but-unavailable proof from absent first-cycle history, using the existing settled-pass query; absence from this returned map does not prove no-link.

`Cva::historical_dream_representatives(source_id, roots)` uses `existing_duplicate_representative` on the pinned completed graph. It intentionally removes the physical publication-order cutoff (using the maximum version bound), while the pure producer enforces historical birth order. Existing durable-birth, same-publication-cohort and canonical representative authority remain intact. A duplicate root without a verified representative rejects instead of selecting an arbitrary chain endpoint.

These APIs perform no inference, publish no score/event and do not settle or rewrite Dream journals. The import coordinator still owns corpus completion, exact proof/cohort binding, direct-versus-rewire evidence, verified duplicate-origin classification, owner authorization and manifest freeze. These input-proof obligations are not satisfied by an unverified boolean or arbitrary representative map.

## Verification

Focused fixtures cover one event with many roots, no edge multiplication, proof/no-link precedence over later adjacency, pure rewires, non-Dream/incomplete links, archived duplicate sources, canonical target dedupe, source/cohort exclusions, strict historical age, owner/missing-reference errors, cohort conflicts, stable identity and an independent identity golden.

A real owner fixture publishes linked and no-link first cycles, reads their completed roots after normal resident settlement, resolves representatives, and verifies byte-identical source files before/after proof reads and across reopen.

## Documentation impact

- Inspected: architecture, invariants, storage format, maintainer map, limitations, roadmap, frozen historical manifest policy, historical design and linear sequence.
- Updated: API, architecture, limitations, maintainer routing, development checks, index, linear sequence and this verification record.
- Not affected: storage bytes, ordinary Dream/use principals, live semantic locality and PHY.
- Compliance check: final results below.
- Known documentation gaps: scoring, superseding checkpoint, transforms and coordinator remain later steps.

## Architecture impact

- Standards added or changed: none; frozen proof precedence and ownership contract implemented.
- Ownership or boundary impact: Dream journal owns proof decoding; Dream authority owns representatives; Freshness owns pure historical event planning.
- Pitlord enforcement impact: existing policy unchanged; refreshed gate uses the Step 12 compatible tool pair.
- Other verification impact: real journal/reopen fixture plus pure production policy oracles; no new model inference.
- Known architectural gaps: no historical score publication or permanent event queue; direct-publication/duplicate-origin proofs and snapshot authorization are still coordinator input obligations. Legacy/transformed marker-only histories remain unsupported as actual root proofs.

## Related docs

- [Historical manifest policy](freshness-historical-manifest-policy.md)
- [Step 17 propagation verification](freshness-historical-propagation-verification-2026-10-12.md)
- [Storage format](storage-format.md)

## Notes

No historical +25 accepted use is inferred. Approximate graph fallback is deliberately scoped; missing proof is never repaired by silently inventing provenance or an activity position.

## Final results

- Root formatting and locked compile passed, including the final indexed implementation.
- `cargo test --locked -j 2` passed 1,136 library tests (one existing ignored measurement), every integration target and doc tests. The final equivalent indexing optimization and strengthened begin-before-relation fixture ordering were subsequently verified with the focused suite.
- Final `cargo test --locked --lib historical_dream -j 2` passed all eight fixtures, including the real completed-proof/reopen owner test and typed-encoding golden.
- CLI formatting, locked check and locked tests passed; all 19 CLI tests passed.
- Refreshed architecture enforcement passed all six rules with the compatible Lexicon/Arcana tool pair, 65,514 source nodes and zero extracted relationships. Zero relationships limits static dependency evidence; ownership/delegation and behavioral fixtures remain explicit.
- Both documentation-policy modes and `git diff --check` passed.

No unresolved test failure remains. Existing unused-code and MSVC runtime-link warnings are unchanged. Verification logs and process-local temporary test output remain inside the worktree under ignored `target/step18-*.log` and `target/test-tmp` paths.
