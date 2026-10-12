# Historical one-shot import coordinator verification — 2026-10-12

## Purpose

Record Step 24 evidence for the one-shot historical import coordinator.

## Overview

`execute_historical_import` composes the already-frozen historical inputs into one deterministic projection:

1. validate the import identity, manifest, timeline, and provenance barrier;
2. convert semantic and first-cycle Dream plans into import-only propagation events;
3. propagate across the authorized graph with the existing bounded worker path;
4. reduce recipient contributions through the sparse chronological scorer; and
5. publish one complete `HistoricalCheckpoint` through the atomic checkpoint store.

The coordinator retains no staged event queue. Repeating the same import identity is handled by the checkpoint store's idempotent publication gate. Semantic/Dream inference remains an input boundary, so the coordinator does not rerun Insomnia or Dream.

Embedding fallback is explicit rather than implicit: callers may supply a bounded, precomputed semantic plan from an opt-in embedding-only operation; the coordinator consumes that plan and its coverage evidence, and never silently launches an unbounded embedding pass.

## Verification

Focused command:

```text
cargo test --locked --lib historical_import
```

Result on 2026-10-12: 5 passed, 0 failed. This includes the coordinator's complete-projection and repeated-import tests plus the historical import recovery fixtures.

The coordinator tests prove:

- a complete import publishes one checkpoint with semantic/Dream counts and propagated output;
- the checkpoint contains the scored records and applied contribution identities;
- re-running the same import identity returns the existing checkpoint without changing its identity; and
- no permanent event queue is required between planning and publication.

## Related docs

- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Historical checkpoint recovery and idempotence verification](freshness-historical-checkpoint-recovery-verification-2026-10-12.md)
- [Historical sparse scorer verification](freshness-historical-scorer-verification-2026-10-12.md)
- [Historical import reconstruction specification](freshness-historical-import-reconstruction-spec-2026-10-03.md)

## Notes

This record covers the coordinator seam and its focused tests. Full retained-corpus measurement and the combined H1–H8 acceptance gate remain Step 25 work. The normal live semantic path remains local and non-propagating.
