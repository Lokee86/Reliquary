# G5-T01 workload and policy evidence — 2026-11-07

## Purpose

Record completion of G5-T01's pre-measurement workload and policy freeze. This evidence does not claim a benchmark run, a performance result, or full G5 acceptance.

## Overview

- Planning specification revision: `b6332a8ff5396a7725c73cbe7643c32950e2de7e`.
- Source branch: `codex/g5-t01-measurement-policy-20261107`, initial policy commit `1c33f0015c20668a78ce5908924712b207de0724`, final policy clarification `77cdabb19011e4a59c56dc0a67914b8586800b59`.
- Independent review accepted the exact final source commit `77cdabb19011e4a59c56dc0a67914b8586800b59`.
- Isolated integration checkpoint: `Reliquary-postg4-pgc02-integration-2026-11-07`, branch `codex/postg4-pgc02-integration-2026-11-07`, merge commit `8f0b736bae49212ecb907b31d4869c103598bc56`, tree `c525672f8ca7a6fa222c2af9788d2fc391734e03`.
- The five integrated files are `benchmarks/g5/machine-profile-v1.schema.json`, `benchmarks/g5/policy-v1.json`, `benchmarks/g5/statistics-selftests-v1.json`, `benchmarks/g5/workloads-v1.json`, and `tests/test_g5_measurement_policy.py`. Their committed blobs match source commit `77cdabb`.
- `policy-v1.json` is marked `frozen-before-measurement`. The nine-case workload manifest and statistical self-tests are synthetic policy inputs; the test suite asserts no live provider calls, private corpus, measurement results, or Hermes scope.

The focused command `python -m unittest discover -s tests -p test_g5_measurement_policy.py -v` ran on the integrated head and exited 0: 16 passed, 0 failed. All four G5 JSON files parsed. `python scripts/check_architecture.py` passed with 23 packages, 76 production edges, and 14 test-only edges; the merge-commit `git diff --check` passed.

Captured test output and its receipt are under `C:\!bin\workspace\Reliquary-postg4-pgc02-integration-2026-11-07\.test-tmp\g5-t01-policy-8f0b736b`. The focused stderr log SHA-256 is `0ea0b590b2c3e79350fd79d3cb1ee4d79b1745e8c50ddc903a5317c4e28d3c36`; the receipt SHA-256 is `92aea73d21a021acc992098e554e91f6518151a7f467e693f7c7e8eb7a9623c5`. These captures are local test output, not committed benchmark data.

## Acceptance boundary

G5-T01 is complete for the versioned workload manifest, machine-profile schema, measurement thresholds/method, and statistical self-tests. No measurement adapters or benchmark results were produced here. G5-T02 through G5-T05 remain not started; comparative measurements still require F03-V04 and D02-T04, and the overall G5 capability remains unaccepted. This does not accept a release or authorize activation.

## Related docs

- [G5 performance and release specification](g5-performance-release-acceptance-spec-2026-11-06.md)
- [Post-G4 parallel execution plan](post-g4-parallel-execution-plan-2026-11-06.md)
- [Canonical execution ticket DAG](post-g4-execution-tickets-2026-11-06.json)
- [Canonical rebuild capability tracker](implementation-rebuild-capabilities-2026-11-03.json)

## Notes

The G5 implementation and focused tests are synthetic/pre-measurement policy work. No authentic corpus files, provider calls, product benchmark, historical comparison, release decision, activation, or Hermes work is included in this result.
