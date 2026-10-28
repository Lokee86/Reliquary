# Vectors service crate extraction evidence

## Purpose

Record the Vectors service extraction, its ownership boundaries and verification results.

## Overview

**Implementation integrated and product-tested.** This record describes the cut after Freshness was committed as `a2313af7`. The Vectors and Vector Store suites, workspace all-target check, full Root suite and CLI suite passed. Architecture and documentation policy gates remain incomplete as described below.

## Baseline

The specification was committed at `87affc63`. At the initial inventory, the checkout had uncommitted Freshness extraction changes across Cargo manifests and lockfiles, Freshness source moves, root wiring and documentation, plus an unrelated Graph evidence edit. Freshness was separately integrated as `a2313af7` before the Vectors changes were applied. The Graph evidence edit remains outside this extraction.

The simulated endpoint with seed 7, 16 dimensions and L2 normalization produces profile ID `6a639dc66fc6e0cac000a921241aa129e2a0ddacee7eee218699bccceeb45423`. The service tests freeze probe order, policy versions, threshold behavior, and little-endian packed bytes. Root tests retain the stored v2-profile reopen check.

## Ownership cut

The extraction moves the embedding endpoint contract, deterministic simulated endpoint and Switchboard adapter into `crates/reliquary-vectors/src/embedding_endpoint.rs`. The service also contains compatibility-profile probe construction and comparison, numeric vector validation, and pure F32 batch packing. Its current direct Reliquary dependencies are Switchboard and Vector Store; `sha2` and the pinned `lodestone-packed` library are external dependencies.

Switchboard retains endpoint transport, credentials and route selection. Vector Store remains the only durable owner of packed vectors, profiles, Archive/Memory bindings and vector generations. Root Cva/Phylactery adapters retain cross-owner authorization, source validation, transactions and publication. Retrieval retains candidate selection and ranking. Freshness and the Root F8 coordinator retain semantic effect policy and publication. No new scheduler, store, journal or ranking behavior is part of this extraction.

Product consumers use the service, and the former root embedding, compatibility-probe and vector-math implementations have been removed.

## Dependency boundary

`reliquary-vectors` depends directly on `reliquary-switchboard` and `reliquary-vector-store`. It does not depend on the product root, Retrieval, Freshness, Runtime, Dream or Container. The Vector Store does not depend on Vectors. Vectors performs no durable writes; it returns packed values to owner coordinators for publication.

## Verification

- `cargo test -p reliquary-vectors --locked --offline -j 1`: 13 unit tests and one downstream trait implementation test passed.
- `cargo test -p reliquary-vector-store --locked --offline -j 1`: 7 wire tests passed.
- `cargo test -p reliquary-switchboard --locked --offline -j 1`, with local loopback access: 44 passed, including bounded transport behavior.
- `cargo check --workspace --all-targets --locked --offline -j 1`: passed on the integrated working tree.
- Focused Root binary filters for compatibility profile, vector generation, archive vector, memory vector, semantic search and memory retrieval: 81 tests passed across the six filters.
- Focused Root `semantic_live` and `historical_embedding` filters: 17 passed with unrestricted loopback.
- CLI format check and `cargo check --manifest-path cli/Cargo.toml --locked --offline -j 1`: passed. CLI tests passed: 32 passed, with local loopback access.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Full `cargo test --lib --locked --offline -j 1`, with local loopback access: 1,478 passed, 5 ignored. An earlier restricted run hit a local-server timeout and was interrupted; the affected test passed with loopback access.
- Documentation policy checks, using an extended-length Windows repository path, report 28 missing required section headings in seven unrelated pre-existing Dream, Ego, External Index, Import and Maintenance extraction documents. No Vectors document findings remain. The normal path invocation fails while traversing a pre-existing long generated path under `target`.
- `ddocs check --indexes --links`: passed after reconciling the index.
- `python scripts/check_architecture.py --refresh`: interrupted after more than 30 minutes of Rust analysis and approximately 5.7 GB resident memory, before producing a fresh snapshot. Architecture enforcement is not verified.
- Pitlord documentation ownership using the available local source and release Arcana ran against the existing stale snapshot and failed with hundreds of unmapped paths, including paths already deleted from this tree. It is not valid evidence for the current change; retry after a successful refresh.

The documentation policy and architecture enforcement checks remain unverified or failing as described above.

## Follow-up verification (2026-10-28)

The current working-tree cut was independently reviewed against the previous probe, comparison, validation and packing implementations; the review found no concrete behavior regression. The Vectors suite passed (13 unit tests and one downstream trait test), Vector Store passed seven wire tests, the locked offline all-target workspace check passed, and workspace formatting and `git diff --check` passed. The CLI suite passed 32 tests. The full Root library suite passed 1,478 tests with five ignored, using repository-local temporary files and local loopback access. A restricted run timed out waiting for local test-server requests and was interrupted before the successful rerun.

Archivist index and link validation passed. The documentation policy check using an extended-length path completed with 31 missing-section findings in existing documents, including three in the committed Retrieval specification; it did not report a Vectors document finding. The installed Pitlord does not support its required `docs` command, and the sibling build reports a malformed Arcana repository manifest. A new architecture refresh was attempted but interrupted during Rust analysis; the current architecture and Pitlord gates therefore remain unverified. These results do not establish the documentation policy or architecture enforcement gates.

## Remaining verification limitations

The architecture snapshot must be refreshed and Pitlord documentation ownership rerun with a compatible tool. The existing documentation policy findings remain separate from the Vectors extraction.

## Notes

The separate six-file `CVAFRS07` corpus gate is not established by this extraction.

## Related docs

- [Vectors service extraction specification](vectors-service-crate-extraction-spec-2026-10-28.md)
- [Vector Store extraction evidence](vector-store-crate-extraction-evidence-2026-10-26.md)
- [Freshness service extraction evidence](freshness-service-crate-extraction-evidence-2026-10-28.md)
