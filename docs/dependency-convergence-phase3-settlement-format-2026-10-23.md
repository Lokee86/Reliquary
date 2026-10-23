# Phase 3 compact semantic settlement format selection

Date: 2026-10-23
Status: Design selected for S3; runtime implementation and C3 acceptance pending.
Parent: [Phase 3 specification](dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md)

## Purpose

Freeze a single current-only Freshness settlement representation that removes copied event-recipient sets and complete derived output postimages while preserving exact accepted numeric authority, attribution, clocks and identity.

## Overview

Select **CVAFRS07: exact semantic scope, retained-journal event references, chronological causal basis and sparse accepted score corrections**. The correction table is part of this one format, not an old-format branch or runtime converter. Native generation normally produces no corrections. The offline development converter preserves previously accepted numeric authority where current reconstruction differs.

The [measurement receipt](../benchmarks/dependency-convergence-phase3-settlement-design-measurements-2026-10-23.json) compares two executable prototypes. The full-authority dictionary control is 1,140,580 payload bytes and retains complete output records. The selected shape is **1,118,112 payload bytes** across three REL settlements (310,786; 693,922; 113,404), including all required score corrections. Both prototypes reconstruct every original CVAFRS06 payload byte exactly; all six frozen input hashes remain unchanged. Python timings and traced allocations are per prototype, not Rust performance claims. The native release regeneration/evidence audit passes in 241.16 seconds.

Current regeneration alone reproduces 14-day record values but changes **2,162** 28-day scores and **three** Ellis scores. Admission/accounting anchors match. Regenerated start-evidence, manifest and input digests also differ. The new representation must preserve the original accepted digests and exact sparse score differences, rather than substitute regenerated authority or pretend that final scores are wholly derivable. All original accepted event leaves independently validate against the pre-settlement Freshness journal.

## Authority and omitted fields

| Persisted evidence | Reason |
| --- | --- |
| Owner UUID, import/start/corpus/manifest/input/policy digests and semantic cuts | Exact accepted identity and public historical coordinates; initial scopes have no later ImportComplete from which all fields can be derived. Preserve original accepted values during conversion. |
| Exact born/affected/cohort/record-key sets | Existing contracts allow proper subsets; do not invent equality or infer a member from a different set. A shared Memory-ID dictionary and shared delta-set table eliminate repeated identity bytes. |
| Exact source Node pairs | Initial/unframed accepted scopes do not enforce equality with the current Archive-derived set; discarded source/vector payload cannot become a new reconstruction dependency. Use one prefix-compressed UTF-8 dictionary, not duplicated inline strings. |
| Accepted event ID, canonical payload digest and accepted turn | Stable exact same-turn attribution. Resolve recipients from the independently retained Freshness journal and compare the canonical event leaf before using it. Missing, changed or ambiguous leaves refuse. No copied per-event recipient sets are persisted. |
| Superseded-event bitmap over the ordered accepted-event table | Retains exact supersession membership; cannot refer to an unaccepted event. |
| Exact pre-scoring record anchors and canonical contribution rows | A self-contained numeric basis across removal of source/vector/graph payload. These are construction-time accepted input facts, never a permanent physical prefix. One shared anchor-value table removes repeated triples. |
| Sparse signed score corrections | Irreducible previously accepted score authority demonstrated by the frozen corpus. Store only nonzero differences against causal recomposition, not complete final FreshnessRecord triples. Corrected scores must remain within -100..100; anchor differences are not silently repaired. |

The decoder reconstructs complete output records with the existing canonical Freshness scorer, settles through the exact settlement cut, then applies the sparse accepted score differences. It never reconstructs a settlement from its own installed output projection. Initial anchor facts and contributions remain sufficient after eligible payload pruning; no cache, endpoint, provider or physical-offset authority is introduced.

Native contribution hashes serve only uniqueness and deterministic same-turn scoring order, and were not persisted as separate accepted journal identities in CVAFRS06. The encoder validates original contribution identity/position consistency before assigning chronological ordinals sorted by `(position, event_id)`. The wire stores positions and those canonical ordinals. Decoding preserves that exact order and rejects duplicate recipient/ordinal pairs. **Actual accepted journal event identities remain exact strings with exact digests and turns**; their identities are never replaced by ordinals.

## Canonical wire grammar

All counts and indexes are minimal unsigned LEB128; signed amounts/corrections use canonical zigzag LEB128. Every count is checked against remaining input before allocation, every index is range checked, and trailing bytes are rejected.

1. Eight-byte `CVAFRS07` magic, owner UUID, the seven existing 32-byte digests, the six existing little-endian u64 cuts, and the existing zero reserved tag.
2. Strictly sorted unique 32-byte Memory-ID dictionary.
3. Strictly sorted unique UTF-8 byte dictionary, each entry represented by the maximal common byte prefix with its predecessor and the exact suffix. Reconstructed entries must be valid UTF-8.
4. Strictly sorted unique ID-set table. Members use increasing global dictionary ordinals encoded as positive deltas from an initial ordinal of -1. Emit born and affected set references, then the exact ordered source-pair text references.
5. Strictly ordered accepted event references: text index, canonical 32-byte event digest, u64 accepted turn. Emit a fixed-size superseded bitmap; unused high bits must be zero.
6. Cohort and record-key set references. Emit the strictly sorted unique exact initial-anchor value table (current record scalar encoding), and one value index per ordered record key.
7. Chronological contribution event table. Encode nondecreasing positions as deltas, then each event's strictly increasing recipient ordinals and nonzero signed amounts. Source contribution IDs are validated before canonical ordinal assignment. Decoder-internal IDs preserve ordinal order for the canonical scorer.
8. Strictly increasing sparse correction recipient ordinals and nonzero signed score differences. Every corrected recipient must be a record key; reject overflow and invalid final score/anchor state.
9. SHA-256 over domain `reliquary-semantic-import-settlement-v7` followed by every preceding wire byte. The exact raw payload is additionally bound by the existing ImportComplete payload digest.

The measured prototype uses a distinct experimental magic/domain of equal wire length; C3 uses only the normative magic/domain above. Runtime decoding exposes no partially resolved semantic settlement: Freshness owns journal lookup, leaf checking and numeric recomposition. History's completion scan may consume immutable binding facts from a structurally validated frame; public owner exposure still follows complete ordered owner validation. There is no upward callback or Import-to-Maintenance edge.

## Validation and transformation contract

Preserve all existing semantic scope, owner/policy, birth/cohort, source, readiness, live/import fence, start/receipt and completion-binding checks. Add fail-closed canonical dictionary/index/rank validation, duplicate contribution/correction rejection, missing/changed retained-event rejection, and deterministic recomposition checks. Keep the pure scorer canonical below the service rather than duplicate its implementation in the codec; the public scorer API remains a thin service adapter if ownership needs adjustment.

The exact base values, row order and score corrections are field-audited against original accepted payloads. Prune/reclaim/repack may relocate or remove eligible unrelated payload without invalidating these numeric facts or the exact independently retained event references. Rewrite remains its existing generation boundary; it does not gain a second import authority.

## Development migration and deletion

Runtime supports only CVAFRS07 at the C3 integration cut. A temporary offline converter reads the hash-pinned CVAFRS06 before-images and the verified numeric-basis measurement, preserves every original metadata/scope field, and emits the new single format. It must compare exact recomposed records and semantic scope before producing a candidate.

The [canonical-tail audit](../benchmarks/dependency-convergence-phase3-conversion-tail-audit-2026-10-23.json) permits a bounded converter only for the three exact canonical REL shapes: a single settlement followed solely by 40-byte `CVACPRT1` ID retirement carriers. Preserve all pre-settlement bytes and copy retirement carriers exactly; reject every other tail shape. PHY inputs remain exact. No general relocation shortcut is authorized.

Convert disposable copies, run all six frozen semantic/clock/source-byte/mtime oracles, then promote the verified development corpus in an auditable checkpoint. Delete the old runtime reader/writer, temporary converter, prototype encoders and old normalization helpers after promotion; retain measurement receipts and frozen originals. Do not refresh the graph before the single stabilized D3 checkpoint.

## Related docs

- [Execution ledger](dependency-convergence-phase3-execution-2026-10-22.md)
- [Interface contracts](dependency-convergence-phase3-contracts-2026-10-22.md)
- [Settlement minimization requirements](semantic-import-settlement-minimization-plan-2026-10-21.md)
- [Selected-shape measurements](../benchmarks/dependency-convergence-phase3-settlement-design-measurements-2026-10-23.json)
- [Dictionary control](../benchmarks/dependency-convergence-phase3-settlement-dictionary-control-2026-10-23.json)

## Notes

Root design review selects this representation against the unchanged component DAG and exact frozen corpus evidence. This is S3 design approval, not runtime, conversion, D3 or A3 acceptance. Full Rust canonical/corruption tests, transformed-history parity, actual runtime byte accounting and the six-file converted-corpus oracle remain mandatory C3 obligations.
