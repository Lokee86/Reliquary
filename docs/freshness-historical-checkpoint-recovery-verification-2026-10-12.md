# Historical checkpoint recovery and idempotence verification — 2026-10-12

## Purpose

Record Step 22 verification for crash-prefix recovery, reopen-derived indexes, repeated historical imports, and post-checkpoint accepted-use replay.

## Overview

The historical checkpoint store now publishes only complete candidate projections, preserves the prior authoritative projection when a staged candidate is invalid, and treats an identical import retry as a no-op. Same-import divergent manifests fail closed. Reopening can rebuild the sparse Freshness due index from committed records, and legitimate post-checkpoint events are applied exactly once by stable event identity.

This is the recovery primitive for the Step 21 checkpoint coordinator; it does not create a second durable event ledger or change the live semantic non-propagation invariant.

## Related docs

- [Freshness semantic + historical linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Historical sparse scorer verification](freshness-historical-scorer-verification-2026-10-12.md)
- [Historical Dream verification](freshness-historical-dream-verification-2026-10-12.md)
- [Historical propagation verification](freshness-historical-propagation-verification-2026-10-12.md)

## Notes

Focused verification:

- cargo test --locked --lib historical_checkpoint
- cargo test --locked --lib historical
- cargo fmt -- --check
- git diff --check
- Documentation policy checks for the repository

The recovery fixtures cover invalid crash-prefix discard, repeated-import no-op, conflicting retry rejection, due-index rebuild, and exactly-once post-checkpoint event application.
