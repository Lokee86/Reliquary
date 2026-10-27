# Freshness Store crate extraction — implementation evidence

## Purpose

Record the ownership boundary and verification for the Freshness Store extraction.

## Overview

The compiled `reliquary-freshness-store` owns the accepted journal, replay, import settlements, rewrite baselines, activation, bounded receipt lookup and disposable rebuild machinery. Container owns physical persistence. The product converts Memory, Fragment and history-generation identities at its boundaries; Graph, Dream, Import and History retain authorization and planning. Container is the store's only Reliquary library dependency. Score records, policy values, event proof and cut shapes, and persisted policy identities have one canonical lower-crate definition.

Semantic import replay receives expected Dream initial-pass identities from the higher owner and validates them against accepted receipts, provenance and scope. Root-stage performance tracing remains; the former lower-store scan and payload-shape trace rows are no longer emitted.

## Notes

- `cargo check --workspace --locked --offline` passed.
- `cargo test -p reliquary-freshness-store --locked --offline` passed 46 tests.
- The full `cargo test --locked --offline -j 2` passed 1,655 library tests with five ignored and all integration targets, with one integration test ignored. An initial higher-concurrency build failed to find compiled library artifacts; a lower-concurrency build succeeded.
- The detached CLI suite passed 32 tests. Workspace and CLI formatting checks passed.
- The 20,000-receipt owner reopen fixture passed twice, preserving bounded residency and exact evicted retry. The semantic frame relocation assertion passed.
- `ddocs check --indexes --links` passed. Shared documentation policy checks report four pre-existing missing sections in the unchanged Ego Store specification. The installed Pitlord does not provide the requested `docs` command. Architecture guard self-tests passed; Lexicon refresh did not complete in two attempts, so Arcana/Pitlord architecture enforcement remains unverified.
- CVAFRS07 recovery and the authoritative six-file corpus gate remain unverified; this extraction does not claim format promotion or corpus acceptance.

## Related docs

See the [extraction specification](freshness-store-crate-extraction-spec-2026-10-26.md) for the accepted boundary and format constraints.
