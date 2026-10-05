# Multiplexing Phase 3 - P3.3 automatic durable scheduler

Parent index: [Documentation index](INDEX.md)

## Purpose

Record coordinator-owned durable scheduling, its API hard cut and acceptance evidence.

## Overview

Status: **P3.3 accepted with the architecture-refresh limitation recorded below.**
Branch: `feat/multiplexing-phase3-conversation`.
Base: P3.2 `e8064013080d5a0d4ccdbe0900de3522e8adb8bb`.
Scope: P3.3 only; P3.4-P3.9 remain planned.

## Current behavior

The existing ConversationCoordinator derives candidates from the sole REL journal. It orders eligible work by persisted verified receive time, random tie key and admission sequence. Selection and live branch reservation share coordinator-then-journal locks. No duplicate queue, caller-selected fresh processing authority or background worker is added.

Busy branches and branches reserved by other unfinished work are skipped without changing durable order. Finalizing captures and incomplete post-commit receipts can resume using their original branch/parent/policy revision. Held host operations are excluded from fresh selection so polling Unknown work cannot starve another branch. Definite Completed/Cancelled host state can be consumed; unresolved host work uses explicit continuation resumption and existing host reconciliation.

The public `process_next_conversation_submission(instance, owner_id, conversation_id)` returns an optional receipt. None means there is no eligible work. The caller no longer supplies a submission ID. `resume_conversation_submission` accepts a stable ID only for an existing durable continuation or terminal receipt; fresh Pending work is rejected. Both preserve SubmitConversation authorization, accepted-work fencing and delivery revalidation. Exact-entry fault-injection seams are compiled only for unit tests.

Client timestamps remain metadata. The journal's existing receive-time regression clamp is unchanged: a delayed older wall/client time does not backdate committed history. Equal verified timestamps retain persisted random tie ordering through compaction and restart. Admission retries do not create another entry.

Accepted work can be dispatched internally after the initiating attachment detaches. Dispatch remains an explicit owner operation; this phase does not add a self-running service or native generation scheduler.

## Verification

Collected against the final production sources on 2026-10-05 UTC:

| Gate | Result |
| --- | --- |
| Library tests, serial | 924 passed |
| Integration tests, serial | 95 passed across 11 targets; library suite also passed again |
| CLI tests, serial | 19 passed |
| Library compile check, locked/offline | Passed |
| CLI compile check, locked/offline | Passed |
| Library/CLI formatting | Both checks passed |
| Phase 2 ownership enforcement | Passed |
| Scheduler source guard | Passed: fresh exact-entry seams are test-only, public dispatch has no submission ID, one registry/coordinator remains |
| Documentation structure/change impact and diff whitespace | Passed |
| Architecture refresh | Timed out after 60 seconds during Lexicon analysis; refreshed Pitlord was not reached |

Five focused scheduler regressions cover ordering across branches, delayed client time, retry deduplication, conversation isolation, live/Unknown-host branch skipping, atomic reservation, equal-timestamp ties through compaction/restart, captured ancestry and detached-submitter lifetime. A real host regression exercises public dispatch/resume and exact-once completion; the public API fixture proves callers no longer select a fresh submission ID.

The initial CLI test build exceeded its five-minute dependency compilation bound before executing tests. Resuming the same build cache with a ten-minute bound completed in 58 seconds and all 19 tests passed. No dependency sources or lockfiles changed. The timed-out architecture process and its Lexicon adapter children were stopped. The inherited unrestricted `archive_open_profile` example allocator conflict remains outside these selected library/integration gates; this record does not claim unrestricted whole-repository test acceptance.

Check logs are retained in `C:\!bin\workspace\multiplexing-p33-verification`. Full library/integration output was collected from Workspace jobs `job_jEXcD_hdXMMdOUz-6InZ4q69` and `job_-DeR4c404-N1P1RU0BqE81NS`.

## Documentation impact

- Inspected: AGENTS, shared ownership/concurrency standards, architecture, invariants, storage format, maintainer map, limitations, roadmap, API, behavioral contracts and P3 plan.
- Updated: this record, P3 plan, API, architecture, invariants, limitations, maintainer map, behavioral contract map and documentation index.
- Not affected: persisted codecs/queue bytes, P4 observation/distribution shapes, PHY behavior and CLI commands.
- Compliance check: documentation structure, change impact against origin/main and diff whitespace checks passed.
- Known documentation gaps: no new gap intended; P3.4-P3.9 remain explicitly planned.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: the existing coordinator atomically selects and reserves journal work; Archive retains head authority.
- Pitlord enforcement impact: existing ownership enforcement and direct scheduler source guard passed; architecture refresh timed out during Lexicon before refreshed Pitlord was reached.
- Other verification impact: new scheduler regressions, production host dispatch/resume and public API signature regression.
- Known architectural gaps: refreshed Pitlord remains unverified after the bounded Lexicon timeout. Native generation/checkpoint/control remains P3.6; transport delivery remains P4.

## Related docs

- [P3 implementation plan](multiplexing-phase3-conversation-implementation-plan-2026-10-04.md)
- [P3.2 acceptance](multiplexing-phase3-branch-acceptance-2026-10-05.md)
- [Architecture](architecture.md)
- [API](api.md)
- [Behavioral contracts](behavioral-contracts.md)

## Notes

P3.4 is next after this sub-phase is accepted.
