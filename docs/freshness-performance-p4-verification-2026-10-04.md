# Freshness P4 verification — bounded receipt lookup

Parent: [Documentation index](INDEX.md).

## Purpose

Execute measured P4 under the [P2–P4 plan](freshness-performance-p2-p4-execution-plan-2026-10-04.md), after P2/P3 commit `b6a589d85ce3aefd236c0344558f16ee21cfa1a6`.

## Overview

P4 is accepted on 2026-10-04. Diagnosis was frozen before production optimization. Eleven samples passed compiler/indexer guards. The depth-1024 batch-64 row (marked *) was contaminated by concurrent compilation and is diagnosis only. Final acceptance below uses a separate clean matched comparison. Preparation, ingest and lookup contain nested inclusive stages; do not sum them.

## Frozen diagnosis

Option C is selected: new, absent accepted-use/source-event identities repeatedly scan the complete accepted journal prefix even during normal in-order append. Each operation performs `2 * batch + 3` scans. Lookup accounts for about 96–99% of the eleven clean samples. Both projections together take at most 0.042 ms; double projection and unnecessary ordering fallback are falsified as dominant targets here. No projection reuse or ordering change is justified.

Frozen archive: `benchmarks/freshness-p4-baseline-2026-10-04.zip`, SHA-256 `4d6ad7229d18835a5a9e78d61f2ecc3a523598ef1532bcf42ff97c956b1a4d79`. It includes raw traces, full baseline source snapshots, an instrumentation patch against P2, hashes and explicit contamination labels. Baseline executable SHA-256: `e5e2cb07b8e63a052edce7bf42b20d430ed8660a94979ff8ea9656d29134f938`.

All times below are milliseconds from one diagnostic sample per shape. Depth counts preceding single-memory accepted-use receipts; every disposable REL also has 64 published roots. New receipts are in canonical order, effects are source-only, and saturation at score 100 makes this a storage-cost fixture rather than historical score calibration.

| Depth | Batch | Total | Prepare | Ingest | Lookup | Preprojection | Postprojection | Encode | Append/sync | Due |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 1 | 14.956 | 2.337 | 6.716 | 14.148 | 0.008 | 0.002 | 0.001 | 0.552 | 0.010 |
| 0 | 4 | 24.780 | 5.791 | 11.978 | 24.152 | 0.008 | 0.003 | 0.005 | 0.368 | 0.009 |
| 0 | 16 | 49.976 | 19.475 | 23.923 | 48.764 | 0.011 | 0.005 | 0.007 | 0.402 | 0.022 |
| 0 | 64 | 164.864 | 76.043 | 81.803 | 162.585 | 0.029 | 0.012 | 0.020 | 0.632 | 0.043 |
| 160 | 1 | 12.616 | 3.295 | 7.196 | 12.084 | 0.002 | 0.002 | 0.001 | 0.413 | 0.004 |
| 160 | 4 | 22.159 | 8.401 | 11.541 | 21.500 | 0.003 | 0.002 | 0.004 | 0.506 | 0.005 |
| 160 | 16 | 64.807 | 29.257 | 32.750 | 63.357 | 0.006 | 0.009 | 0.006 | 0.532 | 0.037 |
| 160 | 64 | 233.609 | 114.298 | 116.705 | 231.620 | 0.026 | 0.014 | 0.016 | 0.448 | 0.042 |
| 1024 | 1 | 27.020 | 8.600 | 13.756 | 26.427 | 0.002 | 0.002 | 0.001 | 0.484 | 0.004 |
| 1024 | 4 | 53.871 | 22.379 | 26.442 | 53.072 | 0.018 | 0.003 | 0.005 | 0.602 | 0.009 |
| 1024 | 16 | 150.766 | 70.545 | 74.917 | 149.173 | 0.007 | 0.005 | 0.006 | 0.437 | 0.015 |
| 1024 * | 64 | 541.952 | 270.481 | 261.101 | 534.462 | 0.025 | 0.014 | 0.015 | 5.669 | 0.040 |

| Depth | Batch | Payload bytes | Events/effects | Scan calls | Chunks visited | Payload bytes scanned |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 1 | 373 | 1/1 | 5 | 2302 | 207396 |
| 0 | 4 | 1273 | 4/4 | 11 | 5065 | 456318 |
| 0 | 16 | 4873 | 16/16 | 35 | 16117 | 1452006 |
| 0 | 64 | 19273 | 64/64 | 131 | 60325 | 5434758 |
| 160 | 1 | 373 | 1/1 | 5 | 3105 | 506030 |
| 160 | 4 | 1273 | 4/4 | 11 | 6831 | 1113266 |
| 160 | 16 | 4873 | 16/16 | 35 | 21735 | 3542210 |
| 160 | 64 | 19273 | 64/64 | 131 | 81351 | 13257986 |
| 1024 | 1 | 373 | 1/1 | 5 | 7425 | 2117390 |
| 1024 | 4 | 1273 | 4/4 | 11 | 16335 | 4658258 |
| 1024 | 16 | 4873 | 16/16 | 35 | 51975 | 14821730 |
| 1024 * | 64 | 19273 | 64/64 | 131 | 194535 | 55475618 |

## Retained optimization and build identity

Option C alone is retained. `FreshnessStore` owns a fixed 8,192-byte bitset and completeness flag; four SHA-256-derived positions distinguish use and event namespaces. A proven negative skips lookup after checking exact recent receipts. Positive, incomplete and saturated hints retain exact lookup. Validated receipts populate the bitset before recent-cache eviction or skipping recent retention for oversized receipts. Rebind invalidates it; finish-rebuild stages the canonical records and a full accepted-prefix membership scan before enabling it. Reopen, repack and reconciliation rebuild through the existing owner lifecycle.

The recent cache remains 128 receipts/256 KiB. The bitset plus its flag/alignment occupies fixed additional space; no receipt map grows with journal history. Preparation remains immutable, both projections remain, and append/sync remains the durability boundary. No storage bytes, effect/proof encoding, policy, ordering, API or owner boundary changed.

Baseline is accepted post-P2 `b6a589d85ce3aefd236c0344558f16ee21cfa1a6` plus the frozen instrumentation patch. Candidate is the separate P4 commit containing this report, with that parent; its exact tested source/binary identities are in the archive manifest.

- Toolchain: `rustc 1.97.1 (8bab26f4f 2026-07-14)`.
- Baseline binary SHA-256: `e5e2cb07b8e63a052edce7bf42b20d430ed8660a94979ff8ea9656d29134f938`.
- Candidate binary SHA-256: `23e1efc0526f6798c0a8d595025cec44a59658e772a626b3dcadeb4739ff1fc6`.
- Shared trace source SHA-256: `48e547b07c0e49e97d1cd54e70aafdc9ccc3fbcb7218dc1b8d6e83f1d1618b95`.
- Candidate membership module SHA-256: `193b5b76292ada8fe77dbb527eb81867a7afdbcff1530e2dd86cc5d7f1431844`.

The baseline and candidate use the same release profile, trace feature, lockfile and fixture construction. Candidate corrects metadata emitted **after** timing: history mode has 64 published Memories and zero active Graph nodes/edges; the normal Dream fixture ends with 68 published Memories, 12 active Graph nodes and 15 edges. Frozen baseline stdout inherited the normal template's old 68/15 fields; those fields are not used for history identity or timing comparisons. The manifest records the actual history shape and preserves both source versions.

## Clean matched acceptance

Three alternating fresh-process rounds cover all twelve shapes. Compiler/indexer guards passed every retained sample; any contaminated attempts were discarded. All 36 baseline/candidate pairs matched payload size, event count, current/historical records, and each executable verified reopen. Storage-interface tests additionally compare exact encoded immutable effects/proofs/receipts with hints disabled.

Final archive: `benchmarks/freshness-p4-final-2026-10-04.zip`, SHA-256 `4666e6287daa40306f8b4bfe961370d371ba7b64ab61a4d194a4f8c9ec69795e`. Its 356 checksummed payload files preserve raw operation/history/follow-up samples, load observations, build manifests, source snapshots, correctness logs and final architecture output. Each archive entry was rehashed after packing; current source hashes were checked against the candidate build and collection manifests.

Medians below are milliseconds. Lookup is the sum of nonoverlapping use/event lookup stage medians; parent prepare/ingest rows remain inclusive in raw traces.

| Depth | Batch | Baseline total | Candidate total | Speedup | Baseline lookup | Candidate lookup | Candidate scan calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 1 | 16.289 | 0.486 | 33.5x | 15.593 | 0.003 | 0 |
| 0 | 4 | 22.151 | 0.575 | 38.5x | 21.386 | 0.005 | 0 |
| 0 | 16 | 51.055 | 1.168 | 43.7x | 49.889 | 0.011 | 0 |
| 0 | 64 | 165.416 | 1.855 | 89.2x | 162.764 | 0.031 | 0 |
| 160 | 1 | 14.676 | 0.443 | 33.1x | 14.144 | 0.007 | 0 |
| 160 | 4 | 22.376 | 0.504 | 44.4x | 21.874 | 0.014 | 0 |
| 160 | 16 | 63.208 | 1.088 | 58.1x | 61.870 | 0.042 | 0 |
| 160 | 64 | 222.180 | 1.720 | 129.2x | 220.535 | 0.141 | 0 |
| 1024 | 1 | 25.530 | 0.477 | 53.5x | 25.031 | 0.007 | 0 |
| 1024 | 4 | 47.622 | 0.517 | 92.2x | 46.498 | 0.013 | 0 |
| 1024 | 16 | 145.903 | 1.046 | 139.5x | 144.449 | 0.042 | 0 |
| 1024 | 64 | 529.942 | 1.736 | 305.2x | 522.853 | 0.135 | 0 |

The dominant lookup stage fell by 99.93–99.97% in the medium/large-history medians. Every complete local operation shape materially improved. Append/sync is unchanged code and remains separately timed; these gains remove repeated journal reads/open/decode work, not durability or host/provider latency.

### Exact-retry investigation

The initial three-round matrix flagged fresh batch 4 at +36.1% (27.4 → 37.3 µs) and large-history batch 4 at +7.2% (29.0 → 31.1 µs). These flags were preserved and investigated individually rather than hidden by a global noise allowance.

Nine additional alternating matched pairs per flagged shape did not reproduce the slowdown:

| Depth | Batch | Baseline retry median µs | Candidate retry median µs | Candidate/baseline |
| --- | ---: | ---: | ---: | ---: |
| 0 | 1 | 22.10 | 18.70 | 0.846 |
| 0 | 4 | 31.80 | 30.00 | 0.943 |
| 1024 | 4 | 27.90 | 24.90 | 0.892 |

Six identical-candidate A/A pairs per fresh shape produced median differences of 1.9% at batch 1 and 19.0% at batch 4; the latter was 37.3 versus 30.2 µs using the same binary. The archived raw samples retain outliers. No repeatable material small-history or exact-retry regression was established. These small sample sets do not establish production tail percentiles or a universal retry speedup.

### Shared Dream and reopen check

Three alternating rounds of the unchanged complete-operation fixture preserved logical event/root counts and executable-local record/reopen checks:

| Stage | Baseline median ms | Candidate median ms | Candidate/baseline |
| --- | ---: | ---: | ---: |
| initial_dream_1_root_prepare_evaluate_commit | 144.611 | 127.593 | 0.882 |
| initial_dream_2_root_prepare_evaluate_commit | 152.581 | 130.876 | 0.858 |
| initial_dream_4_root_prepare_evaluate_commit | 166.503 | 150.664 | 0.905 |
| initial_dream_8_root_prepare_evaluate_commit | 172.922 | 150.329 | 0.869 |
| reopen | 415.326 | 416.886 | 1.004 |

Dream settlement medians improved about 9–14%; reopen differed by 0.4%. Settled Dream retries stayed around 1–6 µs; the 8-root median changed by 0.1 µs, which is timer-scale variation rather than material operation regression. Full Dream/recovery/reconciliation behavioral tests also passed.

## Correctness and compliance

Final production source passed:

- Three new focused membership tests, including incomplete/saturated hints and storage-interface rejection/eviction/rebuild behavior.
- Locked root check; trace-enabled tests/examples check.
- Full root suite: 821 library tests and 61 integration tests, zero failed/ignored; doc-tests passed.
- Root/CLI format checks; CLI locked check and 19 tests.
- Final architecture refresh: snapshot `sha256:44d38781a5e5274b677778c068a1544b5bad7b11bb574c9adab84ab79c949faa`, six rules, 50,452 source nodes, zero relationships, exit 0.
- Documentation structural and changed-from-origin/main checks; whitespace diff check.

An initial new comparison fixture attempted exact scanning before any physical CVA chunk existed and failed. It was corrected to create a valid accepted prefix, then all three new tests and the full suite passed. The output-only metadata correction was checked and rebuilt after the full suite; timed fixture construction and production code did not change. Collector aggregation now handles optional stages absent after scan removal. The final architecture refresh includes that collector version.

Pitlord's zero-relationship output is the configured static gate, not proof of arbitrary runtime dependencies. Behavioral tests provide the owner, replay, failure and historical guarantees.

## Exact commands

Correctness used the warmed target `%TEMP%\\reliquary-r3-verification`, with `CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0`:

```text
cargo fmt --all -- --check
cargo test --locked -j2 --lib membership
cargo check --locked -j2
cargo check --locked -j2 --features freshness-performance-trace --tests --examples
cargo test --locked -j2
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo check --manifest-path cli/Cargo.toml --locked -j2
cargo test --manifest-path cli/Cargo.toml --locked -j2
```

Release binaries used `%TEMP%\\reliquary-freshness-perf-baseline`:

```text
cargo build --release --locked -j2 --features freshness-performance-trace --example freshness_operation_baseline
python scripts/freshness_operation_profile.py --baseline %TEMP%\\reliquary-freshness-p4\\baseline.exe --candidate %TEMP%\\reliquary-freshness-p4\\candidate.exe --out %TEMP%\\reliquary-freshness-p4\\final --rounds 3
python scripts/freshness_history_profile.py --baseline %TEMP%\\reliquary-freshness-p4\\baseline.exe --candidate %TEMP%\\reliquary-freshness-p4\\candidate.exe --out %TEMP%\\reliquary-freshness-p4\\final --rounds 3
```

Final architecture used `LEXICON_ADAPTERS=%TEMP%\\reliquary-freshness-architecture-adapters` with `python scripts/check_architecture.py --refresh`. Final documentation checks:

```text
python ../engineering-standards/tools/docs_policy/check.py --repo .
python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
git diff --check
```

The diagnostic baseline used the history collector with `--rounds 1`, then resumed clean rows after competing compilation. The last contaminated diagnostic row is explicitly labelled in its archive. The final matrix uses new samples and no resume. Retry follow-ups run the archived `retry-focused-harness.py` and `retry-large-harness.py` from the repository working directory; their complete source and 9-pair/6-pair settings are archived for reproduction.

## Null ideas and remaining limits

Projection reuse (A) and ordering changes (B) are rejected as unjustified by this profile. P3 remains a measured deferred/no-change checkpoint; no graph cache or historical corpus was added.

This is a source-only, disconnected, stationary local storage fixture. Scores saturate at 100, so it does not calibrate historical score trajectories; richer ordering/score cases are behavioral tests. Positive evicted retries and historical queries still scan accepted history. The fixed hint can saturate, restoring scan cost safely. Reopen adds one bounded-memory prefix scan to reconstruct the hint. Process peak RSS, real host mutex/provider latency, production event distributions and production tails remain unmeasured. No P5/P6, F8 or Ego delivery work began.

## Documentation impact

- Inspected: execution/hardening plans, P0/P1/P2 evidence, required architecture/invariants/storage/maintainer/limitations/roadmap references, and current storage/runtime contracts.
- Updated: this report, execution checkpoint, hardening status, architecture, storage format note, behavioral contracts, limitations, maintainer routing, coverage, development commands and index.
- Not affected: public API, score policy, immutable effects/identity encoding, durable format and Ego delivery.
- Compliance check: structural and changed-from-origin/main validation plus final diff/format checks passed before commit.
- Known documentation gaps: production host distributions/tails and historical score calibration remain outside this milestone.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: FreshnessStore retains receipt/score authority and owns one private bounded acceleration module; Cva retains commit orchestration.
- Pitlord enforcement impact: policy unchanged; final refresh/check passed with the explicit zero-relationship scope above.
- Other verification impact: exact receipt/effect encoding, bounds, eviction, rejection, oracle, historical/recovery/reconciliation, Dream and CLI gates passed.
- Known architectural gaps: positive history lookups remain linear; saturation loses speed safely; static relationships were empty.

## Related docs

- [Execution plan](freshness-performance-p2-p4-execution-plan-2026-10-04.md)
- [P2 verification](freshness-performance-p2-verification-2026-10-03.md)
- [Hardening roadmap](freshness-performance-hardening-plan-2026-10-03.md)

## Notes

P2–P4 execution is complete. Stop here; broader integrated/deployed-host acceptance and semantic extensions remain separate work.
