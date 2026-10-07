# P6-H reproducible gateway harness

Parent: [Phase 6 plan](multiplexing-phase6-parallel-verification-plan-2026-10-06.md)

## Purpose
Own the runnable H0–H5 fixture/command/measurement contract. This is not a release verdict.

## Overview
Lane `feat/multiplexing-p6-harness` starts at `c24b06273abbab383eb74780bf82cb5c0ceef0b5`. Production source/build inputs are unchanged from accepted P3/P4 `e1c8f88`; verify with `git diff e1c8f88 c24b062 -- src tests Cargo.toml Cargo.lock`. Ten new ordinary real-runtime scenarios and two runner-only ignored crash entrypoints register beneath the existing gateway test module. Production code, exports, semantic owners and host router are unchanged.

## Reproduction
Run from the worktree:

```text
python scripts/run_multiplexing_p6_harness.py --list
python -m unittest discover -s scripts/tests -p test_p6_harness_runner.py
python scripts/run_multiplexing_p6_harness.py --seed 38 --repeat 3 --instances 1,8,32 --output target/p6-results/candidate
```

The runner builds a real library test binary with locked Cargo and one build job, discovers its artifact from Cargo JSON, executes the ten new scenarios with one test thread for each repetition, executes the existing production gateway matrix with four test threads, then runs seven actual abort/reopen subprocess pairs. A required matrix matching zero tests fails; a nonzero crash without the durable READY marker fails. Every command/result/log path and elapsed time is recorded incrementally in `report.json`. Output must be a new directory. Failures preserve partial evidence and physical files. Set `--target-dir` for another isolated Cargo target; do not point concurrent worktrees at the same mutable target.

Fixture randomness concerns physical owner/file/attachment identities only. Seeded permutation and logical submission IDs are repeatable; persisted generated IDs are read from receipts/identity markers. Production admission clocks and UUID ties remain authoritative. Race winners are deliberately not predetermined. Fixtures use real REL/PHY files and gateway/owner runtimes, with no cloned runtime/queue implementation.

`--features` records explicit feature flags. `--timeout` bounds each command, including compilation. Test workload sizes are validated within existing registry limits. The manifest and environment include Rust/Cargo/Python/platform/CPU/RAM, relevant build flags, exact revision, dirty status, fixture hashes, runner hash, binary hash, target, sampler availability and thread policy. No provider credentials are recorded.

## Matrix and source boundaries

| Gate | New harness coverage | Reused production fixtures |
| --- | --- | --- |
| H0 | stable seed/permutation/IDs, isolated targets, command manifest, incremental evidence | accepted P3/P4 input inventory |
| H1 | four REL/PHY/principal topologies, independent selection, shared owner/journal Arc identity, actual PHY worker count, wrong/guessed owner and dependency denial, viewer detach, one held/quiesced owner while others read | frozen attribution/publication/offline PHY/worker lifecycle tests |
| H2 | seeded pending order with reversed client clock, deduplicated retry, zero-viewer disk remount, cancel/finalize winner and replay version | verified equal-time ties, sequential/visible branch policy, multiple branch generations, finalization and journal recovery tests |
| H3 | increasing fanout, fast/slow recipients, overflow/resync acknowledgment, revocation of queued delivery, remount cursor rejection, durable multi-byte checkpoints/late join/private broad-feed exclusion | atomic snapshot/mutation races, byte overflow, authoritative source cut, checkpoint-sync faults and stream reconnect tests |
| H4 | file-backed host receipts/status, lost ack, Unknown without redispatch, cooperative Running cancel, cancel/dequeue one gate, actual abort after host acceptance, persisted cancel before replay | connect/unavailable/downgrade, corrupt digest, blocked ack cancellation, quiesce/detach during handoff, divergent terminal proof tests |
| H5 | source-backed operation latency, owner-lock read probes, fanout/load growth, PHY worker count, checkpoint throughput and exact delivered/replayed bytes, restart/reconcile duration, process RSS/thread sampling and explicit steady window | no performance thresholds or optimizations invented |

REL feed/stream operations are currently frozen crate-private P4 APIs. The harness is registered in the existing test module to call them, without widening exports or adding production hooks. Ordinary actions call public gateway APIs. The crash stages use concrete durable owner/journal operations to construct exact intermediate states, like the existing recovery tests; they do not add a production failure-injection framework.

## Abrupt crash boundaries

Each child writes and syncs an identity marker only after staging the named durable boundary, flushes READY, then calls `process::abort`; no Drop/shutdown runs. A separate process opens actual files, reconciles twice and checks exact Archive turn/activity/branch publication, unchanged Episode/Insomnia work on the second reopen, and exact generation prefix/sequence/hash. Episode scheduling is forced with a one-byte input boundary so replay checks cover real scheduled work.

| Boundary | Persisted state before abort | Required recovery |
| --- | --- | --- |
| admission | accepted journal payload, no Archive turn | one chronological commit |
| finalize-intent | captured branch/parent/principal/time, no turn | commit the captured immutable turn once |
| archive-before-receipt | synchronized matching Archive turn/branch; journal still captured | reconcile receipt without a second turn |
| checkpoint | durable source and journal prefix | Interrupted, stable identity and prefix |
| checkpoint-source-ahead | synchronized source tail beyond acknowledged journal cut | reconcile the synchronized tail as checkpoint 2, then expose an Interrupted full prefix; no invented delta |
| host-after-acceptance | durable fake-host acceptance; local ack lost | query known terminal proof; no new handoff |
| host-cancel-before-replay | local durable cancel request; remote outcome previously Unknown | cancel first when host Pending is queryable; no replay/resurrection |

The controlled adapter uses one mutex to serialize Pending cancel and dequeue, stores exact digest/status in a synced file and retains stable receipts. It models acceptance/status, **not external provider effects**. It is not Warlock conformance.

## Measurements and comparison
`P6_METRIC` records direct observations. The runner aggregates repeated timings as count/min/median/p95/max/population standard deviation. Fanout dimensions remain separate. Queue admission-receipt-to-commit timings include pending wait, viewer detach, physical restart and commit; batch drain time is also recorded. Whole-operation timing includes synchronization and admission; it is not falsely presented as isolated publication cost. Owner wait/hold values are explicitly probes of one direct Archive-version read contending with public summary reads, not instrumentation of all production critical sections. Exact source checkpoint UTF-8/delta/replay bytes are measured; broad-feed retained allocator bytes are not inferred from Debug text or pointer arithmetic.

When psutil is available, the runner samples RSS/threads every 20 ms and reports sampled maxima. A 250 ms explicit steady-state window retains four RELs, two PHYs, 32 attachments/subscribers and capacity 16. The window is for observation, not a timing-based correctness condition. Without psutil these fields are unavailable, not zero. Fresh fixture files are used for each process; OS cache is not flushed, and the first repetition is not claimed cold.

To compare another revision, install these same test-only files and two-line registration in a separate worktree and run the identical seed/repetitions/instances/features with its own Cargo target. Then pass its successful JSON to `--baseline-report`. Hardware/workload/fixture-source/toolchain/build-environment/runner mismatches fail. Identical production trees at different commits are explicitly a same-source repeatability baseline, not evidence of improvement. Other Cargo/rustc processes present at command start are recorded; concurrent builds can confound comparisons. Ratios are observational and are not performance acceptance. Thresholds require external or measured acceptance criteria; this harness supplies no invented latency limits.

## Diagnostic requests and remaining gates
The frozen base exposes private PHY worker count and owner identity, but no exact REL thread count or wake-loop counter. B's new worker diagnostic may be used only after accepted convergence; wake-frequency and comprehensive coordinator/owner critical-section telemetry require a narrow owning-lane diagnostic, not harness-owned runtime changes. Current evidence reports actual PHY counts, process threads, notification/operation behavior and probe waits without manufacturing unavailable measurements. A read-only owning-sequencer diagnostic is also needed for retained recipient queue occupancy/allocator bytes. Broad REL feeds are count-bounded and have no byte-budget contract; byte-overflow mechanics are covered by existing stream-registry fixtures. The new physical-source stream workload measures 64 KiB of durable/delivered/replayed UTF-8 below the 1 MiB source bound. Tiny lock probes can quantize to zero at the platform clock resolution.

P6-R still requires accepted A+B integration, C's upgraded Warlock/real provider, D deletion/no-legacy guard, final full release commands, operator tests and justified performance bounds. Architecture indexing is separately bounded at release; no stale graph is accepted. These gates remain unaccepted by H-only reports.

## Related docs
- [Phase 6 plan](multiplexing-phase6-parallel-verification-plan-2026-10-06.md)
- [Parallel convergence](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [P3/P4 acceptance](multiplexing-p34-main-integration-2026-10-06.md)

## Notes
Runner-only crash entrypoints are intentionally ignored in ordinary Cargo runs and require runner-supplied isolated paths. Do not invoke them against user data. The runner's own fixture directories and command logs are preserved as evidence.
