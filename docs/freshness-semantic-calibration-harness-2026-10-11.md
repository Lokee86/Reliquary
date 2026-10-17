# Freshness semantic calibration harness

Parent: [Documentation index](INDEX.md).

## Purpose

Provide a reproducible offline oracle for threshold/curve/candidate-limit comparisons before implementing live semantic Freshness.

## Overview

**Date:** 2026-10-11. **Status:** Step 3 harness corrected and exercised against retained corpora during Step 4. The [v1 calibration and policy record](freshness-semantic-calibration-2026-10-11.md) owns selected constants and evidence limits.

```text
python scripts/freshness_semantic_calibrate.py
python scripts/freshness_semantic_calibrate.py --input corpus.jsonl --thresholds 0.70,0.75,0.80 --curves linear,steep,fixed --limits 16,32,64 --output calibration.json
python -m unittest discover -s scripts -p test_freshness_semantic_calibrate.py
```

JSONL rows contain one `source` vector or multiple `sources` vectors for a turn, and `targets` with `id`, `vector`, and optional boolean `label`. Labels are turn-scoped; omitted labels are unknown. Eligibility fixture flags cover wrong owner, self source, future target, archived, unadmitted, incompatible profile/generation and obsolete body. Invalid/ineligible rows are removed before the usable cap. Multiple fragments reduce to the strongest Memory match. Equal target scores sort by stable ID. The standard-library heap selection uses O(candidate-limit) selection scratch; the offline oracle separately retains the complete scored population for comparisons.

The schema-2 retained-inventory form is emitted by `examples/freshness_semantic_export.rs`, using only a verified writable copy of each authoritative REL. Current active body/profile bindings supply target vectors. Archive vectors are sampled when present; otherwise explicitly labeled Memory-vector surrogate queries exclude their own target IDs. These are prospective neighborhood/cost inputs, not accepted live turn events. Reproduction and source hashes appear in the policy record.

The three experimental curves have qualifying minimum +1, maximum +10/+25/+5 respectively, inclusive threshold, and explicit floor rounding. Below threshold is zero. Schema 2 corrects the earlier schema-1 zero-at-threshold curve, unknown-label false positives, cross-turn label lookup and misleading bonus-list trajectories.

Reports include above-threshold neighbor counts, labeled-only false-match/recall (null when unavailable), input compatibility coverage, exact cosine work, filtering/invalid counts, cap truncation and controlled chronological score trajectories. Scores use -100/+100 event caps, one-point-per-ten-turn lazy decay and remainder conservation, independently checked against a per-turn oracle. Retained sample ordering is a counterfactual diagnostic timeline, not reconstructed historical activity. Optional `--timing` measures Python rank-only cost and is excluded from deterministic oracle comparisons.

`scripts/freshness_semantic_evidence.py` combines synthetic plus supplied retained inventories into the committed comparison matrix. Each input is scored once, then the 27 policy/limit combinations reuse its scores. The offline oracle may scan all source/target pairs; it does not approve that cost as a live search implementation. Step 7 must provide the bounded production adapter and Step 12 must measure it.

## Related docs

- [v1 calibration and policy record](freshness-semantic-calibration-2026-10-11.md)
- [Live semantic policy/API contract](freshness-live-semantic-policy-contract.md)
- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md)

## Notes

All calibration uses existing vectors or deterministic synthetic samples; no model/provider/inference dependency is invoked. The harness writes comparison reports only, never Freshness receipts, semantic revisions or graph propagation. Eleven focused oracle checks pass. Real relevance labels and genuine single-turn source coverage remain unavailable; the selected implementation policy stays opt-in under its release gate.
