# Historical checkpoint recovery model evidence — 2026-10-12

## Purpose

Record partial Step 22 model evidence. The [Step 26 production audit](freshness-combined-production-review-2026-10-12.md) withdraws native recovery completion: these tests do not exercise Container byte crash prefixes, REL reopening or durable accepted-use receipts.

## Overview

The experimental in-memory model accepts nonempty candidate maps and rejects conflicting snapshots. Its tests build a due index from a model and apply one caller-supplied contribution twice. There is no codec, owner UUID, Container I/O or actual reopen. Clearing a map models invalid input rather than a durable crash prefix; exact import retry after live growth and multi-recipient native receipt semantics are not established.

This model is not wired to the real Step 21 Cva/Freshness owner and must not become a second scoring/receipt authority. Native recovery remains Step 22 work; live semantic non-propagation is unchanged.

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

The three model fixtures cover an empty-map candidate, retry before model mutation, conflicting snapshots, a model due index and one repeated caller contribution. They do not close native crash recovery or prove a real accepted-use event.
