# Owner sessions

Parent index: [Reliquary architecture](./INDEX.md)

## Purpose

Describe the owner-session boundary that composes durable stores and controls authorized mutation for one REL or PHY file.

## Overview

`reliquary-owner-session` owns concrete REL and PHY sessions, their typed store handles, owner authorization inputs, replayed projections, writer leases, and multi-record publication methods. A session is bound to one file identity and owner kind. `RelSession` and `PhySession` provide reads; `RelWriter` and `PhyWriter` provide controlled mutation. Higher layers receive session operations and selected immutable values, never a `Container` or a bag of writable stores.

The crate is the only product boundary that composes the eight persistent domain owners listed in [storage](storage.md). It does not own Application policy such as how to rank candidates or which user workflow should run.

## State, lifecycle, and concurrency

`RelWriter::create_current_path(path, owner_id)` initializes a new current REL with the neutral 2.1 scope value. Existing RELs expose their stored scope on open. Maintenance that preserves owner identity uses the source's container identity.

Opening a session checks the physical file identity/kind, opens the relevant stores, and replays their records into owner-scoped views. The replay passes borrow each frame from a reusable payload buffer. The stores decode and retain the records they own, and the replay checks validate each publication in physical order. A writer obtains an owner-local lease/fence before publication. Prepared work is rechecked against current owner revisions and authorization at commit; a stale or foreign cut is rejected. Grouped commit APIs coordinate related records under one owner writer so readers do not see a higher-level success without its required receipt/history records.

An Archive vector search captures the active generation and shared references to its immutable packed matrix and fragment mapping. The search view keeps those values alive when the owner advances or closes. The generation binds the source Archive cut and the accepted activation record. Hybrid capture pins readable text for each mapped fragment under the same owner read. Text and packed rows remain available to the scorer for the duration of that captured read.

REL and PHY writers are separate authorities. Session clones do not create independent authority to bypass the writer boundary. Runtime-level synchronization and leases serialize mutation through mounted Gateway writers; persistence is still recovered from the file's append/replay state after process restart.

Community recompute captures eligible IDs and active structural edges from one REL read. Capture and publication share the same active-Memory check. The writer rechecks the captured file identity and versions, requires every eligible ID exactly once, and syncs the Community frame through the owner write boundary.

## Failure, recovery, and invariants

Important guarantees are owner/file-kind matching, no cross-owner write, deterministic retry identity, typed replay, and all-or-rejected grouped publication where the operation defines a group. An interrupted or uncertain publication is reconciled from owner records and stable operation ids; callers must not infer success from a prepared value alone. Verified copy/reconciliation APIs preserve source owners and reopen the result before reporting verification.

REL dependency declarations are owner-local, REL-only metadata. The writer checks the expected generation, appends a canonical snapshot and syncs it. Reopen validates the record sequence independently of the mounted graph. Owner-session also exposes the stored Ego synthesis when its Memory version matches the current one.

Ordinary 2.1 owner admission replays retained publications and verifies the owning clocks and references. The offline writer uses the same native codecs and reopens its staged output through that admission. Offline comparison reads through the optional `d02-observation` feature in a separate Cargo workspace. The historical FRS07 reader and its transfer formats run in a separate checkout. REL and PHY files produced by migration follow the usual grant rules.

## Code map and tests

- Public ownership surface: [`crates/reliquary-owner-session/src/lib.rs`](../../crates/reliquary-owner-session/src/lib.rs), [`session.rs`](../../crates/reliquary-owner-session/src/session.rs), and [`session/rel_writer.rs`](../../crates/reliquary-owner-session/src/session/rel_writer.rs).
- PHY admission and copy behavior: [`tests/phy_owner_admission.rs`](../../crates/reliquary-owner-session/tests/phy_owner_admission.rs), [`crates/reliquary-owner-session/src/records/verified_copy.rs`](../../crates/reliquary-owner-session/src/records/verified_copy.rs), and [`crates/reliquary-application/src/verified_copy_workflow.rs`](../../crates/reliquary-application/src/verified_copy_workflow.rs).
- Grouped writes and replay are implemented in the focused `session/` and `records/` modules under `crates/reliquary-owner-session/src/`; owner-session unit tests exercise wire validation, replay, reopen, and foreign-owner rejection.

## Extension seams

Add a session operation when the invariant is inherently owner-local: it must check current owner state, acquire/validate the owner writer, or commit a set of related store records. Keep semantic analysis and user-facing orchestration above this crate. Expose immutable value types deliberately; never expose mutable store handles or low-level codec construction for convenience.

## Related docs

- [Storage and durable state](storage.md)
- [Application workflows](application.md)
- [Gateway and host lifecycle](gateway.md)

## Notes

Exact method signatures and per-operation retry semantics belong to the public API and workflow references.
