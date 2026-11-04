# Reliquary physical dependency architecture and crate layout — Stage 4 (2026-11-03)

Parent index: [Documentation index](INDEX.md)

**Stage 4 design: COMPLETE. Implementation checkpoint (2026-11-04):** the independent rebuild at `95db1b41` compiles, passes its current workspace tests, and matches the selected 23-package/76-edge DAG; **G1–G5 are not yet accepted**. See the [current continuation evidence](rebuild-continuation-2026-11-04.md). The source disposition/counts below describe the original codebase at design time, not the live replacement. This is the **selected physical package layout** grounded in the 2026-11-03 [current inventory](current-system-inventory-2026-11-03.md), direct `Cargo.toml` manifests, and [Stages 1–3](rel-phy-storage-architecture-2026-11-03.md), [semantic services](semantic-service-architecture-2026-11-03.md), and [Gateway/workflows](runtime-orchestration-architecture-2026-11-03.md). Canonical machine-readable edges: [target crate DAG](architecture/target-crate-dag-2026-11-03.json). [ADR 0047](decisions/0047-physical-dependency-architecture.md). [Source disposition map](physical-crate-source-disposition-2026-11-03.md).

**Execution clarification (ADR 0049):** The crates specified here are built **directly in a separate clean replacement worktree** using verified existing implementation. The original remains the stable reference; none of these merge/retirements requires a period of incremental in-place refactoring. See [clean rebuild plan](architecture-rebuild-implementation-plan-2026-11-03.md).

## Purpose

Finish the *where does the code live, and who can depend on whom* design before any code move. The goal is fewer meaningful deep modules, no Root or parent backreach, independently recoverable REL/PHY semantics, and uncomplicated future extension—not an arbitrary crate count or mechanically preserving the previous extraction sequence.

## Overview

Choose **one typed owner-session composition crate**, **deep domain-owner libraries**, **deep semantic service libraries**, **one stateless named-application-workflow library**, **one complete real Gateway library**, **one independent Project Environment library**, and a deliberately narrow **public product Root**. Prefer selective consolidation of small helper crates into a *real* owner rather than many permanent pass-through crates.

The target contains **22 supporting crates + the existing `reliquary-memory` public Root (23 packages)**. The current repo has **30 supporting crate directories + Root**, with `reliquary-perception-store` redundantly listed twice in workspace members. The target retains 20 current supporting crate names, retires/merges ten, and introduces two actual missing ownership boundaries: `reliquary-owner-session` and `reliquary-application`. All 30 current crates have an explicit disposition. Counts are **design decisions, not measured improvements**.

**Authoritative direct edge specification:** `architecture/target-crate-dag-2026-11-03.json`. An edge `A depends_on B` means A can import B. **Every unlisted edge is forbidden** until this design is explicitly revised; the JSON expresses the planned Cargo graph, not the current build. The target graph has **76 direct edges, no cycles** by an independent local traversal. Its 20 Root dependencies are deliberately restricted to curated public reexports and product-facing API composition, never direct storage mutation or inference execution.

## 1. Exact physical packages and their role

| Layer | Selected crates | Rules |
| --- | --- | --- |
| **Physical** | `reliquary-container` | File kind/header/frame/lease/clock/sync; no semantic imports |
| **Durable domain owners** | `reliquary-archive`, `reliquary-memory-core`, `reliquary-graph`, `reliquary-perception-store`, `reliquary-ego-store`, `reliquary-freshness-store`, `reliquary-vector-store`, `reliquary-insomnia-store` | Keep separately justified state/codec/replay modules; each depends on Container only; no owner may import a higher semantic evaluator |
| **Typed owner composition** | **`reliquary-owner-session`** (new) | Concrete `RelSession` and `PhySession`, exact open/replay/validation, typed admission/grouped commits, owner-qualified historical proofs. Depends on Container + all eight durable domain owners; never Switchboard, model inference, workflows or Gateway |
| **Semantic / provider primitives** | `reliquary-chronos`, `reliquary-switchboard`, `reliquary-vectors` | Chronos temporal work; Switchboard endpoint/transport/secret authority; Vectors compatibility/batching. Reusable without opening a REL |
| **Semantic evaluators / composition** | `reliquary-insomnia`, `reliquary-dream`, `reliquary-entity-resolution`, `reliquary-freshness`, `reliquary-retrieval`, `reliquary-context-engine` | Each owns its algorithms and a narrow typed input/output; may import canonical immutable values and genuine pure helper services, but no owner-session or Gateway; context final composition is here, not Ego |
| **Independent external repository** | `reliquary-project-environment` | Git/Lore project authority and exact Archive file/revision correlation; Archive dependency limited to immutable reference types |
| **Application workflows** | **`reliquary-application`** (new) | Named, domain-local read/prepare/evaluate/recheck/commit workflows, plus import/history/maintenance/external-source normalization; depends down on owner-session, services and Project Environment; **no mutable universal Application state** |
| **Gateway execution** | `reliquary-runtime-gateway` | **Real** GatewayRuntime/InstanceRuntime, REL/PHY worker controls, operational journals/host adapter, queue, continuation, stream/updates, mount fencing, recovery; depends on application, owner-session, Switchboard; no Root import |
| **Public product** | `reliquary-memory` (existing Root) | Narrow `Rel`/`Phylactery`, CLI/consumer public exports and trusted construction; no domain implementation, no mutable owner aggregate, no Gateway implementation |

**Ego synthesis** and **Observation synthesis** do not exist as implemented services today. Do not keep a permanent empty `reliquary-ego` package as evidence of extraction, nor create a phantom `reliquary-observation` crate. When there is real semantic behavior, it can join its rightful Perception/Ego deep module and earn a new package only if independent deployment/dependency leverage justifies one.

## 2. Target dependency DAG and meaningful rules

The JSON is authoritative for every direct edge. This simplified view is **not** a substitute for that edge set:

```text
reliquary-memory (public API/reexports only)
     |
     +--> reliquary-runtime-gateway --> reliquary-application
     |               |                       |
     |          owner-session <--------------+
     |               |                       |
     |        8 durable stores        semantic services / primitives
     |               |                       |
     |           container             canonical immutable owner values
     |
     +--> selected direct public value reexports from lower packages

Independent primitives: chronos and switchboard have no Reliquary imports.
Project Environment is independent VCS authority, imported by Application.
```

**Required dependency direction:** `Root → Gateway → Application → owner-session → durable store → Container`. Additionally, `Application → semantic services`; semantic services may import canonical **immutable** value models or legitimate lower pure semantic primitives. Actual service-to-service edges listed in the JSON are permitted (for example Dream→Chronos/Vectors and Entity Resolution→Retrieval helpers); a service must never import a workflow to call another evaluator.

**Specific dependency rules:**

1. `container` has **zero** internal dependencies. Store packages may import Container but must never import `owner-session`, Gateway, Switchboard, or a sibling's semantic inference.
2. `owner-session` is the **only composition point** for multiple mutable durable domain owners. No `stores_mut`, `container_mut`, `with_owner_context`, all-store registry or publicly exposed concrete field bag. It may expose bounded typed read values and named authenticated commit/receipt methods. The group carrier codecs for Insomnia, Dream first-cycle, Freshness and import must be independently replayable **without** loading the evaluator that created them.
3. Service imports of `archive`, `memory-core`, `graph`, `perception-store`, `freshness-store` or `vector-store` are **value/pure-algorithm imports only**. They do not grant a mutable Container/write permit or a durable owner session. Constructor/write routines requiring a live physical lease must not be available on a semantic service code path. This must be enforced by APIs and source-level negative tests; **Cargo DAG alone cannot prove a method-level no-write contract**.
4. `reliquary-application` imports relevant services and owner-session, but exports **named workflows**, not a `WorkflowRegistry` or omniscient `ApplicationContext`. Each implementation module owns its typed inputs, bounded preparation, exact commit command and retry result; code cannot access another workflow's mutable state because no global workflow state exists.
5. `reliquary-runtime-gateway` keeps `InstanceRuntime` **inside the same crate as the trusted Gateway**. This restores effective `pub(crate)` visibility for grant/selection/detach mutators without a new cross-crate controller abstraction. A public `Arc<InstanceRuntime>` exposes observation and safe request APIs only; no public authority mutation. If Instance is later moved into another crate, a private controller capability must be explicitly designed and tested first.
6. Host delegation/cancel/reconcile/stream journals are Gateway-owned **operational** truth. They never become owner-session semantic records. The Host adapter is a real external-provider seam. Switchboard separately implements model providers and contracts; Gateway host queue policy is not a Switchboard credential function.
7. `reliquary-memory` may import any of its **listed** lower packages for a **curated canonical public reexport** or narrow product construction. Such edges are *not* license to keep `src/runtime_inference/**` or `src/facade/cva*.rs` in Root. The normal Root source should be thin and independently testable through public entrypoints. CLI and integrators continue to depend on this public package, not an undocumented Root helper.
8. No package may depend back on `reliquary-memory`; no Gateway import from owner-session/application/service; no application import from durable store internal mutation modules; no persistent circular pair of service/store dependencies.

## 3. Deliberate merges and retirements

| Current crate | Target | Why this is not merely moving the knot |
| --- | --- | --- |
| `reliquary-echo` (~370 production lines) | `reliquary-archive::echo` | Separate *durable Echo authority* remains a private module, but native Archive is the same REL-local source-evidence layer. Context Engine reads only the relevant immutable Echo values. No duplicate Archive history |
| `reliquary-dream-store` (~1,326) | `reliquary-owner-session::records::dream` | First-cycle/Begin proof belongs in independently replayable owner-local record infrastructure, not Dream inference. Keep tests and domain-digest behavior |
| `reliquary-import-store` (~416) | `reliquary-owner-session::records::import` | Epoch admission and markers bound to one owner session; normalization/settlement policy remains Application |
| `reliquary-maintenance-store` (~259) | `reliquary-owner-session::records::maintenance` | Retained prune manifests are proof evidence; maintenance/rewrite decisions remain Application and copy-verify-promote |
| `reliquary-external-index-store` (~591) | `reliquary-owner-session::records::external_index` | Rebuildable external source index is not a standalone top-level service. Preserve distinct source authority and damaged-index behavior |
| `reliquary-owner-lifecycle` (~471) | `reliquary-owner-session::open` | Existing partial replay helper becomes part of the single canonical REL/PHY opener rather than an extra wrapper that still leaves Root's aggregate |
| `reliquary-history` (~108) | `reliquary-application::history` | Current helper is not a complete historical composer; put immutable owner-cut composition with the named higher-level workflow |
| `reliquary-import` (~787) | `reliquary-application::import` | Source normalization and exclusive import settle/authorize are one domain-specific application capability, while owner-side epoch records remain lower |
| `reliquary-maintenance` (~167) | `reliquary-application::maintenance` | Small selection/closure policy alongside the actual prune/rewrite/reconcile workflow, not an empty shell crate |
| `reliquary-ego` (4 lines) | Delete placeholder | No production synthesis exists; future Ego module/package only when real behavior is ready |

The counts are **production source-line snapshots** calculated from tracked crate source (excluding test-named Rust files), used to identify *candidate* shallow boundaries, not proof of semantic shallowness. Merging is selected only when semantic ownership can remain separate in private modules and the cross-package API ceases to earn its cost. **Do not merge** the independent, stateful Archive/Memory/Graph/Entity/Ego/Freshness/Vector/Insomnia stores just to reduce crate count.

### Owner-session internal modules (private by default)

`rel` (typed REL aggregate, identity/read/commands), `phy` (typed PHY aggregate), `open` (ordered replay, kind-checked admission), `publication` (grouped carrier codec and exact commit), `historical` (lower immutable owner proofs), and `records::{dream,import,maintenance,external_index}`. The domain owner crates remain canonical: owner-session does **not** add a second Memory/Graph/Ego state implementation. Internal modules must remain cohesive and reviewable, not giant index files.

### Application internal modules (no global mutable state)

`ingest`, `insomnia`, `dream`, `perception`, `freshness`, `retrieval`, `agentic`, `vectors`, `context`, `external_source`, `history`, `import`, `maintenance`. Each exposes a *named* operation or cohesive small family, with operation-specific proof and receipts. It is acceptable for this upper composition crate to import many services **because it owns no stores or model policies**. Do not replace Root with another 80k-line general-purpose Application engine; give each workflow a small coordinator interface and isolate its concrete implementation/tests.

### Gateway internal modules (one owner, not a crate per helper)

`admission`, `instances`, `mounts`, `owner_execution`, `conversation`, `journals`, `host`, `streams`, `events`, `workers`, `lifecycle` and `live`. Canonical `GatewayRuntime` is the only mounted owner/Instance registry. Canonical `RelJournal`, owner-operation journal and host authority remain scoped operational components, not replicated by Application or owner-session.

## 4. Interface compatibility and Rust visibility

**Source-level witness:** current `src/lib.rs` wires many path-based Root modules, `src/runtime_inference/runtime_host_gateway.rs` owns real Gateway, `runtime_host_instances.rs` has `pub(super)` privileged methods, `interaction_runtime.rs` owns the `Cva` aggregate. The public Root cannot simply reexport an unchanged Gateway while those higher-level private types remain its mutable internals.

**Canonical type location:** Owner IDs and durable domain value types are exported from their owning store or owner-session; `RelSession`/`PhySession` and typed commit/receipt types are in owner-session; `InstanceRuntime`, `GatewayRuntime`, access tokens and operational receipts are in Gateway; semantic proposal/result types are in their evaluator; multi-owner application result types are in Application. Public Root may reexport these types where consumers need them. No duplicate `Rel`/PHY owner struct or parallel `Cva` alias.

**Cross-crate visibility:** Rust `pub(crate)` never crosses crate boundaries. Keep privileged Gateway/Instance state in one crate to make trusted methods private. Authoritative owner-session methods callable by Application/Gateway must be `pub` but require possession of an opaque owner-session handle and typed validation; they cannot expose arbitrary stores. Store write primitives take the private session's `&mut Container`/typed write capability and are never surfaced through a semantic service. Document ordinary offline access separately from authenticated Gateway request access.

**No model I/O across borrow:** A service gets bounded owned immutable facts, not a `&mut RelSession` held across `await`; hot indexed read paths can return scoped views entirely within the short owner read phase. Preserve existing pooling/caching and concurrent owner isolation.

**No intermediate compatibility scaffolding:** Final canonical names are `Rel`, `RelSession`, `PhySession`, etc. Old `Cva` aliases, fake wrappers or legacy runtime readers do not survive the hard cut. The serialized format and identifier migration is **Stage 5**, and must not be inferred from crate names.

## 5. Root source disposition — no exemption

The [source disposition crosswalk](physical-crate-source-disposition-2026-11-03.md) covers each current crate and the Root neighborhoods. These are non-negotiable categories:

- `src/facade/**`: pure public REL/PHY entrypoints to Root; actual owner aggregates, lifecycle, typed readers/writers to owner-session; grouped application actions and maintenance to Application; nothing retained in Root merely because it currently says `impl Cva`.
- `src/runtime_inference/**`: complete Gateway/host/worker/stream operational code to Gateway; source evidence/authorization/commit composition to Application; algorithm/prompt/decision code to its semantic service; no generic runtime leftover.
- `src/perception/**`, `src/semantic_graph/**`: move Entity/Observation and Community policy or durable store implementation to their **actual** service/domain, and evidence/commit paths to Application.
- `src/imports/**`, `src/rewrite/**`, historical facade modules, `src/external_source/**`: source adapters and exclusive/offline transformations to Application, typed underlying records/proofs to owner-session or respective durable store.
- `src/retrieval/**`, `src/chronos/**`, `src/freshness/**`, `src/ego/**`, `src/config_security/**`, `src/memory/**`, `src/echo/**`, `src/project_environment/**`: classification by function to the named service/store/Workflow/Gateway, never left in Root as a default.
- `src/lib.rs`: replace `#[path]` internal links to implementation with curated `pub use` exports and simple public entrypoints; no retained Root owner state or policy implementation.

Root tests may legitimately exercise public integration, but production code cannot retain orphaned implementation simply because an extraction has an unresolved signature. No silent “temporary permanent Root” escape.

## 6. Architectural verification and future feature test

**Design validation already performed on JSON:** All 30 present supporting crates accounted for (20 retained, 10 merged/retired), two new packages, 22 supporting targets + Root, no dangling edges/self-dependencies, no dependency cycles, no lower dependency back to public Root/Gateway/Application. This is *not* `cargo metadata` for the **new layout** and not a compilation claim. Arcana's available graph reports 114,830 nodes/316,488 edges but is an older snapshot, so it is **navigation-only**, not evidence that the target dependency boundary is met.

**Implementation ratchets required:**
1. Actual `cargo metadata` at each integration cut matches the target direct edges; detect edges to Root, reverse imports, cycles, unused vestigial packages and duplicate members. No heavyweight architecture-evidence framework needed: manifest comparison and targeted source searches suffice.
2. Every lower crate compiles independently, followed by Application, Gateway, public Root and detached CLI. Target directories are checked for production code, not empty `lib.rs` markers. `cargo fmt --check`, locked offline `cargo check/test` and binary/CLI contracts at integration gates.
3. Compile-time **negative access tests**: external holder of `Arc<InstanceRuntime>` cannot change grants/selection/detach; semantic evaluator cannot obtain a session write handle or acquire a physical Container through its approved direct imports; public Root cannot expose `&mut` stores. Verify method signatures and actual consumer paths, not only import graph.
4. Replay and recovery gates: current-only six-file corpus (including the still-broken `CVAFRS07` REL reopen), wrong kind, grouped Insomnia commit, first-cycle Dream/Freshness receipt, native/external-source separation, owner local history, tombstones and audit provenance.
5. Concurrency/host gates: owner-local lock timing, provider outside lock, queue cancel/dequeue/finalize crash matrix, multi-Instance/PHY privacy, staged generation checkpoint sync before stream visibility, host ambiguous-status reconciliation.
6. Behavior/performance parity: frozen Insomnia/Dream/Entity/Freshness/Retrieval/Chronos/Vector scenarios and existing baseline measurements, deterministic output/provenance/identity, bounded memory and no copying whole Web for interface crossing.
7. **Growth exercise:** adding an Observation evaluator or Ego producer must not cause edits to Container, existing unrelated semantic evaluators, public Root implementation or an all-owner registry. If durable records are introduced, add only their owner-local codec/replay/read/command plus a focused workflow/worker and documented DAG edge.

**Subsequent design is now available:** [Stage 5](cva-to-rel-format-and-identity-migration-2026-11-03.md) specifies the REL format/data conversion, and [Stage 6](architecture-rebuild-implementation-plan-2026-11-03.md) specifies direct **selective-reuse** capability construction, not old-code relocation. Their implementation, corpus validation and cutover have not begun.

## Related docs

- [Machine-readable target DAG](architecture/target-crate-dag-2026-11-03.json)
- [Current physical inventory](current-system-inventory-2026-11-03.md)
- [Source disposition crosswalk](physical-crate-source-disposition-2026-11-03.md)
- [ADR 0047](decisions/0047-physical-dependency-architecture.md)
- [Stage 1 owner sessions](rel-phy-storage-architecture-2026-11-03.md)
- [Stage 2 services](semantic-service-architecture-2026-11-03.md)
- [Stage 3 runtime/workflows](runtime-orchestration-architecture-2026-11-03.md)
- [CVA naming/wire decision](decisions/0043-rel-terminology-and-wire-identity.md)

## Notes

**Stage 4 is a complete physical design, not a completed source refactor.** The old `docs/architecture/crate-map.json` and `component-dag.json` remain historical evidence until implementation replaces them with the selected target; the new target DAG has an explicit `design-only` marker to avoid being misread as current passing architecture. No new inference capability, data format, or external protocol is implemented here.
