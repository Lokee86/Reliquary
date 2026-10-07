# Multiplexing P6-H — harness acceptance and measurement evidence

## Purpose

Record the isolated P6-H real-runtime harness and its reproducible evidence. This report accepts harness infrastructure only. **P6-R release acceptance remains incomplete.**

## Overview

Branch `feat/multiplexing-p6-harness` starts at common planning base `c24b06273abbab383eb74780bf82cb5c0ceef0b5`. The independent baseline is accepted P3/P4 revision `e1c8f88d2dc9ece24713477a27d77ade516de1eb`, with the identical six test modules, two-line test registration and Python runner overlaid. Git object identity confirms that the two committed bases have identical production `src`, `tests`, Cargo manifests/lockfile, `vendor` and `examples` inputs. Consequently the comparison measures repeatability of the same production source; it does not demonstrate a performance improvement.

Only test registration, new `cfg(test)` fixtures, a runner and documentation change. The gateway, owner/journal, physical REL/PHY storage and authoritative streams are real. The host adapter is controlled and durable; it does not establish actual Warlock/provider conformance. The [guide](multiplexing-p6-harness-guide-2026-10-07.md) maps new scenarios to reused production gateway coverage and explains diagnostic gaps.

## Verification

| Gate | Result |
| --- | --- |
| `cargo fmt --check` | Passed; final run 9.015 seconds |
| `cargo check -j1 --locked` | Passed; existing nonfatal warnings retained |
| `cargo test -j1 --locked` | Passed: 1,095 library tests, two runner-only ignored entries, 128 integration tests, zero doc tests |
| `cargo test -j1 --locked --all-targets --no-run` | Passed; 345.469 seconds |
| P2 ownership and P3 ownership guards, `--enforce` | Passed |
| P4 coverage guard, `--self-test` | Passed: 28 fixtures covering 112 public APIs |
| Python runner unit checks | Five passed |
| Independent baseline runner | Passed: 3 × 10 ordinary tests, 246 gateway tests, seven abort/recovery pairs |
| Candidate runner | Passed: identical matrix and strict baseline-comparison validation |
| Docs structural policy | Passed |
| Docs impact policy, `--changed-from origin/main` | Passed |

The first combined verification wrapper reached its 20-minute timeout while awaiting the Cargo test result; it had completed formatting and Cargo check, but did not retain a completed test result or reach all-target compilation. The unchanged-source cached retry completed the unrestricted test gate in 164.719 seconds. The later all-target gate passed independently. The wrapper timeout is recorded separately from passed test outcomes.

Each complete runner executes three repetitions of ten new ordinary tests, the existing gateway suite, and seven abrupt-child/recovery pairs. Crash children terminate only after a flushed durable identity marker. Recovery reopens actual files in another process and checks two passes for stable Archive versions, activity/turn counters, Episode/Insomnia state, branch heads, generation identity and checkpoint text. Source-ahead checkpoint recovery adopts the synced tail; it does not discard durable source state. Native finalize/Archive receipt boundaries and durable host cancellation-before-replay are exercised without claiming exactly-once external effects.

## Measurement environment and results

Hardware: Intel Core i9-11900H at 2.50 GHz, eight physical/16 logical CPUs, 34,111,545,344 bytes RAM; Windows 10.0.26200, Python 3.11.15, Rust 1.97.1 (8bab26f4f, 2026-07-14), Cargo 1.97.1 (c980f4866, 2026-06-30). Seed 38; fan-out sizes 1, 8 and 32; three repetitions per revision; one harness test thread and four existing-gateway test threads.

Operation timings below are milliseconds. Each cell reports median (minimum–maximum); sigma is population standard deviation. Twelve queued admissions per repetition yield 36 admission-to-commit observations; other rows have three observations per revision.

| Operation | Baseline median (range), ms | Candidate median (range), ms | Baseline sigma, ms | Candidate sigma, ms |
| --- | --- | --- | --- | --- |
| Admission receipt to commit | 141.381 (83.157–186.711) | 96.821 (59.596–162.671) | 29.989 | 27.848 |
| Queue drain | 164.465 (160.853–167.973) | 132.044 (129.545–151.825) | 2.907 | 9.966 |
| 16 durable checkpoints, 64 KiB | 79.737 (71.312–84.928) | 80.552 (73.518–134.675) | 5.611 | 27.323 |
| Full-prefix late join | 2.859 (2.641–3.372) | 2.731 (2.479–3.200) | 0.306 | 0.299 |
| Checkpoint reopen/reconcile | 18.505 (16.426–18.903) | 20.079 (15.189–20.904) | 1.086 | 2.522 |
| Register one instance | 0.370 (0.329–0.695) | 0.539 (0.351–0.649) | 0.164 | 0.123 |
| Register eight instances | 1.895 (1.368–2.070) | 2.868 (2.660–3.541) | 0.298 | 0.376 |
| Register 32 instances | 11.593 (5.983–12.672) | 9.381 (6.907–14.791) | 2.932 | 3.292 |
| 24 synced mutations, one instance | 49.989 (40.895–56.159) | 43.879 (43.419–53.445) | 6.270 | 4.622 |
| 24 synced mutations, eight instances | 51.864 (41.011–75.826) | 46.465 (44.934–53.346) | 14.545 | 3.659 |
| 24 synced mutations, 32 instances | 48.281 (37.635–50.171) | 49.755 (45.813–55.340) | 5.518 | 3.909 |
| Unrelated REL/PHY read pair | 0.074 (0.072–0.084) | 0.073 (0.065–0.100) | 0.005 | 0.015 |

Admission-to-commit includes queued wait, viewer detach, physical restart and commit. Mutation/fan-out totals include real storage synchronization. The 64-KiB checkpoint payload took median 79.737 ms in the baseline and 80.552 ms in the candidate (about 0.784 and 0.776 MiB/s respectively); this is the complete measured workload, not isolated serialization cost.

| Revision/repetition | Sampled peak RSS, MiB | Steady RSS median, MiB | Steady samples | Sampled peak threads |
| --- | --- | --- | --- | --- |
| baseline/harness-0 | 25.023 | 20.320 | 6 | 23 |
| baseline/harness-1 | 24.184 | 19.488 | 5 | 23 |
| baseline/harness-2 | 24.203 | 19.496 | 5 | 23 |
| candidate/harness-0 | 24.184 | 19.488 | 6 | 23 |
| candidate/harness-1 | 24.223 | 19.453 | 6 | 23 |
| candidate/harness-2 | 24.230 | 19.492 | 5 | 23 |

Steady samples cover a labeled 250-ms window with four RELs, two PHYs and 32 instances. The nominal sampler interval is 20 ms; OS/process traversal costs reduce actual sample counts. Peak and steady figures describe the test process, including its fixture/runtime overhead.

baseline owner_lock_probe/wait_ns: 300 samples, median 0 ns, p95 100 ns, maximum 43400 ns. Tiny intervals can quantize to zero; the zero-baseline median deliberately has no ratio.
baseline owner_lock_probe/probe_hold_ns: 300 samples, median 0 ns, p95 100 ns, maximum 800 ns. Tiny intervals can quantize to zero; the zero-baseline median deliberately has no ratio.
candidate owner_lock_probe/wait_ns: 300 samples, median 100 ns, p95 4100 ns, maximum 78900 ns. Tiny intervals can quantize to zero; the zero-baseline median deliberately has no ratio.
candidate owner_lock_probe/probe_hold_ns: 300 samples, median 100 ns, p95 100 ns, maximum 2700 ns. Tiny intervals can quantize to zero; the zero-baseline median deliberately has no ratio.


The [metrics snapshot](multiplexing-p6-harness-metrics-2026-10-07.json) retains exact revisions, source and binary hashes, hardware, build environment, command outcomes, sampled memory/thread observations, timing distributions and baseline ratios. Full command output and physical evidence remain under `target/p6-results/candidate` and copied baseline evidence under `target/p6-results/p34-baseline`. Cargo and guard logs are under `target/p6-logs`. These ignored evidence directories are local; the committed snapshot preserves the reviewable measurements.

Serial locked debug/test builds use no extra features. Fresh physical fixture files are created for each process; the OS file cache is not flushed, and the first repetition is not labeled cold. Dependency compilation is setup time and is excluded from runtime timing rollups. The independent baseline target was populated with dependency byte copies and then validated by Cargo; targets are not shared or hard-linked. The runner records other Cargo/rustc processes at each command start; none were observed at the recorded command starts for either final run. Unobserved background activity and cache state make ratios observational; no performance threshold is invented.

## Remaining diagnostics and release gates

The owner-lock probe measures its own tiny hold interval and acquisition wait against real owner contention; it does not instrument every production coordinator/owner critical section. Exact REL worker counts and wake frequency need narrow owning-lane read-only diagnostics. PHY worker counts are asserted from the real runtime. Sampled process threads do not substitute for exact worker attribution.

Subscriber counts and observed process memory are measured, but allocator-exact retained queue/byte footprints need owning-sequencer diagnostics. Broad REL feeds currently have count bounds, not a byte budget. Existing authoritative stream registry fixtures cover byte overflow; the new physical 64-KiB stream workload stays below the 1-MiB source bound. Twenty-millisecond RSS sampling can miss short peaks and is not allocator accounting.

These limits leave the full H5 diagnostic scope incomplete. P6-R also needs integrated accepted A+B, P5-D hard deletion and no-legacy callers, upgraded C/Warlock pinned to the accepted source and one Arcana, actual provider/Tauri and operator integration, justified performance limits, confirmed CLI baseline, and a fresh bounded architecture refresh. Historical indexing failures are not treated as current acceptance evidence. No merge, main update, Warlock change or release verdict is made here.

## Documentation impact

- Inspected: repository instructions, the P5/P6 plans, accepted P3/P4 report, canonical gateway/ownership/privacy/recovery documents and existing gateway fixtures.
- Updated: harness guide, this acceptance report, metrics snapshot, P6 plan status and documentation index.
- Not affected: production API, current-state semantic contracts and CLI behavior; all Rust changes are test-only.
- Compliance check: both required docs-policy modes are recorded in the verification table.
- Known documentation gaps: final post-deletion release evidence and owning-lane diagnostics remain pending.

## Architecture impact

- Standards added or changed: none; existing ownership and stream contracts are exercised.
- Ownership or boundary impact: none in production; the controlled host exists only in test modules.
- Pitlord enforcement impact: existing P2/P3 guards pass; no fresh architecture graph is claimed.
- Other verification impact: deterministic real-file fault runner, repeated measurements and fail-closed runner checks.
- Known architectural gaps: complete H5 instrumentation and post-deletion/upgraded-host P6-R integration are still required.

## Related docs

- [P6 verification plan](multiplexing-phase6-parallel-verification-plan-2026-10-06.md)
- [Harness guide](multiplexing-p6-harness-guide-2026-10-07.md)
- [Metrics snapshot](multiplexing-p6-harness-metrics-2026-10-07.json)
- [Accepted P3/P4 baseline](multiplexing-p34-main-integration-2026-10-06.md)

## Notes

P6-H validates runnable infrastructure and its stated boundaries. It does not waive missing diagnostics or external release gates.
