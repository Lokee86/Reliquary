# Historical import model composition evidence — 2026-10-12

## Purpose

Record partial Step 24 planning-model evidence. The [Step 26 audit](freshness-combined-production-review-2026-10-12.md) withdraws integrated coordinator completion.

## Overview

`execute_historical_import` composes caller-supplied inputs into an experimental in-memory model; it never freezes or commits a REL:

1. check that IDs/digests are nonzero and timeline/provenance collections are nonempty (not canonical identity/manifest/owner-proof validation);
2. convert semantic and first-cycle Dream plans into import-only propagation events;
3. propagate across the authorized graph with the existing bounded worker path;
4. reduce recipient contributions through the sparse chronological scorer; and
5. assign an in-memory `HistoricalCheckpointStore` snapshot without Container persistence or importer/runtime integration.

The model retains no staged event queue. An exact retry before model live mutation reuses its snapshot; this does not establish native import retry/recovery or a generation barrier. Semantic/Dream plans are supplied inputs, so no Insomnia or Dream call occurs.

No embedding-only operation is implemented. Supplying a precomputed plan does not establish provider limits, opt-in, cancellation, coverage validation or an actual fallback coordinator. Full source/graph/Community/vector/body/cohort proof validation, original Archive-clock checking, source/cohort exclusions and canonical H1 event IDs remain required.

## Verification

Focused command:

```text
cargo test --locked --lib historical_import
```

Result on 2026-10-12: 5 passed, 0 failed. The filter includes two empty-event model tests and three separate native Cva import fixtures; none connects the model coordinator to the native commit path.

Two empty-event model tests establish only:

- a supplied empty-event model input returns a snapshot (zero semantic/Dream/propagated events);
- the model snapshot contains scored records and an empty contribution-ID set;
- retrying before any model live mutation reuses the snapshot; and
- no permanent event queue is required between planning and publication.

## Related docs

- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Historical checkpoint recovery and idempotence verification](freshness-historical-checkpoint-recovery-verification-2026-10-12.md)
- [Historical sparse scorer verification](freshness-historical-scorer-verification-2026-10-12.md)
- [Historical import reconstruction specification](freshness-historical-import-reconstruction-spec-2026-10-03.md)

## Notes

This record covers model composition only. Actual native coordination remains Step 24; integrated H1–H8 acceptance and retained-corpus measurement remain Step 25. The normal live semantic path remains local and non-propagating.
