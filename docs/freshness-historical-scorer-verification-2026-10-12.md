# Historical sparse scorer verification — 2026-10-12

## Purpose

Record Step 20 verification for chronological historical Freshness scoring over sparse recipient contributions.

## Overview

The import-only scorer groups events by recipient, orders each group by historical position and stable event identity, applies signed contributions with the existing Freshness record saturation rules, and settles lazy decay at the requested historical cut. Duplicate (recipient, event identity) pairs are rejected before mutation. A single origin event may credit multiple recipients once each, as verified by the Step 21 correction fixture. Reordering independent input events therefore produces byte-identical records.

The H5 fixtures cover +100 saturation, -100 followed by +50, early-versus-late reinforcement divergence, Dormant recovery, ten-turn remainder preservation, same-position event-ID ordering, duplicate retry rejection, and deterministic reordered-input output.

## Related docs

- [Freshness semantic + historical linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Historical Freshness manifest and timeline policy](freshness-historical-manifest-policy.md)
- [Historical propagation verification](freshness-historical-propagation-verification-2026-10-12.md)
- [Historical recipient-only bridge oracle verification](freshness-historical-bridge-oracle-verification-2026-10-12.md)

## Notes

Focused verification:

- cargo test --locked --lib historical_scorer
- cargo test --locked --lib historical
- cargo fmt -- --check
- git diff --check
- Documentation policy checks for the repository

The scorer is a pure planning projection: it does not create durable receipts or alter live semantic non-propagation.
