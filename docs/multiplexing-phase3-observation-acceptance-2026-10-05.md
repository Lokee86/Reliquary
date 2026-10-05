# Multiplexing Phase 3 — P3.7 authoritative conversation observation

Parent index: [Documentation index](INDEX.md)

## Purpose

Record the internal authoritative conversation snapshot, its durability and lock boundary, and its acceptance gates.

## Overview

Status: accepted with the structural-analysis tool limitation recorded below.
Branch: feat/multiplexing-phase3-conversation.
Base: P3.6 9861f7e.
Scope: P3.7; P3.8–P3.9 remain planned.

## Current behavior

GatewayRuntime::conversation_observation_snapshot(instance, owner, conversation) produces the frozen ConversationObservationSnapshot within the gateway module boundary. It requires current ViewRel and exposes no public re-export. The requesting instance supplies selection; an instance selected on another conversation contributes no selected branch/head. Archive branch heads and canonical policy are read directly, with no ambient session lookup or session creation.

The builder acquires the mounted semantic runtime through with_rel, then the existing coordinator state and REL journal through ConversationCoordinator::observe. All three remain locked while the existing CVA owner is synchronized and the projection is copied. The order is runtime -> coordinator -> journal. Scheduler reservation releases coordinator/journal locks before acquiring the runtime; observation never takes a journal lock and then requests a semantic owner. No provider call or second writer occurs. Sync errors fail closed. This durability barrier does not append records, allocate a version or alter journal state.

Selection is read while the durable owners remain locked. After releasing those locks, with_admission revalidates current attachment/grant generation and mount/fence authority before returning the copied result. Selection is a projection at the snapshot cut, not a persistent conversation-global authority or a guarantee against later selection changes.

The snapshot includes all scoped journal submissions, including compacted terminal tombstones, ordered by verified receive time, persisted tie key and admission sequence. The focused journal accessor exposes only branch identity: captured target, generation target or retained admission branch metadata. The existing journal already retains this metadata during compaction; no format change is required.

Archive inventory includes only actually published branches. A FinalizeWon target does not create a visible branch before Archive publication. If Archive publication precedes its journal receipt, the snapshot accurately shows the new head alongside Finalizing; it does not infer Committed. P3-created automatic branch labels and fork parents come from retained creation captures. Legacy/manual branches without such captures have absent optional metadata.

Native generation identity/phase/transition revision and durable checkpoint sequence remain visible for active, interrupted and terminal operations. Host operation identity/capability/phase/revision and cancellation intent preserve Unknown faithfully. Collections are deterministic: branch ID; generation branch/operation; host submission/operation. Bodies, attachments, request metadata, principal IDs, payload digests, checkpoint hashes and stream text do not enter this contract.

## Persistent and convergence contract

Frozen observation shapes, public exports, Archive format and journal events are unchanged. The REL-global journal sequence is a durable source watermark; it cannot identify Archive publication on its own and is not a P4 delivery cursor. P4 snapshot/register linearization, transition emission, broad REL adapters and exact checkpoint-content delivery remain convergence work. The builder proves a plain P3 cut only.

## Verification

Library: 949 passed, including all seven new snapshot tests. Integration: 95 passed across 11 targets. CLI: 19 passed. Locked/offline library and CLI checks, both formatting checks, source ownership enforcement (zero gateway violations), both documentation-policy modes and git diff --check passed. Doc tests completed with zero cases.

The first CLI test attempt failed with E0463 for ICU dependency artifacts in its separate target cache. The successful rerun used the target directory verified by the library/integration matrix and completed its dependency rebuild in 14 minutes. Repository source and lockfiles were not changed to address that cache failure. Verification logs are under the sibling multiplexing-p3-7-verification directory.

The seven focused production tests cover scoped durable order and terminal compaction/privacy; instance-local selection independent of ambient runtime cursors; FinalizeWon/Archive publication/commit/postcommit handoffs and retained automatic branch metadata; checkpoint and Interrupted identity across restart; owner-before-journal concurrency; current ViewRel/detach and absence of session/semantic-clock mutation; independent branch generation completion plus Unknown/cancelled host operation status. Existing full conversation/journal tests provide the surrounding lifecycle, source integrity and restart cases.

## Documentation impact

- Inspected: architecture, invariants, API, storage format, maintainer map, behavioral contracts, limitations, documentation coverage, roadmap, overall/P3 plans, P3.6 evidence and frozen P3/P4 contract.
- Updated: this record, P3 execution plan, overall multiplexing plan, index, architecture, invariants, API, storage format, maintainer map, behavioral contracts, limitations, documentation coverage and roadmap.
- Not affected: public exports, CLI commands, PHY ownership, P4 cursor/subscriber implementation and frozen observation shapes.
- Compliance check: structural and changed-from origin/main documentation checks passed.
- Known documentation gaps: none within the plain P3.7 snapshot scope.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: the focused observation module reads the existing semantic runtime, coordinator and mounted REL journal; no duplicate mutable domain owner or store is added.
- Pitlord enforcement impact: the 300-second Lexicon refresh timed out while indexing Rust/Python. The subsequent non-refresh check confirmed there is no current Arcana snapshot. Source ownership enforcement passed with zero gateway violations. No completed Pitlord result is claimed.
- Other verification impact: durable handoff, privacy, authority, restart, independent operation and deterministic concurrency tests.
- Known architectural gaps: structural enforcement remains blocked by the indexing limitation above. P4 registration/delivery integration and P3.8 recovery audit remain separate; legacy/manual branch metadata absence and snapshot lock duration are documented limitations.

## Related docs

- [P3 execution plan](multiplexing-phase3-conversation-implementation-plan-2026-10-04.md)
- [P3.6 acceptance](multiplexing-phase3-generation-acceptance-2026-10-05.md)
- [Shared observation contract](multiplexing-p34-observation-contract-2026-10-04.md)
- [Current limitations](current-limitations.md)

## Notes

P3.7 is accepted with the explicit structural-analysis limitation above. This phase does not claim the atomic subscribe/register handshake or full Phase 3 acceptance. The inherited unrestricted cargo test example allocator conflict is unchanged; the library/integration/CLI matrix supplies the regression gate.
