# Vector Store crate extraction — implementation evidence

**Status:** Partial implementation; full extraction acceptance is pending.
**Date:** 2026-10-26

## Purpose

Record the verified implementation boundary and distinguish completed checks from pending product acceptance.

## Overview

The lower crate owns the vector storage implementations. Product boundaries are integrated, while final verification and architecture enforcement remain incomplete.


## Implemented boundary

`crates/reliquary-vector-store/` contains lower-level vector models, codecs,
store and replay modules for packed vectors, Archive-vector bindings,
Memory-vector bindings, compatibility profiles and vector generations. Its only
direct Reliquary dependency is `reliquary-container`; it also uses the packed
matrix codec and SHA-256 dependency required by these records.

The root product remains responsible for the public REL/PHY facade and for
cross-owner Archive/Memory authorization, embedding/provider work, compatibility
policy, search, and lifecycle integration. The lower crate owns the durable
vector state and replay; root adapters retain foreign-owner admission and public
error conversion. Existing wire formats and public product signatures are preserved.

## Verification recorded

- `cargo check --workspace --locked --offline -j 1` passed.
- `cargo test -p reliquary-vector-store --locked --offline -j 1` passed all seven vector wire tests.
- Root `cargo test --lib --locked --offline -j 1 vector` passed 65 tests, including packed, Archive, Memory, generation, prune, reopen and reconciliation coverage. The `compatibility_profile`, `semantic_search`, and `memory_retrieval` filters passed 10, 3, and 31 tests respectively.
- `cargo check --manifest-path cli/Cargo.toml --offline -j 1` passed after updating the CLI lockfile.
- `cargo fmt --all -- --check`, CLI formatting, and `ddocs check --indexes --links` passed.
- The documentation policy checker reports four pre-existing missing sections in the Ego Store extraction specification. The installed Pitlord executable lacks the required `docs` command; a newer local build requires the pending Arcana snapshot before it can run.
- `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1` passed all 32 tests.
- `cargo test --locked --offline -j 1` passed with local loopback and filesystem access; the root library ran 1,655 passing tests and five ignored tests, followed by passing integration tests. The restricted run could not exercise local transport and hard links.
- `CVAFRS07` recovery and the authoritative six-file current-format corpus gate are unverified. This extraction does not claim their acceptance.

These results do not satisfy the full acceptance matrix in the [specification](vector-store-crate-extraction-spec-2026-10-26.md). The architecture refresh and documentation ownership enforcement remain outstanding.

## Related docs

- [Architecture](architecture.md#vector-store-crate-extraction-in-progress)
- [Maintainer map](maintainer-map.md#vector-store-crate-extraction-in-progress)
- [Storage format](storage-format.md#packed-vectors)
- [Semantic maintenance](subsystems/semantic-maintenance.md#compatibility-profiles-and-vectors)
- [Rust API](api.md)

## Notes

The architecture checks remain pending. The separate CVAFRS07 corpus gate is not claimed as passing.
