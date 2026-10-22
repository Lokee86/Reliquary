# REL activity clock

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe the implemented owner-local accepted-conversation activity projection used by Freshness and other deterministic scheduling logic.

## Overview

Archive is the sole authority for a REL's logical accepted-turn activity position. The projection counts first-accepted logical user/assistant turns across both Archive publication paths and reconstructs from durable Archive history on reopen.

It is an activity projection, not a second persistent clock format.

## Semantics

A new REL begins at position zero. Each unique eligible accepted user/assistant turn advances the logical activity count once. Repeated pointers/replay to the same accepted identity do not advance it again.

The projection can answer current and historical activity positions against Archive version cuts.

Importing old material advances activity when that material is accepted into the REL, but the resulting position is not proof of real-world recency or original source time.

## Persistence and transforms

The activity projection is reconstructed from existing durable Archive records. It does not own a parallel semantic ledger.

Repack, rewrite, reconciliation, and migration must preserve logical accepted-turn identity/equivalence even when physical/global version numbers change. Divergent reconciliation counts each destination-accepted eligible identity once in destination history order.

## Consumers

Freshness uses this activity position as its only REL decay clock. Historical planning can use accepted activity order as chronology authority where its contract requires it.

Other consumers may read activity positions, but they may not mutate the clock independently.

## Failure behavior

Malformed or conflicting Archive history fails the owning operation rather than guessing a position. Physical replay duplication must not inflate logical activity.

## Code Map

- `src/archive/**`
- `src/facade/*activity*`

## Related docs

- [Architecture](../architecture.md)
- [Freshness](freshness.md)
- [Storage format](../storage-format.md)
- [ADR 0039](../decisions/0039-unified-deterministic-memory-freshness.md)

## Notes

The former Phase 1 implementation plan is retired. This document owns the current clock contract.
