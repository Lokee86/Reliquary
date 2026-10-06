# Freshness R1 verification — 2026-10-03

## Purpose

Record the directly verified buildable checkpoint and exact remaining focused failure. This is R1 evidence, not completion of R2–R7.

## Overview

Repository: `C:\!bin\workspace\Reliquary`.
Branch: `feat/deterministic-memory-freshness`.
Verification window: 2026-10-03 14:48–14:59 UTC.

The working tree had advanced since the earlier pause. Known fixture and compile repairs were already present when this verification began; they were inspected rather than overwritten. No sub-agents were started by this execution, and no implementation source was changed.

| Check | Collected result |
| --- | --- |
| `cargo check --locked --tests --examples` | Passed, job `job_1clFl3sbKtPA08kk9MjFymVv` |
| `cargo test --locked --lib freshness`, isolated `CARGO_TARGET_DIR=target/r1-verification` | 23 passed, 0 failed, job `job_i9uA4n6MG8bE8QOIZlxp3HL5` |
| Five Freshness integration suites, existing concurrent run inspected directly | 13 passed, 1 failed, job `job_fkfc95BcAYq3Vjq29eVOvav2` |

The integration command selected `freshness_owner_contract`, `freshness_recovery_contract`, `freshness_reconcile_contract`, `freshness_f7_acceptance`, and `freshness_f7_recovery`. Results were respectively 8/8, 0/1, 2/2, 2/2 and 1/1.

A concurrent shared-target library test failed to locate dependency artifacts. The isolated run above rebuilt successfully and passed; that environmental failure is not a Freshness behavior failure.

## Remaining focused failure

Test: `dream_duplicate_enrichment_reinforces_existing_canonical_only` in `tests/freshness_recovery_contract.rs`.

Trigger: publish an extracted Memory, then publish revision 2 with the same immutable title/content and a new mutation identity.

Observed error, before Dream processing:

```text
Freshness error: publication intent does not match first accepted Memory identity
```

Source review: `Cva::publish_memory` treats every `PublishPreflight::New { record, .. }` result as `first_creation = true`. That variant also represents an accepted new metadata revision of an existing Memory. The facade therefore creates a new birth intent using the revision-2 mutation ID; initialization compares it against the first accepted mutation ID and rejects it.

Required correction under R2/R3: distinguish a first Memory revision from a new mutation/revision of an existing Memory before creating birth intent. Preserve the original birth proof and admission anchor; do not relax first-publication identity validation to hide the error. Retain this test as the regression gate.

## Related docs

- [Remaining-work specification](freshness-remaining-work-2026-10-03.md)
- [Main implementation specification](generic-staleness-implementation-plan.md)
- [Behavioral contracts](behavioral-contracts.md)

## Notes

R1's exit gate is satisfied: library/tests/examples compile and focused failures are identified. The checkpoint is not fully green or production-ready.

Documentation impact:
- Inspected: R1 handoff, current producer/facade/storage source and focused fixtures/results.
- Updated: this verification record and its documentation-index entry.
- Not affected: numerical policy and implementation source.
- Compliance check: structural and change-impact checks passed (jobs `job_t2Ag37RRxv9xVtWLHC70lcSG` and `job_BDdGIilzeEMdREMJhGx5Yq7R`).
- Known documentation gaps: earlier “all focused tests passed” evidence describes an older tree; this dated record describes the current observed result.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: none from this verification.
- Pitlord enforcement impact: no policy changes; full enforcement remains R7.
- Other verification impact: fresh combined compile and focused library results above.
- Known architectural gaps: R2–R7 remain outside this checkpoint.
