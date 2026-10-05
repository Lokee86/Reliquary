# Multiplexing Phase 3 — P3.2 branch continuation ownership

Parent index: [Documentation index](INDEX.md)

## Purpose

Record the branch-aware coordinator hard cut and P3.2 acceptance evidence.

## Overview

Status: **P3.2 accepted with the architecture-refresh limitation recorded below.**
Branch: `feat/multiplexing-phase3-conversation`.
Base: P3.1 `5c34387bddf7b7afdce9cf125af95ce3f46565a4`.

## Current behavior

The existing ConversationRegistry still owns one coordinator per (REL, conversation) and one REL journal writer. The coordinator's conversation-wide processing flag is replaced by branch-keyed continuation state in `runtime_host_gateway_conversation_continuation.rs`.

A branch has one live processing lease. The lease covers host dispatch through Archive and post-commit publication; the coordinator mutex is held only while acquiring/releasing it. Host I/O runs outside owner, coordinator and journal locks. Two attempts on the same branch cannot overlap. Independent branches can reach the real host adapter concurrently when capability/resource policy permits.

Journal-owned unfinished work reserves its branch even after the driving call returns. Reservations include Pending entries with a ConversationExecution operation, Finalizing captures and Committed entries lacking post-commit receipts. The same durable submission may resume; a different submission waits. Terminal cancellation or a complete publication/receipt permits later work. Branch-local Pending order is checked before host dispatch. Explicit exact-entry processing remains until P3.3 selects work automatically.

Archive owns branch heads. Each runtime operation binds the existing conversation cursor to its authoritative branch head or immutable captured parent/receipt, under the one owner runtime mutex. This cursor is temporary execution state, not conversation-global head authority. No per-viewer runtime or second writer is created. Rebinding rejects an in-flight native message, validates the Archive parent and preserves existing branch canonical flags. Post-commit scheduling also explicitly selects its committed receipt so another branch's cursor cannot leak into Episode scheduling.

Mount reconstructs branch inventory from Archive and reservations from the journal before workers start. Conflicting durable continuations on one branch fail closed before recovery publishes effects. After normal receipt reconciliation, reservations are rebuilt again. A recovered Unknown host operation keeps its branch reserved; an idle branch can advance through native processing when no host boundary is configured. Owner mount/fence checks, admitted principal identity and viewer-independent lifetime remain unchanged.

Nested locks are coordinator then journal during eligibility/rebuild, and runtime then journal during durable capture. No path holds the journal while acquiring coordinator or runtime. The branch lease itself is an identity reservation, not a mutex held during I/O. FinalizeWon reads the conversation transition revision while holding the journal writer, avoiding stale revision failures from independent branches.

## Scope and remaining work

This accepts branch-aware ownership for the gateway's existing host/native finalization path. Automatic selection is P3.3; complete policy conflict decisions are P3.4/P3.5; native assistant generation/control and branch-bound checkpoint identity/recovery are P3.6. Existing InteractionRuntime checkpoints remain supported by their original owner; P3.2 does not fabricate a gateway generation from those records or claim native simultaneous assistant streams. Observation producers and delivery remain P3.7/P4.

Journal/storage bytes, shared observation shapes, public signatures, gateway root wiring and P4 modules are unchanged.

## Verification

Collected against the final production implementation on 2026-10-05 UTC:

| Gate | Result |
| --- | --- |
| Library tests, serial | 919 passed |
| Integration tests, serial | 95 passed across 11 targets; library suite also passed again |
| CLI tests, serial | 19 passed |
| Library compile check, locked/offline | Passed in isolated check-only target |
| CLI compile check, locked/offline | Passed in isolated check-only target |
| Library/CLI formatting | Both format checks passed |
| Phase 2 ownership enforcement | Passed |
| Source authority inventory | One coordinator, one registry and one REL journal; conversation-wide processing flag removed |
| Documentation structure/change impact and diff whitespace | Passed |
| Architecture refresh | Timed out after 60 seconds during Lexicon initialization; refreshed Pitlord was not reached |

Five focused production regressions cover live same-branch contention, independent host dispatch, detach/switch, restart with Unknown host work, restart with captured Finalizing work, isolated post-commit fencing and rejection of conflicting durable branch owners. A public API regression verifies independent branch admission/finalization and reopened ancestry.

The initial shared-target build failed in external Lore dependencies with E0463 before Reliquary compiled; a fresh target passed all library tests. A later compile check encountered missing dependency metadata in the mixed target; the isolated check-only target passed. Dependency sources were unchanged. The public regression's first run reused admission revision zero; its fixture now uses the returned transition revision and the full integration suite passed.

Logs are retained in `C:\!bin\workspace\multiplexing-p32-verification`. The timed-out Lexicon child process was stopped. The inherited unrestricted `archive_open_profile` example allocator conflict remains outside the selected library/integration gates; this record does not claim an unrestricted whole-repository test pass.

## Documentation impact

- Inspected: AGENTS, shared engineering standards, architecture, invariants, storage format, maintainer map, limitations, roadmap, public API and P3 plan.
- Updated: this record, P3 plan/status, architecture, invariants, API, behavioral contract test map, current limitations, maintainer map and index.
- Not affected: storage codecs, queue bytes, P4 distribution, PHY behavior and CLI commands.
- Compliance check: documentation structure, changed-file impact against origin/main and diff whitespace checks passed.
- Known documentation gaps: none for the implemented P3.2 path; later native generation/checkpoint machinery remains explicitly planned.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: the existing coordinator owns branch leases and journal-derived reservations; Archive remains branch-head authority.
- Pitlord enforcement impact: existing source guard retained and direct enforcement passed; architecture refresh timed out during Lexicon initialization before refreshed Pitlord was reached.
- Other verification impact: focused gateway, full library/integration, public API and CLI regression gates.
- Known architectural gaps: architecture refresh remains unverified after the bounded timeout; native generation/checkpoint ownership is explicitly P3.6.

## Related docs

- [P3 implementation plan](multiplexing-phase3-conversation-implementation-plan-2026-10-04.md)
- [P3.1 acceptance](multiplexing-phase3-policy-acceptance-2026-10-04.md)
- [Architecture](architecture.md)
- [Behavioral contracts](behavioral-contracts.md)

## Notes

P3.3 is the next sub-phase. This record does not accept all of Phase 3.
