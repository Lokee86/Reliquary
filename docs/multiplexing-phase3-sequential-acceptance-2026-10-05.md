# Multiplexing Phase 3 - P3.4 sequential finalization

Parent index: [Documentation index](INDEX.md)

## Purpose

Record default sequential finalization on the real Archive path and its recovery evidence.

## Overview

Status: **P3.4 accepted with the architecture-refresh limitation recorded below.**
Branch: `feat/multiplexing-phase3-conversation`.
Base: P3.3 `38a462b7291e284060bcb8ae3f11c941839a1be6`.
Scope: P3.4 only; P3.5-P3.9 remain planned.

## Current behavior

Fresh admission validates the expected parent against the selected Archive branch head, rather than accepting a different branch's leaf or rejecting a valid head merely because another branch extended its prefix. New branch/empty-conversation admission retains the existing explicit leaf precondition. Accepted retries retain their original semantic preconditions: the journal verifies exact immutable request identity even after head advancement and payload compaction.

Sequential dispatch validates the admitted expected parent against the selected branch ancestry, then captures its current safe head, branch, principal, timestamp and policy revision in durable FinalizeWon. The conversation transition revision remains the journal's optimistic transition authority. An ambient session cursor never chooses ancestry. Invalid/unrelated ancestry fails before finalization and leaves the entry Pending.

Archive publication checks that the selected branch still points to the captured parent or the stable committed node. Publication and recovery fail closed instead of rewinding an independently advanced branch. Stable node identity prevents duplicate turns and Activity increments. Branch publication synchronizes even on an idempotent retry, covering an in-memory branch installed before an earlier sync failed. An Archive receipt is recorded only after Node/branch synchronization. Postcommit Episode and Insomnia scheduling retains existing deterministic identities and sync-before-marker behavior.

The added finalization module contains read-only Archive preconditions under the existing owner runtime lock. It adds no queue, writer, coordinator, policy store or P4 delivery authority. VisibleAutoBranch conflict handling remains P3.5.

## Verification

Collected against the final production sources on 2026-10-05 UTC:

| Gate | Result |
| --- | --- |
| Focused finalization regressions | 7 passed; restart table includes 7 handoffs |
| Full library tests, serial | 931 passed |
| Integration tests, serial | 95 passed across 11 targets |
| CLI tests, serial | 19 passed |
| Library/CLI compile checks, locked/offline | Both passed |
| Library/CLI formatting | Both checks passed |
| Phase 2 ownership enforcement | Passed |
| Finalization source guard | Passed: read-only preconditions, live/recovery head checks, idempotent sync and test-only exact processing |
| Documentation structure/change impact and diff whitespace | Passed |
| Architecture refresh | Timed out after 120 seconds during Lexicon analysis; refreshed Pitlord was not reached |

Tests use locked/offline dependencies, serial test execution and separate library/CLI target directories. The unoptimized dev/test profiles suppress debug symbols through `CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0`; optimizations and debug assertions were not changed. Compile checks used the ordinary dev profile.

Initial attempts using the inherited mixed target failed inside Lore with a Windows archive lock and a missing dependency archive. Fresh separate targets completed successfully; no dependency source or lockfile changed. The architecture process and its Lexicon adapter children were stopped after the bounded timeout. The inherited unrestricted `archive_open_profile` example allocator conflict remains outside the selected library/integration gates; this record does not claim unrestricted whole-repository test acceptance.

Logs are retained in `C:\!bin\workspace\multiplexing-p3-4-verification`. The final focused run was collected from Workspace job `job_7z3QG5YtwRfc5yGDy1bF4XMY`; full library/integration, CLI and architecture results were collected from `job_muJl8ka4qdG5WovxZr83YTsG`.

Seven focused regressions cover branch/conversation mismatches, queued sequential ancestry, provisional-effect exclusion, unrelated retained parents at dispatch, immutable captured publication, committed retries after compaction and valid shared-prefix branch heads. The restart table exercises seven application-level handoffs: admission, FinalizeWon, synced Node before branch publication, synced branch before Archive receipt, Archive receipt before scheduling, scheduled effects before postcommit marker, and completed marker. It proves one turn/Activity increment, immutable capture, stable Episodes and Insomnia work, terminal idempotency, and no Pending turn/Activity/Episode leakage. It uses real journal/Archive operations and fresh gateway mounts, without orderly-unmount reconciliation erasing the simulated cut.

The existing barrier race now uses public automatic dispatch. Existing stale-fence/remount and attachment/host recovery regressions remain part of the library gate. These application-level restart cuts do not simulate arbitrary operating-system or device failures inside a single low-level append/sync operation; existing Archive/journal corruption and torn-write tests cover their own boundaries.

## Documentation impact

- Inspected: AGENTS, shared ownership/concurrency standards, architecture, invariants, storage format, maintainer map, limitations, roadmap, API, behavioral contracts and P3 plan.
- Updated: this record, P3 plan, architecture, invariants, API, limitations, maintainer map, behavioral contracts and documentation index.
- Not affected: persisted codecs/journal schema, shared P4 observation shapes, PHY behavior and CLI commands.
- Compliance check: documentation structure, change impact against origin/main and diff whitespace checks passed.
- Known documentation gaps: P3.5-P3.9 remain explicitly planned.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: existing coordinator/journal/runtime locks and Archive head authority retained; a focused finalization module owns read-only preconditions.
- Pitlord enforcement impact: ownership enforcement and the finalization source guard passed. Architecture refresh timed out during Lexicon analysis before refreshed Pitlord was reached.
- Other verification impact: seven finalization regressions, seven restart cuts and public cancel/finalize race.
- Known architectural gaps: refreshed Pitlord remains unverified after the bounded Lexicon timeout. Native generation/checkpoint/control remains P3.6; transport delivery remains P4.

## Related docs

- [P3 implementation plan](multiplexing-phase3-conversation-implementation-plan-2026-10-04.md)
- [P3.3 acceptance](multiplexing-phase3-scheduler-acceptance-2026-10-05.md)
- [Architecture](architecture.md)
- [API](api.md)
- [Behavioral contracts](behavioral-contracts.md)

## Notes

P3.5 is next after this sub-phase is accepted.
