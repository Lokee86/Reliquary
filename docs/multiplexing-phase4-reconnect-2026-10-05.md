# Multiplexing Phase 4 — P4.5 reconnect and revocation

Parent index: [Documentation index](INDEX.md)
Parent plan: [Phase 4 distribution plan](multiplexing-phase4-distribution-implementation-plan-2026-10-04.md)

## Purpose

Record P4.5 implementation and acceptance after P4.4 commit `8f23ce7`.

## Overview

Reconnect is an authorized snapshot operation, not cursor-based authority. The internal `reconnect_rel` replaces one registered recipient at a fresh source cut under the same publication gate. Old queued notifications and overflow state are covered by the snapshot; only later notifications reach the replacement. Reader, bounds, cursor or authority failure preserves the old recipient. Replacement works at the 1,024-recipient limit without requiring an extra slot.

A same-epoch cursor at or behind the current cursor is accepted as a hint, including an expired cursor. This implementation retains no replay window. It never promises incremental replay from an unretained cursor. Another epoch or a future cursor is rejected. After remount/reopen, disconnect, detach or grant-generation replacement, callers need a fresh authorized subscription rather than reviving the old handle. Reconnect does not silently substitute new authority into an existing subscription.

## Lifecycle and authority

Grant refresh and detach close and drain attachment admission before recipient removal. Delivery and reconnect revalidate attachment identity, current grant generation and the mounted owner fence. Already admitted owner work remains independent of recipient lifetime.

Both trusted and instance-authorized REL reopen paths use one lifecycle publication helper while the REL remains quiesced. Authoritative reconciliation finishes first; source synchronization and source/journal watermark capture precede epoch reset and `OwnerLifecycleChanged` with typed `LifecycleOpen` invalidation. Only then does the fence reopen and worker restart begin. Restart failure closes distribution again. A new subscription observes the reconciled state at its new epoch watermark; prior recipients and cursors cannot cross this boundary.

No network/model/provider I/O runs under the distribution gate. No semantic store or P3 journal owns subscribers. P3 branch/generation policy and conversation hooks remain unchanged.

## Verification

- `cargo fmt --check`, `cargo check --locked --offline -j4`, and `git diff --check`: passed.
- `cargo test --tests --locked --offline -j4 -- --test-threads=1`: 954 library tests and 93 integration tests passed. All ten new reconnect regressions passed; two existing reopen cursor assertions now account for the lifecycle summary at cursor one.
- P4 coverage guard self-test: seven fixtures passed; 104 public APIs classified and REL worker writes admitted. P2 ownership enforcement: no gateway violations.
- Documentation structural and changed-from-origin/main checks: passed.
- Unrestricted `cargo test --locked --offline -j4`: blocked by the pre-existing `archive_open_profile` example global allocator conflict with `lore_base`; this does not affect the passing library/integration run.
- `python scripts/check_architecture.py --refresh`: coverage guard passed, then Lexicon stalled at Rust/Python analysis and timed out after 120 seconds. Architecture refresh/enforcement remains incomplete.
- Two serial dependency rebuild attempts timed out before reaching their tests. The final four-worker verification above completed.
- CLI checks were not required: changes remain internal distribution/lifecycle implementation with no public library exports or CLI configuration changes.

## Documentation impact

- Inspected: architecture, invariants, storage format, maintainer map, limitations, roadmap, behavioral contracts, P4 plan and existing snapshot/mutation records.
- Updated: this record, plan, index, architecture, behavioral contracts, maintainer map, limitations and roadmap.
- Not affected: semantic storage format, source clocks, P3 conversation policy, frozen observation contract and CLI configuration.
- Compliance check: structural and changed-from-origin/main results recorded in Verification.
- Known documentation gaps: public exports and stream/P3 convergence remain later P4 steps.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: reconnect remains sequencer-owned recipient state; quiesced lifecycle code captures synchronized source state before new-epoch publication.
- Pitlord enforcement impact: existing P4 API/worker guard retained; refresh result recorded in Verification.
- Other verification impact: focused reconnect, registry-limit, revoke/publish, epoch and lifecycle regressions, plus library/integration acceptance.
- Known architectural gaps: no optional retained replay window; P4.6 streams and P4.7 P3 convergence remain separate.

## Related docs

- [P4 snapshot and resync](multiplexing-phase4-snapshot-2026-10-04.md)
- [P4 mutation coverage](multiplexing-phase4-mutations-2026-10-05.md)
- [Architecture](architecture.md)
- [Behavioral contracts](behavioral-contracts.md)

## Notes

Same-epoch replay is an optional optimization in the accepted plan. This implementation deliberately uses authoritative snapshot recovery for every reconnect. The gateway allocates mount epochs; distribution adds no separate epoch authority.
