# Retrieval service crate extraction evidence â€” 2026-10-28

**Status:** Implementation integrated and product-tested; documentation and architecture tooling limitations remain below.
**Repository / branch:** `Reliquary-crate-first`, `refactor/crate-first-restart`.
**Baseline:** Vectors service commit `3bacc56d`. The Retrieval extraction follows that baseline.

## Purpose

Record the Retrieval extraction, ownership boundary and verification evidence.

## Overview

The service implements deterministic retrieval over owner-supplied facts.

## Implemented boundary

`crates/reliquary-retrieval/` now contains lexical tokenization, term scoring and disposable Archive/file/raw-turn/Memory posting indexes; exact semantic row scoring; hybrid score fusion, deduplication and diversification; deterministic Community subcentroid generation and routing; Memory retrieval configuration, result models, disposable index construction, routed/global candidate selection, exact seed scoring and watermark comparison; and bounded Graph traversal.

Root keeps the public REL/PHY facades and maps the service's Memory retrieval types directly into the existing product API. Root still reads and validates Archive, Memory, Graph and vector-store data, supplies packed F32 rows for service decoding, constructs authorized owner snapshots, hydrates content, acquires live watermarks and active relations, and maps pure service errors to the existing public error variants. Agentic controller state, Freshness access, model decisions and accepted-use handoff remain in Runtime/Gateway owners. The service has no Container or product-root dependency. Its direct Reliquary dependencies are Archive, Graph, Memory Core, Vectors and Vector Store.

The Community snapshot crosses the existing Root-to-Graph conversion boundary. Retrieval receives the lower Graph snapshot and lower Community ID, converting raw graph Memory ID bytes to Memory Core IDs only when looking up caller-supplied vector rows. No new durable state, graph authority, storage format or search heuristic was introduced.

## Verification performed

- `cargo test -p reliquary-retrieval --locked --offline -j 1`: 18 unit and 5 integration tests passed.
- Vectors and Vector Store locked offline suites passed (14 and 7 tests respectively).
- `cargo check --workspace --all-targets --locked --offline -j 1`: passed.
- `cargo test --lib --locked --offline -j 1`: 1,477 passed, 5 ignored with local loopback and repository-local temporary files. Two earlier runs exposed and then corrected a changed Dream centroid fixture and a test-file encoding error.
- CLI locked offline tests: 32 passed with repository-local temporary files and local loopback; a restricted run had two access-denied failures.
- Workspace and CLI formatting, `git diff --check`, and `ddocs check --indexes --links` passed.
- The changed-from-main documentation policy check reports 28 pre-existing missing-section findings in seven unrelated documents; no Retrieval document findings remain.
- Architecture refresh and Pitlord documentation ownership remain unverified: the earlier refresh did not complete and available Pitlord/Arcana installations were incompatible with the required command or snapshot. Cargo dependencies provide the verified boundary, not a replacement claim for those gates.

The owner-qualified Freshness union and limit remain complete before selection; controller state and accepted-use handoff remain in Runtime. The separate six-file CVAFRS07 admission and historical agentic recall limitation are not resolved by this extraction.

## Notes

The separate CVAFRS07 corpus admission and historical agentic recall limitations remain unverified by this extraction.

## Related docs

- [Retrieval operator manual](manual/retrieval.md)
- [Agentic Memory retrieval](subsystems/agentic-memory-retrieval.md)
- [Architecture](architecture.md#retrieval-deterministic-policy-crate-in-progress)
- [Rust API](api.md#retrieval-policy-service)
- [Maintainer map](maintainer-map.md)
