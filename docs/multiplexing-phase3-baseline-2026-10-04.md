# Multiplexing Phase 3 — P3.0 baseline

Parent index: [Documentation index](INDEX.md)

## Purpose

Record the accepted foundation, conversation/journal inventory and P3.0 verification.

## Overview

Status: **P3.0 accepted. Required focused gates pass; P3.1 is next.**

Accepted Phase 2 head: `c883f3655f86087f7cfd2d7dab395a3944a6df23`.
Foundation branch: `feat/multiplexing-p34-foundation`.
Worktree: `C:\!bin\workspace\reliquary-multiplexing-p34-foundation`.

The Phase 2 worktree contained uncommitted P3/P4 plans and parent/index edits. Those plans were copied without modifying their originals. A concurrent P4 baseline job was rebuilding dependencies there and timed out after five minutes; this is not a failing conversation test. P3 uses a separate worktree and dedicated build targets.

The shared observation module and its [contract](multiplexing-p34-observation-contract-2026-10-04.md) are added here before branching either lane. No scheduler/policy/generation/feed behavior is changed.

## Source inventory

| Source | Current authority and required follow-up |
| --- | --- |
| `src/runtime_inference/runtime_host_gateway.rs` | One GatewayRuntime owns ConversationRegistry and keyed REL/PHY executions; shared observation module declaration is the only source change here |
| `runtime_host_gateway_conversation.rs` | One registry; REL-keyed journal writer; coordinators keyed by REL/conversation; transient viewer registry; conversation-wide processing flag |
| `multiplexing_journal.rs` | Sole provisional queue; durable ordering, scoped IDs/revisions, cancel/FinalizeWon/Archive/postcommit/host transitions, staged attachments and compaction |
| `runtime_host_gateway_conversation_tests.rs` | Real Archive admission, cancellation, fences, detach/restart, fault recovery; nested attribution and personal-publication tests |
| `multiplexing_journal_tests.rs` | Production filesystem/framing/recovery/ordering/compaction tests |
| `runtime_host_owner_operation_journal.rs` | Durable actor/generation attribution and exact personal publication; Phase 2 ownership remains closed |
| `src/archive/conversation_metadata_model.rs` | Conversation ID/title/active only; policy/revision are not implemented yet |
| `src/archive/interaction_stream_record.rs` and `interaction_stream_persistence.rs` | Existing streaming/interrupted checkpoint persistence; full branch-local generation identity/control remains P3.6 |
| `runtime_host_gateway_observation.rs` | Shared P3/P4 data types only; production builder and transition emission remain P3.7/convergence |
| `tests/multiplexing_phase0b_conversation_contract.rs` | G1 reference oracle; not production proof |
| `tests/multiplexing_phase0b_journal_contract.rs` | Journal reference oracle; not production proof |
| `tests/multiplexing_gateway_api_contract.rs` and `multiplexing_gateway_routing_guard.rs` | Public surface and canonical gateway/legacy boundary |

Runtime-inference paths in this table share the `src/runtime_inference/` prefix.

### Existing process-next boundary

`process_next_conversation_submission` already exists but requires a caller-supplied submission ID. `process_accepted_conversation_submission` checks that ID against `pending_order().first()`; it does not autonomously select work. It uses one conversation-wide processing flag and unresolved-receipt fence, rather than branch-local continuations. Admission, capture and recovery currently require fixed policy revision 1.

P3.3 must hard-cut this explicit selection authority rather than add another scheduler alongside it. P3.1 owns durable policy; P3.2 owns branch-local continuation state. Existing owner-scoped lifetime and Phase 2 personal routing are retained.

## Verification

| Gate | Result |
| --- | --- |
| G1 conversation oracle, rebuilt with rustc from foundation source | 13/13 passed |
| Journal oracle, rebuilt with rustc from foundation source | 6/6 passed |
| Production conversation and journal tests | Included in 908/908 passing library tests (serial Windows run) |
| Public gateway contract and routing guard | 3/3 passed; Cargo G1 and journal fixtures 19/19 passed |
| Locked library check | Passed with isolated target: `cargo check --locked --offline --lib -j 2` |
| Formatting/diff/docs/source ownership | Passed; structural and changed-from-origin/main docs checks passed |

The initial pre-seam Cargo attempt was cancelled during dependency compilation so gates could run on the actual frozen foundation; no test result is inferred from it.

### Execution evidence

Verification ran on 2026-10-05 UTC (2026-10-04 local date used by the plan). Logs and ordered gate results are in `C:\!bin\workspace\multiplexing-p3-baseline-verification` outside the source tree. The first parallel library run hit a Windows sharing violation in the unchanged Lore historical-read test while deleting `.lore`; that test passed alone and all 908 tests passed serially. A check using the concurrent attribution target hit missing dependency artifacts; the clean dedicated `multiplexing-p3-check-target` passed. Neither failed attempt is reported as a passing gate.

## Inherited gaps

Accepted Phase 2 reports 908 library, 93 integration and 19 CLI tests passed. Its unrestricted `archive_open_profile` example build has an existing global-allocator conflict. Its bounded architecture refresh timed out during Lexicon indexing before refreshed Arcana/Pitlord enforcement. These are inherited verification gaps, not P2 authority dependencies, and are not described as green gates.

The P3.0 bounded architecture refresh also timed out after 60 seconds during Lexicon initialization (`analyzing rust` / `analyzing python`). Refreshed Arcana/Pitlord enforcement was not reached. Direct source inventory checks and the Phase 2 ownership guard passed, preserving one registry/coordinator/journal and excluding writable handles or distribution authority from the shared contract.

Gateway assistant-result ingestion remains unsupported until a later implemented seam. Unknown host work is held; P3 must not blindly replay it.

## Exit decision

P3.0 is accepted with no unresolved Phase 2 authority dependency. Both lanes must branch from the common seam commit identified in the Phase 3 branch's acceptance record. P3.1 is the next implementation step.

## Documentation impact

- Inspected: P2/P3/P4 plans, G1/G3/G4 contracts, architecture, invariants, storage format, limitations, roadmap, maintainer map and coverage.
- Updated: this baseline, shared contract, plans/index and current ownership documentation.
- Not affected: public API signatures, REL/PHY semantic bytes, CLI contracts, provider policy and Phase 2 private ownership.
- Compliance check: structural and changed-from-origin/main checks passed.
- Known documentation gaps: none for P3.0; later runtime behavior remains explicitly unimplemented.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: data-only observation contract between existing P3 coordinator/journal and P4 distribution lane.
- Pitlord enforcement impact: no new queue/writer/subscriber authority; existing direct guards retained; bounded refresh result recorded under inherited gaps.
- Other verification impact: baseline gates above.
- Known architectural gaps: producer/snapshot linearization and P4 delivery remain explicitly later work.

## Related docs

- [Shared P34 contract](multiplexing-p34-observation-contract-2026-10-04.md)
- [Phase 3 implementation plan](multiplexing-phase3-conversation-implementation-plan-2026-10-04.md)
- [Phase 2 acceptance](multiplexing-phase2-implementation-plan-2026-10-04.md)

## Notes

This phase establishes a baseline and shared foundation. It does not mark Phase 3 complete.
