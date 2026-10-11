# Freshness semantic calibration harness

Parent: [Documentation index](INDEX.md).

## Purpose

Record the Step 3 reproducible oracle harness for calibrating live semantic Freshness threshold and similarity-to-bonus candidates before Step 4 selects production constants.

## Overview

**Date:** 2026-10-11. **Status:** Step 3 complete; no production threshold, curve, candidate limit or policy transition has been selected.

Run the deterministic synthetic fixture:

```text
python scripts/freshness_semantic_calibrate.py
```

Run a retained corpus exported as JSONL:

```text
python scripts/freshness_semantic_calibrate.py --input path/to/corpus.jsonl --output calibration.json --thresholds 0.70,0.75,0.80 --curves linear,steep --limit 8
```

Each JSONL row contains one source vector and target objects with `id`, `vector`, and optional boolean `label` for a known positive match. The harness performs finite dimension/norm validation, cosine scoring, thresholding, deterministic nearest ordering, candidate limiting, and per-turn bonus trajectories. It reports compatible-vector coverage, mean/max nearest-neighbor counts, selected candidates, false-match rate, labeled recall, and the resulting local bonus sequence. Invalid rows do not consume the usable candidate limit.

The fixture has no model-generated inference and is stable across runs. Corpus inputs are read-only evidence. A retained corpus may omit labels; in that case false-match rate and recall describe only the supplied labels and do not claim semantic truth. The harness measures vector work and score behavior; it does not write Freshness state, create events, run graph propagation, or change REL/PHY inference.

The curves are deliberately parameterized:

- `linear` uses a monotone floor-scaled curve.
- `steep` uses the same threshold with a larger scale to expose sensitivity.
- Thresholds, scale families and candidate limits are command-line inputs.

The report schema is versioned at `1`, sorted deterministically, and suitable for checking into a dated Step 4 calibration record. Production constants remain undecided until the Sol review in Step 4.

## Related docs

- [Live semantic Freshness policy/API contract](freshness-live-semantic-policy-contract.md)
- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md)
- [Memory Freshness policy](memory-staleness-plan.md)

## Notes

Step 3 verification on 2026-10-11: the default synthetic run completed successfully and emitted six deterministic configurations (thresholds 0.70/0.75/0.80 crossed with linear/steep curves). The fixture reports 5/7 finite compatible vectors, candidate counts, false-match rates, labeled recall and score trajectories; the 0.75 run produced 0.0 false-match rate and 1.0 labeled recall on the supplied labels. These are fixture observations, not production recommendations. Both documentation-policy checks and `git diff --check` passed.

The harness is intentionally standard-library-only Python so calibration can run without embedding providers or new dependencies. It is an oracle for threshold/curve and bounded-search comparisons, not the Step 5 production policy module. Step 4 must review retained-corpus coverage, false-match neighborhoods, candidate cost and score trajectories before selecting constants.
