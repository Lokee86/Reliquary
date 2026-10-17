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

## Step 15 Archive-clock compatibility

The REL facade now derives historical source identities/roles/timestamps/ancestry directly from retained Archive nodes and checks canonical chronology against actual first-accepted activity positions and total count. The gate is read-only, rejects reordered histories and returns a matching plan on the existing REL clock. No conversion is supported. Historical scoring and checkpoint publication remain later work.

Fixtures cover timestamp and UTF-8 tie reorder rejection, equal-timestamp ancestry, independent conversations, non-activity ancestry, accepted retries, empty owners, reopen stability and unchanged global versions on success/failure.

Step 15 verification: the final focused clock suite passes both integration fixtures. The full library suite passed 1,117 tests with one ignored measurement before the module-only move; the final layout also passes formatting and compile checks. Refreshed architecture enforcement passes using the compatible Lexicon/Arcana pair recorded by Step 12 (six rules; the analyzer reported zero relationships). The aggregate root `cargo test` attempt failed resolving the library crate for examples/integration targets; its two-job retry failed with Windows LNK1318 PDB filesystem errors during disk exhaustion. Generated worktree caches/debug symbols were cleared and the final focused checks rerun successfully. This does not establish a passing aggregate root gate.

Documentation impact:
- Inspected: architecture, invariants, storage format, maintainer map, current limitations, roadmap, historical policy and implementation sequence.
- Updated: API, limitations, maintainer routing, development verification, linear Step 15 record and this evidence.
- Not affected: durable formats, live F8 policy/locality, ordinary use/Dream behavior and PHY.
- Compliance check: structural and committed change-impact documentation checks pass, as does git diff --check.
- Known documentation gaps: historical event/scoring/checkpoint behavior remains future Steps 16–25.

Architecture impact:
- Standards added or changed: none; frozen exact-Archive-clock policy is enforced.
- Ownership or boundary impact: a focused read-only REL facade composes Archive facts and pure Freshness planning; neither owner transfers authority.
- Pitlord enforcement impact: existing policy unchanged; compatible-tool refreshed enforcement passes.
- Other verification impact: focused clock and full-library evidence above; aggregate root gate remains unsuccessful due build/disk failures.
- Known architectural gaps: no reordered-history conversion, historical coordinator or score checkpoint yet; zero extracted relationships limits static dependency evidence.

Final CLI verification: formatting and locked compilation pass; all 19 CLI tests pass after cache cleanup. Root/CLI builds retain existing unused-code and MSVC runtime-link warnings.
