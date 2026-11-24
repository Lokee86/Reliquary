# Storage and durable state

Parent index: [Reliquary architecture](./INDEX.md)

## Purpose

Describe the physical container boundary and the owners of persisted REL/PHY records.

## Overview

`reliquary-container` owns the versioned file header, framed append/read path, physical cursor, publication envelope and version clock. It provides low-level physical operations; it does not interpret memory, conversation, graph, or semantic policy. The current normal runtime accepts the REL 2.1 format. Legacy conversion is a separate offline concern and is not an alternate path through normal owner opens.

The REL 2.1 header stores both the owner ID and a scope value. New RELs use `Unspecified`. The current reader decodes all four original values, and same-owner maintenance retains the original header identity.

The eight durable domain owners are `reliquary-archive`, `reliquary-memory-core`, `reliquary-graph`, `reliquary-perception-store`, `reliquary-ego-store`, `reliquary-freshness-store`, `reliquary-insomnia-store`, and `reliquary-vector-store`. Each defines its record model, codec/replay, and local queries. The stores do not share a generic mutable facade.

## Responsibilities and state owners

- **Archive** owns accepted conversation turns, episode boundaries, attachment/source references, and echo records. Its stable episode and turn identities ground later processing.
- **Memory core** owns memory records, payload state, and memory identity. Leased replay retains verified ranges for standalone and grouped bodies. Historical projection reads the selected body from its owner file. Detached snapshots retain their payloads in memory.
- **Graph** owns relation/community state and graph history; it does not own memory payloads.
- **Perception store** owns Entity descriptions, mentions, manual Relationship history and the typed Observation projection. Cross-REL discovery pages through the existing ordered map of current Entities by seeking past the last Entity ID. Historical and retired descriptions live in separate projections. The candidate keys are derived in memory, and the recording REL persists each judgment in its existing identity receipt history.
- **Ego store** owns owner identity/profile state.
- **Freshness store** owns freshness policy, scores/projections, and historical snapshots.
- **Insomnia store** owns accepted episode order and Insomnia progression/settlement records.
- **Vector store** owns compatibility profiles, packed vectors, archive/memory mappings, and generation records. It owns durable vectors; `reliquary-vectors` only evaluates compatibility and batches.

`reliquary-owner-session` is the composition boundary that opens these stores for a single REL or PHY owner. Application code does not open containers or compose their mutable handles directly. See [owner sessions](owner-session.md) for writer leases, grouped writes, and replay coordination.

## Lifecycle, mutation, and recovery

An owner open validates identity and supported format, constructs the relevant store projections from container frames, and exposes owner-scoped reads. For a mount with an expected owner ID, Container checks the header through the same leased handle used for replay. Mutations append records through the owner session and the Container physical writer; reopen reconstructs current in-memory projections by replay. Container supports read-only opens as well as writer and repair opens. Physical cursors read bounded blocks at explicit offsets through the existing file lease. A complete scan checks its captured length before fetching a new frame header from disk and at the end of the file. Owner replay consumes one physical payload at a time, and the domain stores retain their own decoded state. A failed partial append blocks further ordinary mutation until explicit recovery; callers must not treat a failed append as a committed domain change.

During physical validation, Container keeps the positions and first 64 payload bytes of large frames, up to 32,768 entries per lease. A store can use those prefixes to skip an unrelated payload during subsequent replay. It reads prefixes of smaller frames directly. Selected frames retain their original bytes, position and order. A store that needs the global version at each record also accounts for intervening publications and reservations, reading complete grouped records when their clock requires it. REL checkpoint admission releases the directory before returning the session. An append also discards it. The directory is transient and adds no fields to an owner or checkpoint.

REL and PHY are distinct owner/file kinds, and identical contents do not imply shared mutable authority. Copy/reconciliation work is explicit and verifies the copied file by reopening it. Historical, freshness, vector, and graph state may be derived from primary records, but durable snapshots and journals retain their own owning stores; generated projections are not edited by higher layers.

Container can finish a header-checked lease from a previously admitted physical checkpoint. It binds the checkpoint to the open file's identity and header, hashes the exact covered bytes, and then checks framing and version allocation through the captured extent. The checkpoint carries outstanding version reservations and recorded transaction times. A prefix mismatch restarts physical validation under the same lease. A torn suffix continues to require explicit repair. Indexed readers use positional, digest-checked ranges on the open lease. A failed read fences further writes until reopen.

Owner-session stores derived checkpoints in the local account's private cache. Each slot binds a canonical REL path to its physical owner identity and versioned replay schema. The checkpoint manifest records the accepted history generation, domain cuts, clock frontier, and exact prefix digest. Each state page is bounded and checked against the manifest and page position. The writer stages and syncs the complete derived file before installing it atomically. The version marker must change when the serialized state or replay interpretation changes. The original REL supplies the only authoritative history.

A REL stores new Observation batches in a versioned native publication carrier. Accepted revisions, standing transitions, comparison receipts, Freshness awards and progress are replayed together. Checkpoints are private projections of that history. Verified same-owner copies and supported storage reclamation preserve the carrier. See [Observation replay and retention](owner-session.md) for the supported maintenance boundaries.

Community replay recognizes `RELCOM01` for full stored graph coverage and `RELCMP01` for a subset. Both validate the stored Graph cut, owner-derived IDs, sorted membership, and generation sequence. The owner writer enforces exact active-Memory coverage before publishing a recomputed generation. Fresh-history rewrite preserves valid projected subsets using the matching Community record.

The REL-only `RELDEP01` frame stores a contiguous declaration generation and a sorted set of durable owner IDs. Existing 2.1 headers and file identities are preserved. Verified copies, pruning, reclamation and same-owner rewrites retain the declaration history. Divergent dependency tails and identity-changing rewrites with dependency records fail before publication.

Native retained history uses `RELPUB01` to bind an accepted local Archive, Memory, Graph or Entity publication to its original global clock. A publication can bind an earlier standalone reservation and preserve its signed or absent transaction time. Archive repeats refer to their first-applying publication. A retained project attachment's `RELPRB01` record binds its content identity to the physical global cut where backing became available. The Archive checks that cut and the accepted attachment before admitting the record.

A REL records a cross-REL Entity identity comparison in a clocked `RELIDR01` receipt. A direct Memory citation identifies the resolved mention and its revision, the immutable Memory body and the observed excerpt span. A citation to a migrated typed resolution also binds its source ordinal so replay can verify the decision that existed at publication. Existing Relationship-only receipts retain their original encoding. It records an operator’s confirmation of separation or subsequent withdrawal in a clocked `RELIDS01` event with an independent per-pair revision chain. Cold replay checks the local Entity and reviewed comparison against the publication prefix and validates the structure of the peer reference without reading the peer REL. The endpoint proof hashes the Entity history through the reviewed revision and incorporates retained description digests when an Entity description has been pruned. Verified prefix copies and reclamation preserve both chains. Fresh-history rewrite and divergent reconciliation refuse a source with either record before creating a destination. [Owner-session](owner-session.md)

Leased Archive replay validates turn text and inline attachment bytes at their native records, then retains byte ranges and their content identities. A bounded read checks its byte budget before using the same lease to check the frame boundary and content digest. A detached Archive owns its materialized data after the file lease closes.

The current stores also preserve grouped Insomnia births and their routing, historical Community derivations, Entity Resolution revisions, and the accepted Freshness journal and settlement. During open, Memory applies each grouped Insomnia completion to the store it is building. A failed validation discards that store and fails the open. A live writer applies the same checks to a private copy and installs it only after the group passes. Leased REL checkpoints retain grouped completion metadata and verified references to the original carriers. History-cut checks use the retained cut bounds. Exact retries and reconciliation read the original carrier when needed. Vector activation receipts carry a current physical reference. A verified copy that moves that generation must relocate the receipt with the vector codec.

Archive vectors use the existing REL packed payload and its ordered fragment mapping. Owner replay verifies the packed matrix and mapping before sharing immutable references with readers. Lodestone ranks the accepted F32 rows directly with exact cosine. The REL is the sole durable copy and publication authority for the matrix, mapping, and activation. Existing REL 2.1 files with active Archive generations can use this path on reopen. Native fragment definitions and retained migration records both preserve the source range used for hybrid search. Newly published Archive vectors use separate passage definitions for windows over the embedding byte budget. Each definition identifies a parent fragment and bounds within its first and last accepted turns. Whole-window identities and old records retain their original representation. Archive shares immutable turn text with captured readers through in-memory references. The writer can persist the lexical index as a single `RELALX01` frame containing the sorted dictionary and delta-coded postings. Its checksum covers the owner, vector generation, ordered source-content digest, text coverage, Unicode analyzer version, and index body. The record uses stable IDs and little-endian sizes. Replay uses an accepted matching record and can rebuild from Archive text when a cache is absent or invalid. Explicit tail repair removes a physically incomplete frame. Reclamation keeps active sealed indexes and can remove superseded or damaged frames. Archive-pruning copies drop lexical projections because their source payloads change.

## Invariants and tests

The physical layer rejects unsupported or mixed headers and identity mismatches without mutating the file. Store codecs validate record boundaries and replay conflicts at the owning layer. Application workflows use stable ids and publication receipts to make retries owner-local. Representative protection is in [`crates/reliquary-container/src/lib.rs`](../../crates/reliquary-container/src/lib.rs) (`writes_and_reopens_exact_current_header_and_frames`, `a_failed_partial_append_blocks_writes_until_explicit_recovery`), [`crates/reliquary-archive/src/archive_tests.rs`](../../crates/reliquary-archive/src/archive_tests.rs), [`crates/reliquary-memory-core/src/tests.rs`](../../crates/reliquary-memory-core/src/tests.rs), and [`crates/reliquary-vector-store/src/tests.rs`](../../crates/reliquary-vector-store/src/tests.rs). Owner-level copy/reopen and divergence checks live in [`crates/reliquary-application/src/verified_copy_workflow.rs`](../../crates/reliquary-application/src/verified_copy_workflow.rs).

## Code map and extension seams

- Physical contract: [`crates/reliquary-container/src/lib.rs`](../../crates/reliquary-container/src/lib.rs), including `Container::create`, `open_read_only`, `open_writer`, `open_for_repair`, and append/version operations.
- Domain stores: the eight crates listed above; `reliquary-owner-session/src/session.rs` composes them.
- Add a durable domain concept to its owning store, including replay and malformed-record behavior. Add cross-store coordination at owner-session or Application, according to whether it is a storage invariant or product workflow. Do not put domain encoding in Gateway or add a second source of truth in a convenience cache.

## Related docs

- [Reliquary source and binary distribution terms](../../README.md#license)
- [Owner sessions](owner-session.md)
- [Application workflows](application.md)
- [System architecture](system.md)

## Notes

Format conversion and migration commands are specified by the relevant reference/operations owners; this page only identifies the normal runtime ownership boundary.
