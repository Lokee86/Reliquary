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

## Pruning and reclaim

Pruning is explicit and owner-specific. Domain owners prove retention eligibility before a transformed copy omits historical payload.

Reclaim is a separate conservative operation that removes only proven unreachable physical garbage. Tombstoning alone does not authorize payload destruction.

REL pruning may include Archive conversation history where explicitly selected. PHY pruning operates only on PHY-owned domains.

## Compatibility profiles and vectors

Maintenance can select retained active profiles, verify completeness for current owner data, retire unselected profiles, and optionally prune retired vector payload in a transformed copy.

Profile retirement is semantic owner state, distinct from deleting arbitrary historical bytes.

## Rewrite

The rewrite API separates planning from execution. A `RewritePlan` binds source identity/head and policy digest; execution revalidates that source before creating a destination and refuses an already-existing destination.

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

`src/maintenance/store.rs` and `manifest.rs` own accepted prune manifests, canonical encoding, duplicate refusal and immutable proof reads. They contain no Graph lookup or source-history coordinator. `facade/owner_history_proof.rs` retains the existing source-history digest algorithm and compares those owner-local manifests with accepted Graph/Container evidence before mount or transformation.

Memory produces its own retention proof and unavailable-body markers. `facade/memory_prune_admission.rs` matches those facts against maintenance manifests above both owners, preserving the original target, tombstone, structure and payload digests. Repair, prune/rewrite eligibility policy and transformed-copy publication remain explicit existing higher-level operations.

Archive likewise produces an Archive-owned conversation retention proof and unavailable-content markers. `facade/archive_prune_admission.rs` binds those facts to Maintenance manifests and checks that no retained Archive record still requires a selected payload. Maintenance manifest representation and source-history coordination remain above both payload owners.

## Code Map

- `src/owner_lifecycle.rs`
- `src/imports/**`
- `src/rewrite.rs`
- `src/rewrite_lineage.rs`
- `src/rewrite/**`
- `src/storage_upgrade/**`
- `src/facade/*prune*`
- `src/maintenance/**`
- `src/facade/owner_history_proof.rs`
- `src/facade/archive_prune_admission.rs`
- `src/facade/memory_prune_admission.rs`

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
