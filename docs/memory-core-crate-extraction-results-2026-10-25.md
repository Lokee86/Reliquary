# Memory Core crate extraction results

Parent index: [Documentation index](INDEX.md)

## Purpose

Record implementation ownership and the scoped verification results for the Memory Core extraction.

## Overview

Memory Core production code now lives in its independently compiled crate. The checks below cover its local store, selected root integrations, CLI, formatting, and workspace compilation; promoted CVAFRS07 corpus admission remains a separate gate.

## Related docs

- [Memory Core extraction specification](memory-core-crate-extraction-spec-2026-10-25.md) defines the owner boundary and acceptance criteria.
- [Architecture](architecture.md), [storage format](storage-format.md), and [current limitations](current-limitations.md) describe current ownership and corpus constraints.

## Outcome and ownership

Branch: `refactor/crate-first-restart`. The former root Memory store, current-format codecs, replay, body/routing indexes and immutable record types have moved to `crates/reliquary-memory-core/`. The root `Cva`, `Phylactery`, Insomnia, External Source, Import, Perception, Chronos and historical callers now consume that compiled owner. Its sole Reliquary Cargo dependency is `reliquary-container`.

Archive source checks, processing epochs, External Source source-window receipts, lifecycle translation, Freshness and multi-owner Maintenance remain outside Memory Core. The old root Memory model/store/replay/codec implementations were removed; remaining `src/memory` code is integration or pending higher ownership.

## Grouped replay correction

The initial extracted `MemoryStore::apply_grouped_batch` cloned the entire accumulated MemoryStore for every grouped publication. This was replaced with a bounded overlay of incoming IDs, mutation records, and routing bindings. The complete batch is validated before any insertion (including body spans, body hashes, local/global clock order, revisions, duplicate mutations, terminal tombstones, and routing binding/conflicts); insertion then uses the one existing Memory owner store/index implementation. This removes an O(total prior Memory records) copy per accepted group and retains failure atomicity.

Three focused owner-local tests cover accepted grouped content plus routing and rejection after a valid first record or before routing application, asserting no partial Memory bodies/versions/indexes on failure.

## Verification

| Check | Observed result |
| --- | --- |
| Memory Core `cargo test -p reliquary-memory-core --locked --offline` | 38 passed, 0 failed, including 3 grouped replay cases |
| Memory Core Cargo boundary | `reliquary-container` is the only Reliquary dependency |
| Locked workspace compilation | Passed after the grouped replay correction (`cargo check --workspace --locked --offline`) |
| Root Memory-related filtered tests | 230 passed, 0 failed, 1 ignored; known expensive measurements excluded; 1,622 other tests filtered |
| Detached CLI | 32 passed, 0 failed (`cargo test --manifest-path cli/Cargo.toml --locked --offline`, shared Cargo target directory) |
| Formatting | `cargo fmt --all -- --check` passed |
| Root grouped-publication tests | 11 passed, 0 failed (Insomnia grouped recovery, source cuts, reconciliation) |
| Root External Source tests | 34 passed, 0 failed (publication, window recovery, source binding) |
| Promoted authoritative `CVAFRS07` corpus | Not accepted here; requires separately recovered Freshness/Import semantics |

**Scoped extraction acceptance:** Memory Core, selected root integration, CLI, formatting, and workspace checks pass. The full root suite and promoted `CVAFRS07` corpus admission were not performed; long-running whole-history measurements remain separate. Root test selections overlap and must not be summed as distinct test coverage.

## Notes

Root test selections overlap and must not be summed as distinct coverage. The full root suite and promoted `CVAFRS07` corpus admission were not performed.
