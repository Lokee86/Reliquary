# Live semantic Freshness calibration and v1 policy

Parent: [Documentation index](INDEX.md).

## Purpose

Freeze Step 4 numerical defaults and policy transitions from reproducible offline evidence, with explicit limits on what that evidence establishes.

## Overview

**Date:** 2026-10-11. **Status:** v1 numerical constants are frozen; the former opt-in activation transition is superseded by intrinsic F8. [Step 12 verification](freshness-semantic-release-verification-2026-10-12.md) retains the original mechanics evidence. Missing labelled relevance data affects policy tuning, not whether F8 runs.

### Selected constants

| Parameter | v1 decision |
| --- | --- |
| Semantic policy version | 1, distinct from the existing base Freshness policy version. |
| Numerical / eligibility revisions | 1 / 1. Eligibility remains the Step 2 contract. |
| Inclusive cosine threshold | 0.75. |
| Local bonus | Below threshold: 0. At/above threshold: `min(10, 1 + floor(36 * (cosine - 0.75)))`. |
| Maximum per Memory per accepted turn | +10, using the strongest match across all genuine source fragments. |
| Usable candidate limit | 64 distinct eligible above-threshold Memories across the entire turn. |
| Total effect bound | At most 64 local effects and at most 640 summed bonus points per turn; no redistribution or extra aggregate scaling. |
| Selection | Highest cosine first; MemoryId byte order for ties. Invalid/ineligible targets are removed before the usable cap. |
| Production activation | Intrinsic REL behavior; no enable/disable control. Policy constants remain versioned/tunable. |

Reference points: 0.749999 -> 0, 0.75 -> 1, 0.80 -> 2, 0.90 -> 6, 1.00 -> 10. This is attention credit, independent of +25 accepted use and +50 first-cycle Dream. It remains local and non-propagating.

### Numerical revision 1 and policy identity

For valid finite f32 input vectors, convert components to binary64. Compute squared norm in ascending dimension order with separate multiplication/addition, take its square root, divide each component by that norm, then sum component products in ascending dimension order. Reject dimension mismatch, zero/nonfinite norms and nonfinite components/results; clamp finite cosine roundoff to [-1,1]. No parallel reduction, fused multiply-add, fast-math reassociation or threshold epsilon is permitted. Bonus multiplication, subtraction and floor use binary64 in the written order; integer clamp happens last. Worker partitioning may change work scheduling but cannot change scalar reduction order.

The canonical policy descriptor is exactly this UTF-8 JSON, with no whitespace/newline:

```json
{"bonus_cap":10,"candidate_limit":64,"curve":"inclusive-linear-v1","eligibility_revision":1,"numerical_revision":1,"threshold":0.75,"version":1}
```

Policy digest is SHA-256 of ASCII `reliquary:semantic-freshness-policy`, one zero byte, then the descriptor bytes:

```text
5886dc23e386664b3dbef03a116dbfab5567ad2998be0e21e07c4895ab1d261f
```

Production v1 accepts only that descriptor; arbitrary experiment parameters are confined to the offline harness. Any change to threshold, bonus, cap, candidate limit, numerical rules or eligibility requires a new semantic policy version. Receipts also pin the actual base Freshness policy configuration, not merely its format-version number, because the existing base policy supports coherent nondefault values.

### Evidence and rationale

Machine-readable evidence: [calibration matrix](../benchmarks/freshness-semantic-calibration-2026-10-11.json). It contains 27 configurations per input: three thresholds (0.70/0.75/0.80), three curves (linear cap 10, steeper linear cap 25, fixed 5), and limits 16/32/64. All original RELs were hash-verified before copying and after exporting. Only writable temporary copies were opened; no embeddings, Insomnia, Dream or new REL/PHY inference ran.

The original Step 3 harness had four material gaps: zero credit at an inclusive threshold, missing labels treated as false, globally rather than turn-scoped label lookup, and bonus lists mislabeled as score trajectories. This step corrects them, adds multi-fragment/eligibility fixtures, independent per-turn decay checks, actual operation counts/timing, and retained-vector export. Earlier schema-1 figures are superseded by schema 2.

| Retained fixture | Lifecycle-eligible current bodies / compatible bindings | Query evidence | Cosine evaluations | Max qualifying neighbors at 0.70 / 0.75 / 0.80 |
| --- | ---: | --- | ---: | --- |
| ChatGPT first 14 days | 981 / 981 | 16 deterministic Memory-vector surrogate queries; self excluded | 15,680 | 3 / 2 / 1 |
| ChatGPT first 28 days | 2,164 / 2,164 | 16 deterministic Memory-vector surrogate queries; self excluded | 34,608 | 8 / 3 / 1 |
| Ellis pre-relationship | 656 / 656 | 64 deterministic samples from 498 retained mixed-turn fragment vectors | 41,984 | 2 / 1 / 0 |

Both ChatGPT fixtures have no active Archive vector generation. They share compatibility profile `69aa8cff0441f35faa3494898b14d406620864bbd6da5e905ccb30cb58b25e71`, 1,024 dimensions. Ellis uses `2f404c70725ad1162653bf2762c0f5590d0c3f7a1b83c4b9bf87e39091138fa5`, also 1,024 dimensions, with retained Archive evidence. Profile equality is enforced within each population; vectors from the two profiles are never compared. Binding coverage is 100% of the selected active lifecycle population, not 100% of all Memories or accepted turns. Birth/admission/self-source timing is unavailable in this old evidence and does not become invented live eligibility.

On the controlled labeled boundary fixture, 0.70 selected 22.22% false labeled matches at full labeled recall; 0.75 selected 0% false labeled matches at full labeled recall; 0.80 reduced labeled recall to 71.43%. Labels were intentionally constructed for contract boundaries, so these numbers prove oracle behavior rather than real-world threshold accuracy. Retained inputs have no relevance labels: false-match rates and recall are **unknown**, represented by null, never reported as zero.

**Threshold rationale:** retain the accepted 0.75 candidate as the initial explicitly activated implementation policy. 0.70 increases weak neighborhoods, while 0.80 discards the controlled boundary positives and all sampled Ellis hits. The retained measurements show modest neighborhood breadth but cannot establish true semantic precision. This is a conservative engineering selection, not proof of an optimal corpus threshold.

**Curve rationale:** choose +1..+10 rather than the tested +1..+25 or fixed +5. A strong match compensates at most 100 accepted turns of default decay; a boundary match compensates 10. One semantic hit from -100 cannot jump directly out of Dormant. Repeated high-similarity attention every 100 turns can sustain relevance; rare attention every 1,000 turns cannot. A threshold match every 100 turns still declines by nine points per interval. Fixed +5 discards similarity strength; cap 25 can compensate 250 turns from a vector hit despite the weaker evidence than consumed use. These are policy tradeoffs visible in the controlled trajectories, not relevance labels.

**Candidate rationale:** limits 16/32/64 retain identical results on all retained samples because neighborhoods are small. Choose 64 for headroom across multiple fragments while bounding receipt effects and temporary selection memory. The 512-neighbor synthetic stress fixture proves deliberate truncation: only the strongest 64 distinct targets receive credit, and invalid/future/self/foreign rows consume no usable budget. 64 is a bounded operating choice; this small sample does not prove it never truncates a legitimate live neighborhood.

Measured Python rank-only times were 6.603 s / 14.767 s / 21.196 s for the three retained inputs, approximately 2,200 cosine evaluations per second on this workstation. They include repeated norm calculation in the reference oracle and are single diagnostic samples, not optimized Rust production latency or percentiles. The offline oracle intentionally examines every selected source/target pair. Step 7 must use bounded selection/search with complete eligible ranking; the cap bounds output and selection scratch, not total vector work. No lossy community-routing shortcut is approved by this calibration.

### Historical activation record, retry and policy transitions

1. Current RELs do not enable F8: it is intrinsic. Fresh RELs use activity floor 0; an accepted historical checkpoint supplies the lower bound for already-reconstructed history. Older development RELs may contain a `CVAFSA01` prospective watermark, which is read only as an additional lower bound and is no longer configurable.
2. The legacy activation/cancellation transition existed only in the pre-release implementation. Current code exposes no activation mutation and does not create disabled intervals. Existing base Freshness enrollment is preserved and its base policy must not be reset.
3. Profile rotation changes only evidence for newly accepted eligible turns. Accepted source identities retain their original receipt/profile/effects. A retry resolves that receipt before searching or consulting current profile defaults, and never creates another credit.
4. There is no disable/re-enable configuration. Pausing a worker affects scheduling only; it does not alter F8 eligibility, and pending eligible fragments remain retryable under the same activity/checkpoint floor.
5. v1 implements no in-place upgrade of an enrolled semantic owner. A different version/descriptor after semantic completion fails explicitly. A future v2 needs its own transition design; it cannot replay old turns under new constants. Offline calibration configurations never enter production receipt identity.
6. Reconciliation preserves/rebases the activation boundary by exact Archive identity and preserves pinned local deltas. Different enrolled semantic descriptors or incompatible activation authority are explicit conflicts. A branch without semantic enrollment may contribute ordinary history, but cannot fabricate missing semantic receipts; Step 11 must prove that supported case or reject it.
7. Step 12 may release the implemented lane as opt-in after correctness/recovery/performance verification. Automatic activation or claims of calibrated real relevance additionally require labeled genuine single-turn fragment/Memory evidence for each intended embedding profile. Existing surrogate and mixed-turn samples do not satisfy that gate.

Semantic policy version/profile/cut never enters the Step 2 stable accepted-turn event ID. No transition grants two semantic credits to the same accepted source.

## Related docs

- [Live semantic policy/API contract](freshness-live-semantic-policy-contract.md)
- [Calibration harness](freshness-semantic-calibration-harness-2026-10-11.md)
- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md)
- [Freshness completion plan](freshness-completion-execution-plan-2026-10-03.md)

## Notes

### Reproduction

Build the exporter using the same isolated target as the baseline:

```text
CARGO_TARGET_DIR=C:\Users\archa\AppData\Local\Temp\reliquary-r3-verification
CARGO_PROFILE_TEST_DEBUG=0
cargo build --locked --profile test --example freshness_semantic_export -j 2
```

These environment assignments describe the environment passed to the command, rather than shell-specific Windows assignment syntax. Execute the built exporter separately for each row below, supplying the fixture path, expected SHA-256 and an output JSON path. The exporter always opens a hash-verified temporary copy and checks source preservation; it fails on any mismatch. It clears Windows read-only attributes only on that temporary copy.

| Fixture under `C:\!bin\workspace\reliquary-fixtures\local\authoritative\` | SHA-256 |
| --- | --- |
| `chatgpt-first14d\project.prj.rel` | `49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b` |
| `chatgpt-first28d\project.prj.rel` | `8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4` |
| `rhelm-david-r-ellis-pre-relationship\project.prj.rel` | `3efd9c666aa992686abd8aac9b77cfc7dc2a0e15d6fb26ec01f8666db175e476` |

Then regenerate the complete comparison matrix:

```text
python scripts/freshness_semantic_evidence.py --inventory <14d-export.json> --inventory <28d-export.json> --inventory <ellis-export.json> --output benchmarks/freshness-semantic-calibration-2026-10-11.json
python -m unittest discover -s scripts -p test_freshness_semantic_calibrate.py
```

The raw exported vectors remain temporary local data. The committed aggregate contains hashes, counts, metrics and controlled trajectories; it contains no transcripts, embeddings or Memory IDs. Timings vary; configuration results and input digests are reproducible. Sampling is uniform rank over stable byte-sorted IDs, 16 surrogate queries or 64 fragment queries per profile, not randomized or selected for favorable results.

Verification: eleven Python oracle tests passed; all three source-preserving retained exports and the full four-input matrix completed; the exporter built with the lockfile and bounded two-job compilation; whole-root `cargo fmt --check` passed. The focused core command `cargo test --locked --lib freshness:: -j 2 -- --nocapture` passed 28 tests; both documentation-policy checks and `git diff --check` passed. The committed matrix has SHA-256 `0ac481dc477ee2c3f3d021232793a7622c26ad11d8996f07650045c3586a919a`, and its input/code hashes and all 108 configurations were validated. Existing source dead-code/unused-import and Windows linker warnings remain. No production source/public API/storage codec changed.

Documentation impact: inspected ADR 0039, the completion/linear plans, policy contract, score engine and retained-vector APIs. Updated the calibration harness description, policy contract links, linear status and index; added this policy record and machine evidence. Current API/storage/roadmap claims remain unchanged because F8 is unimplemented. Known gap: labeled genuine-turn coverage and optimized live latency remain release/activation gates.

Architecture impact: standards unchanged; Freshness retains scoring/event authority, Archive/vector owners retain evidence, and the container remains physical mechanics. Pitlord ownership rules unchanged; the added exporter is offline diagnostic code using public APIs. No runtime semantic owner changed.
