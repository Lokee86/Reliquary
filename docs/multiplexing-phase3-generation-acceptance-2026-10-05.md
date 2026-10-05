# Multiplexing Phase 3 — P3.6 generation and control lifetime

Parent index: [Documentation index](INDEX.md)

## Purpose

Record branch-local generation ownership, durable checkpoints, independent viewer lifetime, cancellation authority and restart acceptance.

## Overview

Status: accepted with the architecture-tool limitation recorded below.
Branch: `feat/multiplexing-phase3-conversation`.
Base: P3.5 `d2d7b8c`.
Scope: P3.6; P3.7–P3.9 remain planned.

## Current behavior

The existing ConversationCoordinator reserves a branch while the REL journal records its nonterminal generation. Generation identity is deterministic from owner, conversation and accepted submission identity. Starting a reply requires a fully committed user submission and the current branch head; historical submissions cannot reserve a branch after it advances.

Native owner execution uses start, checkpoint and completion operations on GatewayRuntime. These trusted accepted-work operations use owner mount/fence authority and continue after submitter detach. They do not initiate model inference themselves. Checkpoint, completion and native interruption acquire the same branch continuation lease and reread durable state before changing it. Pending submissions on that branch wait; independent branches remain eligible. Viewer attachment creates no writer and viewer count never cancels a generation.

CancelOwnPendingSubmission applies only to the initiating principal's undispatched Pending turn. CancelSharedGeneration controls an already dispatched operation by stable operation ID and cannot cancel a foreign undispatched submission. Control uses current attachment/grant authority; native interruption synchronizes the preserved stream before publishing terminal Interrupted. Routed host work continues through the existing capability router. Missing host authority or uncertain cancellation records Unknown and retains branch debt.

A checkpoint is a full UTF-8 prefix. Its sequence is contiguous, duplicate retries require the same hash/length, and new prefixes must extend the synchronized content. Interaction-stream persistence and sync precede the journal checkpoint identity. The journal exposes only sequence, byte length and SHA-256; it allocates no P4 delivery cursor.

Completion validates the durable prefix, writes one stable principal-tagged assistant node, synchronizes branch publication, runs existing Episode scheduling and then records Completed. Archive identity and receipt validation prevent duplicate assistant publication. Interruption preserves partial stream content outside Archive.

Restart treats local compute as lost. It reconciles any stream prefix synchronized before its journal marker, records Interrupted and releases branch debt. If the assistant node was already durably written, recovery validates its identity/content, completes branch publication and scheduling, and records Completed. Unknown host work remains reserved and is never blindly replayed.

## Persistent contract

The existing REL journal adds optional generation state; older entries default to none. GenerationUpdated stores immutable operation/generation/branch/parent/start identity, a contiguous transition revision, phase and optional checkpoint sequence/length/hash. The existing interaction-stream owner stores checkpoint text. Compaction preserves nonterminal generation payloads and retains terminal generation identity/status in journal tombstones. No second queue, coordinator or stream store is introduced.

Public receipt types are exported as GatewayGeneration, GatewayGenerationPhase and GatewayGenerationCheckpoint. conversation_generation_status requires ViewRel and provides the minimal reconnect read; P3.7 still owns the full authoritative observation snapshot.

## Verification

Library: 942 passed. Integration: 95 passed across 11 targets (the integration invocation also repeated all 942 library tests). The focused generation filter passed 19 tests before the final full suite. CLI: 19 passed. Library and CLI cargo check, formatting checks, source ownership enforcement, both documentation-policy modes and git diff --check passed. Verification jobs reached their ten-minute limit during CLI dependency compilation; the CLI test gate passed after clearing the affected Lore/windows-sys cache. The resumed build initially encountered a Rust linker panic because its windows-sys archive was missing; dependency cache regeneration resolved it. Tests cover detach and zero viewers, a late authorized viewer, independent branch dispatch, checkpoint integrity and retries, cancellation permissions, restart and publication handoffs. The required library/integration/CLI and source-policy/documentation matrix is collected; structural architecture enforcement remains blocked as recorded below.

## Documentation impact

- Inspected: architecture, invariants, API, storage format, maintainer map, behavioral contracts, limitations, coverage, roadmap, P3 plan, P3.5 evidence and the frozen P3/P4 observation contract.
- Updated: this record, P3 execution plan, index, architecture, invariants, API, storage format, maintainer map, behavioral contracts, limitations, coverage and roadmap.
- Not affected: P4 sequencing/subscribers, PHY ownership and CLI commands.
- Compliance check: structural documentation and changed-from origin/main checks passed.
- Known documentation gaps: none in the P3.6 lifecycle scope.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: generation/control deepens the existing coordinator and journal; focused generation module owns no separate state.
- Pitlord enforcement impact: gateway source ownership checks include the new module; Lexicon refresh exceeded the 180-second job timeout during Rust/Python indexing. The non-refresh architecture check confirmed there is no current Arcana snapshot. Source ownership enforcement passed with zero gateway violations; no completed Pitlord result is claimed.
- Other verification impact: separate pending and dispatched cancellation contracts and deterministic recovery tests.
- Known architectural gaps: full observation builder and recovery audit remain P3.7/P3.8; provider inference and host assistant-result ingestion are outside this lifecycle API.

## Related docs

- [P3 execution plan](multiplexing-phase3-conversation-implementation-plan-2026-10-04.md)
- [P3.5 acceptance](multiplexing-phase3-auto-branch-acceptance-2026-10-05.md)
- [Shared observation contract](multiplexing-p34-observation-contract-2026-10-04.md)

## Notes

P3.6 is accepted with the explicit structural-analysis tool limitation above. P3.7 observation and P3.8 audit remain separate. The known unrestricted cargo test example allocator conflict is unchanged; verification uses the library/integration/CLI matrix, as in P3.5.
