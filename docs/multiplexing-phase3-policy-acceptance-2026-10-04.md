# Multiplexing Phase 3 — P3.1 durable policy ownership

Parent index: [Documentation index](INDEX.md)

## Purpose

Record durable conversation policy ownership and P3.1 acceptance evidence.

## Overview

Status: **P3.1 accepted.**
Branch: `feat/multiplexing-phase3-conversation`.
Base: P3.0 acceptance `31af314d470458ebbd73f01927aba82f4c3717a4`; common seam `388bd8f319ae0baafcc89f6c4c5288b78248a3ef`.

## Current behavior on this branch

Archive conversation metadata is the sole durable policy owner. `ConversationPolicy` is defined beside `ConversationMetadata`; the shared observation module imports that canonical vocabulary without changing its variants or envelope shapes.

Missing metadata and readable CVACONV1/CVACONV2 records mean `SequentialChronological` at policy revision 1. Current writes use CVACONV3 in the existing Archive publication/history path. A changed policy consumes one Archive/global semantic publication and increments the policy revision once; an identical policy is a no-op. Title and active changes preserve both policy fields. Revision overflow fails before publication.

The existing Cva facade delegates to Archive. Gateway policy reads require a current exact-owner `ViewRel` grant. Policy writes require `ManageRel`, not SubmitConversation, ManageProject, or a cancellation grant. The normal gateway attachment and owner admission leases revalidate dispatch authority, and acknowledgement follows synchronization. An idempotent management retry also synchronizes any prior pending publication.

The owner runtime lock spans canonical policy read, sync and durable FinalizeWon. Policy mutation uses that same runtime lock. Nested acquisition is runtime then REL journal; no journal writer is held while acquiring the runtime. Provider/host I/O remains outside these locks. Pending entries capture the current canonical revision; Finalizing/Committed entries retain their exact journal capture and ancestry, including across mount recovery after a policy change.

The immutable admission metadata's pre-existing `admission.policy_revision = 1` identifies its authorization-policy contract. It remains part of the unchanged request fingerprint; it is not the conversation metadata's mutable policy revision. Policy changes cannot conflict with an otherwise identical admission retry.

## Scope and remaining work

P3.1 establishes policy storage, authorization and immutable finalization revision capture. Branch-local continuations, automatic scheduling and policy-specific parent/conflict decisions remain P3.2–P3.5. Persisting VisibleAutoBranch does not claim that visible automatic branch creation is implemented. No queue, policy sidecar, distribution cursor, subscriber or second writer was added.

Gateway root declarations and instance grant definitions are unchanged. Current-state changes are limited to the policy owner, lock order and public API. P4 root wiring and distribution behavior remain reserved for convergence. Storage-format documentation owns the new bytes.

## Verification

Collected on 2026-10-05 against the final implementation:

| Gate | Result |
| --- | --- |
| Library tests, serial | 914 passed |
| Integration tests, serial | 94 passed across 11 targets |
| CLI tests, serial | 19 passed |
| Library and CLI compile checks | Passed, locked/offline |
| Library and CLI formatting | Passed |
| Phase 2 ownership enforcement | Passed |
| Documentation structure and impact against origin/main | Passed |
| Git diff whitespace check | Passed |
| Source authority inventory | One canonical policy enum, coordinator, registry and REL journal |
| Architecture refresh | Timed out after 60 seconds during Lexicon initialization; refreshed Pitlord was not reached |

Focused tests cover legacy default/upgrade/reopen, policy round-trip and metadata preservation, corrupt/unknown/truncated policy records and exhausted revisions, exact-owner/current management authorization, admission retries, and Pending/Finalizing/Committed behavior across restart. The interrupted CLI compilation attempt was resumed and all 19 tests passed. Logs are retained in `C:\!bin\workspace\multiplexing-p31-verification`.

The inherited unrestricted `archive_open_profile` example allocator conflict remains outside the selected library/integration gates; this record does not claim an unrestricted whole-repository test pass.

## Documentation impact

- Inspected: AGENTS, shared engineering rules, architecture, invariants, storage format, maintainer map, limitations, roadmap, P3 plan and shared P34 contract.
- Updated: this record, P3 status, storage-format contract, API, architecture/invariants, shared observation ownership note and documentation index.
- Not affected: P4 distribution behavior, PHY storage, queue bytes, CLI commands and existing owner grant vocabulary.
- Compliance check: documentation structure and changed-file impact checks passed.
- Known documentation gaps: none for P3.1; later policy-specific scheduling remains planned.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: existing Archive conversation metadata gains canonical policy/revision; existing gateway owner gates enforce management.
- Pitlord enforcement impact: existing Phase 2 authority guard retained and direct enforcement passed; refreshed Pitlord was not reached because architecture refresh timed out during Lexicon initialization.
- Other verification impact: Rust and CLI regression gates listed above.
- Known architectural gaps: architecture refresh remains unverified after the bounded timeout; later scheduler/branch/generation/observation behavior remains explicitly unimplemented.

## Related docs

- [P3 implementation plan](multiplexing-phase3-conversation-implementation-plan-2026-10-04.md)
- [P3.0 baseline](multiplexing-phase3-baseline-2026-10-04.md)
- [Storage format](storage-format.md)
- [P34 observation contract](multiplexing-p34-observation-contract-2026-10-04.md)

## Notes

This accepts P3.1 only with the architecture-refresh limitation recorded above. P3.2 is next.
