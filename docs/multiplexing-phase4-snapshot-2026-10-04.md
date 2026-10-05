# Multiplexing Phase 4 — P4.2 atomic REL snapshot/subscription

Parent index: [Documentation index](INDEX.md)
Parent plan: [P4 plan](multiplexing-phase4-distribution-implementation-plan-2026-10-04.md)

## Purpose

Record the canonical P4 snapshot/register handshake on the independent branch above P4.1 `bcf8f7f`, rooted at `388bd8f`.

## Overview

Internal `subscribe_rel` returns a safe snapshot and opaque attachment-bound subscription under one sequencer gate. `next_rel_event` checks live authority under leases. Public exports wait for convergence.

Current readers include every Archive conversation (including inactive ones), branch heads, safe queue identities/order/state, durable host status, Insomnia counts, project kind/management/correlation and semantic/retrieval source clocks. They exclude transcript, request and attachment bodies, host digests, checkpoint text, paths and PHY state.

Final P3 policy/branch/generation observations remain the unchanged optional `RelObservationSummary` seam. Missing production producer data stays None. Compacted queue branch metadata also stays unavailable.

## Source and synchronization

Dedicated sources live under `src/runtime_inference/`:

- `runtime_host_gateway_snapshots.rs`: handshake, opaque recipient authority, live delivery checks and common gated mutation seam.
- `runtime_host_gateway_snapshot_reader.rs`: safe source projections and bounded journal-cut retry.
- `runtime_host_gateway_snapshots_tests.rs`: deterministic races and privacy/lifecycle acceptance.

Lock order is attachment/owner leases, sequencer, one source reader at a time. Registry locks are released before source reads. The journal sequence brackets the semantic snapshot without nested journal/runtime locks; unstable cuts retry at most eight times. Collections have a 16,384-item acceptance bound.

Registration occurs last, after successful read and final authority validation. Source watermark refresh never allocates a notification cursor. Snapshot and participating mutation+sync+publication share the same gate; events after registration have greater cursors. Failed distribution does not negate an already synchronized owner mutation.

Existing-mutator coverage remains P4.4/P4.7. The race tests exercise the common gate with actual synchronized Archive mutations; they do not establish complete legacy gateway publication. No network/provider or backpressure wait belongs under this gate.

The only conversation-lane integration edit widens the existing journal-handle accessor to sibling visibility for the authorized reader. No journal bytes, coordinator policy or frozen observation shapes change. Reconcile this visibility edit and root module declaration at convergence.

## P4.3 bounded recovery and disconnect

Recipient delivery remains bounded at its configured 1–4096 entries. Overflow
replaces ordinary delivery with one latest-cursor ResyncRequired latch. Reading
that state never acknowledges it.

Internal resync_rel obtains a fresh authorized source snapshot under the same
sequencer gate used for publication. It records that exact epoch/cursor cut,
discards pre-cut pending events and keeps delivery latched. Later events are
buffered within the original capacity. acknowledge_rel_resync accepts only the
prepared snapshot watermark and revalidates attachment/grant/epoch authority.
It then resumes with the buffered events after the snapshot cut.

If the buffer overflows before acknowledgement, the prepared cut is invalidated,
pending events are discarded and a newer snapshot is required. An old/future cut,
wrong epoch, absent preparation or repeated acknowledgement fails without clearing
the latch. Source clocks and notification cursors remain separate.

disconnect_rel removes only the exact attachment-bound recipient, releasing its
pending events and recovery state. It does not change REL work or other recipients.
A new subscription obtains the current snapshot without replaying discarded gaps.
Detach/revoke/remount retain the existing lifecycle invalidation.

Four new gateway acceptance cases cover exact capacity and repeated overflow,
snapshot-to-ack publication, reoverflow/stale acknowledgement, disconnect while
latched, resubscribe, recipient isolation, epoch mismatch and revoked grants.
P4.3 acceptance:
- All 14 snapshot/recovery cases passed within the full run, including four new cases.
- Locked/offline library/integration suite: 932 library and 93 integration tests passed.
- Library check, formatting, direct Phase 2 source ownership, diff whitespace and both documentation checks passed.
- The initial test run reached Reliquary compilation but exceeded 600 seconds while rebuilding dependencies. The restarted run completed successfully.
- Architecture refresh reached Lexicon Rust/Python analysis and timed out at 240 seconds before Arcana/Pitlord enforcement. Refreshed enforcement remains incomplete.
- The inherited unrestricted example allocator conflict remains as recorded at P4.1. Public/CLI signatures did not change; CLI gates were not repeated.

P4.3 changes no storage bytes, semantic authority, frozen observation shape or
production P3 policy. Recovery state remains in the sequencer-owned registry.
The coverage and behavioral-contract maps now record these recovery guarantees.

## Verification

- Focused snapshot acceptance: 10 passed (including both subscribe/mutation orderings, simultaneous subscribers, revocation during snapshot, inactive conversations, journal retry/exhaustion, wrong epoch/future cursor, remount/detach and private PHY exclusion).
- Locked/offline library and integration run: 928 library and 93 integration tests passed.
- Locked/offline library check, formatting, direct Phase 2 ownership enforcement and diff whitespace check: passed.
- Structural and change-impact documentation policy checks: passed; repeated after final acceptance edits.
- Architecture refresh retried with a 180-second limit: timed out during Lexicon initialization (Rust/Python analysis), before Arcana/Pitlord enforcement. Refreshed enforcement remains incomplete; direct source ownership checks passed.

The first build found a test-only title argument type error, corrected before the passing runs. The unrestricted example allocator conflict recorded at P4.1 remains outside this selected library/integration gate. CLI/public signatures did not change; CLI gates were not repeated.

P4.2 accepts the internal handshake and current safe source readers. Complete production feed coverage and final P3 observation integration remain the named later steps.

## Documentation impact

- Inspected: required ownership docs, frozen observation contract, P4 plan/core.
- Updated: this record, P4 plan/core and index.
- Not affected: storage format, P3 policy and public/CLI signatures.
- Compliance check: structural and change-impact checks passed, including final acceptance edits.
- Known documentation gaps: full publication and final P3 observation adapters remain later phases.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: recipient state stays in REL distribution; source readers retain semantic/journal authority.
- Pitlord enforcement impact: refresh timed out during Lexicon initialization; no refreshed Pitlord success is claimed. Direct ownership enforcement passed.
- Other verification impact: snapshot races, live authorization and regressions.
- Known architectural gaps: P4.4 coverage and P4.7 production P3 adapter.

## Related docs

- [P4.1 core](multiplexing-phase4-core-2026-10-04.md)
- [Frozen seam](multiplexing-p34-observation-contract-2026-10-04.md)
- [Architecture](architecture.md)

## Notes

Known cursors are hints; calls return fresh snapshots. Prior-epoch/future cursors fail. P4.5 owns reconnect/replay.
