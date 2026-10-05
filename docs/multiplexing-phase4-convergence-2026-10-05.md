# Multiplexing Phase 4 — P4.7 P3/P4 convergence

Parent index: [Documentation index](INDEX.md)  
Parent plan: [Phase 4 distribution plan](multiplexing-phase4-distribution-implementation-plan-2026-10-04.md)  
P3 source: [P3.7 observation acceptance](multiplexing-phase3-observation-acceptance-2026-10-05.md)  
P4 stream foundation: [P4.6 stream hub](multiplexing-phase4-streams-2026-10-05.md)  
Status: **P4.7 production bindings and verification complete.** The convergence base `f14b733` merges P3.7 and P4.6 prerequisites; it is not the P4.7 implementation commit.

## Purpose

Record the production binding between P3's durable conversation authority and P4's already bounded REL and exact-stream distribution. This is an integration record, not P3.8/P3.9 acceptance or P4 completion.

## Overview

The convergence base `f14b733` merges the P3.7 line (`a833446`) with P4.6 (`c0bbd8e`), preserving prerequisite history; P4.7 implementation changes are subsequent working-branch changes, not contained in that base commit. P3 supplies reusable authoritative snapshot, receipt, and checkpoint builders from the existing runtime, coordinator, and REL journal. P4 retains all sequencers, subscribers, queues, event cursors, and exact-stream cursors. Nothing moves into the coordinator as subscriber or delivery state; no public library exports are added in this step.

Conversation facts use two distinct surfaces. The conversation_observation builder produces the canonical P3 summary for broad REL snapshots and body-free queue, policy, published-branch, generation, and host-status deltas. Validated checkpoint prefix/delta content is delivered only through its exact conversation stream. Broad events never carry transcript/checkpoint text or private PHY bodies.

## Named durable transitions and staging

Production hooks capture and publish facts at named durability boundaries:

| Transition | Source fact and distribution |
| --- | --- |
| FinalizeWon | The journal's frozen finalization receipt supplies stable submission, branch, and policy facts; broad queue/operation summaries are published after its durable transition. |
| Synced Archive branch-head publication | After syncing an actually published branch head, emit the safe generic ArchiveChanged/Conversation invalidation immediately, before a journal receipt exists. Emit BranchCreated only when the new branch is actually published; a frozen FinalizeWon capture alone is insufficient. |
| ArchiveCommitted | Emit the typed ArchiveCommitted fact only when the exact journal receipt is recorded; do not infer it from the earlier ArchiveChanged event or Archive head. |
| PostCommitRecorded | The journal receipt supplies the durable completion/host outcome; broad status and queue summaries follow recording. |
| Checkpoint/status persistence | The exact stream receives only validated, synchronized checkpoint/status facts. Broad REL summaries contain status and safe identities only. |

The stage helpers (`with_accepted_work` and `with_conversation_admission`) keep the mounted P4 publication gate across each short persistence, source capture, and publication stage. Provider/model calls between stages remain outside that gate and outside owner/journal lock spans. Admission/fence checks remain at each stage. A rejected or unchanged operation emits nothing. A partial persistence, source-capture, or publication failure invalidates distribution continuity; an already successful durable mutation result is preserved rather than reported as rolled back.

## Exact stream reconstruction and bounds

The stream source uses a full validated durable prefix for join/recovery, then emits deltas only after the corresponding checkpoint and journal receipt are durable. Archive content can be ahead of the journal after an interrupted handoff; delivery trims to the journal-authoritative UTF-8 length and verifies the journal's SHA-256 before exposing the prefix. It does not infer commit from Archive state alone.

P4 retains its bounds: broad snapshots are capped at 16,384 source items with at most 128 notifications per publication cut; 256 stream identities per epoch; 1,024 recipients; 128 queued events and 1 MiB queued text per recipient; and a 1 MiB checkpoint/prefix limit. Resync is bounded and fail-closed. No universal event log or chunked large-prefix transfer is introduced. Internal production subscribe/reconnect wrappers remain internal; public-export migration is later work.

## Verification status and inherited gaps

**P4.7 production implementation and verification are complete.** The focused convergence suite passed 12/12, included in the `cargo test --tests` matrix (1,023 library and 95 integration tests, zero failures or ignored tests). Runtime and CLI `cargo check`, root and CLI formatting checks, 19 CLI tests, the coverage guard (9 fixtures/112 APIs), and both documentation-policy modes passed. The unrestricted `cargo test` example path still has the inherited allocator conflict in `archive_open_profile`; it is outside the passing `--tests` matrix. The final suite includes the receipt fork-origin metadata correction and corrected legacy snapshot-hook fixture.

The architecture refresh was attempted with a 120-second bound and timed out while Lexicon analyzed Rust/Python; Arcana/Pitlord did not run, so no refreshed architecture pass is claimed. The unrestricted `cargo test` example path still has the inherited allocator conflict in `archive_open_profile`; the passing test matrix used `--tests`. P3.8 recovery cleanup and P3.9 local acceptance remain pending. P4.8 coverage/lock/privacy audit is next; P4.9 Phase 4 acceptance remains pending.

## Documentation impact

- Inspected: P3/P4 plans and acceptance records, frozen observation contract, architecture, invariants, maintainer map, limitations, roadmap, behavioral contracts, coverage map, and shared documentation standards.
- Updated: this record, index, P4 plan, architecture, invariants, maintainer map, behavioral contracts, limitations, roadmap, and coverage map.
- Not affected: public API shape, durable encodings, independent P3/P4 source ownership, and frozen observation types.
- Compliance check: both documentation-policy modes passed.
- Known documentation gaps: none for P4.7; the architecture refresh remains incomplete as stated below.
- Status: P4.7 implementation and verification are complete; later P3/P4 gates remain pending.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: P3 remains authoritative for durable conversation state and source observations; P4 remains authoritative for all distribution state.
- Pitlord enforcement impact: inherited Lexicon refresh timeout; refreshed architecture enforcement remains incomplete.
- Other verification impact: 1,023 library tests, 95 integration tests, 19 CLI tests, runtime/CLI checks, formatting checks, 9-fixture/112-API coverage guard, and both documentation-policy modes passed; focused convergence tests passed 12/12.
- Known architectural gaps: bounded Lexicon analysis timed out before Arcana/Pitlord; P3.8/P3.9 and P4.8/P4.9 remain.
- Status: no new persistent owner, coordinator, or public distribution API.


## Related docs

- [Phase 4 distribution plan](multiplexing-phase4-distribution-implementation-plan-2026-10-04.md)
- [P3.7 observation acceptance](multiplexing-phase3-observation-acceptance-2026-10-05.md)
- [P4.6 conversation stream hub](multiplexing-phase4-streams-2026-10-05.md)
- [Current limitations](current-limitations.md)

## Notes

This record describes P4.7 implementation and verification. The P4.7 gates listed above passed; this does not claim Phase 4 acceptance. P3.8/P3.9 and P4.8/P4.9 remain tracked above.
