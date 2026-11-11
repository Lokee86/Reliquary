# Storage and durable state

Parent index: [Reliquary architecture](./INDEX.md)

## Purpose

Describe the physical container boundary and the owners of persisted REL/PHY records.

## Overview

`reliquary-container` owns the versioned file header, framed append/read path, physical cursor, publication envelope and version clock. It provides low-level physical operations; it does not interpret memory, conversation, graph, or semantic policy. The current normal runtime accepts the REL 2.1 format. Legacy conversion is a separate offline concern and is not an alternate path through normal owner opens.

The eight durable domain owners are `reliquary-archive`, `reliquary-memory-core`, `reliquary-graph`, `reliquary-perception-store`, `reliquary-ego-store`, `reliquary-freshness-store`, `reliquary-insomnia-store`, and `reliquary-vector-store`. Each defines its record model, codec/replay, and local queries. The stores do not share a generic mutable facade.

## Responsibilities and state owners

- **Archive** owns accepted conversation turns, episode boundaries, attachment/source references, and echo records. Its stable episode and turn identities ground later processing.
- **Memory core** owns memory records, payload state, and memory identity.
- **Graph** owns relation/community state and graph history; it does not own memory payloads.
- **Perception store** owns entity descriptions, mentions, and relationship descriptions/history.
- **Ego store** owns owner identity/profile state.
- **Freshness store** owns freshness policy, scores/projections, and historical snapshots.
- **Insomnia store** owns accepted episode order and Insomnia progression/settlement records.
- **Vector store** owns compatibility profiles, packed vectors, archive/memory mappings, and generation records. It owns durable vectors; `reliquary-vectors` only evaluates compatibility and batches.

`reliquary-owner-session` is the composition boundary that opens these stores for a single REL or PHY owner. Application code does not open containers or compose their mutable handles directly. See [owner sessions](owner-session.md) for writer leases, grouped writes, and replay coordination.

## Lifecycle, mutation, and recovery

An owner open validates identity and supported format, constructs the relevant store projections from container frames, and exposes owner-scoped reads. Mutations append records through the owner session and the Container physical writer; reopen reconstructs current in-memory projections by replay. Container supports read-only opens as well as writer and repair opens. A failed partial append blocks further ordinary mutation until explicit recovery; callers must not treat a failed append as a committed domain change.

REL and PHY are distinct owner/file kinds, and identical contents do not imply shared mutable authority. Copy/reconciliation work is explicit and verifies the copied file by reopening it. Historical, freshness, vector, and graph state may be derived from primary records, but durable snapshots and journals retain their own owning stores; generated projections are not edited by higher layers.

## Invariants and tests

The physical layer rejects unsupported or mixed headers and identity mismatches without mutating the file. Store codecs validate record boundaries and replay conflicts at the owning layer. Application workflows use stable ids and publication receipts to make retries owner-local. Representative protection is in [`crates/reliquary-container/src/lib.rs`](../../crates/reliquary-container/src/lib.rs) (`writes_and_reopens_exact_current_header_and_frames`, `a_failed_partial_append_blocks_writes_until_explicit_recovery`), [`crates/reliquary-archive/src/archive_tests.rs`](../../crates/reliquary-archive/src/archive_tests.rs), [`crates/reliquary-memory-core/src/tests.rs`](../../crates/reliquary-memory-core/src/tests.rs), and [`crates/reliquary-vector-store/src/tests.rs`](../../crates/reliquary-vector-store/src/tests.rs). Owner-level copy/reopen and divergence checks live in [`crates/reliquary-application/src/verified_copy_workflow.rs`](../../crates/reliquary-application/src/verified_copy_workflow.rs).

## Code map and extension seams

- Physical contract: [`crates/reliquary-container/src/lib.rs`](../../crates/reliquary-container/src/lib.rs), including `Container::create`, `open_read_only`, `open_writer`, `open_for_repair`, and append/version operations.
- Domain stores: the eight crates listed above; `reliquary-owner-session/src/session.rs` composes them.
- Add a durable domain concept to its owning store, including replay and malformed-record behavior. Add cross-store coordination at owner-session or Application, according to whether it is a storage invariant or product workflow. Do not put domain encoding in Gateway or add a second source of truth in a convenience cache.

## Related docs

- [Owner sessions](owner-session.md)
- [Application workflows](application.md)
- [System architecture](system.md)

## Notes

Format conversion and migration commands are specified by the relevant reference/operations owners; this page only identifies the normal runtime ownership boundary.
