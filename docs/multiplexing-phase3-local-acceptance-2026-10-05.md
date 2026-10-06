# Multiplexing Phase 3 — P3.9 local acceptance

Parent index: [Documentation index](INDEX.md)

## Purpose

Close the Phase 3 local acceptance gate against the final conversation implementation, with a reproducible gate matrix and explicit convergence boundary.

## Overview

Status: P3.0–P3.9 accepted locally with the documented structural-analysis tool limitation.
Branch: feat/multiplexing-phase3-conversation.
Implementation under test: 8babb57a16759e984cdb84e8f44fbf66c3e54bca (P3.8).
Foundation: 388bd8f319ae0baafcc89f6c4c5288b78248a3ef.
Scope: P3.0–P3.9 local conversation acceptance. No P4 merge or distribution acceptance is claimed.

P3.9 adds acceptance evidence and updates current project status. It changes no production source, public signature, persistence format or frozen observation shape.

## Current behavior and completion audit

| Completion requirement | Production authority and evidence |
| --- | --- |
| One coordinator owner and provisional queue | ConversationRegistry owns coordinators; RelJournal remains the one REL-scoped durable queue/writer. The P3 source guard enforces the named constructions and excludes production test bypasses. |
| Automatic durable chronological order | ConversationCoordinator.acquire_next selects and reserves eligible work using persisted verified receive-time/tie-key/admission-sequence order; scheduler tests cover delayed admissions, fixed ties, compaction/restart and blocked-branch skipping. |
| Sequential and VisibleAutoBranch finalization | Archive conversation metadata owns policy/revision. The finalization module captures safe ancestry or deterministic visible conflict branches. Production tests cover concurrent writers, invalid parents and restart handoffs. |
| One active generation per branch | Coordinator leases and journal reservations fence generation/finalization; unrelated branches progress. Viewer and initiator detach do not end accepted work. |
| Explicit cancellation authority | Pending cancellation and generation control have separate authorization/state transitions. Production race tests verify a single durable cancel/finalize winner. |
| Durable checkpoint and operation recovery | The journal and interaction stream bind exact identity, sequence and acknowledged UTF-8 prefix. Recovery validates Archive receipts, reconstructs reservations and holds uncertain host work without replay. |
| Coherent authoritative observation | P3.7 reads runtime -> coordinator -> journal under the existing owner synchronization boundary; snapshots exclude transcript bodies, delivery cursors and subscriber state. |
| Hard cut | Fresh production dispatch uses automatic selection; exact acquisition only resumes existing durable work. Named source guard and seven fixture tests protect the production boundary. |

The per-subphase records retain implementation details and fault-injection evidence. Reference G1 fixtures are policy oracles; the real gateway/Archive/journal tests provide production proof.

## Verification

The 2026-10-05 final gate run passed against the P3.8 implementation:

| Gate | Result |
| --- | --- |
| Formatting and compilation | cargo fmt --check and locked offline cargo check passed |
| Complete library suite | 955 passed; 0 failed, ignored or filtered |
| Gateway conversation tests | 90 passed, included in the library suite |
| Production REL journal tests | 15 passed, included in the library suite |
| G1 conversation reference fixtures | 13 passed |
| G1 journal reference fixtures | 6 passed |
| Public gateway API contract | 3 passed |
| Gateway routing guard | 2 passed |
| Complete integration suite | 95 passed across 11 targets, including the reference/public/guard cases above |
| P2 ownership guard | No gateway violations |
| P3 ownership guard | No violations |
| P3 source-policy fixtures | 7 passed |
| Documentation | Structural and origin/main change-impact modes passed, rerun after the final record/status edits |
| Working/staged diff checks | Passed |

The conversation/journal counts were extracted from passing test names in the full library log, rather than inferred from source declarations. The complete matrix includes existing interaction/session tests. Compilation rebuilt Lore dependencies before tests ran; no timed-out or unexecuted test command is counted as a passing gate.

### Reproduction

Run from the Phase 3 repository. Cargo used the warmed isolated target at C:\!bin\workspace\multiplexing-p3-5-verification\target-lib, without changing source dependencies or Cargo.lock:

~~~text
cargo fmt --check
cargo test --locked --offline --target-dir <isolated-target> --lib --tests -- --test-threads=1
cargo check --locked --offline --target-dir <isolated-target>
python scripts/check_multiplexing_phase2_ownership.py --enforce
python scripts/check_multiplexing_phase3_ownership.py --enforce
python scripts/test_multiplexing_phase3_ownership.py
python ../engineering-standards/tools/docs_policy/check.py --repo .
python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
git diff --check
~~~

Serial test execution avoids the previously observed Windows sharing violation in an unchanged historical-read cleanup test. Complete gate logs are retained outside the source tree at C:\!bin\workspace\multiplexing-p3-9-verification.

## Verification limitations

The inherited unrestricted Cargo example build has a global allocator conflict in archive_open_profile. The explicit library/integration matrix above passes without claiming that example build passes.

The P3.8 architecture refresh on this exact implementation reached Lexicon Rust/Python analysis but exceeded its five-minute bound; the follow-up check confirmed no current Arcana snapshot. Pitlord enforcement remains unverified. P3.9 changes documentation only, so no new source-ownership refresh is required; the direct P2/P3 guards are rerun here. Their lexical checks do not substitute for structural analysis.

CLI source and public library signatures are unchanged by P3.9; CLI validation is not part of this documentation-only acceptance change. No live provider inference, P4 feed/subscriber tests or external consumer migration is claimed.

## Exit decision and convergence handoff

Phase 3 local acceptance is complete. All named P3.9 gates pass; no unresolved conversation implementation failure remains. Acceptance retains the explicitly recorded example-build and structural-analysis verification limitations.

P3's production snapshot/checkpoint ownership is ready for the P3/P4 integration lane. Convergence must wire post-durability transitions and checkpoint content to P4 delivery with a shared mutation/snapshot-registration linearization boundary. P4 owns sequencers, cursors, recipient checks, resynchronization and backpressure; it must not add subscriber authority to the conversation coordinator. Reauthorization at delivery and control boundaries remains independent. Frozen shapes and root/public wiring must be reconciled on the integration branch.

Local acceptance does not claim an atomic subscribe handshake, production conversation text delivery, a completed Phase 4, or the remaining Phase 5 consumer migration.

## Documentation impact

- Inspected: repository/shared engineering guidance, P3 execution plan, G1 and frozen P34 contracts, P3.0/P3.8 acceptance, architecture, invariants, storage format, maintainer map, limitations, roadmap, coverage and public gateway contract.
- Updated: this acceptance record, documentation index, P3/parent execution status, roadmap, limitations and coverage.
- Not affected: production API/code, storage invariants, Phase 4 implementation/status, CLI and external consumer integration.
- Compliance check: structural and origin/main change-impact modes passed after the final record/status updates.
- Known documentation gaps: none within P3 local scope; convergence remains explicitly separate.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: none; existing coordinator/journal/Archive and frozen P34 boundaries are accepted.
- Pitlord enforcement impact: P3.8's documented Lexicon/Arcana tool limitation remains; no completed structural check is claimed.
- Other verification impact: final full library/integration and ownership matrix; no redundant implementation-mirroring tests added.
- Known architectural gaps: structural analysis tool limitation and P3/P4 production delivery convergence; external consumer migration remains Phase 5.

## Related docs

- [P3 implementation plan](multiplexing-phase3-conversation-implementation-plan-2026-10-04.md)
- [P3.8 recovery acceptance](multiplexing-phase3-recovery-acceptance-2026-10-05.md)
- [P3.7 snapshot acceptance](multiplexing-phase3-observation-acceptance-2026-10-05.md)
- [P3.6 generation acceptance](multiplexing-phase3-generation-acceptance-2026-10-05.md)
- [P3.5 branching acceptance](multiplexing-phase3-auto-branch-acceptance-2026-10-05.md)
- [P3.4 sequential acceptance](multiplexing-phase3-sequential-acceptance-2026-10-05.md)
- [P3.3 scheduler acceptance](multiplexing-phase3-scheduler-acceptance-2026-10-05.md)
- [Frozen P34 contract](multiplexing-p34-observation-contract-2026-10-04.md)

## Notes

This record accepts the local conversation lane. Merging, P4 convergence and publishing are separate actions.
