# Maintenance Store extraction evidence

## Purpose

Record the ownership decision and verification for the Maintenance Store extraction.

## Overview

Baseline: `3a389464fa70f3ca3524fbe2229596cf4d2a931f`; the unrelated Graph extraction evidence was already modified and was preserved. Baseline workspace locked offline check and format check passed.

## Ownership inventory and decision

Extract: `CVAHPR01` is an existing durable, non-semantic prune manifest, not a hypothetical journal. REL and PHY prune coordinators create descriptors after owner eligibility decisions, encode the manifest and append it as a Container object to a transformed copy before sync. On open, both lifecycle paths ingest the object into a digest-keyed projection and reject duplicates. Archive, Memory, Entity, Relationship and Vector validation consume the manifest and their own markers; decoded evidence is not independent authorization. The manifest's canonical checksummed JSON framing, bounds and descriptor invariants belong to Maintenance Store. There is no separate maintenance journal or physical transaction mechanism.

The cross-owner source history fingerprint and its binding checks remain in the product, because they reconstruct Archive, Memory, Graph, Perception, Vector and other owner evidence. Domain codecs and pruning decisions remain with their owners; Container retains physical mechanics. The crate has no direct Reliquary dependency. Borrowed manifest references remain borrowed from the single lower projection; no public product API changes. No format or append/sync sequence changes.

## Verification

- Baseline `cargo check --workspace --locked --offline` and `cargo fmt --all -- --check`: passed.
- After extraction `cargo check --workspace --locked --offline -j 1`, `cargo fmt --all -- --check`, and `cargo fmt --manifest-path cli/Cargo.toml -- --check`: passed.
- `cargo test -p reliquary-maintenance-store --locked --offline -j 1`: 4 passed, including fixed wire digest, replay, corruption and canonicality.
- `cargo test --lib --locked --offline -j 1 prune -- --test-threads=1`: 35 passed, including REL/PHY owner proofs, tombstones and cross-owner retention.
- `cargo test --test history_rewrite_contract --locked --offline -j 1 -- --test-threads=1`: 16 passed, including REL/PHY normal and fresh rewrite, tombstones, planning and output refusal.
- `cargo test --lib --locked --offline -j 1 rewrite -- --test-threads=1 --skip memory_selection_and_rewrite_controllers_use_selected_general_or_shared_route`: 25 passed. The unfiltered run had 25 passes and one unrelated local HTTP decision-integration failure: its test server timed out and the endpoint request failed.
- `cargo test -p reliquary-container --locked --offline -j 1 incomplete -- --test-threads=1`: 4 passed, including incomplete-prefix recovery and non-mutating open.
- `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`: 32 passed.
- The broad recovery run passed the exhaustive native checkpoint byte-prefix and complete corrupt-checkpoint tests; the broad run was stopped during the subsequent exhaustive live-use prefix case, so the full recovery filter is not claimed as passed.
- `ddocs check --indexes --links`: passed. Both shared documentation policy checks reported 32 missing-section findings in other extraction documents.
- The locally built Pitlord documentation guard with the sibling Arcana executable ran and failed with 1036 of 1297 files mapped; 261 existing branch-wide files have no codemap ownership. The new Maintenance Store file is mapped by this change.
- The architecture refresh was stopped after more than 15 minutes in the Lexicon Rust scan without a new snapshot. `python scripts/check_architecture.py` passed against the existing snapshot: both self-test guards passed and Pitlord reported no violations across 25 rules. This is not fresh architecture evidence. Parallel broad test compilation initially encountered Windows paging-file exhaustion (OS error 1455); single-job runs above avoided that failure.

## Related docs

- [Extraction specification](maintenance-store-crate-extraction-spec-2026-10-27.md)
- [Architecture](architecture.md)

## Notes

This evidence does not claim CVAFRS07 corpus acceptance.
