# Phase 3 local execution ledger — 2026-10-22

Parent index: [Documentation index](INDEX.md)

## Purpose

Track the 98 tasks and 12 gates of the [Phase 3 specification](dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md) against actual local source, tests and corpus evidence.

## Overview

Execution starts at `b113bcf` on `main`; accepted Phase 2 source is `1380d0b`. Preserve the unrelated `.gitignore` change. No push is authorized. Full phase acceptance requires all gates, including the independently invoked six-file semantic oracle after format conversion.

The [baseline](dependency-convergence-phase3-baseline-2026-10-22.md) and [current baseline receipt](../benchmarks/dependency-convergence-phase3-baseline-2026-10-22.json) pin 1,134 source hashes with zero mismatches and six canonical hashes independently verified against Phase 2. Immutable local before-images are held under `.verification-tmp/phase3-before/`. Earlier Phase 2 test/oracle passes remain prior receipts, not new Phase 3 passes.

## Gates

| Gate | State | Evidence / remaining obligation |
| --- | --- | --- |
| B0 | Accepted locally | Source/hash verification, bounded operation inventory and starting guards in baseline. |
| B1 | Accepted locally (design) | [Concrete contracts](dependency-convergence-phase3-contracts-2026-10-22.md), reviewed against exact DAG; implementation proofs remain pending. |
| H3 | Accepted locally (source cut) | One read-only proof path; final targeted 23 History + source-order regression passed. Full-run identity anomaly is retained below for A3. |
| M3 | Pending | Maintenance authorization. |
| P3 | Pending | Prune and reclaim. |
| W3 | Pending | Rewrite. |
| R3 | Pending | Generic reconciliation. |
| I3 | Pending | Import coordination. |
| S3 | Pending | Measured settlement alternatives and selected design. |
| C3 | Pending | Format/corpus conversion, parity, promotion and deletion. |
| D3 | Pending | Stabilized graph refresh and source enforcement. |
| A3 | Pending | Full integration, corpus and documentation acceptance. |

## Task register

Evidence for the baseline tasks is in the baseline document and its receipt. Later entries are updated only when their source/test/deletion obligations have evidence.

| Task | State | Owner / evidence |
| --- | --- | --- |
| P3.0.01 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.0.02 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.0.03 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.0.04 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.0.05 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.0.06 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.0.07 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.1.01 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.02 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.03 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.04 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.05 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.06 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.07 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.08 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.2.01 | Accepted | Production cuts/frontier separated from test fingerprints |
| P3.2.02 | Accepted | History owner-cut composition over immutable store facts |
| P3.2.03 | Accepted | Lifecycle bounded physical cursor; existing Container parser |
| P3.2.04 | Accepted | Source, Graph, vector, Community and manifest proof modules |
| P3.2.05 | Accepted | Normalized owner-history digest; relocation/membership tests |
| P3.2.06 | Accepted | Ephemeral historical base replay; publication remains outside History |
| P3.2.07 | Accepted | Byte/mtime/clock snapshots and live Freshness record/receipt assertions |
| P3.2.08 | Accepted | Old digest/frontier/encoding files deleted; integrated REL/PHY checks pending |
| P3.3.01 | Pending | Awaiting prerequisite gate |
| P3.3.02 | Pending | Awaiting prerequisite gate |
| P3.3.03 | Pending | Awaiting prerequisite gate |
| P3.3.04 | Pending | Awaiting prerequisite gate |
| P3.3.05 | Pending | Awaiting prerequisite gate |
| P3.3.06 | Pending | Awaiting prerequisite gate |
| P3.3.07 | Pending | Awaiting prerequisite gate |
| P3.3.08 | Pending | Awaiting prerequisite gate |
| P3.4.01 | Pending | Awaiting prerequisite gate |
| P3.4.02 | Pending | Awaiting prerequisite gate |
| P3.4.03 | Pending | Awaiting prerequisite gate |
| P3.4.04 | Pending | Awaiting prerequisite gate |
| P3.4.05 | Pending | Awaiting prerequisite gate |
| P3.4.06 | Pending | Awaiting prerequisite gate |
| P3.4.07 | Pending | Awaiting prerequisite gate |
| P3.4.08 | Pending | Awaiting prerequisite gate |
| P3.5.01 | Pending | Awaiting prerequisite gate |
| P3.5.02 | Pending | Awaiting prerequisite gate |
| P3.5.03 | Pending | Awaiting prerequisite gate |
| P3.5.04 | Pending | Awaiting prerequisite gate |
| P3.5.05 | Pending | Awaiting prerequisite gate |
| P3.5.06 | Pending | Awaiting prerequisite gate |
| P3.5.07 | Pending | Awaiting prerequisite gate |
| P3.5.08 | Pending | Awaiting prerequisite gate |
| P3.6.01 | Pending | Awaiting prerequisite gate |
| P3.6.02 | Pending | Awaiting prerequisite gate |
| P3.6.03 | Pending | Awaiting prerequisite gate |
| P3.6.04 | Pending | Awaiting prerequisite gate |
| P3.6.05 | Pending | Awaiting prerequisite gate |
| P3.6.06 | Pending | Awaiting prerequisite gate |
| P3.6.07 | Pending | Awaiting prerequisite gate |
| P3.6.08 | Pending | Awaiting prerequisite gate |
| P3.7.01 | Pending | Awaiting prerequisite gate |
| P3.7.02 | Pending | Awaiting prerequisite gate |
| P3.7.03 | Pending | Awaiting prerequisite gate |
| P3.7.04 | Pending | Awaiting prerequisite gate |
| P3.7.05 | Pending | Awaiting prerequisite gate |
| P3.7.06 | Pending | Awaiting prerequisite gate |
| P3.7.07 | Pending | Awaiting prerequisite gate |
| P3.7.08 | Pending | Awaiting prerequisite gate |
| P3.8.01 | Pending | Awaiting prerequisite gate |
| P3.8.02 | Pending | Awaiting prerequisite gate |
| P3.8.03 | Pending | Awaiting prerequisite gate |
| P3.8.04 | Pending | Awaiting prerequisite gate |
| P3.8.05 | Pending | Awaiting prerequisite gate |
| P3.8.06 | Pending | Awaiting prerequisite gate |
| P3.8.07 | Pending | Awaiting prerequisite gate |
| P3.8.08 | Pending | Awaiting prerequisite gate |
| P3.9.01 | Pending | Awaiting prerequisite gate |
| P3.9.02 | Pending | Awaiting prerequisite gate |
| P3.9.03 | Pending | Awaiting prerequisite gate |
| P3.9.04 | Pending | Awaiting prerequisite gate |
| P3.9.05 | Pending | Awaiting prerequisite gate |
| P3.9.06 | Pending | Awaiting prerequisite gate |
| P3.9.07 | Pending | Awaiting prerequisite gate |
| P3.9.08 | Pending | Awaiting prerequisite gate |
| P3.9.09 | Pending | Awaiting prerequisite gate |
| P3.9.10 | Pending | Awaiting prerequisite gate |
| P3.9.11 | Pending | Awaiting prerequisite gate |
| P3.10.01 | Pending | Awaiting prerequisite gate |
| P3.10.02 | Pending | Awaiting prerequisite gate |
| P3.10.03 | Pending | Awaiting prerequisite gate |
| P3.10.04 | Pending | Awaiting prerequisite gate |
| P3.10.05 | Pending | Awaiting prerequisite gate |
| P3.10.06 | Pending | Awaiting prerequisite gate |
| P3.10.07 | Pending | Awaiting prerequisite gate |
| P3.10.08 | Pending | Awaiting prerequisite gate |
| P3.11.01 | Pending | Awaiting prerequisite gate |
| P3.11.02 | Pending | Awaiting prerequisite gate |
| P3.11.03 | Pending | Awaiting prerequisite gate |
| P3.11.04 | Pending | Awaiting prerequisite gate |
| P3.11.05 | Pending | Awaiting prerequisite gate |
| P3.11.06 | Pending | Awaiting prerequisite gate |
| P3.11.07 | Pending | Awaiting prerequisite gate |
| P3.11.08 | Pending | Awaiting prerequisite gate |

## History source cut evidence

The source cut moves reusable read-only behavior into `src/history/` and routes physical reads through `src/owner_lifecycle/history_source.rs`. Container remains the framing, extent, clock, path identity and lease authority. Domain stores supply their own local-version-at-global-cut queries and normalized marker encoding. The old production `facade/owner_history_proof.rs`, test frontier implementation and duplicate canonical manifest encoder are removed. Archive/Memory prune-evidence comparison now lives under History; Archive owner proof/publication reads accept immutable Container access through lifecycle. Test-only global-cut adapters and fingerprint/corpus oracles remain test-only.

History contains no Cva, Phylactery, Import coordinator, Container writer, provider invocation, sync or settlement publication. The existing upper adapters collect author/service facts (including External Sources and Dream authority) and map errors at the public boundary. Import owns historical event preparation because it invokes Freshness planners; this preserves the explicit no History-to-Freshness-service and no Import-to-Maintenance rules.

The integrated `history::` filter passed 21 tests before the final retention/error-order corrections (`.verification-tmp/phase3-history-current-tests3.log`), including normalized Graph prune/reopen/reclamation and source snapshots covering vector relocation, malformed ObjectRef rejection, typed mid-frame refusal and unchanged bytes/clocks/mtime. Locked check passed after the immutable Archive retention bridge (`phase3-check8.log`). The actual read-only History/import boundary guard passed after both retention moves (`phase3-history-boundary9.log`); self-tests alone are not treated as source enforcement. Review corrected double-wrapped policy errors and restored per-source lookup/content-validation refusal order. A focused manifest test pins deferred policy errors and descriptor-before-digest precedence.

The broad `historical_` filter was cancelled before completion: byte-by-byte recovery tests were still running, and unrelated sandbox filesystem/process/provider fixtures failed. A configured-runtime historical embedding fixture failed while saving configuration under sandbox permissions and passed when rerun directly with local permissions. No broad historical-suite pass is claimed. The interrupted test executable temporarily prevented Windows relinking; it was stopped before rebuilding the updated source. The first full-library attempt was also interrupted after its historical manifest/event/matrix/base cases passed: it inherited system temporary paths, and configuration/process fixtures failed under sandbox restrictions. The replacement full-library run uses repository-local TEMP/TMP and local subprocess permissions (`phase3-h3-library-local-temp.log`); its result is pending. The latest source includes additional retention moves and error-order fixes. Interrupted runs are not passes. Exhaustive recovery and six-file oracle remain integration-gate work.

The repository-local full library run completed with 1,970 passed, one failed and five ignored. The single failure was `Container(StaleIdentity)` on the second reopen of a checkpoint-prefix fixture (`historical_recovery_tests.rs:88`), not a proof comparison. The guard remains unchanged. The retained failed artifact also opened and reconstructed successfully on a disposable copy (`phase3-checkpoint-probe.log`). The concurrency/OS cause is not established; the failed full invocation is not reported as a pass. A clean final full integration run remains an A3 obligation. The exact isolated checkpoint rerun passed all 1,046 non-vector and 1,484 vector-backed prefixes (`phase3-checkpoint-failure.log`). This does not erase the full-run failure. All 25 public integration suites passed: 163 tests, zero failures, one existing ignored case (`phase3-h3-integration2.log`). The latest source-order regression passed at runtime (`phase3-scoped-order-test.log`); all 23 final History-filter tests passed (`phase3-history-final-tests.log`). All six canonical source hashes still match the baseline (`phase3-h3-canonical-hashes.json`).

## Verification

Current baseline: `cargo check --locked` and `cargo check --locked --tests` passed; physical boundary, owner boundary self-tests and component DAG self-tests passed. Focused frontier baseline passed: 2 tests, zero failures. The CLI suite passed all 32 tests (`phase3-cli-tests.log`); doctests completed with zero examples; full-library and isolated results are recorded above; no full Phase 3 pass or source graph refresh is claimed. Full and changed documentation policy checks and `ddocs check --indexes --links` passed before the latest documentation updates.

## Related docs

- [Specification](dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md)

- [Baseline](dependency-convergence-phase3-baseline-2026-10-22.md)

- [Contracts](dependency-convergence-phase3-contracts-2026-10-22.md)

## Notes

Current format remains CVAFRS06. No semantic feature or format migration is implied by an extracted source owner. Source refresh is reserved for stabilized D3.
