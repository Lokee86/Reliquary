# Historical timeline/provenance planner verification — 2026-10-12

## Purpose

Record the Step 14 H2 planner implementation and focused verification.

## Overview

The pure planner in src/freshness/historical_planner.rs deduplicates immutable source identities, validates same-conversation ancestry, detects cycles and timestamp regressions, orders activity turns deterministically, validates an external Archive clock, resolves source/content/grounding/Episode evidence, reports unsupported coverage, enforces cohort consistency, and derives a canonical manifest identity.

The module has no storage or scoring side effects. Checkpointing, propagation, Archive integration, and Dream/Insomnia event production remain later steps.

## Related docs

- [Historical manifest and timeline policy](freshness-historical-manifest-policy.md)
- [Historical reconstruction implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)

## Notes

Focused verification commands:

- cargo test --locked --lib historical_planner
- cargo fmt -- --check
- git diff --check

Fixtures cover equal timestamp ordering, ancestry precedence, non-activity ancestry, missing parents, Archive-clock mismatch, and provenance availability. Step 15 connects the planner clock proof to actual Archive acceptance order.
