# Physical crate and Root source disposition — Stage 4 (2026-11-03)

Parent index: [Documentation index](INDEX.md)

**Status:** target package mapping against the existing tracked Rust layout; **not a completed move or compiler-qualified call-site migration**. The [authoritative allowed-edge DAG](architecture/target-crate-dag-2026-11-03.json) lists every package and all ten retirements. Target decisions: [Stage 4 architecture](dependency-architecture-and-crate-layout-2026-11-03.md), [ADR 0047](decisions/0047-physical-dependency-architecture.md).

## Purpose

Give each existing Cargo package and Root production neighborhood a concrete future destination and identify the dangerous transitions. Prevent claiming architectural completion when Root implementation or an old helper crate still owns a large responsibility.

## Overview

Current manifest: 30 distinct `crates/reliquary-*` directories and one `reliquary-memory` Root (one duplicate `reliquary-perception-store` workspace-member entry). Proposed: 22 supporting crates and Root, with two new real owners and ten merges/deletions. The current Root `src/lib.rs` composes and reexports extensive `#[path]` modules; this physical design must disassemble it by **actual authority**, not rename the entire Root as Application.

## 1. Existing 30 supporting packages — exhaustive disposition

| Current crate | Target | Decision and boundary |
| --- | --- | --- |
| `reliquary-container` | **Retain** | Physical file framing/identity/leases; no semantic imports |
| `reliquary-archive` | **Retain**; also absorb Echo modules | Archive native evidence and independent durable Echo subdomain |
| `reliquary-memory-core` | **Retain** | Owner-local Memory body, versions, tombstones, provenance |
| `reliquary-graph` | **Retain** | Typed Graph, Community/derived topology authority |
| `reliquary-perception-store` | **Retain** | Entity and transitional Relationship records; future Observation owner state |
| `reliquary-ego-store` | **Retain** | Identity/Personality/Anchors; semantic Ego synthesis not present |
| `reliquary-freshness-store` | **Retain** | Durable accepted events, scores and replay history |
| `reliquary-vector-store` | **Retain** | Vector/profile/generation domain authority |
| `reliquary-insomnia-store` | **Retain** | Episode work/lease/completion durable evidence |
| `reliquary-echo` | **Merge into `reliquary-archive::echo`** | Keep a separate named source-keyed Echo owner module and proof tests |
| `reliquary-dream-store` | **Merge into `reliquary-owner-session::records::dream`** | First-cycle journal/Begin proof and canonical digest independent of Dream inference |
| `reliquary-import-store` | **Merge into `reliquary-owner-session::records::import`** | Owner-local import epoch markers/codec; import logic remains higher |
| `reliquary-maintenance-store` | **Merge into `reliquary-owner-session::records::maintenance`** | Prune-manifest proof metadata rather than a separate mutation authority |
| `reliquary-external-index-store` | **Merge into `reliquary-owner-session::records::external_index`** | Disposable host-source derived indexes; retain source separation and corruption/rebuild semantics |
| `reliquary-owner-lifecycle` | **Merge into `reliquary-owner-session::open`** | Ordered replay/helper becomes canonical full REL/PHY open composition, not an intermediate facade |
| `reliquary-switchboard` | **Retain** | Credential/provider/endpoint routing, not domain policy |
| `reliquary-chronos` | **Retain** | Temporal inference/deterministic parsing, no persistence |
| `reliquary-vectors` | **Retain** | Embedding/compatibility/batch pure capability, despite small extracted surface |
| `reliquary-insomnia` | **Retain** | Cohesive extraction/ownership evaluator; no durable work ownership |
| `reliquary-dream` | **Retain** | Pair verification, canonical/lifecycle/naming evaluator |
| `reliquary-entity-resolution` | **Retain** | Canonical Entity matching/admission, **not** all of future Perception |
| `reliquary-freshness` | **Retain** | Deterministic scoring/propagation, separate accepted-use proof |
| `reliquary-retrieval` | **Retain** | Hybrid/Community/graph candidate algorithms, agentic turn-local selection |
| `reliquary-context-engine` | **Retain** | Final model context assembly and compaction; Echo immutable value import moves to Archive |
| `reliquary-project-environment` | **Retain** | Git/Lore repository integration; immutable Archive reference types only |
| `reliquary-import` | **Merge into `reliquary-application::import`** | Normalize source and coordinate exclusive epoch, no general-purpose import runtime |
| `reliquary-history` | **Merge into `reliquary-application::history`** | Immutable owner-cut historical composition, no heavy partial-helper package |
| `reliquary-maintenance` | **Merge into `reliquary-application::maintenance`** | Rewrite/retention/reconciliation decisions with copy/verify/promote workflows |
| `reliquary-ego` | **Delete placeholder** | No working Ego synthesis yet; preserve existing Ego Store. Add real service only with new behavior |
| `reliquary-runtime-gateway` | **Retain name; replace placeholder with full real runtime** | All mounted Gateway/Instance/journals/stream/host/cancellation/worker responsibilities leave Root |

**New:** `reliquary-owner-session` (typed private REL/PHY aggregate and exact replay/commit) and `reliquary-application` (stateless named workflows, history/import/maintenance integration). **Root:** existing `reliquary-memory` remains as public product package but cannot own old behavior.

## 2. Root neighborhoods — exhaustive top-level directory disposition

The following inventories use a simple count of `src/**/*.rs` paths (includes tests/fixtures and is not a code-size or semantic-ownership proof). The relevant implementation can split across several owners, but **each family must have no unjustified production leftovers in Root**.

| Current Root path | Files observed | Target ownership |
| --- | ---: | --- |
| `src/facade/` | 218 | `owner-session` for durable aggregate/lifecycle/typed mutations/proofs; `application` for multi-domain/historical/maintenance/workflow code; product-only methods as direct narrow Root facade |
| `src/runtime_inference/` | 255 | `runtime-gateway` for host/Instances/workers/queue/journals/streams; `application` for evidence/commit coordination; semantic evaluators in respective service packages |
| `src/perception/` | 71 | `entity-resolution` pure policy, `application::perception` authorization/hydration/commit, `perception-store` durable facts; future Observation service stays unimplemented |
| `src/retrieval/` | 54 | `retrieval` pure rank/candidate algorithms, `application::retrieval` authorized query/hydration, `vectors` compatibility/packing and `owner-session` read proofs |
| `src/imports/` | 29 | `application::import` source normalization/exclusive workflow; `owner-session::records::import` durable epochs |
| `src/semantic_graph/` | 26 | `graph` topology/Community derivation; `dream` naming policy where appropriate; `application::dream` publish/verification |
| `src/memory/` | 17 | `memory-core` canonical type/store/persistence; `owner-session` typed projection; `application` genuine cross-owner workflows |
| `src/external_source/` | 11 | `application::external_source` validation/hydration/host evidence; `owner-session::records::external_index` durable index |
| `src/config_security/` | 8 | `switchboard` config/credential/provider security; only trusted host composition at Gateway |
| `src/ego/` | 8 | `ego-store` actual persistence, `owner-session` read/commit; no invented Ego synthesis |
| `src/rewrite/` | 6 | `application::maintenance` plan/transform/reconcile; `owner-session` retained proof/admission only |
| `src/chronos/` | 4 | `chronos` temporal algorithms/adapters; application workflow for body/source binding |
| `src/freshness/` | 4 | `freshness` math/policy, `application::freshness` source event proof, owner-session durable event acceptance |
| `src/echo/` | 1 | `archive::echo` retained record code or `context-engine` read/render, according to actual body |
| `src/project_environment/` | 1 | `project-environment` Git/Lore source authority, application-level management |
| `src/archive/` | 0 | No current Root source requiring a destination |

**Top-level Root files:** `src/lib.rs` becomes curated public exports; `src/owner_lifecycle.rs` moves to owner-session; `src/rewrite.rs` and `src/rewrite_lineage.rs` split to Application and durable lineage; `src/freshness_dream_journal.rs` splits owner journal from Freshness workflow; `src/project_environment.rs` moves to Project Environment/Application as appropriate. Remaining top-level `*_tests.rs` are behavior fixtures to relocate or keep as product integration tests.

## 3. Specific type/authority hazards

| Currently convenient shortcut | Target correction and regression risk |
| --- | --- |
| `Cva` with 35 fields and `Phylactery` with 19 shared through `pub(crate)` | Only typed `RelSession`/`PhySession` privately compose; preserve all owner/history fields and keep runtime due/notifications above |
| `InteractionRuntime { cva: Cva, sessions }` | Split durable aggregate into owner-session, sessions/selection into Gateway, native accepted-turn and Episode commit into Application |
| `RelShared { runtime: Arc<Mutex<InteractionRuntime>> }` | Own one session handle and one runtime-only worker coordination context; no model call while owner lock held |
| `InstanceRuntime::replace_grants`, `detach`, `select_rel` are `pub(super)` | Keep Instance and Gateway in same crate and privileged mutation private; never give arbitrary callers escalation rights |
| `reliquary-context-engine` imports `reliquary-echo` | Import **immutable Echo value types** via Archive after Echo merge; do not give Context Engine Archive write APIs |
| `reliquary-entity-resolution` imports Retrieval | Limit to pure ranking/candidate helpers; no full search controller/owner hydration dependency |
| `reliquary-freshness` imports `freshness-store` | Pure policy types/history facts only, no direct store commits |
| `reliquary-vectors` imports `vector-store` | Immutable profile/generation types; service cannot persist without a private owner session |
| `reliquary-dream` imports Graph/Memory types | Pure value/semantic policy only; no backdoor Graph or Memory owner mutations |
| `reliquary-project-environment` imports Archive | File/revision value references only; VCS is its own authoritative source |
| `reliquary-memory` Root direct imports many lower libraries | Only curated public type reexports and narrow construction; no retained hidden implementation under `src/lib.rs` |

## 4. Migration classification rules and acceptance

For every **production** file in Stage 6, record a target package/module, action `keep`/`move`/`merge`/`split`/`delete`, exact provider/owner boundary and tests. A whole source file containing *both* semantic evaluation and grouped commit must be **split**, not assigned intact to an arbitrary crate. No blanket `Cva` rename is considered a valid ownership move.

The current package map above is exhaustive for Cargo membership and Root directory families, **not each of the ~hundreds of individual functions**. That more granular migration crosswalk must be done before code changes. Implementation should verify `Cargo.toml` direct edges match the [target graph](architecture/target-crate-dag-2026-11-03.json), enforce no Root reverse edges, and compare current behaviors, durable recovery, host queue/stream/security and performance.

## Related docs

- [Stage 4 physical architecture](dependency-architecture-and-crate-layout-2026-11-03.md)
- [Machine-readable target DAG](architecture/target-crate-dag-2026-11-03.json)
- [Current-system inventory](current-system-inventory-2026-11-03.md)
- [Root dependency audit](crate-first-root-dependency-audit-2026-10-30.md)
- [ADR 0047](decisions/0047-physical-dependency-architecture.md)
- [Stage 1 method ownership crosswalk](rel-phy-method-ownership-crosswalk-2026-11-03.md)
- [Stage 2 service crosswalk](semantic-service-boundary-crosswalk-2026-11-03.md)
- [Stage 3 runtime crosswalk](runtime-orchestration-source-crosswalk-2026-11-03.md)

## Notes

This is a source-grounded package and directory disposition map. The directory counts include tests and are not claimed as a compiler AST inventory. The existing `docs/architecture/crate-map.json` remains historical evidence, not the selected new DAG. The tracked non-design Graph evidence modification and untracked scratch files are not part of this design.
