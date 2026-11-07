# G5 — measured performance and release acceptance

Parent index: [Architecture](INDEX.md). Parent plan: [post-G4 execution](post-g4-parallel-execution-plan-2026-11-06.md).

**Status: implementation-ready specification; G5 not started or accepted.** Benchmark harness development and native 2.1 measurements may proceed now. Comparative historical measurements require F03-admitted old owners and D02-admitted equivalent new owners. Final G5 acceptance depends on D03-T04; no benchmark result alone authorizes activation.

## Purpose

Confirm the replacement is operationally usable at the preserved corpus sizes, not merely logically correct. Detect regressions in open/replay, retrieval, writer/worker latency, memory, queue fairness and recovery while preserving exact privacy and accepted-use semantics.

## Overview

Freeze workloads and pass/fail policy before measuring the candidate. Compare the verified historical FRS07 implementation and the accepted 2.1 runtime under matched source semantics and the same host conditions. Use the generated G4 corpus for deterministic functional/fault coverage, the three historical REL/PHY pairs for parity-scale measurements, and generated larger populations for boundedness trends. Report unsupported old capabilities as not comparable, never as zero cost.

## Target architecture and measurement boundary

A small external benchmark runner invokes separately built old/new binaries; it is not a runtime service. Native benchmark adapters live at existing public/owner/Gateway seams with no old dependency in the new graph. Warm loops may run inside a process; process startup/open is measured separately. Build in release mode with locked dependencies and record compiler, optimization/features, OS, CPU, memory, storage, process affinity if used, source code/binary hashes, file hashes and dataset counts.

Pin `c6010fd3` or a separately validated FRS07-capable historical source for old corpus runs. The F01 `f62b1fb4` reader's three REL failures cannot serve as a usable baseline. Freeze deterministic endpoint responses and embedding arrays; measure local machinery independently of provider network latency. Any optional live-provider demonstration is labeled separately and cannot replace deterministic parity or performance acceptance.

## Workload matrix

| ID | Workload | Measurements and correctness check |
| --- | --- | --- |
| G5-01 | First-process open/replay and repeated read-only reopen, REL and PHY | Wall/CPU time, peak private/working-set bytes, I/O bytes; exact heads/history/source hashes |
| G5-02 | Lexical, vector/hybrid and agentic refine/traverse | Median/p95/p99 latency, allocations/peak memory, candidate counts and exact frozen order/IDs |
| G5-03 | Accepted native turn -> Episode -> scripted extraction/publication | End-to-end local latency plus stage timings; bounded evidence, correct routing, single durable completion |
| G5-04 | Dream/Entity/vector/Freshness workers | Throughput, queue lag, peak memory and lock-held duration; idempotent resume and no provider call under mutation lock |
| G5-05 | Concurrent sessions/owners | 1, 2, 8 and 16 clients over one/shared and separate owners; throughput/fairness/p95, no duplicate writer, privacy or cancellation regression |
| G5-06 | Host queue/stream/reconnect/recovery | Checkpoint-to-visible latency, replay size and recovery time; no unsynced text, blind redispatch or repeated credit |
| G5-07 | History, maintenance and verified-copy workflows | Runtime/peak memory/temp disk amplification; unchanged sources, exact closure and second-pass no-op |
| G5-08 | Bounded larger generated populations | 1k, 10k and 50k Memories with fixed query/evidence windows; measured scaling and bounded hydration, not a marketing accuracy result |
| G5-09 | Quiesce/remount/staged-generation rehearsal | Time to drain/hold, restart and read-only health; D03 state transitions and rollback boundary |

Cold-start means a fresh process, not a guaranteed cold operating-system cache. State cache conditions explicitly. Do not flush the user's machine caches, disable security tools, alter global performance settings or kill unrelated workloads. If comparable host conditions cannot be established, record the run as inconclusive and rerun rather than modifying the pass threshold.

## Fixed acceptance policy

Existing tighter, directly applicable accepted baselines take precedence. For newly specified comparisons this specification sets these defaults; they are policy, not claimed measurements:

- Exact correctness, privacy, history and recovery checks: zero unexplained differences or violations, regardless of speed.
- Local-operation median latency: candidate no more than 1.10 times reference. p95 latency: no more than 1.25 times reference. Sustained throughput: at least 0.90 times reference.
- Peak private/working-set memory for the same workload: no more than 1.15 times reference. Report both metrics where available; do not substitute a lower metric to pass.
- No new whole-file copy/materialization on ordinary bounded retrieval or per-turn operation; no provider call under an owner mutation lock; no unbounded queue or stale-owner retry loop.
- A workload with no trustworthy reference measurement is `UNBASELINED`, not automatically passing. Freeze a workload-specific absolute limit with rationale before release measurement; it remains an explicit G5 item until reviewed.

Use at least 5 warmup iterations and 30 measured samples for short operations; use 10 alternating old/new samples for costly open/maintenance runs. Repeat across 3 independent sessions, with no concurrent compilation or benchmarks. Record individual samples, not just aggregates. Use a reproducible paired bootstrap for ratio confidence intervals: pass only when the upper 95% bound meets latency/memory thresholds (lower bound for throughput). A bound straddling the threshold is inconclusive, not a performance defect or pass. Increase samples within an explicitly recorded cap or investigate noise; never silently drop outliers. For expensive resource metrics report observed maxima and all samples in addition to the interval.

Near-zero reference timings need batching to exceed timer resolution, not arbitrary ratio exemptions. p99 is diagnostic at these sample counts; do not present it as a precise release threshold. Memory leak/queue growth checks compare settled idle after repeated equivalent cycles, with allocator high-water behavior distinguished from retained live state by evidence.

## Resource and safety policy

Benchmarks require an exclusive machine measurement window, not merely a separate worktree. Normal independent editing may continue; CPU-heavy builds/tests, disk scans and model inference must not overlap measured runs. Establish free-space requirements before release compilation; reuse one isolated cache per incompatible runtime and serialize Cargo operations. Do not create a target directory per ticket or delete another lane's cache. All mutation workloads use copies; source/candidate custody is rechecked afterward.

## Tickets

| Ticket | Hard predecessors | Deliverable | Acceptance and evidence |
| --- | --- | --- | --- |
| G5-T01 — freeze workload/policy | PG-C01 | Versioned workload manifest, benchmark machine description, thresholds and comparison method | Source-controlled case IDs/data/model inputs, correct statistical self-tests, no post-result threshold changes |
| G5-T02 — native runner and baseline adapters | G5-T01 | Reproducible runner, 2.1 generated workloads and separately built old adapter | Timers/peak-memory units verified, no fake cold-cache claims, workload semantic self-checks; native results marked provisional until paired sources exist |
| G5-T03 — comparative corpus measurements | G5-T02, F03-V04, D02-T04 | Paired old/new evidence for G5-01..08 on exact parity-approved files | All mandatory thresholds pass or explicitly remain inconclusive/fail; raw samples, source hashes and no source mutation |
| G5-T04 — final release/recovery rehearsal | G5-T03, D03-T04, PG-C02 | Exact packaged-runtime regression, privacy/host/cutover rehearsal and independent release review | G4/GM/P01 + full Rust/CLI/consumer suites, DAG, packaging old-reader absence, D03 prewrite rollback and postwrite fence all pass |
| G5-T05 — release decision and authorized activation | G5-T04 | Signed-off immutable release decision; operator-approved selector/write transition | Exact artifact/manifest hashes; recorded authorization; all-owner read-only health before writes; acceptance and activation state reported separately |

G5-T05 may produce `release-ready / not activated` while awaiting operator authorization. Do not mark canonical G5's "activate rebuilt product" deliverable complete until the authorized activation and post-activation checks actually ran. This is not an invitation to request approval before specification or safe preparation.

## Testing decisions and commands

The runner added by T02 must expose a reproducible `benchmark --workloads <manifest> --old <binary> --new <binary> --cohort <approved-manifest> --output <directory>` interface, structured per-case results and nonzero for failures/incomplete mandatory evidence. Freeze tool naming in T01. It records diagnostics separately from pass/fail and rejects accidental debug/release mismatches.

Final product gates from `Reliquary-rebuild` remain:

```text
python scripts/check_architecture.py
cargo fmt --all --check
cargo fmt --manifest-path cli/Cargo.toml --check
cargo fmt --manifest-path tests/external-consumer/Cargo.toml --check
cargo check --workspace --locked --offline -j 1 --target-dir target-codex-check
cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1
cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1
cargo check --manifest-path tests/external-consumer/Cargo.toml --locked --offline -j 1 --target-dir target-codex-check
```

Also run the corrected G4 fresh-process harness and explicitly enabled consumer test; a skipped fixture-dependent test is not counted. Verify the measured runtime is the staged release binary rather than just this debug test build. Preserve command logs, exits and durations. Documentation checks must pass for changed release/spec files; pre-existing unrelated failures remain explicitly identified, not erased by a new blanket baseline.

## Completion and exclusions

Release readiness requires D03 staging, paired performance acceptance, complete functional/privacy regressions, independently reviewed artifacts and an explicit recovery runbook. Actual activation requires recorded authorization and the validated write boundary. Excluded: provider-quality/LME claims, speculative fleet deployment, hidden threshold relaxation, sacrificing history for speed, or a new backwards-compatibility reader.

## Related docs

- [D03 staging and rollback](d03-safe-activation-implementation-spec-2026-11-06.md)
- [D02 parity](d02-independent-parity-verifier-implementation-spec-2026-11-06.md)
- [Shared evidence](post-g4-corpus-evidence-contract-2026-11-06.md)
- [Rebuild execution authority](../architecture-rebuild-implementation-plan-2026-11-03.md)

## Notes

Native benchmarking preparation is parallel-ready; noisy competing benchmark runs are not. This spec sets acceptance policy but reports no performance measurement. Hermes hackathon demonstration is a separate native-data product milestone, not a substitute for this historical-data release gate.
