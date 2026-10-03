# Freshness R3 publication and Dream crash-recovery verification — 2026-10-03

## Purpose

Record the R3 publication and Dream crash-prefix repair, focused test outcomes, build-environment diagnosis, and remaining independent verification gates.

## Overview

Repository: `C:\\!bin\\workspace\\Reliquary`; branch: `feat/deterministic-memory-freshness`. The focused R3 recovery gate passed on October 3, 2026, with all changes remaining in the shared working tree.

## Scope and disposition

R3's focused crash-recovery and producer contracts passed on the shared `feat/deterministic-memory-freshness` worktree. This record is **not** a full Freshness/CLI production gate or approval of the independently outstanding R4–R7 work. No shared implementation commit or push was made.

## Repair made during this execution

A crash after direct Memory publication but before its Freshness enrollment leaves a durable publication intent and accepted first Memory revision. Ordinary `Cva::open` correctly reconstructs the derived birth without writing. However, retrying the same accepted publication previously saw that derived score in memory, consumed the only pending intent, and returned without persisting initialization. A **second** reopen could lose the birth.

- `src/freshness_storage.rs`: distinguish a genuinely durable owner policy from a policy synthesized for read-only derived birth projection (`durable_policy_seen`). Durably appended/scanned policy chunks set this bit. The rare recovery path checks for an actual durable initialization with the streaming journal lookup, not the synthesized recent cache.
- `src/facade/cva_freshness.rs`: if an accepted first Memory already has a reconstructed birth but still has its publication intent, persist the original policy and birth at the **original accepted cut** before consuming the intent. Both normal enrollment and retry use one `persist_accepted_freshness_birth` path.
- `src/facade/freshness_dream_history_tests.rs`: extend the direct-publication gap test through exact publication retry and a second reopen. Add a distinct crash prefix with source admission durably committed but linkage reinforcement not yet written; recover the existing Dream pass once at its accepted cut, reopen, and prove retry adds no second effect.
- `src/runtime_inference/insomnia/completion_tests.rs`: strengthen the grouped-completion-only read-only-reopen test with explicit exact cohort equality for two sibling Memories.

No new global receipt ledger, publication clock reset, graph repropagation on recovery, or legacy `+100` enrollment was introduced.

## Collected runtime results

All commands below used `--locked` with `CARGO_TARGET_DIR=C:\\Users\\archa\\AppData\\Local\\Temp\\reliquary-r3-verification` and `CARGO_PROFILE_TEST_DEBUG=0`, unless noted.

| Contract | Exact command or test | Collected result |
| --- | --- | --- |
| Direct publication crash prefix, including second reopen | `cargo test --locked --lib freshness_direct_birth_gap_rebuilds_without_append_or_anchor_reset -j 2` | **1 passed**, job `job_Z3KdEnEIiGlJ6ngfIba1SAY5` |
| Combined Dream and owner recovery | `cargo test --locked --lib cva_freshness::recovery_tests -j 2` | **5 passed, 0 failed**, job `job_lxY5DoizJ32ZU9GMBaZFe_-P` |
| Admission before linkage event, independently | `cargo test --locked --lib admission_committed_before_linkage_event_recovers_exactly_once -j 2` | **1 passed**, job `job_abBZMlDvmUY-SjnUst8jjeWL` |
| Durable grouped Insomnia completion without enrollment; exact same-publication cohort | `cargo test --locked --lib freshness_grouped_birth_recovers_from_completion_without_appending -j 2` | **1 passed** after cohort assertions, job `job_txcPqd-UHFyvXVAvRRQA-irQ` |
| Existing canonical Dream duplicate representative | `cargo test --locked --test freshness_recovery_contract -j 2` | **1 passed**, job `job_ajrOSFbZA0lWewhTqgHprIeq` |
| R2 storage regression after R3 edits | `cargo test --locked --lib freshness_storage::tests -j 2` | **4 passed**, job `job_ZsNztRqdxm3gDPyYYtoZMAtu` |
| R2 facade and integration regressions | `cargo test --locked --lib cva_freshness::r2_tests -j 2`, `cargo test --locked --test freshness_r2_contract -j 2` | **1 passed each**, jobs `job_hgzKAhVzLWGpJPTkB2dX-1AL`, `job_vtXNgdblNwWUZv7jZ2bRo08a` |
| Rust formatting | `cargo fmt --all -- --check` | Passed after the source changes; rerun after the final grouped-cohort assertion |

The five-test combined recovery suite includes the accepted first-pass partial graph publication, subsequently inactivated graph link and original snapshot, admission-before-event prefix, direct birth/second-reopen prefix, and bounded-cache durable receipt recovery. The grouped completion test truncates the file immediately after the accepted completion chunk and verifies that read-only reopen reconstructs both births **without appending**. The historical graph test also removes the final settled marker to verify exactly-once replay of an already committed score event.

## Notes

R3 acceptance is scoped to the directly collected tests in this report. Subsequent normal-target compilation failures caused by disappearing dependencies are not counted as an implementation regression, and there is no claim that the full R7 suite or benchmarking is complete.

## Build environment and limits

The first recovery test attempt on `target/r2-owner-verification` failed **before Reliquary tests ran**: Rust could not locate previously built Lore and other dependency `.rlib` files. The installed Cargo artifact reclaimer was active, configured with `roots=["C:/!bin"]`, and its run log contained deletion plans for that exact verification target during compilation. No machine-wide reclaimer settings were changed. Building and testing under the temporary path **outside** that configured root avoided the missing-artifact failure; the isolated focused runtime runs above completed.

These are R3-focused test results on the shared working tree, not evidence that the full workspace, CLI, architecture enforcement, all R4–R7 gates, or production calibration have passed. Preserve the R3 source and test changes without staging unrelated shared changes.

## Related docs

- [R2 durable event-contract verification](freshness-r2-verification-2026-10-03.md)
- [Remaining Freshness phases and acceptance gates](freshness-remaining-work-2026-10-03.md)
- [Unified implementation specification](generic-staleness-implementation-plan.md)
- [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md)
