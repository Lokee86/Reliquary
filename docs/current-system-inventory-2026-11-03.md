# Reliquary current implementation inventory — 2026-11-03

Parent index: [Documentation index](INDEX.md)

**Purpose:** factual inventory of the *implemented* system before a new architectural design. This is not an endorsement of the old crate map and not a proposed replacement architecture.

**Inspected checkout:** `C:\!bin\workspace\Reliquary-crate-first`, branch `refactor/crate-first-restart`, HEAD `628aab50` (2026-10-30). No implementation source was changed during this inventory. Existing unrelated modifications and temporary files must be preserved.

**Evidence quality:** Current files/manifests and code signatures were inspected directly. Historical numerical measurements and test results below are attributed to their dated records, not represented as newly rerun. Arcana's available snapshot reports 114,830 nodes and 316,488 edges but has no runtime-confirmed relationships; use it as navigation, not independent proof that the intended ownership DAG is satisfied. The October 30 [Root audit](crate-first-root-dependency-audit-2026-10-30.md) is the measurement baseline.

## 1. Product and packaging overview

- Product/root crate: `reliquary-memory` (`Cargo.toml`, `src/lib.rs`) owns public product API, REL/PHY concrete aggregates, orchestrators, host integration and substantial residual domain behavior.
- Independent workspace libraries: **30 distinct directories** under `crates/reliquary-*`. `Cargo.toml` lists `reliquary-perception-store` twice in workspace members; this does not represent another physical crate.
- Detached CLI: `cli/Cargo.toml`, outside the core workspace, uses the root library's public API. Command implementations cover REL/PHY, Archive, imported sources, config/credentials, Insomnia, vectors, repair and rewrite (`cli/src/`).
- Other integrators: Warlock, Hermes and other hosts consume public APIs; their repository-level implementation/acceptance was **not** audited here.

The 2026-10-30 tracked-source audit, excluding test-named paths, reported **81,996 Root lines in 407 files** and **66,134 crate lines in 413 files**: 148,130 combined, 55.4% in Root. This is a lexical, name-filtered source measure, *not* a percentage of incorrect code or an AST ownership analysis. Root's largest measured neighborhoods were runtime/inference (36,971), facades (24,882), perception (4,483), semantic graph (2,858), import (2,646), rewrite (2,614), retrieval (2,244), and external sources (1,636).

## 2. Physical crates — current ownership and dependency inventory

Dependencies listed are **direct local Reliquary crate dependencies in current Cargo manifests**, not all third-party dependencies. “Implemented” means production Rust exists, not that complete system/regression/corpus acceptance is established.

| Crate suffix (`reliquary-*`) | Existing production responsibility | Local Reliquary dependencies | Implementation boundary |
| --- | --- | --- | --- |
| `container` | REL/PHY physical framing, headers, file leases, read/replay cursors, clocks, explicit tail repair | — | Implemented physical owner |
| `archive` | Native transcript, Node/Branch/Episode, fragments, files, receipt and stream/compaction history | container | Implemented semantic source owner |
| `memory-core` | Memory identity/body, revision, mutation, routing, tombstone/history and codecs | container | Implemented semantic store |
| `echo` | Source-keyed reasoning/tool/activity execution evidence and `CVAECHO1` replay | container | Implemented evidence store |
| `graph` | Typed Graph relations, node identity/topology, projections, derived Communities and lineage | container | Implemented graph store |
| `perception-store` | Entity, mention-resolution state and transitional Relationship persistence | container | Implemented semantic stores; Observation synthesis absent |
| `ego-store` | Durable Identity, Personality, Anchors, revisions/compact synthesis state | container | Implemented durable Ego state |
| `freshness-store` | Score, event/accepted-use, semantic/import settlement, retained Freshness history | container | Implemented but promoted corpus recovery blocked |
| `vector-store` | Archive/Memory/packed vectors, compatibility/generation persistence and replay | container | Implemented vector storage |
| `insomnia-store` | Episode-keyed work/attempt queue, lease transitions, replay and persistence | container | Implemented queue owner; Root still validates grouped completion |
| `dream-store` | Begin/journal codec, population digest and first-cycle replay projection | — | Implemented supporting durable records; product proof stays Root |
| `import-store` | ImportStart/Complete markers, epoch codec/store values | container | Implemented lower record owner |
| `external-index-store` | Nonauthoritative external-source snapshot/CAS/index persistence | container | Implemented derived index store |
| `maintenance-store` | Prune manifests, retention metadata and passive proof replay | — | Implemented supporting evidence store |
| `owner-lifecycle` | Physical ordered reopen/classification, core and evidence replay helpers | container, memory-core, perception-store, archive, graph, echo | Implemented helper; **not** concrete REL/PHY aggregate owner |
| `chronos` | Deterministic temporal parsing, matching, bounded validated candidates | — | Implemented temporal service |
| `switchboard` | Model/decision routes, endpoints/provider transports, credentials, config/security | — | Implemented model runtime |
| `context-engine` | Conversation compaction budgets, context/evidence rendering | echo | Implemented deterministic current-session layer |
| `vectors` | Embedding/compatibility probes, batch assembly, validation | switchboard, vector-store | Implemented service extraction; outstanding wider verification noted |
| `retrieval` | Lexical/exact-semantic ranking, hybrid/Community routing, candidate indexes, traversal | archive, graph, memory-core, vectors, vector-store | Implemented policy extraction; Root hydration/controller remain |
| `freshness` | Deterministic decay/propagation, historical evaluation, due scheduling | freshness-store, memory-core, graph | Implemented service; Root event assembly remains |
| `insomnia` | Episode-to-Memory extraction, inference stages, ledger/candidate/evidence policy | archive, memory-core, switchboard | Implemented evaluator; Root work application/publishing remains |
| `dream` | Pair classification/verification, candidate/duplicate planning and lifecycle intent | memory-core, graph, switchboard, chronos, vectors | Implemented inference/policy; Root processor publishes |
| `entity-resolution` | Entity candidates, matching/admission, resolution decision and policy | perception-store, memory-core, retrieval, switchboard | Implemented policy; Root still coordinates owner commitments |
| `history` | Read-only active/tombstoned/pruned historical payload classification | archive | **Small helper**, not whole historical subsystem |
| `import` | Deterministic native import normalization/preparation; ChatGPT/Claude/Hermes/agent adapters | archive | Implemented normalization, not complete epoch workflow |
| `maintenance` | Shared explicit-retain/exclude and semantic-closure selection for rewrite | memory-core | **Small helper**, not full maintenance/rewrite |
| `project-environment` | Git/Lore project file/revision/checkout/upload mechanics | archive | Implemented repository service, Root lifecycle still present |
| `ego` | No extracted semantic synthesis service | — | **Placeholder only** (`src/lib.rs` comment) |
| `runtime-gateway` | No extracted runtime implementation | — | **Placeholder only** (`src/lib.rs` comment) |

**Interpretation:** Cargo enforces the present *inter-crate* acyclic dependencies. It does not enforce ownership between hundreds of Root modules sharing `crate::` access. The real Root gateway and owner aggregates are still inside `reliquary-memory`; crate count must not be used as architectural acceptance.

## 3. Concrete durable authority (current, not proposed)

| State / authority | Current canonical owner | Where cross-owner operations still occur |
| --- | --- | --- |
| Physical REL/PHY bytes, header, read/write admission, global physical publication | Container | Root open/reconcile/rewrite and owner lifecycle |
| Native source conversation/Branch/Node/Episode, Archive receipts | Archive | `Cva`, `InteractionRuntime`, Gateway |
| Memory bodies/revisions/provenance/terminal state | Memory Core per REL/PHY | Root authorizes origin, routing and grouped publication |
| Typed Graph edges, Community snapshot/name/lineage | Graph | Root validates foreign IDs, Dream publishes decisions |
| Entity, mention decisions, Relationship transition state | Perception Store | Root entity resolution, reconciliation, Graph association |
| Vectors, profile/generation bindings | Vector Store | Root hydrates source, compatibility and publication |
| Freshness accepted-use/events/settlement | Freshness Store | Root/Gateway confirms external consumption and Dream/Import evidence |
| Ego identities/personality/anchors | Ego Store per REL/PHY | Root facades, transformations/reconciliation; synthesis service absent |
| Work/attempt receipts | Insomnia Store | Root source authentication, grouped completion, worker routing |
| Dream Begin/journal | Dream Store | Root Dream process/first-cycle publication and proof |
| Import epoch boundaries | Import Store | Root imports, readiness, completed-epoch reconciliation |
| Prune proof/manifest | Maintenance Store | Root source authorization, transformed-copy rewrite |
| External derived source metadata/index | External Index Store | Root/Gateway source authorization, hydration and search |
| Runtime queue/host result/stream/owner-operation journals | Root Gateway/runtime modules (REL-persisted records) | Root itself; not the empty Gateway crate |
| Project source revision/file ancestry | External Git/Lore repository | Project Environment references, Root/REL correlations |

A REL is a durable project/conversation semantic owner; a PHY is a distinct private user semantic owner. Sharing a model/service implementation does not give one file authority to mutate the other. REL dependency closure belongs to host/runtime authorization and context composition, not implicit cross-file write access. See [scope ownership](subsystems/scope-ownership.md).

### Current persistence boundaries

- Current Container format is 2.0; REL and PHY have a 40-byte header with typed kind/durable owner UUID. Ordinary open validates and does not silently repair a trailing incomplete frame.
- `CVAPUB01` physical publication framing binds global coordinate and optional transaction timestamp. Owners retain separate local semantic clocks and history; derived indexes are not semantic authority.
- Ordinary records, source turns, mutable owner semantic history, temporal provenance, tombstones, Dream and Freshness receipts, checkpoint/replay metadata and vector generations have separate, owner-specific validation.
- Rewrite/prune create transformed copies with explicit evidence; neither project VCS history nor a derived index substitutes for the owners' retained semantic history.
- Current-only format policy rejects retired formats in normal runtime. Migration happens explicitly/offline; do not introduce permanent legacy readers.
- **Release blocker:** Promoted six-file corpus uses `CVAFRS07`; this restart's Freshness semantic implementation still recognizes `CVAFRS06`. The last recorded disposable-copy check reopened three PHY files but rejected three REL files. The exact six-file test at `tests/current_format_boundary.rs` is ignored until recovery. This is a *compatibility/recovery gap*, independent of the crate architecture.

## 4. Implemented processing paths

### A. Native conversation to Memories

1. Gateway/Instance authenticates principal, exact REL/PHY grants and mount/lease; the actual implementation resides in `src/runtime_inference/runtime_host_gateway*.rs`, `runtime_host_instances.rs`, and related host modules.
2. `InteractionRuntime` accepts turns into a Root-held `Cva`; Archive owns the accepted source and activity clock; REL journals/checkpoints protect running conversation work.
3. Episode construction and durable Insomnia queue schedule accepted source evidence. The compiled Insomnia evaluator produces bounded, provenance-checked, ownership-classified Memory candidates (and metadata/enrichment).
4. Root coordinates authorization, REL-vs-PHY destination, atomic grouped completion and Memory/Freshness publication. Memory Core owns accepted state; Insomnia Store owns work/attempt state.
5. Dream evaluates owner-local Memory candidates/pairs, duplicate/canonical/lifecycle decisions, Graph relations and derived Community updates. Lower Dream policy and journal exist; Root processor/publisher authorizes reads and commits.
6. Freshness consumes REL activity, accepted-use receipts, semantic and first-cycle Dream effects through deterministic evaluation, owner-local persistence and replay. Internal reads do not reinforce.
7. Retrieval consumes authorized Archive, Memory, vector, Graph and Freshness state; the Root controller and Gateway perform scoped hydration, agentic decision selection and successful-handoff confirmation. Context Engine renders bounded current-session model context. Ego cross-chat/web synthesis remains future work.

**Live source anchors:** `src/runtime_inference/interaction_runtime.rs`, `src/runtime_inference/insomnia/processor.rs`, `src/runtime_inference/dream_processor.rs`, `src/runtime_inference/agentic_memory_retrieval.rs`; `docs/subsystems/{insomnia,dream,freshness,agentic-memory-retrieval,context-engine}.md`.

### B. Gateway execution and multiplexing

Implemented in Root: one gateway mounting REL/PHY workers, short-lived authorized instances, principal-bound PHY sharing, grants and revocation, per-branch conversation policy, durable pending work, generation/host capability routing, cancellation, reconnect, checkpoints/streams/subscriptions, owner-operation journal and recoverable finalization. Source examples: `runtime_host_gateway.rs` defines `GatewayRuntime`, `runtime_host_instances.rs` defines `InstanceRuntime`, `runtime_host_rel_execution.rs` contains an `Arc<Mutex<InteractionRuntime>>`, `runtime_host_gateway_host.rs` defines capability/handoff types, and `multiplexing_journal.rs` and `runtime_host_owner_operation_journal.rs` define durable journals.

This **is implemented product behavior** despite an empty `reliquary-runtime-gateway` crate. External Warlock/Hermes release acceptance and cross-process exclusion are separate unresolved/limited scopes.

### C. External host source and native import

- **Reference-only external sources:** host retains transcript authority. Root `src/external_source/` and `runtime_host_gateway_external*` handle authorization, feed/hydration, provenance, semantic use and derived search; the external-index store persists only rebuildable index rows. Host-specific adapters are not all production-qualified.
- **Historical native import:** `reliquary-import` normalizes supplied exports; Root `src/imports/` and `facade/cva_historical_import.rs` handle epoch admission, source receipts, readiness, semantic owner replay, completion and reconciliation. Open epochs, unsupported cross-PHY effects and missing readiness evidence fail closed; mounted completion is not claimed.
- These are deliberately different authority models; an external host transcript must not be silently materialized as a fake native Archive authority.

### D. History, maintenance, and project source

- Historical reads and cross-owner immutable proof assembly remain extensively in `src/facade/historical_*`; `reliquary-history` currently only classifies supplied results.
- Tombstones, retained history, pruning, explicit fresh-history rewrite, copy reconciliation, storage reclamation and import settlement span `src/facade/cva_history_prune.rs`, `src/rewrite/`, `src/imports/` and numerous Root facade modules. `reliquary-maintenance` holds shared selection helpers, not the whole maintenance coordinator.
- Git/Lore file/revision operations are in `reliquary-project-environment`; Root `src/project_environment.rs` and lifecycle still attach project metadata and owner correlation.
- CLI remains separately buildable and public-API-only; no unsupported “production complete” claim follows from this separation.

## 5. Root responsibilities and architectural friction

| Root region (2026-10-30 non-test lines) | Current responsibilities | Classification for *future* design |
| --- | --- | --- |
| `runtime_inference/` (36,971) | Actual Gateway, instance/REL/PHY workers, model interactions, Insomnia/Dream flow, streams, queue, journals, agentic controller | **Mixed**: execution runtime + true semantic orchestration + domain adapters |
| `facade/` (24,882) | `Cva`/`Phylactery` concrete stores, open/replay, public APIs, cross-owner mutation, history, prune, reconcile | **Mixed**: state authority + lifecycle + public facade + transactions |
| `perception/` (4,483) | Entity/mention evidence, resolution integration, reconciliation and committing | **Mixed**: authorized coordination and remaining behavior |
| `semantic_graph/` (2,858) | Graph/Community integration, tests/projection, live owner bridges | **Requires source-level disposition**, not automatically misplaced |
| `imports/` (2,646) | Epoch workflow, source/proof/settlement and reconciliation | Substantial coordinator still Root |
| `rewrite/` (2,614) | REL/PHY transformed-copy planning, closure, emission | Substantial maintenance still Root |
| `retrieval/` (2,244) | Live owner hydration/index bridges, public queries and adapters | Mixed: some legitimate data bridges, some need classification |
| `external_source/` (1,636) | Provider-facing source integration, transient hydration, publication | Mixed Gateway/host/semantic coordination |
| Other Root code (3,662) | Config, Chronos, Ego/Freshness bridges, testable glue | Classify by authority, not path |

**Confirmed structural knot:** `src/facade/cva.rs` has mutable Container, Archive, Memory, Graph, Community, Entity, Freshness, Ego, Vector, Dream, Insomnia, Import, external-index and other state; `src/facade/phylactery.rs` has a comparable private aggregate. Their `pub(crate)` field access makes many Root modules depend on mutable owner representations. `reliquary-owner-lifecycle` routes replay but does not own these aggregates. `InteractionRuntime` directly contains `Cva`, and Gateway directly contains/uses `InteractionRuntime` and `Phylactery`. The final owner-composition boundary is **undecided**. An extracted crate-level DAG alone does not resolve this.

**Confirmed Gateway gap:** `crates/reliquary-runtime-gateway/src/lib.rs` explicitly exports no production implementation. The existing Gateway specification starts with only the small `InstanceRuntime` portion and is insufficient to close the full cut.

**Confirmed partial-service gaps:** History and Maintenance are small policy helpers; Import primarily performs normalization. They cannot be counted as full ownership of historical composition, rewrite/reconciliation, or epoch coordination.

**Do not assume every Root module must move:** exact source and permission checks, cross-owner coordination and product API projection may legitimately live above storage/semantic owners. The redesign must first decide the *one canonical concrete owner aggregate* and then classify each remaining Root operation by invariants, mutability and consumers.

## 6. Capabilities implemented vs unfinished

**Implemented behavior (not equivalent to production-qualified):** native REL/PHY storage, Archive history and Episodes, Memory revision/provenance lifecycle, Graph/Communities, typed Entity and mention-resolution foundations, basic Ego durable records, Insomnia work/extraction, Dream association/duplicate lifecycle, Chronos, Freshness and explicit historical operations, vectors/profiles, lexical/semantic/hybrid and agentic retrieval, current-session Context Engine compaction, Gateway multiplexing/durable conversation execution, Git/Lore project adapter, external-source core, native import and maintenance/rewrite paths.

**Unfinished functionality or verification:** Perception Observation synthesis and its relationship/receptor/ambiguity lifecycle; Ego Memory-Web/cross-chat/cross-REL synthesis; complete workspace management and provider-specific interoperability; production Warlock/Hermes consumer acceptance; full release-level stream/recovery/security coverage; frozen corpus `CVAFRS07` reopen; proven retrieval quality against representative retained corpora; all extraction-wide regression/documentation gates. The existing [Roadmap](roadmap.md), [Current limitations](current-limitations.md) and subsystem docs are definitive about intended future scope, but some broad legacy “implemented” statements predate the crate-first restart.

## 7. Verification and preservation baseline

- **Directly inspected this inventory:** current Git status/commit, Cargo root and all 30 crate manifests, crate/source directories, empty placeholder implementations, Root `Cva`, `Phylactery`, `GatewayRuntime`, `InstanceRuntime`, `InteractionRuntime`, `DreamProcessor`, Insomnia processor, import reconciliation, relevant subsystem docs and current-format gate.
- **Historical measured evidence:** [Root dependency audit (2026-10-30)](crate-first-root-dependency-audit-2026-10-30.md), which used tracked Rust source and a successful offline `cargo tree`; it did **not** run a fresh full build.
- **Earlier recorded functional gate:** [Context Engine verification](context-engine-crate-extraction-evidence-2026-10-28.md) reports 1,457 Root library tests passed, five ignored, on its recorded commit. This is **not** a newly verified whole-system status at HEAD `628aab50`.
- **Current worktree at inventory start:** one pre-existing tracked modification to `docs/graph-crate-extraction-evidence-2026-10-26.md`, plus many untracked temporary/fixture entries (Git status output is truncated; no complete count is claimed). These must not be cleaned or swept into an inventory commit.
- **Not run here:** new `cargo check`/`cargo test`/CLI suite, corpus reopen, independent semantic workload replay or full current architecture-lint acceptance. Their status remains **unverified at this HEAD**.

## 8. Design inputs still to be decided (not decisions made by this inventory)

1. Canonical REL/PHY aggregate owner and its authorized read/mutation/transaction interface; how owner-local persistence composes without policy back-edges.
2. The correct full Runtime/Gateway boundary: instance authority, queue/journals, host contracts, stream/recovery, owner worker lifetimes and background scheduling.
3. Location of real cross-owner Insomnia/Dream/Entity/Import/History/Maintenance coordination vs owner-local policy and codecs.
4. Whether adjacent store/service crate pairs earn their independent interfaces, using *depth* and the deletion test; whether any current placeholders should remain.
5. How public `Cva`/`Phylactery` API paths survive without duplicate live owner aggregates or long-lived compatibility scaffolding.
6. Exact unchanged invariants and tests required for each moved capability, and independent corpus recovery before promoting any final architecture.

**Next artifact:** an alternative-target architecture comparison and selected explicit ownership/dependency contract, *after* approving the factual inventory. Do not treat the old 30-crate map as that answer.

## Source map

- [Current architecture](architecture.md), [invariants](invariants.md), [storage format](storage-format.md), [scope ownership](subsystems/scope-ownership.md), [runtime multiplexing](subsystems/runtime-multiplexing.md), [subsystem index](subsystems/INDEX.md).
- [Crate-first architectural target](crate-first-architecture.md), [Root audit](crate-first-root-dependency-audit-2026-10-30.md), [history evidence](history-service-crate-extraction-evidence-2026-10-29.md), [maintenance evidence](maintenance-service-crate-extraction-evidence-2026-10-29.md).
- Actual Cargo manifests and production Rust at `Cargo.toml`, `src/lib.rs`, `src/facade/`, `src/runtime_inference/`, `src/imports/`, `src/rewrite/`, `src/perception/`, `crates/*/src/`, `cli/src/`, `tests/current_format_boundary.rs`.
