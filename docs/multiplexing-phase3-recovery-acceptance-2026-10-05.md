# Multiplexing Phase 3 — P3.8 recovery and hard-cut acceptance

Parent index: [Documentation index](INDEX.md)

## Purpose

Record the completed recovery audit, concrete hard-cut repairs and verification for P3.8.

## Overview

Status: accepted with the documented structural-analysis tool limitation.
Branch: feat/multiplexing-phase3-conversation.
Base: P3.7 a833446.
Scope: P3.8; P3.9 local acceptance remains next.

## Current behavior

The existing runtime/coordinator/journal owners remain canonical. A focused recovery module now owns mount recovery and the validated submission-receipt path shared by quiescence. Both paths verify admitted identity/payload, captured receipt, stable Archive turn and branch before post-commit acknowledgement. Missing publication stays Finalizing for coordinator dispatch; it does not become a fabricated receipt.

Generation recovery validates the previously acknowledged UTF-8 prefix against journal length/hash. A synchronized extension is reconciled before Interrupted or Archive-backed Completed, including the Archive publication handoff. Invalid prefix evidence fails mount and retains nonterminal journal state. Native compute is interrupted after restart; host Unknown and sticky cancellation retain their original identity and branch reservation.

Exact acquisition verifies the durable branch and rejects fresh Pending work inside the coordinator. Automatic selection remains the only production fresh-dispatch path. Exact fault-injection acquisition is explicitly cfg(test), without a production compatibility API.

## Audit results

| Boundary | Canonical owner and evidence |
| --- | --- |
| Scheduler authority | ConversationCoordinator.acquire_next reads durable journal order and reserves branches atomically; exact acquisition independently rejects fresh work. |
| Pending/restart order | RelJournal remains the single provisional queue; equal verified timestamp ties and compaction/restart have existing production-coordinator tests. |
| Branch policy | The focused finalization module decides SequentialChronological versus VisibleAutoBranch from synchronized Archive metadata; captured ancestry remains immutable. |
| Writer/viewer lifetime | attach/select viewer paths create no session; explicit writer admission reuses the one semantic runtime. Repeated mount starts with no viewers or instance selection even with durable active metadata. |
| Recovery reservations | Archive branches plus unfinished journal state rebuild before effects/workers and after generation/receipt reconciliation; conflicting owners fail closed. |
| Host uncertainty | Unknown and cancellation intent survive repeated restart; independent branches advance; authoritative terminal proof releases debt without repeating the original handoff. |
| Source enforcement | The direct guard checks one registry/coordinator/journal construction, finalization and policy sites, and test-only bypass exclusion. Negative fixtures prove rejection and correct cfg(test) handling. |

No persistent schema, public export, gateway root wiring or P4 distribution contract changed. No second queue, writer, coordinator, policy store or viewer-derived authority was added.

## Verification

- Focused conversation suite: 90 passed, including six new recovery/hard-cut regressions.
- Full library/integration matrix: 955 library tests and 95 integration tests across 11 integration targets passed.
- cargo fmt --check and locked offline cargo check passed.
- P2 ownership guard: no gateway violations; P3 ownership guard: no violations.
- Seven P3 source-policy fixture tests passed.
- Both structural and origin/main change-impact documentation checks passed; git diff --check passed.
- Architecture refresh attempted with a five-minute bound. Lexicon init reached Rust/Python analysis but timed out before producing a current Arcana snapshot. The non-refresh check confirmed no current snapshot; Pitlord enforcement did not complete.
- CLI commands and public exports are unchanged, and cli/src has no gateway conversation consumers. CLI checks were not required or run for this internal change.

## Documentation impact

- Inspected: AGENTS, shared engineering guidance, architecture, invariants, storage format, maintainer map, limitations, roadmap, API, behavioral contracts, coverage, P3 plan and P3.6/P3.7 evidence.
- Updated: this record, index, P3 and parent plans, roadmap, architecture, invariants, storage format, maintainer map, limitations, API, behavioral contracts and coverage.
- Not affected: P4 sequencer/subscriber implementation, PHY ownership, CLI commands and frozen observation shapes.
- Compliance check: both structural and origin/main change-impact modes passed, including after final evidence updates.
- Known documentation gaps: none in the implementation scope.

## Architecture impact

- Standards added or changed: none; a repository-local P3 source guard and negative fixtures were added.
- Ownership or boundary impact: recovery moved into a focused module beneath the existing conversation owner; mount/quiescence share validation and exact acquisition enforces the hard cut.
- Pitlord enforcement impact: refresh was attempted but Lexicon timed out before current Arcana evidence; no completed Pitlord check is claimed. The direct lexical guard passed and supplements structural enforcement.
- Other verification impact: six production regressions plus seven source-policy fixture tests; existing policy, finalization, ties, branch, checkpoint and public contracts remain in the gate matrix.
- Known architectural gaps: structural analysis remains blocked by the Lexicon/Arcana tooling limitation. P3.9 local acceptance and P3/P4 convergence remain separate; no transport/provider inference implementation is claimed.

## Related docs

- [P3 execution plan](multiplexing-phase3-conversation-implementation-plan-2026-10-04.md)
- [P3.7 snapshot evidence](multiplexing-phase3-observation-acceptance-2026-10-05.md)
- [P3.6 generation evidence](multiplexing-phase3-generation-acceptance-2026-10-05.md)
- [Behavioral contracts](behavioral-contracts.md)

## Notes

The source guard is a concrete lexical check, not a Rust parser. The inherited unrestricted Cargo example allocator conflict remains outside the explicit library/integration matrix. P3.8 does not claim final Phase 3 acceptance or P4 convergence.
