# Reliquary crate structure and layout — implementation specification

Status: **Container, Archive and Memory Core extracted; remaining owner crates are reserved targets** (2026-10-25).
Branch: `refactor/crate-first-restart`; foundation: verified pre-convergence commit `c43d33c`.  
Scope: correction-only architectural migration; no new semantic, API, storage, or roadmap feature.

## Purpose

Reliquary's one-crate implementation places physical storage, durable owner state, semantic services, orchestration, and public facade in the same Rust dependency scope. Rust successfully compiled code that violates the intended domain dependency order. The abandoned dependency-convergence refactor tried to prove those boundaries before moving code across compiled units, producing substantial custom verification infrastructure.

The replacement must yield **separately compiled, coherently owned components with narrow public contracts**, one physical dependency direction, preservation of existing behavior and data, and no permanent adapters added merely to ease extraction. Cargo dependencies and compilation are the ordinary structural check, not an auxiliary compiler-call/graph evidence project.

This is a specification of **where code belongs and who may call whom**, not 30 rewrite tickets or an instruction to implement any roadmap work.

## Related docs

| Authority | Document or decision | Constraint carried into this specification |
| --- | --- | --- |
| Fundamental ownership | [ADR 0001](decisions/0001-purpose-built-database-ownership.md), [Architectural invariants](invariants.md), [Scope ownership](subsystems/scope-ownership.md) | One physical REL/PHY container; distinct semantic authorities; explicit cross-owner rights |
| Accepted dependency design | **October 21 logical target:** commit `6150a3f`, `docs/dependency-convergence-target-dag-2026-10-21.md` (preserved in the Step 1 Git backup); adapted [component DAG](architecture/component-dag.json) | Downward-only, explicit dependencies; accepted logical responsibilities, not the abandoned execution machinery |
| Current crate-first decision | [Crate-first architecture](crate-first-architecture.md) and [crate map](architecture/crate-map.json) | Original semantic design + physical crates; extraction before monolith-wide untangling |
| Echo | [ADR 0014](decisions/0014-echo-historical-reasoning-traces.md) | Independent source-scoped owner, separate from Archive and Memory |
| Memory lifecycle | [ADR 0012](decisions/0012-deterministic-episodes-and-insomnia-memory-authority.md), [Dream](subsystems/dream.md), [Perception](subsystems/perception.md) | Archive creates deterministic Episodes; Insomnia authors Memory; Dream and Perception own their distinct work |
| Durability and history | [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md), [ADR 0042](decisions/0042-minimal-lossless-current-history.md), [Storage format](storage-format.md), [Semantic maintenance](subsystems/semantic-maintenance.md) | Current-only persisted format, lossless owner history, read-only History vs mutating Maintenance/Import |
| Runtime and integrations | [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md), [Runtime multiplexing](subsystems/runtime-multiplexing.md), [External sources](subsystems/external-sources.md), [Model/provider runtime](subsystems/model-runtime.md) | One Gateway authority, per-instance grants, no duplicate worker or transcript owner; provider transport separate |
| Scope exclusions | [Roadmap](roadmap.md), [Multiplexing consumer/release plan](multiplexing-implementation-plan.md), [External-source integration plan](conversation-index-integration-plan.md) | Future Observations, broader Ego/context synthesis, cross-repo consumer releases and external-source redesign are not silently implemented |
| Existing behavior and navigation | [Behavioral contracts](behavioral-contracts.md), [Maintainer map](maintainer-map.md), [Subsystem index](subsystems/INDEX.md), [Development guide](development.md) | Current verified behavior and concrete source/test seam are the migration reference |

**Version distinction:** `c43d33c` is a sound library-test baseline (1,942 passed), but the archived newer refactor and promoted authoritative corpus use **CVAFRS07**. The new architecture must recover that current-format implementation and verify the corpus before declaring the replacement product complete. Do **not** silently reinterpret the baseline's older `CVAFRS06` documentation as a license for runtime legacy readers. The Step 1 recovery inventory names the preserved source commits and corpus.

Precedence for a conflict: accepted ADR and explicit current durable/behavior contract → this correction-only target DAG and crate map → concrete source/tests → historical execution-plan prose. A surprising change in ownership or an additional edge requires an explicit reviewed amendment; an old phase receipt is not architectural authority.

## Overview

One repository and Cargo workspace; no independent repositories, daemon processes, semantic databases, or package-release schedule are created by extraction.

```text
Reliquary/
├── Cargo.toml                         # [workspace] + existing product package
├── Cargo.lock                         # shared Rust workspace lock
├── crates/
│   ├── reliquary-container/
│   ├── reliquary-archive/
│   ├── reliquary-memory-core/
│   ├── reliquary-echo/
│   ├── reliquary-graph/
│   ├── reliquary-entity-resolution/
│   ├── reliquary-freshness/
│   ├── reliquary-insomnia/
│   ├── reliquary-dream/
│   ├── reliquary-chronos/
│   ├── reliquary-ego/
│   ├── reliquary-retrieval/
│   ├── reliquary-context-engine/
│   ├── reliquary-vectors/
│   ├── reliquary-history/
│   ├── reliquary-switchboard/
│   ├── reliquary-project-environment/
│   ├── reliquary-import/
│   ├── reliquary-maintenance/
│   ├── reliquary-runtime-gateway/
│   ├── reliquary-perception-store/
│   ├── reliquary-ego-store/
│   ├── reliquary-freshness-store/
│   ├── reliquary-vector-store/
│   ├── reliquary-insomnia-store/
│   ├── reliquary-dream-store/
│   ├── reliquary-import-store/
│   ├── reliquary-maintenance-store/
│   ├── reliquary-external-index-store/
│   └── reliquary-owner-lifecycle/
├── src/                                # legacy root product; shrinks to public facade
│   └── lib.rs
├── tests/                              # end-to-end public and integration contracts
├── cli/                                # existing separately locked repo-local CLI
│   └── src/
├── examples/                           # real public-API usage and smoke benchmarks
├── docs/
│   ├── crate-first-architecture.md
│   ├── crate-first-layout-spec-2026-10-25.md
│   └── architecture/
│       ├── component-dag.json
│       └── crate-map.json
└── vendor/
```

**Packages:** 20 substantial first-class library crates + 10 narrowly scoped support libraries + current root `reliquary-memory` public/product package = **31 workspace packages**. The currently separate `cli/` package is retained outside the workspace during the architecture cut; verify its separate lock and public API independently rather than silently changing its release boundary.

The Memory domain is `reliquary-memory-core` only because the existing product package already uses `reliquary-memory`. Do not rename the product package as part of this correction.

### Inside each library

```text
crates/reliquary-<domain>/
├── Cargo.toml                          # package, explicit used dependencies
├── src/lib.rs                          # concise public export surface
├── src/<domain-owned modules>.rs       # concrete implementation, not generic scaffolding
└── tests/<public-contract>.rs          # only where cross-crate API merits an integration test
```

Module names (`store`, `model`, `codec`, `replay`, `query`, `policy`, `transport`) describe **real domain responsibilities**. Do not generate empty module hierarchies, `interfaces.rs` for every crate, or `api.rs` facades that mirror implementation. Keep private implementation modules private. Use `pub(crate)` internally; export stable IDs/value types, typed errors, and narrow operations that actual consumers need. No public re-export of another owner's mutable store handle for convenience.

Placement of tests: small invariants and codec tests alongside their implementation; meaningful crate public-contract tests under that crate's `tests/`; multi-owner/host behavior tests under root `tests/`. Migrate, do not duplicate, existing tests; remove tests whose sole purpose was the abandoned graph/ratchet infrastructure.

## 4. First-class library ownership and existing source crosswalk

Paths below are *source migration neighborhoods*, not instructions to move every file without inspection. File-level routing must respect semantic ownership rather than old `mod.rs`, `facade`, or `runtime_inference` placement.

| Target crate | Intended internal layout / exported capability | Existing production neighborhoods | Canonical documentation |
| --- | --- | --- | --- |
| **Container** | framing, file identity, exclusive writer lease, inspection/repair, typed clocks and pure byte codecs; physical errors | `crates/reliquary-container/src/**` (extracted from the former root Container, compact-delta and pure review-clock modules) | [ADR 0001](decisions/0001-purpose-built-database-ownership.md), [Storage format](storage-format.md), [Review clocks](subsystems/review-clocks.md) |
| **Archive** | source conversation, deterministic Episode, fragment, checkpoint, activity/branch history; no Echo | `src/archive/**` except Echo files; Archive-facing source methods in `src/facade/**` | [ADR 0012](decisions/0012-deterministic-episodes-and-insomnia-memory-authority.md), [Architecture](architecture.md) |
| **Memory Core** | Memory identity, immutable revisions, admitted mutation, lifecycle, provenance, replay | `crates/reliquary-memory-core/src/**`; source authorization and carrier coordination remain in root | [ADR 0001](decisions/0001-purpose-built-database-ownership.md), [Scope ownership](subsystems/scope-ownership.md) |
| **Echo** | source-keyed ordered reasoning/commentary/tool/activity evidence, native encoding and scoped reads | `crates/reliquary-echo/src/{echo_model,echo_codec,echo_store}.rs`; REL lifecycle and reconciliation remain in root facades | [Echo extraction](echo-crate-extraction-spec-2026-10-26.md), [Architecture](architecture.md#echo-execution-evidence-crate), [ADR 0014](decisions/0014-echo-historical-reasoning-traces.md) |
| **Graph** | typed relationship topology, accepted edges, Community state/derivation | `src/semantic_graph/**`, `src/facade/cva_graph.rs` owner-facing portion | [Dream](subsystems/dream.md), [Scope ownership](subsystems/scope-ownership.md) |
| **Perception** | candidate, admission, merge, mention resolution policy and model calls; uses Perception Store | `src/perception/**` excluding durable store/codec; entity resolution in `src/runtime_inference/**` | [Perception](subsystems/perception.md), ADR 0033 |
| **Freshness** | scoring, triggers, propagation, event/accepted-use policy; uses Freshness Store | `src/freshness/**` policy, `src/runtime_inference/**` grading; store files separated | [Freshness](subsystems/freshness.md), [Freshness retrieval boundary](freshness-retrieval-boundary-verification-2026-10-15.md) |
| **Insomnia** | `reliquary-insomnia` owns Episode-to-Memory evaluation, ledger and candidate policy, inference stages, and evidence-response validation. Root owns authorized evidence hydration, temporal/source validation, publication, queue adaptation and scheduling; Insomnia Store owns durable work/attempt state. | `crates/reliquary-insomnia/src/**`; Root adapters in `src/runtime_inference/insomnia/**` | [Insomnia](subsystems/insomnia.md), [service extraction evidence](insomnia-service-crate-extraction-evidence-2026-10-28.md), [ADR 0012](decisions/0012-deterministic-episodes-and-insomnia-memory-authority.md) |
| **Dream** | pair decisions, duplicates, settlement, Community synthesis; uses Dream Store | `crates/reliquary-dream/src/` for semantic policy; `src/runtime_inference/dream*.rs` and `src/facade/**` for owner coordination | [Dream](subsystems/dream.md) |
| **Chronos** | temporal parsing, matching and bounded candidate selection | `crates/reliquary-chronos/src/**` and higher decision/Memory boundary in `src/chronos/**` | [Chronos](subsystems/chronos.md), ADR 0035 |
| **Ego** | Identity/Personality/Anchor synthesis and Memory-Web summary generation; uses Ego Store | `src/ego/**` synthesis and higher `src/runtime_inference/**` Ego producers | [Ego](subsystems/ego.md), [Ego synthesis plan](ego-web-synthesis-plan.md) |
| **Retrieval** | lexical/semantic scoring, hybrid ranking, disposable indexes, Community routing and bounded Graph traversal over authorized supplied facts; owner reads/error translation stay in Root | `crates/reliquary-retrieval/src/**`, with product adapters in `src/retrieval/**` and agentic orchestration in `src/runtime_inference/**` | [Retrieval manual](manual/retrieval.md), [Agentic Memory retrieval](subsystems/agentic-memory-retrieval.md), [Retrieval implementation evidence](retrieval-service-crate-extraction-evidence-2026-10-28.md) |
| **Context Engine** | present deterministic budgets, evidence rendering, compaction, model-facing assembly of authorized supplied context | `crates/reliquary-context-engine/src/**`; Root API re-exports in `src/lib.rs`; public Echo integration in `src/echo/echo_tests.rs` | [Context Engine](subsystems/context-engine.md), [Cross-chat plan](ego-cross-chat-context-plan.md), [extraction specification](context-engine-crate-extraction-spec-2026-10-28.md) |
| **Vectors** | embedding generation, compatibility decisions and vector publication; uses Vector Store | `src/retrieval/**` embedding/profiles/generation algorithms, `src/runtime_inference/**` vector steps | [Model runtime](subsystems/model-runtime.md), [Storage format](storage-format.md) |
| **History** | shared read-only historical revision result classification over owner-authorized facts; broader owner-cut/history reconstruction stays in existing owners pending a real production seam | `crates/reliquary-history/src/**` for the shared evaluator and result; REL/PHY Memory, Entity and Relationship historical read paths in Root; test-only fingerprint and global-cut fixtures stay in Root | [History extraction specification](history-service-crate-extraction-spec-2026-10-29.md), [ADR 0042](decisions/0042-minimal-lossless-current-history.md), [Semantic maintenance](subsystems/semantic-maintenance.md) |
| **Switchboard** | configured routes, typed model endpoints, credentials/config and provider transport only | `src/config_security/**`, `src/runtime_inference/configured*.rs`, `*endpoint.rs`, `model_switchboard*.rs`, `openai_*/**` | [Model runtime](subsystems/model-runtime.md), [Configuration](configuration.md) |
| **Project Environment** | validated project identity, authorized paths, Lore/Git revision access | `crates/reliquary-project-environment/src/**`, Root lifecycle and project host adapters | [Project Environment](subsystems/project-environment.md), ADR 0037 |
| **Import** | compiled source normalization and canonical preparation; exclusive epoch workflow, completion and historical reconciliation remain in Root/Cva | `crates/reliquary-import/src/lib.rs`, `prepare.rs`, `providers/**` own the canonical DTOs, provider adapters, digests and Archive receipt/turn preparation; `src/imports/contract.rs` reexports DTOs and adapts the narrow error while `src/imports/**` retains epoch/report/owner coordination; `src/facade/**` retains historical import orchestration; durable epoch values/codecs remain in `crates/reliquary-import-store/src/**` with Root adapters | [Native import](native-import.md), [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md) |
| **Maintenance** | prune/reclaim/repair/rewrite/reconcile under validated owner proofs | `src/rewrite/**`, maintenance/historical reconcile and mutation in `src/facade/**` | [Semantic maintenance](subsystems/semantic-maintenance.md), [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md) |
| **Runtime Gateway** | one Gateway, instances/grants, REL/PHY execution, queue/worker lifecycle, streams, host-owned external transcript integration | `src/runtime_inference/runtime_host*.rs`, `runtime_host_gateway*.rs`, `interaction_*.rs`, `src/external_source/**` host adapter portion | [Runtime multiplexing](subsystems/runtime-multiplexing.md), [External sources](subsystems/external-sources.md), [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md) |

In the target, each semantic library is a **deep domain module**, not just a re-export crate around the old root. Runtime may coordinate services; no service imports Runtime or the root product.

## 5. Supporting libraries and the strict lower layer

The original 25-library draft had zero *cycles*, but an important layering defect: `owner-lifecycle` (logical rank 3) and `history` (rank 4) depended on the **high-level** Dream, Insomnia, Ego, Freshness and Vectors packages simply to read each one's rank-2 store. That would compile but still let low-level code import upper semantic policy and inference.

**Correction: five additional owner-state crates.** Keep the original five, yielding ten. Each has a direct owner-specific data/codec/replay interface with **no provider, runtime, higher semantic service or root-product dependency**.

| Support crate | Extract from | Allowed responsibility; reason for separation |
| --- | --- | --- |
| **Perception Store** | `src/perception/**` Entity/Resolution/Relationship stores/codecs | Retrieval reads accepted Entity evidence without a dependency on the Perception decision service |
| **Ego Store** | `src/ego/**` store, identity/anchor/codecs/replay | History and owner opening read persistent Ego independently of Ego synthesis |
| **Freshness Store** | `src/freshness/**` storage/settlement codecs, `src/freshness_storage.rs` | Historical score and effect replay without importing Freshness semantic policy |
| **Vector Store** | `src/retrieval/**` packed/vector/compatibility storage/codecs | Read vector/profile generations without invoking embedding generation/provider routes |
| **Insomnia Store** | `src/runtime_inference/insomnia/**` durable queue/receipts | Replay work/completion evidence without importing the Insomnia processor |
| **Dream Store** | `src/runtime_inference/dream*.rs`, durable Dream journal/begin codecs | Replay Dream evidence without invoking Dream model/settlement algorithms |
| **Import Store** | `src/imports/epoch_store.rs`, `epoch_codec.rs`, related durable markers | Owner/History access to epoch state without Import coordination |
| **Maintenance Store** | owner maintenance manifests/retention evidence in current facade/helper files | History reads evidence without depending on mutating Maintenance |
| **External Index Store** | `src/external_source/store.rs`, index/cursor/checksum codecs | Owner lifecycle can open derived host index state without importing Gateway |
| **Owner Lifecycle** | `src/facade/**`, `src/owner_lifecycle.rs`, validated open/replay | REL/PHY aggregate owner open/replay and held writer/cut authorization; depends only on Container and lower **store** crates |

Owner Lifecycle is **not** a generic mutable service locator; History is **not** a second writer or semantic decision engine. Where a field/type crosses from semantic service to its store, preserve IDs and immutable values; transfer mutation via narrow domain-owned operations with exact errors. If the store genuinely requires an upper semantic callback, move the orchestration *upward* instead of weakening this boundary.

The [crate map](architecture/crate-map.json) is the canonical allowlist for physical package references. The [component DAG](architecture/component-dag.json) remains the original logical contract with Echo added. **Neither permission implies an actual Cargo dependency**: add a path dependency only when the code using it has moved and its interface exists. Echo now has a real root-product path dependency; its own only direct Reliquary dependency is Container.

## 6. Public API and responsibility rules

- **Physical Container:** accept opaque owner payloads plus authenticated physical clocks/identity; return typed framing/I/O errors. Never import Archive/Memory/Echo/semantic policy or interpret a domain record.
- **Store crates:** own durable decode/validation/replay/publication, historical revisions, lifecycle and bounded source facts. No store calls inference, schedules workers, changes another store, or owns ACL decisions.
- **Services:** own semantic decisions and their operation-level contracts; request validated read facts or call the rightful store publication seam; do not borrow unrestricted mutable owner fields, pretend to own other services' durable data or duplicate state.
- **History / Maintenance / Import:** History reads immutable accepted cuts; Maintenance authorizes transformation and writes new files only through validated owners; Import owns epoch coordination and completion, not a generic write bypass.
- **Switchboard:** provider execution/transport/config, never semantic state; secret values do not leak into logs or public errors.
- **Context Engine:** deterministic context selection and compaction over *authorized supplied scope*; no new authority or transcript storage. Ego produces summaries; Context Engine decides model-facing inclusion.
- **Gateway:** one shared mounted worker/owner authority; Instance is separately authorized and disposable. No lower-level crate imports Gateway just to discover active REL, attached PHY or credentials. Model host/external source are narrow ports; transient hydration never duplicates host-canonical transcript.
- **Echo:** exact source-scoped retrieval, scoped *before* ranking, never global cross-history reasoning search; provider traces need not imply complete plaintext reasoning.
- **Product root (`reliquary-memory`):** owns public Cva/Phylactery and stable integration facade/re-exports only. It depends down on the new crates; none depends back on it. Tests and CLI validate the public behavior.

No boilerplate trait per crate, generic `*Manager`, cross-owner mutable registry, compatibility shim or giant `contracts` dependency bucket is authorized by this layout. Introduce a port/trait only for actual remote/provider/host variation or a narrow ownership contract that otherwise produces a cycle.

## 7. Migration and build gates

**Migration mode: staged hard cut.** Original `main`, its dirty changes, archived refactor, and authoritative fixture repository remain preserved. On `refactor/crate-first-restart`, each completed move has **one canonical owner**. Bounded subgraphs may be temporarily red; do not create dual live implementations to keep every intermediate commit green.

Recommended extraction topological groups, **not another 30-step phase ledger**:

1. **Lower foundations:** Container (including codecs/clocks); independent store crates (Archive, Memory, Echo, Graph and the nine lower store packages), plus Switchboard/config where external transport is needed.
2. **Owner assembly and shared capabilities:** Owner Lifecycle, Chronos, Freshness and Vectors. The first actual hard cut should prove a real Container and one consumer through Cargo.
3. **Semantic services:** Retrieval → Perception → Insomnia; Dream and Ego according to their approved dependencies. Behavioral tests move with each actual owner.
4. **Historical operations:** History, Maintenance, Import, Project Environment, and the existing independent derived external index where it is consumed. Move coordinators once rather than into transient facades.
5. **Composition and consumers:** Context Engine, Runtime/Gateway, root facade, public integration tests and separately locked CLI. Complete the public API and corpus replay acceptance.

### Crosswalk to the abandoned dependency-convergence plan

These are **semantic work areas carried forward**, not imported task counts or acceptance gates. The old plan is retained in the Git backup and on the original `main` at `docs/dependency-convergence-architecture-refactor-2026-10-21.md`. Its `Phase 0–7` ledgers do not become this migration's tickets.

| Old plan area | Where its genuinely necessary work now belongs |
| --- | --- |
| Phase 0: target and inventory | [component DAG](architecture/component-dag.json), [crate map](architecture/crate-map.json), Step 1 recovery inventory; no recurring source-evidence engine |
| Phase 1: physical storage/owner opening | Container, all lower owner stores and Owner Lifecycle; enforce separation by compiling their actual crate APIs |
| Phase 2: owner-local semantics | Memory, Archive, Graph, Echo, Perception, Freshness, Insomnia, Dream, Ego, Vectors, Retrieval; service/store divisions in §5 |
| Phase 3: History/Maintenance/Import | Three explicit coordinators plus Import and Maintenance stores; preserve current-format corpus and historical exactness |
| Phase 4: project ownership | Project Environment crate and its existing validated Lore/Git adapter tests |
| Phase 5: services/runtime/context | Switchboard, Chronos, Context Engine and Runtime/Gateway; preserve current implementation, do not implement deferred roadmap behavior |
| Phase 6: public consumers | Slim root `reliquary-memory` facade, independently checked CLI and host-facing API contracts |
| Phase 7: giant convergence/graph proof | **Abandoned** as an independent project: Cargo rejects crate cycles, scoped source searches catch stragglers, and existing behavioral tests prove operation semantics |

The source work in commits `8614547` (physical boundary), `1380d0b` (owners), selected P3 import/history changes including `c6010fd` (current format) and `10cbe4b` (Project Environment) is **salvage input**, not a blanket cherry-pick request; consult the Step 1 inventory for the full list.

At each completed domain: independently compile the library and immediate consumers; run the relevant transferred behavior tests; inspect remaining old source imports with `rg` (Arcana only if freshly indexed and useful); remove displaced source and re-exports. No synthetic per-call verification manifests or million-line graph receipt datasets.

At the final integration gate:
- `cargo check --workspace --locked`, `cargo test --workspace --locked`, and workspace formatting pass on the final tree, with explicitly identified known example/allocator exceptions rather than concealed skips;
- `cargo check/test --manifest-path cli/Cargo.toml --locked` pass independently;
- tests through public facade cover REL/PHY scope/permissions, deterministic Archive/Echo ordering, Insomnia/Dream/Perception, Freshness nonreinforcing reads, retrieval, authorization and stream checkpoint ordering;
- exact retained-history, import completion/reconciliation and selected current-format corpus oracles pass on **disposable copies**, and authoritative fixture bytes/hashes remain unchanged;
- `CVAFRS07` is the **only normal-runtime accepted format**; any offline conversion used during recovery is retired after promotion, with no permanent V1/CVAFRS06 route;
- every intended production component is actually hosted in its target crate, the root product no longer houses duplicate owners/semantics, no forbidden crate import or cycle exists, and documentation code maps point to the final paths.

**Not accepted by placeholder compilation:** the empty reserved crate targets currently in the workspace establish only file layout. Completion requires real implementations behind independent crate APIs, migrated callers and tests, and deletion of the old monolith copies.

## Notes

No new Observations, Entity identity redesign, broader Ego synthesis, new cross-REL context machinery, external transcript authority redesign, provider change, fresh storage schema, history checkpoint optimization, release packaging or Warlock/Hermes integration. Existing roadmap and consumer plans continue to own those features.

Update owner documents **as each crate becomes real**: [Architecture](architecture.md), [Maintainer map](maintainer-map.md), [Development](development.md), [Subsystem docs](subsystems/INDEX.md), [Behavioral contracts](behavioral-contracts.md), [Storage format](storage-format.md) where the on-disk contracts are affected, and [Documentation coverage](documentation-coverage.md). The Echo source map now reflects its moved implementation; other source maps remain targets until their migrations occur. Preserve the governing ADRs; add an ADR only for a genuinely new, surprising, durable decision (the Echo authority itself is already covered by ADR 0014).

**Implementation status:** `reliquary-container`, `reliquary-archive`, `reliquary-memory-core` and `reliquary-echo` own their production storage and owner tests. The other 26 libraries remain reserved targets. Echo implementation details and acceptance status are recorded in the [Echo extraction specification](echo-crate-extraction-spec-2026-10-26.md); focused extraction gates pass, with broader-suite and architecture-tool exceptions recorded there. Current-only CVAFRS07 semantic owner recovery and disposable promoted-corpus acceptance remain pending; this extraction is not final product acceptance.

**Specification acceptance:** mapping accounts for all 35 logical components; the 30-library physical DAG is acyclic **and** has no lower-component dependency on any package housing a higher-rank semantic service; every crate has a single responsibility and a checked source/docs crosswalk; scope/corpus gates and non-goals are explicit. Implementation acceptance is a separate future event.
