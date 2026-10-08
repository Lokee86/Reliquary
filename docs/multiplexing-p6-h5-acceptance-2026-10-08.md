# Multiplexing P6-H5 — N0 diagnostics and measurements

Parent: [Phase 6 verification plan](multiplexing-phase6-parallel-verification-plan-2026-10-06.md)

## Purpose

Complete the diagnostic tail defined by the [next-wave plan](multiplexing-phase5-next-parallel-execution-plan-2026-10-07.md), using the real N0 GatewayRuntime and physical REL/PHY files. This record accepts runnable N0 diagnostics; post-D measurements and the P6-R product release remain later gates.

## Overview

The isolated `feat/multiplexing-p6-h5` branch starts at N0 `999fbf7d85c9f892a1bf979293588033bcc1484b`. It carries the accepted test-only P6 harness from `81a64b679ce8ecd650b649cf2cd419c4c6d62fad` and adds read-only `cfg(test)` observations in the owning P3/P4/PHY modules. Production fields, runtime authority, persistence, public APIs and host routing are unchanged. Historical P3/P4 measurements remain on the accepted harness branch; these new reports establish the N0 baseline, not a performance improvement over that historical source.

| Observation | Actual source | Meaning |
| --- | --- | --- |
| REL/PHY workers | Accepted B `background_worker_state_for` | Live owner JoinHandles; exactly 3 per owner with fixture Insomnia workers set to zero |
| Notification requests | Existing REL/PHY control epoch | Real wrapping epoch deltas over 10 route replacements, including concurrent signals; excludes timeout wakes and executed wake-loop counts |
| Coordinator contention | Existing P3 coordinator mutex | 100 read-only branch-count probes during 100 public admissions |
| Owner contention | Existing REL runtime mutex | 100 direct Archive-version probes during 100 public summary reads |
| Broad queue occupancy | P4 recipient VecDeque | Actual pending lengths, allocated Arc queue slots, and resync latch counts |
| Stream retained bytes | P4 `pending_text_bytes` | Maintained logical bytes per recipient; shared payloads counted per recipient |
| Memory and threads | psutil process-tree sampler | RSS/thread samples every 20 ms; 250 ms steady window |
| Checkpoint/replay/restart | Existing P3/P4 public paths | 16 durable UTF-8 checkpoints, 64 KiB exact delivery, late join, physical reopen and reconciliation |

Probe hold durations are deliberately scoped; they do not instrument all production critical sections. Broad-feed payload bytes are null because there is no maintained byte counter. Queue slot bytes exclude event payloads and Arc headers. Stream logical bytes and sampled RSS are not allocator-exact footprints. No test-only mutable control is introduced.

## Reproduction

Use a dedicated target directory and new evidence directory for each invocation:

```text
CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0
CARGO_INCREMENTAL=0
python scripts/run_multiplexing_p6_harness.py --seed 38 --repeat 3 --instances 1,8,32 --target-dir <isolated-target> --output target/p6-h5-results/n0-baseline-final-v2
python scripts/run_multiplexing_p6_harness.py --seed 38 --repeat 3 --instances 1,8,32 --target-dir <isolated-target> --output target/p6-h5-results/n0-repeat-final-v2 --baseline-report target/p6-h5-results/n0-baseline-final-v2/report.json
```

Environment variables above describe the tested Windows invocation; set them using the invoking shell's syntax. Both measurements use the same source, binary, features and hardware. Physical files are fresh per process; OS caches are not flushed and the first repetition is not called cold. Timing variance is observational with no invented acceptance thresholds.

The runner hashes test fixtures, read-only observer bodies and their owning production modules separately. Comparison permits production-source changes but rejects fixture/observer changes. It records effective build flags, and refuses hardware, workload, toolchain, runner or fixture mismatches. It rejects missing H5 diagnostics, incomplete topology/lock samples, and retained-byte disagreement. Output directories and crash files survive failure. Seven durable crash boundaries each run an abort child followed by physical reopen/recovery.

## Evidence

The [committed metrics snapshot](multiplexing-p6-h5-metrics-2026-10-08.json) contains exact recorded values, distributions, source hashes, hardware, build settings and crash verdicts. Raw report/log/fixture paths and report hashes are retained in the snapshot.

Measured on 11th Gen Intel(R) Core(TM) i9-11900H @ 2.50GHz, 8 physical/16 logical CPUs and 34,111,545,344 RAM bytes. Each report contains three repetitions; both run the same test binary. The corrected steady fixture uses exactly 32 existing attachments for diagnostics and 32 subscriptions across 4 RELs/2 PHYs.

| Median timing | N0 baseline (ms) | Matched repeat (ms) |
| --- | ---: | ---: |
| Physical owner startup | 59.338 | 53.543 |
| Accepted receipt to commit | 79.734 | 79.010 |
| 16 checkpoints / 64 KiB | 56.543 | 56.333 |
| Late-join full-prefix replay | 2.077 | 1.962 |
| Physical reopen/reconciliation | 13.761 | 13.265 |

| Memory observation | N0 baseline (MiB) | Matched repeat (MiB) |
| --- | ---: | ---: |
| Median of sampled process peaks | 25.23 (25.19–25.27) | 25.17 (25.16–25.25) |
| Median of per-run steady RSS medians | 18.86 (18.86–18.89) | 18.86 (18.86–18.89) |

| Fan-out recipients | Maximum observed queued broad events | Maximum allocated queue slot bytes |
| ---: | ---: | ---: |
| 1 | 2 | 32 |
| 8 | 58 | 480 |
| 32 | 250 | 2016 |

All six repetitions observe 12 live REL workers and 6 PHY workers. The maintained stream queue holds 65,536 logical text bytes across 16 queued events before draining and zero events/bytes afterward. Notification epoch deltas and all probe distributions are preserved verbatim in the snapshot.

Probe windows synchronize their start but do not force overlapping critical sections. Timer-resolution zero readings do not prove absence of contention. These results establish reproducibility and bounded diagnostic visibility, with no optimization or release-performance claim.

Earlier unaccepted measurement directories used extra diagnostic attachments and are superseded by the two 32-attachment reports referenced here. Setup builds interrupted while diagnostic changes were being finalized are not acceptance evidence.

## Verification

- `cargo fmt --check` and `cargo check --locked -j1`: passed.
- `cargo test --locked -j1 -- --test-threads=4`: 1110 library tests passed (2 ignored crash helpers); 128 integration/doc-test passes.
- Each final runner invocation: three repetitions of 11 ordinary H5 scenarios, existing gateway matrix, and seven abort/reopen pairs passed; zero-test/missing-metric success is rejected.
- P2/P3 ownership guards, P4 coverage self-test and eight Python runner tests: passed.
- Documentation structural and change-impact checks against `origin/main`: passed; final documentation edits receive the same two checks before commit.
- No public library/CLI code or persistence change; CLI rebuild and graph-corpus smoke are not required for these test-only observations.
- Verification logs and command manifest: `target/p6-h5-verification/`. The production check and full matrix also passed before the diagnostic-attachment correction; final results above apply to the corrected fixture.


## Documentation impact

- Inspected: AGENTS, architecture, invariants, storage format, maintainer map, limitations, roadmap, P6 plan, next-wave plan and accepted harness guide/record.
- Updated: this acceptance record and metrics snapshot; Phase 6 and next-wave status; index, architecture, maintainer map and coverage.
- Not affected: public API, storage format, CLI and semantic behavior.
- Compliance check: both structural and change-impact checks passed; final record is checked again before commit.
- Known documentation gaps: post-D measurements and P6-R are explicitly deferred to their dependency gates.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: none in production; read-only test observers remain colocated with P3/P4/PHY owners and expose only counts/timings.
- Pitlord enforcement impact: no production ownership/policy change and no new refresh attempted. The inherited architecture graph-refresh gap is not claimed fixed; P2/P3/P4 direct enforcement passed.
- Other verification impact: actual worker, notification, queue and retained-byte observations augment the existing real-runtime/crash fixtures.
- Known architectural gaps: comprehensive production lock-hold/wake-loop telemetry and allocator-exact accounting are not claimed by this diagnostic tail.

## Related docs

- [N0 convergence acceptance](multiplexing-n0-convergence-acceptance-2026-10-07.md)
- [P5-B capability ledger](multiplexing-p5b-capability-ledger-2026-10-06.md)
- [P4 stream ownership](multiplexing-phase4-streams-2026-10-05.md)
- [Phase 6 plan](multiplexing-phase6-parallel-verification-plan-2026-10-06.md)

## Notes

This closes the N0 diagnostic gate in the next-wave plan. Rerun on the actual post-D tree after N4, then execute upgraded Warlock/provider/Tauri release conformance at N5. No P6-R acceptance or full multiplexing hard-cut completion is implied.
