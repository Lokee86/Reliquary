# Dependency Convergence Phase 2 Baseline Verification — 2026-10-22

Parent index: [Documentation index](INDEX.md)

## Purpose

Record the source-grounded Phase 2 boundary contract and verification evidence.

## Overview

This dated execution record distinguishes accepted baseline evidence, implementation work and pending acceptance. The Phase 2 specification remains the governing scope.


This record complements the [Phase 2 source and owner inventory](dependency-convergence-phase2-baseline-2026-10-22.md) and the [frozen owner contracts](dependency-convergence-phase2-contracts-2026-10-22.md). It records baseline commands run before source changes. All new logs and test temporary paths are under `.verification-tmp/phase2-evidence/`.

## Source and corpus before-image

The working tree was at `529f2a81df3c12d4767af487e3ce655b76c6bb36` when the baseline commands began. Concurrent B0 documentation work left `docs/INDEX.md`, the source inventory, its JSON ledger, and the frozen contract document modified or untracked; no source or corpus change was made by this baseline run. The inventory records the accepted Phase 1 checkpoint `8614547` and the source-reference ledger.

The frozen history oracle is `benchmarks/canonical-history-equivalence-2026-10-21.json`; it records zero owner-domain mismatches and these six canonical source hashes:

| Fixture | Canonical SHA-256 |
| --- | --- |
| `chatgpt-first14d/project.prj.rel` | `7e452de7aadff26f5a891538bd8923fe7b499e680d0491499cba2ea187b4fd8d` |
| `chatgpt-first14d/user.phy` | `7e690b4853afe0936702dc023f511dd7c78ffaae3e18e8e66f6ab1c6891a6c63` |
| `chatgpt-first28d/project.prj.rel` | `1381fa663a41ebff3c69cb0418ff3f2369294195d10e36096bbec01cec82cf92` |
| `chatgpt-first28d/user.phy` | `32970f3975bfcd6de0fcdb78df9c80c78b76787832f528c1b09fd75dfa4f501d` |
| `rhelm-david-r-ellis-pre-relationship/project.prj.rel` | `dd7f0f0cf675901d456495a5ee93eb6c8884d793b4b9dd8b563a1ad4746043ab` |
| `rhelm-david-r-ellis-pre-relationship/user.phy` | `58ac7872215e126749f024fc742828abc9a8a8f84775b39c0706d638392ce9e0` |

The canonical comparison is an explicit ignored test and takes about 6,373 seconds on this Windows host. It was not repeated for B0: the accepted Phase 1 run in `.verification-tmp/phase1-corpus-acceptance.log` passed all 6 files with unchanged bytes and modification times. The Phase 2 final gate must run the same opt-in oracle after the owner cuts.

## Commands and results

The PowerShell process set `TMP`, `TEMP`, and `TMPDIR` to the repository-local `.verification-tmp/phase2-evidence/temp` directory for Cargo and test subprocesses.

| Command | Result | Evidence |
| --- | --- | --- |
| `cargo check --locked` | Passed, exit 0, 1.78 seconds | `.verification-tmp/phase2-evidence/cargo-check-locked.log` |
| `cargo check --manifest-path cli/Cargo.toml --locked` | Passed, exit 0, 23.19 seconds | `.verification-tmp/phase2-evidence/cli-check-locked.log` |
| `cargo test --manifest-path cli/Cargo.toml --locked` | Passed, 32 passed, 0 failed, exit 0, 75.38 seconds | `.verification-tmp/phase2-evidence/cli-test-locked.log` |
| `python scripts/check_physical_boundary.py` | Passed, 13 fixtures, exit 0, 0.42 seconds | `.verification-tmp/phase2-evidence/physical-boundary.log` |
| `python scripts/check_component_dag.py --self-test` | Passed, 34 components, 163 allowlisted edges, no cycles/upward dependencies; negative fixtures passed; exit 0, 0.25 seconds | `.verification-tmp/phase2-evidence/component-dag-self-test.log` |
| `cargo test --locked --lib` (ordinary sandbox) | Started and ran the suite but was interrupted after broad filesystem/process/provider-related failures and multiple tests exceeded 60 seconds. No final test count or pass claim. | `.verification-tmp/phase2-evidence/cargo-test-lib-locked.log` |
| `cargo test --locked --lib container::file_lease_process_tests::cross_process_writer_lock_is_shared_by_hardlink_and_symlink_aliases -- --exact --nocapture` (elevated local run) | Passed, 1 test, exit 0, 0.05 seconds. This reproduces one sandbox-sensitive failure as passing with elevated local process permissions. | `.verification-tmp/phase2-evidence/baseline-hardlink-focus.log` |
| `cargo test --locked --lib` (elevated local run) | Passed, 1,957 passed, 0 failed, 5 ignored, exit 0, 1,043.98 seconds. The long history/recovery/scaling tests completed successfully. | `.verification-tmp/phase2-evidence/cargo-test-lib-elevated.log` |
| `cargo test --locked --tests` | Pending; not run before implementation work began. | — |

The opt-in oracle command retained for the final gate is:

```text
cargo test --locked --config 'profile.test.package.reliquary-memory.opt-level=1' --config 'profile.test.package.sha2.opt-level=3' --config 'profile.test.package.serde_json.opt-level=3' --features history-oracle-fixtures --lib canonical_current_corpus_matches_frozen_history_oracle -- --ignored --nocapture
```

No architecture refresh was run. The baseline DAG self-test and physical-boundary guard are source checks only; they do not refresh architecture evidence. The ordinary sandbox failures are superseded only by the separately recorded elevated library baseline; the unexecuted integration baseline must not be represented as passed. Long-running opt-in and post-cut owner tests remain explicit Phase 2 verification work.

## Related docs

- [Phase 2 execution ledger](dependency-convergence-phase2-execution-2026-10-22.md)
- [Phase 2 specification](dependency-convergence-phase2-owner-local-boundaries-spec-2026-10-22.md)

## Notes

Unexecuted tests and incomplete gates are pending; source compilation does not imply behavioral or corpus acceptance.
