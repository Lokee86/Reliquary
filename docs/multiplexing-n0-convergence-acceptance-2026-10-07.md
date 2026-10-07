# Multiplexing N0 convergence acceptance — 2026-10-07

Parent: [Next parallel execution wave](multiplexing-phase5-next-parallel-execution-plan-2026-10-07.md)
Branch: `feat/multiplexing-p5-convergence`
Tested convergence base: `f9dafba2a885c140b1aa879dc0494c9d2d44b1c9`
External consumer: Warlock `feat/multiplexing-p5c-final-cut` @ `3a6247061e64422821f676c5bb9e7af8cba17c48`

## Purpose

Record the common accepted implementation base and the combined N0 verification required before new Phase 5 production lanes branch.

## Overview

The convergence worktree starts from Reliquary C integration `4ce4911dfcae265fdca055cb4ecd43ea7820b0ea` and merges planning/status main `2407dbe55e5d943f3b7a044c10949aca759c0026`, including the planned `dfa82ae` documentation and the next-wave plan itself. Accepted A `7350149` and B `c45f50e` are ancestors. All implementation-branch acceptance records remain present.

The merge changes documentation only. A direct comparison of source, CLI, manifests, lockfiles, scripts, tests and tools with `4ce4911` is empty. The two A/B status conflicts retain the newer accepted status and link to the preserved integration evidence.

No Warlock source is merged into Reliquary. The P6 harness remains on its accepted separate branch. No legacy host deletion, dependency repinning, new E/F/G/D0 production work or worktree pruning occurs in N0.

## Verification

| Gate | Executed result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --locked -j4` | PASS |
| `cargo test --locked -j4 -- --test-threads=4` | PASS: 1,099 library tests and 128 integration tests; 0 doctests |
| `cargo fmt --manifest-path cli/Cargo.toml -- --check` | PASS |
| `cargo check --manifest-path cli/Cargo.toml --locked -j4` | PASS |
| `cargo test --manifest-path cli/Cargo.toml --locked -j4 -- --test-threads=4` | PASS: 19 tests |
| `python scripts/check_multiplexing_phase2_ownership.py --repo . --enforce` | PASS: no gateway ownership violations |
| `python scripts/check_multiplexing_phase3_ownership.py --repo . --enforce` | PASS: no violations |
| `python scripts/check_multiplexing_phase4_coverage.py --self-test` | PASS: 42 positive/negative fixtures; 148 public APIs classified |
| Documentation structural / change-impact checks | PASS: structural and changed-from `origin/main`, including this final record |
| Warlock local wrapper `check --all-targets` | PASS against exact Reliquary base `f9dafba` |
| Warlock local wrapper `test --lib -- --test-threads=4` | PASS: 202 tests, 0 failures, 1 existing ignored Arcana benchmark requiring an external fixture |
| Warlock formatting / docs / 56-rule policy / frontend build / whitespace | PASS; policy check uses existing indexed evidence and does not claim a fresh source index |

Warlock commands were run from its isolated C worktree with `-ReliquaryPath C:\!bin\workspace\Reliquary-p5-convergence` and `-CargoArgs @('-j4','--all-targets')` or `@('-j4','--lib','--','--test-threads=4')`. The wrapper remains local upgraded-consumer evidence, rather than a production pin test.

The initial full Warlock run exposed three consumer regressions (199 passed, 3 failed, 1 ignored). They were repaired in the owning C branch at `3a62470`: reject a different PHY identity before closing the current attachment, restore authorized project upload ingestion, and make the search integration fixture await asynchronous owner-profile readiness with a bounded deadline. The full repaired suite supersedes the initial failed run.

The original combined Reliquary job reached the CLI test build after all root/library/integration and CLI format/check gates passed, then hit its thirty-minute process limit before CLI assertions. A separate locked CLI test rerun completed successfully in 1m30s with all 19 tests passing. This timeout is retained as historical build evidence, not waived as a passing test.

Root and CLI checks use `C:\Users\archa\AppData\Local\P5CVerification\reliquary-cli`; the upgraded Warlock wrapper uses `C:\Users\archa\AppData\Local\P5CVerification\warlock`. These targets are outside the cargo-reclaim scan root. Existing compiler warnings are not promoted into acceptance failures.

The local Warlock wrapper patches only Reliquary for each command and restores the tracked lockfile. Its temporary graph still includes two Arcana revisions; this proves local upgraded compilation/tests, not final production dependency alignment. That hard cut remains a C/N3 gate.

## Disposition

**N0 accepted.** The common convergence base is ready for the next plan's C-close/E/F/G/D0/P6 diagnostic work. This turn implements N0 only; no new production lane is started by this work.

This record certifies the exact tested source revision above. The following documentation-only acceptance commit may serve as the branch point; it changes no tested implementation.

## Documentation impact

- Inspected: next-wave and parent plans, A/B/C acceptance and consumer records, architecture, invariants, storage format, maintainer map, current limitations, roadmap, and the Warlock local wrapper.
- Updated: this branch-scoped record, documentation index, and N0 status in the next-wave plan; the owning C commit updates Warlock's Foundation, Chat, Runtime operations, Behavioral contract matrix, Current limitations and continuation record.
- Not affected: persisted formats, Reliquary public API behavior and source ownership, CLI behavior, unrelated Warlock main, and accepted branch history.
- Compliance check: Both structural and change-impact policy checks pass, including the final acceptance record/index/plan update; whitespace validation passes.
- Known documentation gaps: final C/E/F/G convergence, D deletion and P6 release remain pending.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: none in Reliquary N0; accepted A/B/C library implementation is preserved unchanged. The external C regression repairs compose existing authorized APIs and enforce the stable principal/PHY contract.
- Pitlord enforcement impact: no policy changes; direct P2/P3 enforcement and P4 coverage guards pass.
- Other verification impact: Complete root/library/integration/CLI acceptance and the exact local upgraded Warlock matrix pass.
- Known architectural gaps: the inherited bounded Rust-indexing refresh gap remains open. N0 does not use stale Arcana snapshots as acceptance evidence. Real multi-context consumer behavior, actual provider conformance, public P4 reconnect/delivery, E/F convergence, production dependency alignment and final deletion/release remain separate gates. Project upload ingestion now passes its manual-Lore regression, but exact ProjectFileRef propagation through the byte-staging conversation journal, payload deduplication and retry/rejection side effects remain C closure work.

## Related docs

- [Next parallel execution wave](multiplexing-phase5-next-parallel-execution-plan-2026-10-07.md)
- [Parent convergence plan](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [Integrated A/C acceptance](multiplexing-p5a-integrated-acceptance-2026-10-07.md)
- [B acceptance](multiplexing-p5b-acceptance-2026-10-07.md)
- [External consumer lane](multiplexing-phase5c-external-consumers-2026-10-06.md)

## Notes

N0 acceptance establishes a common branch base; it does not certify completion of Phase 5 or the final consumer/deletion/release gates.
