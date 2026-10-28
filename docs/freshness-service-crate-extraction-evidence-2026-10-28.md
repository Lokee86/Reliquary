# Freshness service extraction evidence

## Purpose

Record the source cutover and verification status for the [extraction specification](freshness-service-crate-extraction-spec-2026-10-28.md).

## Overview

The source cutover is integrated. `reliquary-freshness` owns deterministic propagation, semantic and historical evaluation, boundary indexing and due dispatch. The product composes authorized owner snapshots and publishes effects; `reliquary-freshness-store` remains the durable score and receipt owner. The service depends directly on Freshness Store, Memory Core, Graph and `sha2`.

## Verification

- The pre-move independent policy oracle passed 17 tests; see the [baseline](freshness-service-crate-extraction-baseline-2026-10-28.md).
- The locked offline service suite passed 69 tests, and the locked offline CLI suite passed 32 tests with repository-local temporary files.
- Locked offline workspace all-targets compilation, formatting, and Archivist index/link checks passed.
- The locked offline Freshness Store suite passed 46 tests; the product Freshness filter passed 43. With local-server access and four long fixtures excluded, semantic passed 65 (one ignored), historical 140 (one ignored), Dream 175, and agentic 80 (one ignored). The excluded fixtures are `owner_reopen_keeps_bounded_receipts_and_exact_evicted_retry`, `historical_native_checkpoint_every_container_byte_prefix_reopens_and_retries`, `historical_native_due_reopen_and_every_live_use_prefix_preserve_all_recipients`, and `historical_matrix_semantic_live_local_vs_native_propagation_and_stale_community`. A subsequent full product run with local-server access passed all 1,478 non-ignored tests, including those long fixtures; five fixture-dependent tests were ignored.
- Documentation policy checks report existing missing-section findings. A clean archive of HEAD `0cfba254` produced 29 findings; the same archive with the extraction documentation overlaid produced 28, with no new findings. After HEAD advanced to `87affc63`, the changed-from check reported 29 findings, including a pre-existing finding in the newly committed Vectors specification. Pitlord documentation enforcement failed loading an Arcana manifest with a wrong field count. The architecture refresh was retried, but its Rust adapter remained active without producing a result for over 12 minutes and was interrupted; architecture enforcement remains unverified.

## Related docs

- [Freshness subsystem](subsystems/freshness.md)
- [Freshness Store extraction evidence](freshness-store-crate-extraction-evidence-2026-10-26.md)

## Notes

This record does not claim CVAFRS07 recovery or six-file corpus admission. Those remain separate acceptance gates.
