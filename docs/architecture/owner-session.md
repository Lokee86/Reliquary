# Owner sessions

Parent index: [Reliquary architecture](./INDEX.md)

## Purpose

Describe the owner-session boundary that composes durable stores and controls authorized mutation for one REL or PHY file.

## Overview

`reliquary-owner-session` owns concrete REL and PHY sessions, their typed store handles, owner authorization inputs, replayed projections, writer leases, and multi-record publication methods. A session is bound to one file identity and owner kind. `RelSession` and `PhySession` provide reads; `RelWriter` and `PhyWriter` provide controlled mutation. Higher layers receive session operations and selected immutable values, never a `Container` or a bag of writable stores.

The crate is the only product boundary that composes the eight persistent domain owners listed in [storage](storage.md). It does not own Application policy such as how to rank candidates or which user workflow should run.

## State, lifecycle, and concurrency

`RelWriter::create_current_path(path, owner_id)` initializes a new current REL with the neutral 2.1 scope value. Existing RELs expose their stored scope on open. Maintenance that preserves owner identity uses the source's container identity.

Opening a session checks the physical file identity/kind, opens the relevant stores, and replays their records into owner-scoped views. A trusted path mount checks an expected owner ID at the leased header, before physical or semantic replay. The replay passes borrow each frame from a reusable payload buffer. The stores decode and retain the records they own, and the replay checks validate each publication in physical order. A writer obtains an owner-local lease/fence before publication. Prepared work is rechecked against current owner revisions and authorization at commit; a stale or foreign cut is rejected. Grouped commit APIs coordinate related records under one owner writer so readers do not see a higher-level success without its required receipt/history records.

Ordinary REL writer admission and scoped local reads look for a private checkpoint. It captures the typed projections and history locators at a verified file boundary. Core checks the header, physical file identity, clock, and covered digest on the original lease. A verified checkpoint can continue native clock reservations, Archive turn and branch publications, REL dependency changes, the supported Memory publications and grouped user mutations, and Entity separation confirmations and withdrawals. Other suffixes take full native admission on the same lease. Full audits, maintenance sources, PHY opens, and detached REL snapshots replay their owners from the start.

During cold admission each store selects the physical records it can interpret. Archive validates turn bodies once for its own projection. A grouped Insomnia completion or an external Memory intent also needs the preceding Archive activity to verify its position. Owner-session checks for those records before requesting that replay. Import settlements still hash their complete source prefix, and graph and Dream history validators can skip unrelated Archive and packed vector payloads. See [physical replay](storage.md) for the lease and clock rules.

Archive text and attachments, Memory bodies, and packed matrices can be read through digest-checked positions on the leased REL. A requested historical cut selects its retained revision metadata before reading its body. A row lookup reads only the relevant packed pages. An invalid position or changed payload fails the read and fences the writer. The initial checkpoint follows full writer admission. Later refreshes use the existing synchronized writer boundary and the accepted cut. Commit receipts come from the durable REL, and checkpoint publication is best effort. On Windows, the cache directory and staged checkpoint files are created with the current user's account as the owner. Checkpoint admission verifies the account owner and private access list. Windows rebuilds derived checkpoints in a new cache directory to recover from older caches created with a group owner.

An open native import with an accepted source receipt authorizes Archive vector publication for the owner cut.
The import writer validates the epoch before preparing the input and again at publication. It uses the same
fragment definitions, packed matrix, and activation records as ordinary Archive writes. The native import's
semantic plan and settlement are independent of the Archive generation. The same publication path prepares eight-turn windows with two-turn overlap for ordinary and native import Archive vectors. A window above 12,000 UTF-8 bytes yields one vector per passage. The accepted passage range is used for both lexical and semantic ranking. Archive embedding documents and configured Archive queries pass through local redaction before inference. Recognizers mask identifiers such as email addresses, phone numbers, government IDs, payment and bank numbers, and network addresses. They also mask contextual names, addresses and birth dates, and credentials such as API keys, authorization tokens, passwords, private keys, cookies, and connection credentials. Detection runs on the original turn before the embedding passage is clipped. Citations and lexical evidence use the original text.

An Archive vector search captures the active generation and shared references to its immutable packed matrix and fragment mapping. The search view keeps those values alive when the owner advances or closes. The generation binds the source Archive cut and the accepted activation record. Hybrid capture pins readable text and source coordinates under the same owner read. It checks the persisted index against the digest of the readable source range. The first lexical lookup builds from captured text when there is no matching record. A temporal search also captures the readable ancestor path of each fragment with its original turn timestamps. It uses the active generation's accepted fragment coordinates. The ordinary cached lexical view does not retain duplicate turn payloads. Explicit index publication prepares the same index outside the writer lock, then rechecks the file, active generation and write fence. The writer appends and syncs the sealed frame before recording publication. A retry returns the existing publication with zero added bytes. A new generation drops the session's reference to the previous index, and captured views can finish with their original index and packed rows.

REL and PHY writers are separate authorities. Session clones do not create independent authority to bypass the writer boundary. Runtime-level synchronization and leases serialize mutation through mounted Gateway writers; persistence is still recovered from the file's append/replay state after process restart.

Community recompute captures eligible IDs and active structural edges from one REL read. Capture and publication share the same active-Memory check. The writer rechecks the captured file identity and versions, requires every eligible ID exactly once, and syncs the Community frame through the owner write boundary.

REL owner-session also owns the directional Entity identity receipt projection and typed publication. A derived, lazily built index maps an Entity to its resolved mention keys. It includes typed resolutions retained during migration. A subsequent decision in the REL takes precedence for the same key, including withdrawal or reassignment. Opaque retained entries have no verified span and supply no excerpt. Capture checks the active Memory revision and direct source and records the field, exact mention position and surrounding excerpt span. The receipt stores those references and any grounded Relationship references in the recording REL. On historical inspection each owner verifies its retained mention resolution, Memory revision and body before returning the excerpt. Gateway checks access to both RELs and coordinates their sessions.

For automatic Entity identity discovery, REL owner-session projects seekable pages of active non-principal Entities in Entity ID order. It folds case and whitespace in complete names and aliases and hashes them separately from structured identifiers found in the bounded, directly sourced resolved mention excerpts. Serial codes and reverse-domain package names preserve their separators and match across case variations. The matcher skips ordinary words, common date strings and version forms. Gateway compares the keys within the current authorized mount context and refreshes them when the evidence cut changes. A shared key only nominates a pair for the existing evidence comparison. Receipt publication leaves the evidence cut unchanged.

## Failure, recovery, and invariants

Important guarantees are owner/file-kind matching, no cross-owner write, deterministic retry identity, typed replay, and all-or-rejected grouped publication where the operation defines a group. An interrupted or uncertain publication is reconciled from owner records and stable operation ids; callers must not infer success from a prepared value alone. Verified copy/reconciliation APIs preserve source owners and reopen the result before reporting verification.

Cold replay validates Entity separation confirmations against the recording REL's publication prefix without opening the peer. Existing 2.1 files start with an empty confirmation history. An owner-session historical proof includes confirmations and withdrawals through their own cut and verifies that the cut includes the Memory, Perception and comparison-receipt state present at publication.

Exact copies, exact-prefix reconciliation and verified maintenance transforms retain the audit chain. Entity description pruning preserves the digest of any reviewed description. Fresh-history rewrites and divergent reconciliation refuse sources with confirmation records before creating a destination.

REL dependency declarations are owner-local, REL-only metadata. The writer checks the expected generation, appends a canonical snapshot and syncs it. Reopen validates the record sequence independently of the mounted graph. Owner-session also exposes the stored Ego synthesis when its Memory version matches the current one.

Ordinary 2.1 owner admission replays retained publications and verifies the owning clocks and references. The offline writer uses the same native codecs and reopens its staged output through that admission. Offline comparison reads through the optional `d02-observation` feature in a separate Cargo workspace. The historical FRS07 reader and its transfer formats run in a separate checkout. REL and PHY files produced by migration follow the usual grant rules.

## Code map and tests

- Public ownership surface: [`crates/reliquary-owner-session/src/lib.rs`](../../crates/reliquary-owner-session/src/lib.rs), [`session.rs`](../../crates/reliquary-owner-session/src/session.rs), and [`session/rel_writer.rs`](../../crates/reliquary-owner-session/src/session/rel_writer.rs).
- PHY admission and copy behavior: [`tests/phy_owner_admission.rs`](../../crates/reliquary-owner-session/tests/phy_owner_admission.rs), [`crates/reliquary-owner-session/src/records/verified_copy.rs`](../../crates/reliquary-owner-session/src/records/verified_copy.rs), and [`crates/reliquary-application/src/verified_copy_workflow.rs`](../../crates/reliquary-application/src/verified_copy_workflow.rs).
- Grouped writes and replay are implemented in the focused `session/` and `records/` modules under `crates/reliquary-owner-session/src/`; owner-session unit tests exercise wire validation, replay, reopen, and foreign-owner rejection.
- [Checkpoint integration](../../crates/reliquary-owner-session/src/session/checkpoint.rs) binds the physical cut, handles derived cache publication, and delegates suffix replay to the owning validators. The [checkpoint test](../../crates/reliquary-owner-session/src/session/checkpoint/tests.rs) compares grouped suffix admission and historical reads with full verification.

## Extension seams

Add a session operation when the invariant is inherently owner-local: it must check current owner state, acquire/validate the owner writer, or commit a set of related store records. Keep semantic analysis and user-facing orchestration above this crate. Expose immutable value types deliberately; never expose mutable store handles or low-level codec construction for convenience.

## Related docs

- [Reliquary source and binary distribution terms](../../README.md#license)
- [Storage and durable state](storage.md)
- [Application workflows](application.md)
- [Gateway and host lifecycle](gateway.md)

## Notes

Exact method signatures and per-operation retry semantics belong to the public API and workflow references.
