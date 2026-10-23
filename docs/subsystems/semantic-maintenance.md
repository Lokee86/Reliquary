# Semantic maintenance and history operations

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe implemented tombstone, import-settlement, pruning, compatibility-profile, rewrite, repair, and history-maintenance behavior without retaining completed execution plans as current documentation.

## Overview

Reliquary separates semantic lifecycle from physical reclamation.

Terminal lifecycle records make current objects unavailable without erasing historical evidence. Explicit maintenance operations can later produce validated transformed copies, prune authorized historical payload, retire profiles, reclaim unreachable bytes, or rewrite selected current/history state under a declared policy.

## Terminal lifecycle

Memory, Entity, Relationship, Archive conversation, and selected Ego domains have terminal lifecycle semantics under their owning stores/clocks.

A terminal identity cannot be silently resurrected by later publication. Exact deletion retries are idempotent under the owner's retry contract. Current projections exclude terminal objects while explicit historical readers can retain prior revisions/evidence until authorized pruning.

Independent semantic owners are not cascade-deleted merely because their source/provenance object became terminal.

## Native import epochs and settlements

Native imports use durable ImportStart/ImportComplete epoch metadata plus Archive receipts. Import completion is coordinator-owned and derives readiness from retained owner evidence rather than caller-provided proof.

Freshness import settlements are ordered, sparse, and scoped to accepted semantic evidence. Current scoped imports use semantic settlement identity rather than durable physical-offset authority.

Multiple sequential imports across live activity and reopen are supported. Zero-result epochs can settle without inventing Memory admission or score effects.

Concrete REL/PHY prune, reclaim, tail repair, EntityResolution repair and compaction operations live under `src/maintenance/`. Public facade entrypoints gather owner borrows and dispatch. Domain stores retain local retention/publication authority; immutable History helpers validate accepted evidence and lifecycle owns physical reconstruction and frozen-copy mechanics.

## Pruning and reclaim

Pruning is explicit and owner-specific. Domain owners prove retention eligibility before a transformed copy omits historical payload.

Reclaim is a separate conservative operation that removes only proven unreachable physical garbage. Tombstoning alone does not authorize payload destruction.

REL pruning may include Archive conversation history where explicitly selected. PHY pruning operates only on PHY-owned domains.

## Compatibility profiles and vectors

Maintenance can select retained active profiles, verify completeness for current owner data, retire unselected profiles, and optionally prune retired vector payload in a transformed copy.

Profile retirement is semantic owner state, distinct from deleting arbitrary historical bytes.

## Rewrite

The rewrite API separates planning from execution. Its canonical implementation is `src/maintenance/rewrite/`; the supported public module only reexports it. Immutable planning views exclude physical writers, and emission uses concrete lifecycle owners and owner-local publishers. A `RewritePlan` binds source identity/head and policy digest; execution revalidates that source before creating a destination and refuses an already-existing destination.

Policy controls owner identity/generation, selected Memories, embedding profiles, Freshness treatment, Archive selection/provenance for REL, and permitted Ego state. PreserveOwner history rollover records explicit predecessor lineage; incompatible history generations fail closed during reconciliation.

Rewrite is not an implicit migration reader and does not preserve obsolete format support merely because old bytes existed in the source.

## Development-format hard cut

During development, format transitions use temporary offline conversion against retained corpora, exact equivalence verification, canonical corpus promotion, and deletion of temporary compatibility readers/tooling. Runtime readers/writers target only the current format unless a separately justified compatibility contract says otherwise.

## Failure and recovery

- Maintenance writes a separate destination/copy rather than mutating authoritative input in place unless the operation explicitly owns an atomic replacement protocol.
- Source identity/head/policy proofs are revalidated before publication.
- Unsupported owner payload or cross-owner authority blocks a transform.
- Reconciliation rejects incompatible history-generation authority.
- Import completion refuses unresolved owner work/evidence.
- Reclaim never substitutes for semantic pruning authorization.


## Maintenance proof storage and composition

`src/maintenance/store.rs` and `manifest.rs` own accepted prune manifests, canonical encoding, duplicate refusal and immutable proof reads. They contain no Graph lookup or source-history coordinator. `history/source_digest.rs` owns the canonical source-history digest algorithm and compares those owner-local manifests with accepted Graph/Container evidence before mount or transformation.

Memory produces its own retention proof and unavailable-body markers. `history/memory_retention.rs` matches those facts against maintenance manifests above both owners, preserving the original target, tombstone, structure and payload digests. Repair, prune/rewrite eligibility policy and transformed-copy publication remain explicit existing higher-level operations.

Archive likewise produces an Archive-owned conversation retention proof and unavailable-content markers. `history/archive_retention.rs` binds those facts to Maintenance manifests and checks that no retained Archive record still requires a selected payload. Maintenance manifest representation remains owner-neutral evidence. History compares that evidence with retained source history during open using immutable lifecycle access; this comparison does not authorize a new prune. PHY prune publication and PHY reclamation now dispatch through concrete Maintenance operations; REL and the remaining entrypoints are still converging. `history/phylactery_prune.rs` owns read-only descriptor reconstruction and accepted evidence comparison. `maintenance/reclamation.rs` and `reclamation_scan.rs` own liveness selection over immutable source evidence. `owner_lifecycle/reclamation_source.rs` holds concrete REL/PHY sources privately and owns the physical transformed copy, using `owner_lifecycle/relocation.rs`. Frozen sources reject physical growth both before policy reads and immediately before copying. No format changes or new semantic policy accompany this extraction.

Explicit retired-Entity reference repair and its read-only inspection are implemented by `maintenance/entity_resolution.rs`. PHY incomplete-tail repair uses `maintenance/tail_repair.rs`, retaining the exclusive repair lease and staged accepted-prefix validation before truncation. REL tail repair remains above the unfinished REL reconstruction cut. These moves do not enable repair on ordinary open.

## Code Map

- `src/owner_lifecycle.rs`
- `src/imports/**`
- `src/rewrite.rs`
- `src/rewrite_lineage.rs`
- `src/rewrite/**`
- `src/storage_upgrade/**`
- `src/facade/*prune*`
- `src/maintenance/**`
- `src/history/source_digest.rs`
- `src/history/archive_retention.rs`
- `src/history/memory_retention.rs`

## Related docs

- [Architecture](../architecture.md#semantic-maintenance-and-history-rewrite)
- [Storage format](../storage-format.md)
- [Native import](../native-import.md)
- [Tombstone/import final verification](../tombstone-import-history-final-verification-2026-10-18.md)
- [Semantic import/rewrite execution evidence](../semantic-import-pruning-history-rewrite-evidence-2026-10-18.md)
- [ADR 0040](../decisions/0040-terminal-tombstones-and-import-epochs.md)
- [ADR 0041](../decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md)

## Notes

Completed tombstone/import/pruning/rewrite execution plans are retired. Verification artifacts remain as evidence; current behavior lives here and in the exact API/storage documents.


Generic divergent reconciliation is implemented in `src/maintenance/reconciliation_repack.rs`; common-prefix comparison is in `reconciliation_compare.rs`, physical-copy authority checks in `reconciliation_copy.rs`, and Freshness merge/turn policy in `freshness_reconciliation.rs`. Producer-owned ordered replay and explicit lifecycle admission preserve existing conflict/error values, transaction timestamps and terminal-state rules. Import-owned epoch continuation and historical reconstruction policy consume immutable History facts and lower replay/copy primitives; public Cva modules are API adapters. On failure, cleanup checks the exact created destination identity before removing a path. Public reconcile/promote adapters retain source leases and no-overwrite/atomic-promotion behavior. The Phase 3 ledger still governs final acceptance and promotion gates.
