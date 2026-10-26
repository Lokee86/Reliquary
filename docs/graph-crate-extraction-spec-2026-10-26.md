# Graph crate extraction — implementation specification

**Status:** Implemented; Graph and Community state live in `reliquary-graph`. Verification and existing tooling/test limitations are recorded in the [execution evidence](graph-crate-extraction-evidence-2026-10-26.md).
**Branch:** `refactor/crate-first-restart`. **Specification baseline:** `ad25bab`; **execution baseline:** `13016c4a`, where Echo was already committed and the working tree was clean. Echo implementation was preserved.
**Target:** `crates/reliquary-graph/` (fifth first-class crate extraction: Container → Archive → Memory Core → Echo → Graph).  
**Policy:** correction-only staged hard cut. No feature work, source format migration, extra architectural framework, or prerequirement to finish another domain's extraction.

## Purpose

Execute the accepted Graph and Community ownership cut without changing stored bytes or product admission.

## Overview

The staged implementation and verification contract follows.

## 1. Developer problem and outcome

The current Graph implementation resides in root `src/semantic_graph/**`, with `GraphStore` and `GraphOpenState` registered through root `src/lib.rs` and used by `Cva`/`Phylactery`. Cargo cannot enforce its accepted low-level ownership boundary because Graph code directly holds/validates `MemoryStore`, `EntityStore`, Perception principal kinds, and History rewrite types. The same source neighborhood contains real durable Community persistence, deterministic structural derivation, retrieval/vector routing, and synthetic benchmarks. Moving the entire folder blindly would create cycles and relocate work that is not Graph-owned.

**Done:** One compiled `reliquary-graph` crate owns the existing graph topology, typed relationship state, source-order/compact transaction replay, Graph versions and projection, **Community snapshot/name/lineage storage**, and deterministic structural membership calculation. Product root and higher owners consume those real implementations. No second Graph/Community owner or parallel codec remains in root. The crate has exactly **one Reliquary Cargo dependency: `reliquary-container`**; external `arcana` runtime topology/Leiden dependencies already in use are permitted where genuinely needed.

This is **Graph state**, not Dream's semantic relation decisions, Perception's Entity truth, Memory's record truth, Freshness's propagation, Retrieval's vector ranking, or Maintenance's authority to rewrite historical state.

## 2. Governing contracts and source of authority

| Existing source | Constraint |
| --- | --- |
| [Crate-first layout](crate-first-layout-spec-2026-10-25.md), [logical DAG](architecture/component-dag.json), [physical crate map](architecture/crate-map.json) | `graph-store` rank 2; `reliquary-graph` depends down only on Container. Existing accepted consumers are Perception, Freshness, Dream, Ego, Retrieval, History, Maintenance and Owner Lifecycle. Cargo boundaries enforce the actual direction, not an external graph ledger. |
| [ADR 0001](decisions/0001-purpose-built-database-ownership.md), [scope ownership](subsystems/scope-ownership.md), [storage format](storage-format.md) | One REL or PHY Container per owner. Graph/Community bodies share that physical stream and are scoped to **one** owner. They do not authorize a peer owner's Memory/Entity presence. |
| [ADR 0024](decisions/0024-owner-local-dream-processing.md), [Dream](subsystems/dream.md) | Dream decides *whether and why* to create Memory relationships, duplicates, rewiring, and derived Community work. Graph validates typed relation shape and commits exact accepted changes; it does not run classifiers, Dream scheduling, Memory lifecycle or Freshness. |
| [ADR 0025](decisions/0025-owner-local-derived-communities.md) | Community snapshots are deterministic derived Graph state, not a second relationship authority. Memory-projection watermark (not every Entity-edge change), deterministic scan/merge 2,048-node leaf shards, fixed Leiden seed/resolution, exact-membership Community ID and generation contract remain unchanged. |
| [ADR 0031](decisions/0031-dream-derived-community-semantic-names.md), [ADR 0032](decisions/0032-community-lineage-and-name-continuity.md) | Graph/Community persist current-name metadata and deterministic lineage from consecutive snapshots; **Dream authors inferred names**, user authors explicit names. Direct-name precedence, mutually unique strongest-Jaccard inheritance, ambiguous-link rejection and cumulative 0.750 Dream-name retention remain unchanged. No extra graph version/time ticket for naming. |
| [ADR 0033](decisions/0033-perception-entities-observations-and-ambiguity.md), [ADR 0036](decisions/0036-typed-semantic-graph-endpoints.md) | Graph supports typed Memory/Entity associations and historical/inert references. Perception owns Entity semantics, class/kind admission, principal definition and Observation policy. Unsupported Observation nodes stay rejected until explicitly designed elsewhere. |
| [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md), [ADR 0042](decisions/0042-minimal-lossless-current-history.md), [behavioral contracts](behavioral-contracts.md) | Preserve literal current Graph/Community codecs, ordinal bindings, compact publication grouping, global/local clocks, historical cut behavior, owner proofs and replay failure precedence. Cross-owner history validation/pruning/rewrite/reconciliation remain higher-level. |
| [Container spec](container-crate-extraction-spec-2026-10-25.md), [Memory Core spec](memory-core-crate-extraction-spec-2026-10-25.md), [Echo spec](echo-crate-extraction-spec-2026-10-26.md) | Consume only the **real** physical API; Memory is a peer, not a Graph dependency. Do not modify or accidentally stage the currently in-progress Echo extraction. |

**Pre-refactor salvage:** Inspect *specific* older Graph/Community correctness fixes in the preserved original main/backups where relevant (Graph compact publication, historical cut, semantic owner validation, rewrite baselines), not an old Phase 7 plan or massive graph/ratchet apparatus. Use source history selectively; the accepted DAG and live bytes take precedence.

**Format recovery gate:** Canonical promoted corpus requires post-baseline `CVAFRS07` semantic Freshness/Import recovery; that is **not** a Graph codec change or excuse for adding deprecated runtime formats. Graph passes its own stage on scoped current-format disposable data and validated consumers. Whole-corpus acceptance remains separate.

## 3. Move-once source layout and ownership crosswalk

Use the existing placeholder `crates/reliquary-graph/`. Keep existing descriptive module granularity rather than adding a generic architecture kit:

```text
crates/reliquary-graph/
  Cargo.toml                        # reliquary-container + truly needed external deps
  src/
    lib.rs                          # compact, intentionally public owner API
    graph_model.rs                 # Graph-local typed node and relation values
    semantic_node_model.rs         # GraphNodeRef/kind and opaque foreign source IDs
    graph_error.rs
    graph_codec*.rs                 # current source-node/version + compact txn codec
    graph_publication_codec.rs
    graph_rebuild.rs                # ordered Graph owner replay, local validation
    graph_store.rs                 # canonical GraphStore, topology/projection
    graph_store_{read,write,validation,compact}.rs
    graph_query.rs                 # graph-local adjacency/shortest path, no retrieval scoring
    graph_rewrite_baseline.rs      # exact Graph-local marker bytes/facts only
    community_{model,error,codec,store}.rs
    community_lineage.rs
    community_{leiden,scan_merge,scan_merge_*}.rs   # deterministic, graph-local derivation
    *_tests.rs                     # only genuinely Graph/Community owner-local tests
```

**Move real durable owner code:**
- `src/semantic_graph/graph_{model,error,codec,codec_mutation,compact_codec,compact_contract,publication_codec,rebuild,store,store_compact,store_read,store_write,store_validation,query,rewrite_baseline}.rs`; `semantic_node_model.rs`.
- `community_{model,error,codec,store,lineage,leiden,scan_merge,scan_merge_plan,scan_merge_reduce,scan_merge_workers}.rs`. Derivation implementations remain private to the store/algorithm; Dream controls when to request recomputation/naming.
- Keep proper source/owner-local tests with those modules; leave root `Cva`, `Phylactery`, Import, History, runtime and multi-owner tests at their current integration seam.

**Do not blindly move every file in `src/semantic_graph/`:**
- `community_routing.rs` uses `MemoryRef`, vector embeddings and retrieval-specific medoids/representatives: keep in **Retrieval/consumer** until its designated extraction. `community_subcentroid_routing.rs` uses `memory_retrieval_index` and belongs to Retrieval. Their routing tests/benchmarks and `community_{routing,retrieval,end_to_end,lexical_escape,traversal}*bench*` follow actual algorithm consumers, not Graph persistence.
- Bench fixture modules importing Dream candidate ranking, Dream owner vectors, or Memory retrieval indexes must remain with their owners (or be retired if genuinely obsolete); do not turn those libraries into Graph dependencies to preserve benchmark file locations.
- `src/facade/cva_graph.rs` combines Graph-local calls with Memory/Entity admission, principal lookup, and product error mapping. Only pure Graph behavior moves; the **higher product facade** retains cross-owner validation/authorization and externally supported typed API. `src/facade/**` historical fingerprint, reconciliation and maintenance orchestration stay above Graph.
- Graph rewrite baseline record encoding/decoding and immutable identity facts belong to Graph; the actual `RewriteGenesis`/generation authorization, semantic cut coordination and repack are **Maintenance/History**, not Graph.

### Foreign-identity dependency: design once, avoid new Cargo cycles

Current `GraphStore` and `SemanticNodeRef` refer to root `MemoryId` and `EntityId` types, while Graph's only allowed Reliquary dependency is Container. The target crate must retain **byte-identical foreign identity**, not import Memory Core or future Perception Store.

Represent Graph-owned semantic node identity canonically as `GraphNodeRef { kind: GraphNodeKind, id: [u8; 32] }` (or retain `SemanticNodeRef` with these same fields) and graph-local relation change/value types with that opaque ID shape. Local `memory([u8;32])`, `entity([u8;32])` constructors do not themselves prove the foreign record exists. At the **root product API / Owner Lifecycle** seam convert between supported public `MemoryId`/`EntityId` and Graph-local IDs **without modifying persisted bytes or underlying identity derivation**. If the existing product-facing typed `GraphRelation`/`GraphRelationChange` API needs typed Memory IDs, define those as the thin product-facing representation; **one** Graph store/codec implementation remains below. Do not create two interchangeable independent Graph models, persistent coercion tables or generalized adapter traits.

Use Graph-local error identities for missing Memory/Entity references, mapped at the caller boundary to existing externally visible `GraphError` variants and exact error precedence. No `reliquary-memory-core`, `reliquary-perception-store`, `reliquary-archive`, root product or Dream crate as a Graph dependency.

## 4. Real dependency inversions and their corrections

| Current implementation dependency | Target hard cut |
| --- | --- |
| `graph_store.rs::finish_rebuild`, `graph_rebuild.rs::finish`, `graph_store_validation.rs` take `&MemoryStore` and `&EntityStore` | Split **Graph-local replay/shape/ordinal/version validation** from **cross-owner endpoint existence and Entity principal-kind validation**. Owner Lifecycle/root finishes and validates Memory/Entity projections before admitting externally dependent Graph endpoints. It then passes *validated primitive endpoint facts or an explicit set of known current/historical IDs* where an internal Graph pass truly needs them; do not hand Graph a Memory/Entity store. Preserve current vs historical existence distinctions, deleted/retired node visibility, and fail-closed error sequencing. |
| `graph_store_write.rs::set_*` and replay/import variants take both stores | Product root/Owner Lifecycle resolves Memory/Entity validity and principal-kind authority *before* Graph publication. Graph checks intrinsic kind/origin/shape, duplicate keys, optimistic graph version, node ordinal bindings, transaction order and capacity; stages local changes until its existing physical append succeeds. Import/reconcile caller retains the authorized use of source transaction shape and historical postimages. Avoid letting arbitrary Graph public callers bypass owner admission. |
| `graph_store_validation.rs` references `entity_principal::PRINCIPAL_ENTITY_KIND` | Principal policy belongs to Perception/Entity authority. Root validates actual target kind and owner-local current/historical existence; Graph retains only shape `Memory→Entity` and allowed relation kinds/origins. Do not move principal constants or EntityStore into Graph. |
| `semantic_node_model.rs`, `graph_model.rs`, `community_model.rs`, `community_store.rs` depend on typed `MemoryId`/`EntityId` | Use exact `[u8; 32]` foreign identities in the Graph crate; map to existing strongly typed product IDs only at higher caller boundary. Preserve semantic-kind tags, sort order, node ordinals, `CommunityId` SHA-256 inputs and all binary encodings. |
| `graph_rewrite_baseline.rs` embeds `HistoryGenerationId` and validates `RewriteGenesis` | Graph owns `CVAGBL01` baseline bytes, member ordinals and read-only parsed baseline identity. Maintenance/History verifies lineage owner/generation/policy digest against authoritative `RewriteGenesis`. No Graph→Rewrite dependency, new baseline publisher, or live owner mutation on ordinary open. |
| `community_store.rs` accepts `GraphStore` internally and `CommunityOpenState` is finished against Graph | **Both** stores and their private replay/value projections move into the same Graph crate. Snapshot generation and deterministic derivation are graph-local; Dream supplies trigger/schedule and naming inference, never a second Community store. Preserve snapshot generation, graph-projection watermark, exact-membership IDs, user vs Dream labels and lineage. |
| `community_routing.rs`, `community_subcentroid_routing.rs` depend on `MemoryRef`, vectors, retrieval index and representative selection | Stay in root Retrieval consumer until Retrieval/Vector crate cuts. Read owner-local Graph/Community facts via narrow immutable calls; do not move vector ranking, embedding generation or candidate selection into Graph. |
| `Cva`/`Phylactery` Graph APIs, Dream, Perception, Freshness, Ego, History, Import and runtime | They consume `reliquary-graph` directly or through the root's existing owner admission seam when multi-owner authority is needed. Only Graph state mutates in the low-level crate; no root-owned second store. |

**Data shape rule:** when Graph consumes foreign facts, upstream validates them under the same held owner/Container authorization context before physical publication, with the same ordering and failure behavior. Never replace required cross-owner validation with “Graph trusts everything” or an unchecked direct public writer.

## 5. Frozen Graph/Community behavior

- **Graph record identities and codec:** Preserve current `CVAGFMT1`, semantic node/ordinal `CVAGNOD2` format, existing accepted `CVAGVER1` history where applicable, compact `CVAGCM01`/`CVAGCB01` operations and current publication envelope `CVAPUB01`. The existing recognized historical `CVAGNODE`, `CVAGMUT1/2`, `CVAGBAT1/2` read behavior may not be broadened; do not add compatibility branches or rewrite the promoted corpus.
- **Graph versions:** Graph semantic versions are contiguous owner-local transactions; a batch consumes one Graph transaction version/global allocation. A transaction carries uniform `graph_version` and `global_version`. Memory projection watermark changes only for Memory-Memory changes, not for Entity-only association.
- **Node mapping:** Persisted `SemanticNodeRef` and Arcana `NodeId` ordinal mapping remains deterministic and contiguous; accepted nodes cannot be rebound to another semantic identity. Staged mappings publish only after required physical append succeeds. Failed writes must not add visible nodes/edges or advance in-memory Graph version.
- **Relation contract:** Retain typed `Memory` relationships and `Memory→Entity` associations, kind/origin/direction, single vs batch publication, source order and exact replay/idempotent or conflict behavior. Invalid self-relations, endpoint kind, duplicate changes, incompatible origin, missing current/historical foreign owner records, and invalid principal associations fail closed.
- **REL/PHY:** Graph is owner-local. Both owner kinds can hold their permitted Graph/Community state; PHY has no Archive source dependency and must not resolve an unrelated REL without authorization.
- **Read and projection:** Preserve semantic neighbor queries, current/historical node references, Memory-only topology, shortest path, undirected structural projection, exclusion/inactive behavior, relationships ordered consistently, and clear error reporting on corrupt indexes.
- **Community:** Preserve `CVACOMM1` snapshot and `CVACNAM1` name bodies, generation, exact membership identity, algorithm version 2 scan/merge, deterministic seed `0x4c454944454e0001`, resolution `1.0`, worker-count-invariant membership and currentness bound to `memory_graph_version`. Name changes consume no semantic/global version. Unambiguous unique-max-Jaccard lineage and direct/user precedence remain authoritative; ambiguous ties do not transfer names. Generated name inherited across cumulative similarity below `0.750` is invalid; no inference inside Graph.
- **Maintenance/History:** Snapshot regeneration is explicit and does not rewrite Graph semantic relationships. Historical reads remain non-mutating, preserve exact accepted chronological cuts and reject unresolved markers. Community data is derived and may be recomputed after an authorized divergent semantic reconciliation; genuine semantic no-op preserves canonical bytes.
- **Runtime performance:** Do not clone the accumulated Graph/Community store per compact batch or build a second full topology for every historical query without need. Retain efficient existing Arcana runtime topology/projection; no new repeated all-owner scans just to satisfy the extraction. No custom graph verification sidecar.

## 6. Staged hard-cut execution

1. **Baseline/caller inventory.** Confirm working tree/HEAD; **do not stage/commit/alter** Echo's in-flight uncommitted extraction. List Graph and Community code and actual root consumers with `rg`; compare selected abandoned-refactor source corrections. Use Arcana only if its index is current; current Git/Cargo/source is the verification authority.
2. **Graph-local identity, codec, state and replay.** Move the real `GraphStore` models, ordinal maps, compact publication, decode/replay, topology, query and error modules directly into `reliquary-graph`. Convert cross-owner typed IDs to fixed foreign-ID bytes locally. Compile the real crate against Container, `arcana` and necessary existing external dependencies. No scaffold-only acceptance.
3. **Community state/derivation.** Move current `CommunityStore`, codecs, snapshots/names/lineage and deterministic Leiden/scan-merge algorithm with Graph because it depends only on Graph-owned facts. Keep Dream inference/cadence and retrieval/vector representation outside. Migrate genuinely owner-local tests, not every benchmark under `src/semantic_graph`.
4. **Lift cross-owner admission.** Update root Owner Lifecycle/`Cva`/`Phylactery` to validate Memory/Entity existence, principal kinds and lineage/authorization at their actual owning stores; hand canonical, admitted primitive operations to Graph. Migrate Dream/Perception and historical/import/reconcile callers directly to the new crate through the appropriate authorized existing seam. Preserve current physical replay order and product errors; avoid unbounded duplicate validation/owner scans.
5. **Delete displaced code.** Remove former Graph/Community owner modules and `#[path]` declarations in `src/lib.rs`; leave higher-level routing/benchmarks under their real owners. Update `Cargo.toml`/`Cargo.lock`, public product re-exports, code maps and documented crate ownership after implementation. One reviewable implementation commit at the completed extraction gate. Temporary root compile breakage is acceptable only within the bounded migration window.

Do **not** extract or redesign Perception Store, Dream Store, Retrieval, History, Import, Maintenance, Freshness, Ego, Vector Store or Gateway in this work item.

## 7. Existing tests, acceptance and verification

**Graph owner-local test candidates** (move/split, not copy): `graph_codec_tests.rs`, `graph_compact_codec_tests.rs`, `graph_publication_codec_tests.rs`, `graph_publication_overflow_tests.rs`, `graph_compact_owner_tests.rs` (only local portions), `community_tests.rs` (pure store portions), `community_lineage_tests.rs`, `community_scan_merge_tests.rs`. New tests only for a *real* boundary created here—specifically opaque foreign identity roundtrip, safe owner-admitted publication, and cross-owner absence/conflict handling absent from existing coverage.

**Root integration tests retain** `graph_publication_replay_tests.rs`, `graph_compact_writer_tests.rs`, `graph_tests.rs`, `semantic_graph_tests.rs`, `semantic_graph_reconcile_tests.rs`, `semantic_graph_runtime_host_tests.rs`, historical fingerprint/rewrite/compaction tests, REL and PHY graph/profile cases, Dream duplicate/Community derivation, Perception Entity associations, Freshness neighbor projection, community naming/lineage and retrieval/agentic graph traversal. Keep bench fixtures where the consuming algorithm actually belongs.

**Required behavior gates**:
- Exact `CVAGFMT1`/`CVAGNOD2`/`CVAGCM01`/`CVAGCB01`/`CVAPUB01` identity, complete-invalid vs incomplete-tail rejection, chronological Graph/global clock and ordinal replay; no write during ordinary owner open.
- Direct Memory relation, Entity association and principal validation through original supported `Cva`/`Phylactery` endpoints (including historical-retired reference handling). No fake Graph permission to read a foreign store.
- Single/batch transaction conflict, optimistic version, duplicate/no-op, failed-append atomicity and exact authorized import/reconciliation postimages.
- Memory-only projection watermark invariant under Entity-only changes; correct Community currentness; Graph topology/adjacency/shortest-path equivalence across reopen and historical cuts.
- Community scan/merge determinism, membership identity, name precedence and source contract, lineage ambiguity and cumulative Dream retention; explicit rebuild does not mutate relationships or consume semantic clocks.
- Root Dream/Perception/Freshness/Ego/Import/History consumers compile and preserve their current publication/read behavior without Graph's upward Cargo imports.

**Commands and structural acceptance:** `cargo check -p reliquary-graph --locked --offline`, `cargo test -p reliquary-graph --locked --offline`, focused root Graph/Community/Dream/Perception/Historical tests, `cargo check --workspace --locked --offline`, `cargo fmt --all -- --check`, plus `cargo test --manifest-path cli/Cargo.toml --locked --offline` with its separate lockfile. Direct `rg`/Cargo inspection must establish **only Container below Graph**, one Graph/Community store and codecs, no stale `src/lib.rs` declarations, no retained parallel root version or hidden root-to-Graph-to-root dependency.

High-cost historic/benchmark test cases must be explicitly identified as run or skipped, not silently counted. Physical current-format corpus must be tested only in disposable copies. **`CVAFRS07` full product admission remains a separate later recovery gate.** Do not use massive fixture growth, architecture ratchets, compiler-level proof systems or generated graph reports as proxies for working source and tests.

## 8. Explicit exclusions

No new Community algorithm, migration to a new Graph backend, new Observation entity semantics, new Dream classifier, new Memory lifecycle rule, Freshness propagation change, bulk derived vector construction, global search, novel real-time graph processing, cross-owner Graph, feature roadmap changes or legacy-format compatibility layer. No modification or staging of Echo's current uncommitted implementation.

**Completion is real Graph/Community ownership behind a narrow crate API and working consumers, not successful compilation of an empty reserved crate.**
## Related docs

- [Crate-first layout](crate-first-layout-spec-2026-10-25.md)
- [Architecture](architecture.md)
- [Maintainer map](maintainer-map.md)

## Notes

CVAFRS07 whole-product corpus recovery remains a separate gate.
