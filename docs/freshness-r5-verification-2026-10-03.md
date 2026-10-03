# Freshness R5 verification — 2026-10-03

Parent: [Documentation index](INDEX.md).

## Purpose

Record physical and divergent-history preservation evidence.

## Overview

Public transforms preserve immutable accepted effects, original provenance and pinned policy. Divergent destination turns are derived from exact Archive source identity, and effect replay does not rerun propagation over the merged graph.

## Collected evidence

Final root invocation `cargo test --locked -j 2` in job `job_KEDYwixNQjbm3N-JxjBYGT8u` passed 814 unit tests and all 54 integration tests, zero failed/ignored. Test-profile build used Rust 1.97.1, `CARGO_PROFILE_TEST_DEBUG=0`, two build jobs and isolated target `C:\Users\archa\AppData\Local\Temp\reliquary-r3-verification`. Earlier focused owner/recovery jobs also passed; the full final suite is acceptance evidence.

## Verified matrix

| Operation | Exercised contract |
| --- | --- |
| Identical-history strict reconciliation | Current/historical scores, policy, receipt retry, second reopen and unchanged source bytes |
| Principal repack and reclamation | The same public preservation matrix via `freshness_survives_public_copy_repack_and_reclamation` |
| Divergent reconciliation | Common/disjoint receipts, exact turn rebasing, source proof preservation, repeated divergence/second merge and no second credit in `freshness_reconcile_contract` |
| Grouped completion replay | Birth cohort/accepted anchor through existing grouped reconciliation tests |
| Legacy migration | Existing Archive preservation/migration tests; legacy rows do not acquire fabricated Freshness births |
| Current identified owner migration | Explicitly rejected without output; it does not need legacy migration |
| Unsupported pending Dream/publication work | Refused before output; source bytes unchanged |
| Corrupt or incompatible owner/provenance | Existing R2 owner/proof negatives and transform validation fail closed |

Settled Dream IDs and original admission/birth flags are replayed separately from numerical effects. Missing or incompatible historical Community evidence is not replaced by a current snapshot. The full root invocation is the final integrated gate; exact count/job/source identity are recorded in the core report.

## Limits

Pending first-pass recovery transfer remains unsupported. Safe refusal is the accepted contract, not a claim that arbitrary in-flight work can be transported. No history retention/compaction format or legacy event migration was invented.

## Related docs

- [Completion execution plan](freshness-completion-execution-plan-2026-10-03.md)
- [Core verification](freshness-core-release-verification-2026-10-03.md)
- [Development](development.md)

## Notes

F8/S1–S6 are excluded by the user's execution scope. Measurements are diagnostic, not release-build latency guarantees. Exact frozen source identity and final repository gates are recorded in the core report.
