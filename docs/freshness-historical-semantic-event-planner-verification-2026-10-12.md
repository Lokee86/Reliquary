# Historical semantic event planner verification — 2026-10-12

## Purpose

Record the Step 16 H4 semantic-event planning implementation.

## Overview

The pure planner in src/freshness/historical_semantic.rs consumes the Step 14 provenance plan and Step 15 Archive-clock-compatible timeline, then reuses the frozen F8 SemanticPolicy and plan_local_effects machinery. It does not write the Freshness ledger, propagate effects, score records, rerun Insomnia/Dream, or perform checkpointing.

For each reconstructed activity source it:

- requires the source to belong to the owner and have retained vector evidence;
- filters future, unsupported, archived/superseded, self-source, and incompatible-profile targets before candidate limiting;
- computes deterministic cosine matches and strongest-per-Memory F8 effects;
- emits stable event identity, historical source position, policy digest, profile proof, and source/target evidence digests; and
- reports bounded coverage for missing vectors and no-match causes.

## Related docs

- [Historical manifest and timeline policy](freshness-historical-manifest-policy.md)
- [Historical timeline/provenance planner verification](freshness-historical-planner-verification-2026-10-12.md)
- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)

## Notes

Focused verification:

- cargo test --locked --lib historical_semantic
- cargo fmt -- --check
- git diff --check

The focused fixtures cover eligible matches, future/ineligible budget exclusion, missing source vectors, and incompatible profiles. Event application and import-only propagation remain later steps.
