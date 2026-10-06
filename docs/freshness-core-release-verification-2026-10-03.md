# Freshness core verification — 2026-10-03

Parent: [Documentation index](INDEX.md).

## Purpose

Record R4–R7 execution, final-source checks and calibration with F8/S1–S6 excluded by user instruction.

## Overview

R4 canonical history, R5 transformation preservation and R6 bounded scheduling/consumer adapters passed their integrated library gates. Final root suites passed 814 unit and 54 integration tests; CLI passed 19 tests, with zero failures or ignored tests. All applicable repository library gates passed. Architecture/documentation checks and release custody are recorded below. End-to-end external host consumption/Ego integration and unavailable live-provider measurements remain explicitly outside this library acceptance.

## Source and environment

Repository `C:\!bin\workspace\Reliquary`, branch `feat/deterministic-memory-freshness`, starting HEAD `a6f580ce2ea22298c6ae894958271485f20e9b7a`. Entry audit found 77 dirty entries and 47 tracked files changed (1,017 insertions/171 deletions). No active Workspace jobs existed at entry. Existing R1–R3 source and evidence were preserved; no multiplexing branch was merged or edited.

Frozen Rust source SHA256: `636d3cbc8414631e377f42135ca65ffd358888bedf03b6ace76cfc785ded5dd6`. Digest concatenates sorted Windows-relative paths and raw bytes from `src/**/*.rs`, then `tests/**/*.rs`, then `examples/freshness_calibration.rs`. Source was frozen before final root/CLI checks and calibration; subsequent changes affect documentation and ownership declarations only. Git commit identity is recorded by release custody after acceptance.

Rust `1.97.1 (8bab26f4f 2026-07-14)`, Windows 10 build 26200, 16 logical CPUs. `CARGO_TARGET_DIR=C:\Users\archa\AppData\Local\Temp\reliquary-r3-verification`, `CARGO_PROFILE_TEST_DEBUG=0`, `--locked -j 2`. This isolated target avoids the installed artifact reclaimer under `C:\!bin`. Diagnostic binaries use the unoptimized test profile; timings are not release-build guarantees. Raw command/calibration JSON is retained in the target's `freshness-evidence` directory.

Unchanged root lock SHA256 `5362fb09abc0995f99bb33462eede0666f9721f734f8f29cef54ab944170dd85`; CLI lock `2b8bb7d25d984266f23e65e1ac405398261dbe5ab1ec55d399eb29d82a47dc9e`.

## Final correctness commands

| Command | Exit/result | Seconds |
| --- | --- | ---: |
| `cargo fmt --all -- --check` | 0 | 1.605 |
| `cargo check --locked --tests --examples -j 2` | 0 | 10.098 |
| `cargo test --locked -j 2` | 0; 814 unit + 54 integration | 152.902 |
| `cargo test --locked freshness_storage:: -j 2 -- --nocapture` | 0; 10 storage tests | 46.271 |
| `cargo fmt --manifest-path cli/Cargo.toml -- --check` | 0 | 0.283 |
| `cargo check --manifest-path cli/Cargo.toml --locked -j 2` | 0 | 13.028 |
| `cargo test --manifest-path cli/Cargo.toml --locked -j 2` | 0; 19 CLI tests | 33.570 |
| `cargo build --profile test --locked --example freshness_calibration -j 2` | 0 | 8.175 |
| `archive_roundtrip.exe <temporary graph.jsonl> <temporary output.cva>` | 0 | 0.283 |

Root commands are completed subcommands of job `job_KEDYwixNQjbm3N-JxjBYGT8u`, saved as `final-root-1.json` through `final-root-4.json`; the wrapper was cancelled only during an unnecessary subsequent DEV-profile example rebuild. The saved root test invocation itself exited 0. CLI/example job `job_4jTk9n-FCl5JPAgMdII3E5c0` succeeded, with `final-cli-0.json` through `final-cli-3.json`. Source digest and saved results were collected by `job_gJQ3fp5YorW4ne3Vg9PXFTxJ`.

The integration inventory comprises F1 (17), F7 acceptance (2), F7 recovery (1), owner contracts (17), exhaustive all-four-node-graphs propagation oracle (1), R2 (1), reconciliation (3), recovery (1), multiplexing baseline (1), and REL activity (10). Root includes deterministic crash-prefix, birth/admission/graph proof, cycles/re-entry, excluded newborn cohorts, duplicate chains, negative recipients, worker-count equality and mixed batch tests.

Archive smoke job `job_I6cwSNTX8WtrI1f9m6n9ECwY` generated a disposable 500-node/three-branch graph: 84 fragments, 500 content objects, 900 expanded references, 155,349 output bytes. Reopen preserved stats, branch paths and exact Unicode bodies. This is a controlled branching corpus, not an authoritative real fixture.

## Phase acceptance and observed repairs

[R4](freshness-r4-verification-2026-10-03.md) records one-prefix bounded sorting and temporary disk receipt validation, exact old retry and immutable canonical replay. Replay/old-retry milliseconds for 128, 512, 4,096, 20,000 batches: 5/1, 27/8, 215/51, 1,498/264. A native 20,000-batch Cva reopened in 22,154/21,659 ms; old retry took 239/244 ms and historical read 120/120 ms. Both runs retained 128 receipts/130,432 accounted bytes. Construction uses prevalidated batches; this is not 20,000 public producer-call throughput.

[R5](freshness-r5-verification-2026-10-03.md) records public strict copy, repack, reclamation, exact divergent rebase and repeated reconciliation. Original effects/receipt identities are preserved without live graph repropagation. Pending first-pass/intents and unsupported migration/provenance are refused before output.

[R6](freshness-r6-verification-2026-10-03.md) records exact boundaries, 1,024-Memory backlog with at most 64 transitions per callback, reversals/reopen, pure lazy reads and Ego small-Web/hard-keep contracts. Runtime Dream freezes under the mutex, evaluates/joins outside it, then validates accepted proof/idempotence at commit. Detached-work tests prove later graph edges receive no historical credit.

An intermediate storage edit accidentally wrote a truncated read and restored an older codec tail. The focused suite caught weakened Community-proof equality; the complete tail/helper and exact equality were restored before the final passing suites. Earlier dependency compilation/wrapper timeouts and cancelled DEV-profile rebuilds are environmental outcomes, not passing tests. No failed earlier run is used as acceptance evidence.

## Architecture and documentation impact

Shared standards changed: none. Freshness owns immutable numerical/event authority; Memory owns semantic revisions; Archive owns accepted turns; Graph/Dream own graph proof and first-pass provenance; Cva composes them. Disposable sort/rebuild files confer no new authority.

Initial full architecture refresh job `job_BdbpTrtjZDZhkfG4xJCT9rnX` exited 1 after successfully refreshing Lexicon/Arcana: snapshot `sha256:6ac3907760581be2850471fa3284fe0f9562cd44bcc0d5f258f23e0fb5c64146`, zero compatibility warnings. Pitlord found 28 existing unowned Context Engine, Project Environment and typed-clock source files. Source-level checking independently found the same omissions and no forbidden-content finding. Repository ownership declarations now reflect their existing documented responsibilities; source and domain ownership were not moved. Freshness test/module paths are explicitly covered. Full refreshed enforcement passed in job `job_K-ERWwE_xAxOsfM9Hyo8WDvH`, exit 0: 15 areas, six rules, 50,213 source nodes, no violations or warnings. Arcana used the same final Rust snapshot with zero compatibility warnings. The current policy does not assert dependency/cycle rules; zero reported relationships is not proof of absence of cross-owner dependencies.

The installed adapter's default Cargo launcher stalled. Verification used `LEXICON_ADAPTERS` pointing to a temporary supported adapter layout containing the sibling prebuilt Rust adapter, Python adapter and generic Go source; shared tool repositories were unchanged. Early 240-second architecture wrapper timeout was not a green gate.

Inspected/reconciled: architecture, invariants, storage/API, behavioral contracts, limitations, maintainer map, coverage, roadmap, docs-standard and ADR 0039. Current journal docs describe `CVAFRS02`, complete `CVDREAM1` kinds and atomic `CVAINSC6` birth proof. V5 compatibility and explicit pending-transform refusal remain documented. F8 accepted future design remains deferred. Final structural/change-impact documentation checks and whitespace audit follow below.

The inherited `archive_open_profile` change removes its conflicting global allocator and labels allocator retained/peak metrics unavailable; root examples compile with the dependency-provided allocator. It is retained as the verified R1 build prerequisite rather than an unrelated feature.

## Integration and measurement limits

The library exposes authenticated accepted-use receipts and graded Ego selection adapters. A universal actual accepted-context delivery producer and live Ego summarizer consumer are outside this repository and remain pending; passive retrieval does not earn +25. Derived notices are coalescing at-least-once hints, not a durable exactly-once consumption log.

Calibration measures prospective current graph topology, never reconstructed historical Freshness trajectories. The real fixtures lack historical admission/link/use observations. Owner open and graph pinning are measured; live inference-provider end-to-end Insomnia/Dream overhead and concurrent-host mutex-wait percentiles are unavailable in this deterministic harness. Source review and detached-work tests establish worker lock placement, not measured contention latency.

Recent receipts and sort runs are bounded; birth/first-pass state scales with Memories. Temporary disk usage scales with accepted history, and individual large payloads are indivisible. Evicted retry remains a linear prefix scan and repeated late appends are not constant time. Abrupt process termination can leave disposable scratch directories.

## Related docs

- [Execution specification](freshness-completion-execution-plan-2026-10-03.md)
- [R2 verification](freshness-r2-verification-2026-10-03.md)
- [R3 verification](freshness-r3-verification-2026-10-03.md)
- [R4 verification](freshness-r4-verification-2026-10-03.md)
- [R5 verification](freshness-r5-verification-2026-10-03.md)
- [R6 verification](freshness-r6-verification-2026-10-03.md)
- [Development and fixture hashes](development.md)

## Notes

F8/S1–S6 are explicitly outside this execution. Calibration tables and final release disposition below belong to this report, not to the prospective specification.

## Calibration evidence

The executable was built by the final test-profile build above. Each run uses 11 repeated 64-event batches for workers 1/2/4/8; all worker outputs are compared to serial. Real inputs are SHA256-checked before copying, copy hashes verified, and original hashes checked after completion. Disposable owner copies/scratch are removed. Native Rust analysis overlapped some real runs; host load makes timing noisy.

| Fixture | Elapsed s | Owner open ms | Graph pin ms | Nodes | Edges | Density | Community memberships |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| synthetic | 20.306 | n/a | n/a | 2048 | 8224 | 0.00392341 | 2048 |
| chatgpt-first14d | 265.98 | 1062 | 24 | 1084 | 7029 | 0.01197473 | 1084 |
| chatgpt-first28d | 257.821 | 2873 | 85 | 2810 | 22464 | 0.00569192 | 0 |
| ellis | 92.847 | 892 | 20 | 674 | 4901 | 0.02160925 | 0 |

| Fixture | Workers | Batch ms p50 / p95 / p99 | Event us p50 / p95 / p99 | Median speedup |
| --- | ---: | --- | --- | ---: |
| synthetic | 1 | 464.993 / 497.334 / 497.334 | 8244.900 / 10174.700 / 11134.900 | 1.000 |
| synthetic | 2 | 447.843 / 491.642 / 491.642 | 6896.300 / 9774.900 / 10588.100 | 1.038 |
| synthetic | 4 | 440.846 / 452.340 / 452.340 | 6449.100 / 9323.400 / 9862.100 | 1.055 |
| synthetic | 8 | 440.950 / 458.355 / 458.355 | 7436.900 / 9255.000 / 10973.800 | 1.055 |
| chatgpt-first14d | 1 | 5884.946 / 6579.188 / 6579.188 | 28409.900 / 180900.600 / 191215.700 | 1.000 |
| chatgpt-first14d | 2 | 5819.812 / 11985.813 / 11985.813 | 87031.600 / 316848.000 / 395543.800 | 1.011 |
| chatgpt-first14d | 4 | 6426.292 / 8015.025 / 8015.025 | 45277.100 / 227921.200 / 260155.000 | 0.916 |
| chatgpt-first14d | 8 | 4740.465 / 5811.132 / 5811.132 | 25097.700 / 158104.500 / 183447.300 | 1.241 |
| chatgpt-first28d | 1 | 5501.496 / 5809.961 / 5809.961 | 9843.900 / 192868.900 / 208447.300 | 1.000 |
| chatgpt-first28d | 2 | 5742.736 / 6263.807 / 6263.807 | 12271.500 / 205497.300 / 225728.400 | 0.958 |
| chatgpt-first28d | 4 | 5833.874 / 6283.929 / 6283.929 | 11775.700 / 206675.300 / 234286.300 | 0.943 |
| chatgpt-first28d | 8 | 5942.597 / 6918.201 / 6918.201 | 13190.300 / 210302.100 / 233702.300 | 0.926 |
| ellis | 1 | 2103.633 / 2310.920 / 2310.920 | 11419.600 / 68839.700 / 77518.900 | 1.000 |
| ellis | 2 | 2060.275 / 2103.511 / 2103.511 | 16083.800 / 66338.900 / 71906.300 | 1.021 |
| ellis | 4 | 2149.094 / 2260.260 / 2260.260 | 11630.000 / 70022.100 / 81225.100 | 0.979 |
| ellis | 8 | 2010.523 / 2106.353 / 2106.353 | 16300.000 / 65144.200 / 72507.800 | 1.046 |

The following raw output records all cost distributions exactly.

### synthetic

Verified original SHA256: synthetic; no authoritative input.

```text
fixture=synthetic_dense_local_v1 projected_nodes=2048 active_memory_edges=8224 community_memberships=2048
workers=1 batch_ms_p50=464.993 p95=497.334 p99=497.334 event_us_p50=8244.900 p95=10174.700 p99=11134.900 touched_p50=525 p95=754 p99=763 examined_edges_p50=4249 p95=8360 p99=8601 max_frontier_p50=178 p95=213 p99=219 batch_speedup_vs_1=1.000
workers=2 batch_ms_p50=447.843 p95=491.642 p99=491.642 event_us_p50=6896.300 p95=9774.900 p99=10588.100 touched_p50=525 p95=754 p99=763 examined_edges_p50=4249 p95=8360 p99=8601 max_frontier_p50=178 p95=213 p99=219 batch_speedup_vs_1=1.038
workers=4 batch_ms_p50=440.846 p95=452.340 p99=452.340 event_us_p50=6449.100 p95=9323.400 p99=9862.100 touched_p50=525 p95=754 p99=763 examined_edges_p50=4249 p95=8360 p99=8601 max_frontier_p50=178 p95=213 p99=219 batch_speedup_vs_1=1.055
workers=8 batch_ms_p50=440.950 p95=458.355 p99=458.355 event_us_p50=7436.900 p95=9255.000 p99=10973.800 touched_p50=525 p95=754 p99=763 examined_edges_p50=4249 p95=8360 p99=8601 max_frontier_p50=178 p95=213 p99=219 batch_speedup_vs_1=1.055
```

### chatgpt-first14d

Verified original SHA256: 49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b.

```text
fixture_copy_hash_verified=true opening_owner=true
owner_open_ms=1062
real_fixture=project.prj sha256=49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b copy_hash_verified=true community_snapshot_current=true rel_turns=5857
graph_pinning_ms=24 graph_density=0.01197473
fixture=project.prj projected_nodes=1084 active_memory_edges=7029 community_memberships=1084
workers=1 batch_ms_p50=5884.946 p95=6579.188 p99=6579.188 event_us_p50=28409.900 p95=180900.600 p99=191215.700 touched_p50=978 p95=1084 p99=1084 examined_edges_p50=29203 p95=112304 p99=112349 max_frontier_p50=1086 p95=2449 p99=2588 batch_speedup_vs_1=1.000
workers=2 batch_ms_p50=5819.812 p95=11985.813 p99=11985.813 event_us_p50=87031.600 p95=316848.000 p99=395543.800 touched_p50=978 p95=1084 p99=1084 examined_edges_p50=29203 p95=112304 p99=112349 max_frontier_p50=1086 p95=2449 p99=2588 batch_speedup_vs_1=1.011
workers=4 batch_ms_p50=6426.292 p95=8015.025 p99=8015.025 event_us_p50=45277.100 p95=227921.200 p99=260155.000 touched_p50=978 p95=1084 p99=1084 examined_edges_p50=29203 p95=112304 p99=112349 max_frontier_p50=1086 p95=2449 p99=2588 batch_speedup_vs_1=0.916
workers=8 batch_ms_p50=4740.465 p95=5811.132 p99=5811.132 event_us_p50=25097.700 p95=158104.500 p99=183447.300 touched_p50=978 p95=1084 p99=1084 examined_edges_p50=29203 p95=112304 p99=112349 max_frontier_p50=1086 p95=2449 p99=2588 batch_speedup_vs_1=1.241
source_hash_unchanged=true
```

### chatgpt-first28d

Verified original SHA256: 8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4.

```text
fixture_copy_hash_verified=true opening_owner=true
owner_open_ms=2873
real_fixture=project.prj sha256=8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4 copy_hash_verified=true community_snapshot_current=false rel_turns=16465
graph_pinning_ms=85 graph_density=0.00569192
fixture=project.prj projected_nodes=2810 active_memory_edges=22464 community_memberships=0
workers=1 batch_ms_p50=5501.496 p95=5809.961 p99=5809.961 event_us_p50=9843.900 p95=192868.900 p99=208447.300 touched_p50=1283 p95=2807 p99=2809 examined_edges_p50=32424 p95=288411 p99=299803 max_frontier_p50=1552 p95=11036 p99=11286 batch_speedup_vs_1=1.000
workers=2 batch_ms_p50=5742.736 p95=6263.807 p99=6263.807 event_us_p50=12271.500 p95=205497.300 p99=225728.400 touched_p50=1283 p95=2807 p99=2809 examined_edges_p50=32424 p95=288411 p99=299803 max_frontier_p50=1552 p95=11036 p99=11286 batch_speedup_vs_1=0.958
workers=4 batch_ms_p50=5833.874 p95=6283.929 p99=6283.929 event_us_p50=11775.700 p95=206675.300 p99=234286.300 touched_p50=1283 p95=2807 p99=2809 examined_edges_p50=32424 p95=288411 p99=299803 max_frontier_p50=1552 p95=11036 p99=11286 batch_speedup_vs_1=0.943
workers=8 batch_ms_p50=5942.597 p95=6918.201 p99=6918.201 event_us_p50=13190.300 p95=210302.100 p99=233702.300 touched_p50=1283 p95=2807 p99=2809 examined_edges_p50=32424 p95=288411 p99=299803 max_frontier_p50=1552 p95=11036 p99=11286 batch_speedup_vs_1=0.926
source_hash_unchanged=true
```

### ellis

Verified original SHA256: 3efd9c666aa992686abd8aac9b77cfc7dc2a0e15d6fb26ec01f8666db175e476.

```text
fixture_copy_hash_verified=true opening_owner=true
owner_open_ms=892
real_fixture=project.prj sha256=3efd9c666aa992686abd8aac9b77cfc7dc2a0e15d6fb26ec01f8666db175e476 copy_hash_verified=true community_snapshot_current=false rel_turns=2972
graph_pinning_ms=20 graph_density=0.02160925
fixture=project.prj projected_nodes=674 active_memory_edges=4901 community_memberships=0
workers=1 batch_ms_p50=2103.633 p95=2310.920 p99=2310.920 event_us_p50=11419.600 p95=68839.700 p99=77518.900 touched_p50=573 p95=673 p99=674 examined_edges_p50=25386 p95=75196 p99=76155 max_frontier_p50=1168 p95=2242 p99=2381 batch_speedup_vs_1=1.000
workers=2 batch_ms_p50=2060.275 p95=2103.511 p99=2103.511 event_us_p50=16083.800 p95=66338.900 p99=71906.300 touched_p50=573 p95=673 p99=674 examined_edges_p50=25386 p95=75196 p99=76155 max_frontier_p50=1168 p95=2242 p99=2381 batch_speedup_vs_1=1.021
workers=4 batch_ms_p50=2149.094 p95=2260.260 p99=2260.260 event_us_p50=11630.000 p95=70022.100 p99=81225.100 touched_p50=573 p95=673 p99=674 examined_edges_p50=25386 p95=75196 p99=76155 max_frontier_p50=1168 p95=2242 p99=2381 batch_speedup_vs_1=0.979
workers=8 batch_ms_p50=2010.523 p95=2106.353 p99=2106.353 event_us_p50=16300.000 p95=65144.200 p99=72507.800 touched_p50=573 p95=673 p99=674 examined_edges_p50=25386 p95=75196 p99=76155 max_frontier_p50=1168 p95=2242 p99=2381 batch_speedup_vs_1=1.046
source_hash_unchanged=true
```

Parallel speedup is workload-dependent: all parallel counts were slower for 28-day, four workers were slower for 14-day and Ellis, while synthetic improved modestly. No universal throughput gain is claimed. The raw lines contain exact p50/p95/p99 touched Memories, edges examined and maximum frontier width.

## Final repository gate disposition

`python scripts/check_architecture.py --refresh` passed with the supported prebuilt adapter override (job `job_K-ERWwE_xAxOsfM9Hyo8WDvH`, exit 0). `python ../engineering-standards/tools/docs_policy/check.py --repo .` passed in 1.678 s; the same command with `--changed-from origin/main` passed in 1.204 s. `git diff --check` passed in 0.058 s (job `job_BhWAQoOG6M4ineJGlgeFNWcA`, exit 0). Only Git CRLF-normalization warnings occurred. Final source digest remained unchanged. Documentation-only final status corrections are checked again before commit.

## Release custody

The accepted milestone is the core library release (R4–R7), with R1–R3 retained and F8 excluded. Stage explicit audited Freshness source/tests/docs and build/policy prerequisites only. No independent multiplexing implementation, shared standards, authoritative fixtures, generated architecture caches, temporary outputs or lockfiles belong to the commit. Confirm staged full diff, untracked content, source digest and staged whitespace before commit. Push only HEAD to refs/heads/feat/deterministic-memory-freshness on origin (https://github.com/Lokee86/Reliquary.git), setting that branch upstream; do not merge or push main. The enclosing verified commit and final Git status provide the custody identity without a circular self-hash in this report.
