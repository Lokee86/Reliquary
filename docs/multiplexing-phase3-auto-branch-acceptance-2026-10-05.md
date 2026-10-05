# Multiplexing Phase 3 - P3.5 visible automatic branching

Parent index: [Documentation index](INDEX.md)

## Purpose

Record VisibleAutoBranch stale-head conflict handling on the real coordinator/journal/Archive path and its restart evidence.

## Overview

Status: **P3.5 accepted.**
Branch: `feat/multiplexing-phase3-conversation`.
Base: P3.4 `4ad5851`.
Scope: P3.5 only; P3.6-P3.9 remain planned.

## Current behavior

VisibleAutoBranch uses the same coordinator-owned durable queue ordering as sequential policy. At finalization it reads the durable admitted expected parent and compares it with the authoritative selected Archive branch head. If that parent still is the head, the submission advances the selected branch normally. If another accepted submission advanced the same branch first while the admitted parent remains in that branch ancestry, the stale submission finalizes on a distinct conflict branch preserving its original parent. An unrelated retained parent fails closed and leaves the submission Pending.

Automatic branch identity is deterministic from the durable conversation ID, admitted branch ID, stable submission ID and admitted expected parent. The resulting branch ID and human-facing conflict label, exact parent, verified principal, timestamp and policy revision are captured together in durable FinalizeWon before Archive publication. Automatic branches are non-canonical; ordinary caller-selected branches preserve their existing canonical state. Recovery validates the frozen identity rather than reclassifying against current policy or current branch heads.

Archive publication remains exactly-once through the existing stable submission/node identity and Archive receipt. A crash after FinalizeWon but before branch publication recreates the same branch identity. A crash after Node/branch publication but before the Archive receipt reconciles that exact published branch and records one receipt. Replay cannot manufacture a second branch or rewind the original branch.

After the Archive receipt is durable, the gateway consults the existing durable admission attribution to identify the original submitting instance. Only that instance's still-current attachment may move its instance-local selected branch to the new automatic branch. Processing by another instance does not move the processor; unrelated viewers remain where they were. A detached, revoked or otherwise unavailable submitting attachment does not block already-accepted finalization.

The implementation adds no second queue, coordinator, policy owner or P4 delivery authority. Display-label durability lives with FinalizeWon in the existing REL operational journal; P3.7 may project that owned fact into the authoritative conversation observation snapshot.

## Verification

Collected against the P3.5 production sources on 2026-10-05:

| Gate | Result |
| --- | --- |
| Focused VisibleAutoBranch regressions | 5 passed |
| Full library tests, serial | 936 passed |
| Integration tests, serial | 95 passed across 11 targets |
| Library compile check, locked/offline | Passed |
| Library formatting | Passed |
| Phase 2 ownership enforcement | Passed; zero gateway violations |
| Documentation structure/change impact and diff whitespace | Passed |
| CLI formatting/check/tests | Passed; 19 tests |

Tests use locked/offline dependencies where applicable, serial test execution and an isolated library target at `C:\!bin\workspace\multiplexing-p3-5-verification\target-lib`. The inherited mixed repository target produced missing dependency artifacts during the first ordinary check, so acceptance verification uses the same separate-target strategy established by P3.4. No dependency source or lockfile changed.

The focused suite covers:
- two accepted writers on one stale head: one advances `main`, the other receives one deterministic non-canonical conflict branch;
- three accepted writers on one stale head: one main winner and two distinct deterministic conflict branches;
- submitter-only instance selection with another viewer left on the original branch;
- restart before automatic branch publication and restart after branch publication but before the journal Archive receipt;
- detached submitter finalization;
- fail-closed unrelated retained ancestry with no Archive or automatic-branch publication.

The restart test exercises both sides of the branch-publication handoff with real persistent journal and Archive owners. It proves the frozen FinalizeWon identity is reused exactly and terminal resume is idempotent. Existing P3.4 finalization/restart regressions and P3.1-P3.3 policy/scheduler/branch tests remain in the 936-test library gate.

## Documentation impact

- Updated: P3 plan, architecture, invariants, API, limitations, maintainer map, behavioral contracts, storage format, documentation index and this acceptance record.
- Persisted operational format: FinalizeWon now carries an optional automatic branch display label; older captures deserialize with no label.
- Not affected: Archive branch record shape, P4 observation data types, PHY behavior, network/subscriber delivery and CLI command surface.
- Remaining conversation work: P3.6 native generation/control lifetime, P3.7 authoritative snapshot, P3.8 recovery/hard-cut cleanup and P3.9 local acceptance.

## Architecture impact

- Standards added or changed: none.
- Ownership impact: none; the existing conversation coordinator, REL journal, owner runtime mutex and Archive branch authority remain canonical.
- Finalization targeting is policy-aware in `runtime_host_gateway_conversation_finalization.rs`; durable capture/publication and submitter-local selection remain in the existing gateway conversation owner.
- Automatic branch selection is advisory instance-local state after durable commit and cannot veto accepted owner work.
- P4 sequencing/subscriber/cursor authority is untouched.

## Related docs

- [P3 implementation plan](multiplexing-phase3-conversation-implementation-plan-2026-10-04.md)
- [P3.4 acceptance](multiplexing-phase3-sequential-acceptance-2026-10-05.md)
- [P3.1 policy acceptance](multiplexing-phase3-policy-acceptance-2026-10-04.md)
- [Architecture](architecture.md)
- [API](api.md)
- [Behavioral contracts](behavioral-contracts.md)

## Notes

P3.6 is next after this sub-phase.