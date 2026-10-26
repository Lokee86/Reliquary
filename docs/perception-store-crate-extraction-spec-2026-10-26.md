# Perception Store crate extraction — implementation specification

**Status:** Implemented; verification and remaining limits are recorded in the [extraction evidence](perception-store-crate-extraction-evidence-2026-10-26.md).

**Repository / branch:** `C:\!bin\workspace\Reliquary-crate-first` · `refactor/crate-first-restart` · committed baseline `2519c3b`.

**Target:** `crates/reliquary-perception-store/`; **sixth concrete owner extraction**, after Graph.

**Scope:** Correction-only, hard-cut durable-owner separation. The Perception *service* is a later, independent extraction. A pre-existing, unstaged modification to `docs/graph-crate-extraction-evidence-2026-10-26.md` is **not part of this change**.

## Purpose

Extract the existing three durable Perception stores into one compiled,
Container-only owner package while preserving persistence and product admission.

## Overview

Sections 1–8 define the approved boundary, migration and acceptance requirements.
The implementation additionally permits the explicitly approved owned return
value from `Cva::entity_resolution` and `Phylactery::entity_resolution`; their
Memory Core mention-key inputs and record contents remain supported.

## 1. Problem and target

The 138 Rust files under `src/perception/` combine three durable stores with policy, inference, candidate search, principal provisioning, Graph association, and REL/PHY product facades. The live Entity, mention-resolution, and Relationship stores still depend on root-local `MemoryStore`, `EntityStore`, `MemoryError`, `owner_lifecycle` and some `Cva`/`Phylactery` helpers. That prevents low-level owner replay without dragging the Perception decision service (and higher semantic dependencies) into rank-2 lifecycle/history code.

Create **one real, compiled `reliquary-perception-store` crate** containing exactly these existing persistent responsibilities:

- **EntityStore**: durable Entity identity, globally qualified UUID, revisions, mutations, alias/name lookup, terminal deletion/merge, historical structural retention, compact publication, replay and version accounting.
- **EntityResolutionStore**: persisted Memory-mention resolution statuses (resolved/rejected/pending/dormant), retry evidence, candidate fingerprints, mention-key revision/idempotence, wake/maintenance *state* and exact compact decode/replay.
- **RelationshipStore**: the **already implemented, transitional durable Relationship record family**, including owner-qualified Entity participants, Memory evidence references, revisions, terminal state, retention, compact publication and indexes.

These three stores share an owner package but **not one combined mutable semantic object store**. Only the existing per-store authority changes location; the service still decides what to propose, reconcile, merge, wake, or infer. No Observation synthesis is added. The existing Relationship family must not be silently rewritten into the future Observation model.

**Required Cargo boundary:** `reliquary-perception-store` may depend on `reliquary-container` and necessary external primitive/hash dependencies, **not** `reliquary-memory-core`, `reliquary-graph`, `reliquary-archive`, `reliquary-perception`, Owner Lifecycle, the root `reliquary-memory` package, or any Runtime/History/Maintenance crate.

## 2. Governing decisions and already accepted layout

| Authority | Binding outcome |
| --- | --- |
| [Crate-first layout](crate-first-layout-spec-2026-10-25.md), [logical component DAG](architecture/component-dag.json), [physical crate map](architecture/crate-map.json) | Perception Store is rank 2, with Container as its only lower Reliquary dependency. Owner Lifecycle, History and Retrieval must read storage without importing the rank-7 Perception decision service. Extract the store **before** attempting the Perception service cut. |
| [ADR 0001](decisions/0001-purpose-built-database-ownership.md), [scope ownership](subsystems/scope-ownership.md) | REL and PHY remain independent physical owner sessions. Bare Entity/Relationship identities do not authorize cross-owner reads. Perception Store is not a second Container or an authorization layer. |
| [ADR 0033](decisions/0033-perception-entities-observations-and-ambiguity.md), [Perception](subsystems/perception.md), [resolution state](entity-resolution-state.md) | Entity is a durable referent, not a Memory proposition. Insomnia records immutable Memory mention evidence; Perception resolves/admits Entities and manages unresolved/dormant evidence. Observation inference remains future work. |
| [ADR 0034](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md), [ADR 0036](decisions/0036-typed-semantic-graph-endpoints.md) | Owner-qualified Entity participants and Memory evidence may be represented inside an accepted Relationship, but Perception Store does **not** turn cross-owner participant references into Graph edges or grant authority to read attached owners. Typed Graph topology remains in `reliquary-graph`. |
| [ADR 0040](decisions/0040-terminal-tombstones-and-import-epochs.md), [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md), [ADR 0042](decisions/0042-minimal-lossless-current-history.md) | Tombstone terminality, exact historical structural facts, current compact record validation, original global/local coordinates and authenticated prune proof are preserved. Prune authorization and historical rewriting remain with Maintenance/History. |
| [Storage format](storage-format.md), [behavioral contracts](behavioral-contracts.md), [Graph extraction evidence](graph-crate-extraction-evidence-2026-10-26.md) | Preserve existing literal wire contracts and admission order. Graph endpoint checks use owner-local Entity facts but the Graph crate does not depend on Perception Store. No new runtime format branch or compatibility adapter. |

**Recovery boundary:** The preserved abandoned dependency-convergence work is a source of **specific correctness fixes** only. Consult its Entity owner/history/reconciliation changes when directly relevant, not its graph call census, ratchets, generated fixtures, or Phase 0–7 ledgers. Canonical promoted `CVAFRS07` semantic Freshness/Import acceptance is a separate whole-product gate, not a Perception Store codec redesign.

## 3. Source crosswalk — move once, not a folder copy

The placeholder `crates/reliquary-perception-store/Cargo.toml` and `src/lib.rs` currently have **no real implementation**. Fill them with the existing code; do not count an empty compiled package as acceptance.

**A. Move the durable Entity owner**, adjusted for imports and the owner boundary:

`entity_model.rs`, `entity_error.rs`, `entity_codec.rs`, `entity_publication_codec.rs`, `entity_compact_{codec,contract,values,apply,publish}.rs`, `entity_rebuild.rs`, `entity_replay.rs`, `entity_store.rs`, `entity_store_{read,validation}.rs`, `entity_retained_structure.rs`, `entity_description_{proof,prune}.rs` and the genuine lower-level helpers they directly use.

Entity ID, global ID, record/data/retention definitions and immutable lookup projections belong here. Preserve the existing SHA-256 mutation identity, canonical/alias indexing, optional global UUID and exact terminal merge/replacement evidence. Separate root's owner profile/principal *workflow* from lower-level Entity value validation.

**B. Move the durable resolution owner:**

`entity_resolution_{model,codec,delta_codec,retained}.rs`, `entity_resolution_store.rs`, `entity_resolution_store_{index,lifecycle,write}.rs` and any exclusively used current-record helpers. Keep valid status state transitions, index maintenance, exact accepted revisions, candidate fingerprints, terminal decisions, deterministic retry facts, dormant/rejected maintenance facts and retention/replay semantics.

`MemoryEntityMentionKey` is currently defined in **Memory Core** (`memory_id`, `field`, `start_byte`, `end_byte`). It is *foreign* to this store. Use a **single store-local exact-wire mention key/value** with the same four fields, explicit byte/enum tags and a narrow product-boundary conversion from Memory Core's mention key. Do not make Perception Store depend on Memory Core, construct a generic contracts crate, duplicate a competing persistent Memory identity, or change the encoded key. Candidate references are locally typed Entity IDs; validating that a mention actually belongs to the immutable Memory body is upper owner admission.

**C. Move the existing durable Relationship owner:**

`relationship_model.rs`, `relationship_error.rs`, `relationship_codec.rs`, `relationship_publication_codec.rs`, `relationship_compact_{codec,contract,values,apply,publish}.rs`, `relationship_rebuild.rs`, `relationship_replay.rs`, `relationship_store.rs`, `relationship_store_{read,validation}.rs`, `relationship_retained_structure.rs`, `relationship_description_{proof,prune}.rs` and lower-level data-only helpers.

The current `RelationshipDraft` references `EntityRef` (owner ID + Entity ID) and `MemoryRef` (owner-qualified Memory evidence). Keep owner-qualified **wire identities** and order exactly; represent foreign Memory references as primitive stable IDs/owner strings within this crate, converting from public typed `MemoryRef` at the root. No Memory Core dependency, no accidental merging with Graph's `SemanticGraphRelation`, and no implementation of planned Observation publication.

**D. Remain above the low-level store:**

- `entity_{admission,candidates,candidate_rank,candidate_routing,identity_guard,merge,audit,audit_collisions,creation_guard*,reconciliation*,resolver*}.rs` when they decide policy, compare candidates or invoke configured inference.
- `entity_resolution_{batch,commit,decision,engine,evaluate,evidence,owner,prepare,prepared,processor,processor_support,tests*,wake_tests}.rs` where they coordinate Memory/Entity/Graph, schedule work, or author resolution decisions. Distinguish data-only types consumed by the store from host/model-dependent policy code; inspect individual modules rather than moving purely by filename.
- `entity_owner.rs`, `relationship_owner.rs`, `entity_resolution_owner.rs` implementing `Cva`/`Phylactery`; higher-level principal orchestration in `entity_principal.rs` (e.g. `resolve_principal_entity_mention`, `sync_principal_profile`) and Graph association/Memory provenance.
- `entity_reconciliation_commit`, batch retarget/merge orchestration, cross-owner relation publication, Import/History/Maintenance and runtime host adapters.
- Relationship semantic synthesis, Observation/receptor machinery and any new policy remain out of scope entirely.

**Principal identity split:** `entity_principal.rs` currently mixes deterministic `principal_entity_id` / global UUID derivation and the product profile/mention workflow. Move only the **pure deterministic identity function/constant needed by EntityStore's validation** to store-local code, preserving existing identity/hash outputs. Keep REL-only principal provisioning, PHY rejection, user-profile synchronization, Graph edge publication and owner authorization above.

## 4. Concrete dependency inversions and final API

| Current upward coupling | Required hard cut |
| --- | --- |
| `EntityStore` stores `owner_lifecycle::TombstoneCoordinates` and returns `TombstoneOutcome`/payload unavailability types | Store owns immutable **Entity-local terminal coordinate and payload-availability facts**. Root maps them to supported `TombstoneOutcome`, `HistoricalPayload` and error surfaces. No root/Owner Lifecycle import from the crate. |
| `EntityResolutionStore` `use crate::memory_store::MemoryStore` and `entity_store::EntityStore` in write/compact/validation methods | Store accepts **prevalidated mention/evidence facts and Entity ID sets** under existing product owner authorization. Higher caller validates immutable Memory mention/body identity, Memory active/historical existence and referenced Entity existence with current vs historical semantics. Store checks intrinsic resolution status/transition, normalized candidates, fingerprint/revision, retry identity and persisted shapes; no raw MemoryStore or product facade parameter. |
| Resolution store uses root `MemoryError` for `CVAERS03` failures | Define a concrete store-local resolution error (or typed store error) and preserve outward `MemoryError` mapping and exact prior rejection precedence at root; no root error dependency in store. |
| `RelationshipStore::publish`/`validate_local_references` take MemoryStore, EntityStore, owner ID | Owner/product validates owner-qualified participant/evidence authority, entity/principal restrictions, current/historical evidence existence and exact owner-local references **before** calling the store's normalized publication. Store verifies field structure, participant cardinality/roles, revision/mutation/version, reference shape and durable indexing. Do not simply remove validation or pass untrusted identity bytes as proof. |
| Relationship, Entity and resolution records carry root type aliases and lifecycle errors | Use canonical Entity/Relationship owned IDs in store; foreign Memory and mention references remain **byte-exact store values**. Keep existing strongly typed public `EntityId`, `RelationshipId`, `MemoryEntityMentionKey`, `MemoryRef`, `EntityRef` and `Cva`/`Phylactery` signatures at **one root product boundary**, with explicit non-persistent conversions only where required. No second writer/reader, dual backend, or generic compatibility layer. |
| `entity_owner.rs` tombstones change Graph projection; merge/reconciliation retarget Graph and mention resolutions | Storage returns accepted results; root/Perception policy coordinates Graph exclusion and multi-store retargeting with unchanged failure sequence. Do not mutate Graph from Perception Store, and do not claim cross-owner all-or-nothing behavior beyond existing contracts. |
| `entity_description_prune.rs`/`relationship_description_prune.rs`, retained-source codecs and historical readers | Store validates exact current compact values, authenticated retained structural facts and payload availability. **Maintenance** authorizes and performs destructive pruning/rewrite, and **History** composes read-only owner cuts. Ordinary `open` remains byte-neutral. |
| REL/PHY Owner Lifecycle currently opens three stores in root and validates after replay | Use the compiled `EntityOpenState`, `EntityResolutionStore`, `RelationshipOpenState` in original physical frame order, then coordinate cross-store validation after all relevant owner projections exist. Preserve format/clock/expected-ID and error precedence; no auxiliary replay pass copying the entire accumulated store per frame. |

**Export only narrow real operations:** construct/open/ingest/finish and publication/read APIs for `EntityStore`, `EntityResolutionStore`, `RelationshipStore`; their typed IDs/data/errors/status values; retained-revision/terminal facts and exact immutable reference views needed by root/History/Retrieval. Internal records, mutation indexes, codecs and proof parser helpers remain private unless a concrete owner consumer must use them. Root product supported re-exports are an **external API boundary**, not a second implementation.

## 5. Frozen persistence and behavior

- **Entity:** `CVAENTF1`, Entity record / ID and global UUID rules, compact `CVAENC02` and `CVAEND02` bodies, `CVAETD02` direct terminal and `CVAETM02` merge terminal inside `CVAPUB01`. Preserve current field masks, explicit optional clears, immutable identity, dense Entity revisions/version tickets and accepted legacy *read-only comparison evidence* where already required. Do not reintroduce retired physical readers.
- **Resolution:** Current `CVAERS03` statuses and literal mention coordinates, predecessor/delta reconstruction, accepted ordering, resolved/rejected terminality, dormant/pending evidence, retry time/attempt count and bounded pending-by-Entity index. `CVAERL01` retained `CVAERS02` bytes are **inert authenticated retention**, not revived old decisions; no `CVAERS02` active reader.
- **Relationship:** `CVARELF1` format, current compact `CVARNC02` / `CVARND02` and `CVARTD02` in the existing `CVAPUB01` envelope; preserve participant kind/owner string/roles and `MemoryRef` evidence, relationship-local versions, direct terminal markers, structural-history pruning and indexes. Legacy comparison forms remain comparison evidence only under the already accepted converter contract.
- **Entity/Relationship lifecycle:** a terminal entity cannot be resurrected; merge replacement retains historical reference structure and correct Graph exclusion in the *higher coordinator*. Tombstones are exact and idempotent; unchanged retry allocates no new global or owner version.
- **Global/local clock:** every standalone versioned Entity/Relationship write reserves/commits the exact current V2 envelope once. Resolution metadata uses its existing clock-neutral owner semantics. Do not add clocks for derived/scheduler state.
- **Provenance and privacy:** Memory-body mention spans must match exact immutable UTF-8 boundaries; current and historical Entity/Memory reference checks use the right owner and grants. Bare cross-owner references are identifiers **not** permission to fetch data. PHY principal workflows retain current exclusions.
- **History:** retained revision proof, exact source revision, structural coordinates and unsupported tombstone/prune evidence fail closed; incomplete physical tail repair remains Container + higher-authority maintenance work.
- **Performance:** avoid full-store cloning for each accepted grouped resolution or compact batch; use existing indexes, preflight and staged mutation. Do not introduce an O(total-history) replay scan per event or a second independent reconciliation data graph.

No wire format alteration, index migration, new API feature, new synthesis policy or compatibility shim is authorized.

## 6. Implementation strategy — bounded hard cut

1. **Inventory/freeze:** verify committed HEAD and current dirty Graph evidence file; inspect actual store/codec consumers and selected abandoned owner fixes with `rg`, current source and Cargo. Enumerate genuinely store-local tests versus REL/PHY and model/service tests. Do **not** modify or stage the existing Graph document.
2. **Extract Entity foundation:** move canonical Entity values, codecs, replay, storage and description/terminal facts directly into real `reliquary-perception-store`. Cut root lifecycle type and principal product workflow dependencies. Independently compile with Container; keep outward product error mapping in root.
3. **Extract Relationship and resolution owners:** keep existing separate store state and present wire contracts. Introduce minimal primitive foreign-ID/mention boundary; split cross-owner Memory/Entity validation into existing root/Owner Lifecycle callers while preserving error ordering and idempotent failed-write behavior. Do **not** move full Perception inference, reconciliation or scheduling at this stage.
4. **Migrate consumers once:** wire `Cva`, `Phylactery`, Owner Lifecycle, History, Import/reconcile, Graph/Perception call sites and native CLI through the canonical compiled stores. Cross-owner proof remains in product root. Update current code maps, specs and actual Cargo manifest dependencies after code moves.
5. **Delete displaced owner modules:** remove old `src/lib.rs` `#[path]` declarations and source duplicates; retain the policy/owner composition modules in root until their designated service or Lifecycle extraction. Do not create internal forwarding wrappers to keep a transient build green. A bounded red migration window is permitted until the final workspace gate.

This is **one** library extraction covering three tightly adjacent persistent owners, not a new three-phase semantic rewrite or a justification for creating three more crates.

## 7. Existing tests and acceptance gates

Move/split **owner-local** tests first: `entity_compact_{apply,codec,contract}_tests`, `entity_publication_codec_tests`, `entity_rebuild_overflow_tests`, `entity_replay_tests`, `entity_retained_structure_tests`, `entity_resolution_delta_tests`, store-local resolution status/retry/retention tests, `relationship_compact_{apply,codec,contract}_tests`, `relationship_publication_codec_tests`, `relationship_rebuild_overflow_tests`, `relationship_retained_structure_tests`. Split writer tests only if they really drive `Cva` or external owner coordination. **Reuse** existing suites; add at most focused tests for new explicit foreign-evidence boundaries.

Keep **multi-owner/service tests in root**: `entity_creation_guard_tests`, `entity_principal_tests`, `entity_resolution_{batch,processor,owner,reconcile,wake}_tests`, `entity_lifecycle_tests`, `relationship_lifecycle_tests`, `relationship_maintenance_tests`, `entity_import_evidence_tests`, `entity_publication_{copy,prune,replay}_tests`, `relationship_publication_{copy,prune,replay}_tests`, `entity_reconciliation_tests`, Graph association/owner exclusion, REL/PHY historical views, privacy/attached-owner and CLI import.

**Behavioral acceptance:**
1. Bit- and field-equivalent current `CVAENC02`/`CVAEND02`/`CVAETD02`/`CVAETM02` and `CVARNC02`/`CVARND02`/`CVARTD02` publication, `CVAERS03` deterministic status replay, checksum/truncation/unknown-version rejection and exact owner clocks.
2. Entity stable IDs, canonical aliases, principal deterministic IDs/global UUID, global-ID backfill and merge terminal state; identical mutation retry is idempotent, conflicting mutation/revision rejects without publishing state.
3. Mention-key field/span literal identity roundtrips between Memory Core and Perception Store; invalid body binding/UTF-8 source mention rejected at caller before publishing; resolved/rejected/pending/dormant statuses retain exact evidence, TTL behavior and wake indexes across reopen.
4. Relationship cross-owner participant/evidence owner qualification, local grant checks, active/historical Entity and Memory references, terminal retention, import/reconcile structural facts and failure ordering.
5. REL and PHY open/create/reopen, no implicit repair, historical cuts, terminal tombstone/discard guards and no Graph/Memory/Relationship state diverges as a side effect of a failed validated publication.
6. No unexpected full-history replay copy, duplicated store, hidden root back-edge, runtime inference path or new `CVAERS02` active decoder.

**Commands:** `cargo check -p reliquary-perception-store --locked --offline`, `cargo test -p reliquary-perception-store --locked --offline`, focused root `entity` / `relationship` / `resolution` / REL-PHY/import/history/perception tests, `cargo check --workspace --locked --offline`, `cargo fmt --all -- --check`, and `cargo test --manifest-path cli/Cargo.toml --locked --offline` using the separate CLI lockfile. Inspect Cargo, `rg` and actual source for stale root owner implementations and upward imports. Report known expensive historical/test fixtures as skipped or failed, not silently passed.

**Final stage gate:** all real store/codec/replay code lives in the compiled Container-only Perception Store; the root uses it; error/version/provenance behavior passes scoped integration; public `EntityId`/`RelationshipId` and mention API contract remains supported; no duplicate persistent implementation. Do not claim full product acceptance or promoted `CVAFRS07` corpus replay from this stage.

## 8. Explicit exclusions

No new Entity classifier, candidate calibration, Perception service extraction, Observation synthesis, Graph relationship redesign, role semantics redesign, root Owner Lifecycle crate extraction, import-settlement/Freshness recovery, provider-model change, new cross-owner access policy, new storage schema, generic ports/contracts crate, compiler graph proof framework, or production benchmark campaign.

**Completion means a real Perception Store and direct consumers—not an empty placeholder, not service-policy migration, and not a new roadmap feature.**

## Related docs

- [Architecture](architecture.md) records current compiled owner boundaries.
- [Maintainer map](maintainer-map.md) locates storage and product consumers.
- [Rust API](api.md) records the approved owned resolution lookup result.

## Notes

The owned lookup result is a product-boundary conversion, not an additional
persistent store or synchronized projection cache. All frozen wire requirements
and the separate promoted `CVAFRS07` acceptance boundary still apply.
