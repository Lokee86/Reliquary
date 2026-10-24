# Phase 5 — Runtime and Orchestration Dependency Convergence

**Status:** Implemented; final acceptance and worktree closure are recorded in the [execution receipt](dependency-convergence-phase5-execution-2026-10-23.md).  
**Date:** 2026-10-23.  
**Repository:** Reliquary; integrated Phase 4 baseline `10cbe4b7`, Phase 5 source checkpoint `fe9948e`. The original planning checkpoint was `4785ad1`.  
**Campaign:** [Dependency Convergence Architecture Refactor](dependency-convergence-architecture-refactor-2026-10-21.md), Phase 5.  
**Execution prerequisite:** The bounded C-01–C-04 source-contract dispositions and committed Phase 4 acceptance were verified before implementation. Full symbol-level production graph proof remains unavailable and is explicitly bounded in the execution receipt.  
**Scope:** Architecture correction only. No new model capability, scheduling feature, Context Engine compaction policy, ingestion format, or persistent schema.

## Purpose

Remove runtime-to-domain backedges and accidental runtime ownership of domain state without combining the existing runtime lifetimes. Replace obsolete `ReliquaryRuntimeHost` ownership assumptions, duplicated forwarding, and cross-owner state reach-through with one-way execution and explicit owner/service contracts. Make **actual compiled production source dependencies**, not just the hand-authored component DAG, the principal architectural acceptance evidence.

## Overview

The desired change is **dependency convergence, not runtime consolidation**. `GatewayRuntime`, `InstanceRuntime`, `ConfiguredRuntime`, owner-specific REL/PHY workers, the Context Engine, and semantic services remain distinct where they already own distinct responsibilities.

The current `src/runtime_inference/**` tree contains approximately **309 Rust files / 84,434 lines including tests** at the inspection point. This is the *inventory surface*, not a rewrite estimate. `GatewayRuntime` and `InstanceRuntime` already implement important authority separation under ADR 0038. `RelRuntime` and `PhyRuntime` already execute shared owner work. `ConfiguredRuntime` already wraps configured provider routes. The existing `runtime_host.rs` is mostly a module/re-export hub: there is **no verified live `struct ReliquaryRuntimeHost` to replace**. Its remaining `ReliquaryRuntimeHostError`, `ReliquaryRuntimeRoutes`, modules and public symbols must be classified by behavior and external usage, not deleted by name.

The hard cuts are about removing the wrong **arrows** between these parts. Preserve all surviving capabilities and user-observable semantics of mounted owner execution.

## Problem statement and source-backed baseline

| Verified current source | Ownership problem / preservation contract |
| --- | --- |
| `src/runtime_inference/runtime_host_gateway.rs` | `GatewayRuntime` holds maps of keyed REL and PHY executions, instances, mount identity, route state, host routing, admission fencing and conversation state. **Retain** one authoritative execution per durable owner ID; do not turn this into a generic service locator. |
| `src/runtime_inference/runtime_host_instances.rs` | `InstanceRuntime` owns principal, attached PHY, grant generations, selected REL and managed conversation selection. It **does not own a shared REL/PHY worker**, and detaching it must never unmount another instance's owner. |
| `src/runtime_inference/runtime_host_rel_execution.rs`; `runtime_host_phy_execution.rs` | `RelRuntime` runs owner-scoped Insomnia, Dream, vectors, and Perception workers through a shared owner handle; `PhyRuntime` runs PHY work. Retain the one-owner worker lifetime, quiesce/reopen fencing and cancellation semantics. |
| `src/runtime_inference/interaction_runtime.rs` | `InteractionRuntime` currently holds a `Cva` plus session state; `RelRuntime` wraps it. Do not create another authority, and do not relocate public `Cva` API cleanup into this phase without an actual dependency-cut reason. |
| `src/runtime_inference/runtime_host_gateway_conversation.rs`; `multiplexing_journal.rs` | Submission ordering, provisional durable queue, conflict policy, final Archive admission, idempotent recovery and pending cancellation are already implemented; maintain one decision owner per state transition. |
| `src/runtime_inference/runtime_host_gateway_host.rs` | `GatewayHostAdapter` is an existing **true external** capability boundary: negotiation, handoff, cancellation, status and reconciliation. Preserve selective host delegation, not a duplicate native/host dispatch of the same work. |
| `src/runtime_inference/runtime_host_gateway_snapshots.rs`, `runtime_host_gateway_streams.rs`, `runtime_host_gateway_reconnect.rs`, `interaction_live.rs` | REL feeds, conversation streams, watermark/snapshot resync, durable-before-visible checkpoint and privacy delivery. Preserve status and ordering; no parallel duplicate stream registries. |
| `src/runtime_inference/configured_runtime.rs`; `model_switchboard.rs`; `embedding_endpoint.rs` | Configuration and provider endpoints exist independently from Gateway and Instance. `ConfiguredRuntime` also exposes explicit offline Cva operations. Keep the externally meaningful offline behavior; separate endpoint contract values from accidental runtime ownership where an actual lower-level backedge exists. |
| `src/runtime_inference/runtime_host_gateway_ambient_project.rs`; `runtime_host_gateway_project_lifecycle.rs` | Gateway handles mounted grants, pre/post-I/O reauthorization and delivery around Project Environment calls. Consume **Phase 4's final validated project-scope contract**, do not rewrite its path/authority logic. |
| `src/context_engine.rs`; `src/context_engine/**` | Deterministic context projection/compaction already exists; planned wider cross-chat/Ego assembly does **not**. Make the current Conversation/Echo/Inference call direction correct; do not implement the deferred compaction redesign. |
| `src/runtime_inference/runtime_host_gateway_external.rs` and `src/external_source/**` | Keep external transcript integration distinct from Project Environment and Import. Host-provided transcript authority and model host routing should cross named existing ports, never a lower-to-Gateway callback. |
| `src/runtime_inference/runtime_host.rs`; `src/lib.rs` | Legacy-named runtime modules and broad exports may include supported user-facing contracts. **Only remove proven obsolete internal forwarding** here; Phase 6 owns general public-facade/export retirement. |

### Evidence inherited from Phase 1–3 closure

The original Arcana area SCC mixes owner stores, services, public `impl Cva` extensions, runtime, tests and immutable value references. A validated acyclic 34-component *target* is not proof the source obeys it. Use the active C-01–C-04 **actual production-source component mapping** for a Phase 5 before image. Earlier accepted graphs and the original Phase 0 snapshot remain immutable. Do not count test-only edges, immutable IDs, or file-path-only re-exports as unapproved runtime callbacks.

Known actual reverse dependencies warrant special review: vector/profile values stored under Runtime's `embedding_endpoint.rs` are consumed by lower Retrieval; Insomnia owner/store/codec files are physically under `runtime_inference`; the runtime tree has `Cva` and `Phylactery` composition code. Treat each as **value-only**, **owner store**, **service**, **runtime execution**, or **public extension** before deciding the cut. The C-01–C-04 closure receipts are now recorded: source-import SCCs have classified residual edges (including an exact Phase 6 Perception façade register), while Arcana's induced file-node graph is explicitly not a complete symbol-level proof. At B5 use the accepted post-Phase-4 source tree and refresh only changed module ownership and source-edge deltas; do not repeat a whole-node Arcana export or treat classification as elimination.

## Controlling decisions and invariants

1. **ADR 0038 is the runtime lifetime authority.** Gateway is shared and authoritative for mounted REL/PHY execution; Instance is an independently disposable, authorized view. It is not a second semantic store or independent worker. A principal may have multiple instances; authorized instances may share a REL and the same attached PHY. Never leak one principal's PHY state into another principal's REL feed.
2. **One durable owner, one active execution, one writer.** Stable owner ID, mount epoch, physical writer exclusion, lease generations, worker shutdown/reopen and reconciliation remain valid across remounts.
3. **Gateway owns cross-instance coordination and authorization.** Admission, queue state transition, cancellation, subscription, reauthentication, post-I/O authorization, and delivery belong above owner state. Lower owners may validate and publish their own semantic evidence but never choose a principal or grant.
4. **REL and PHY processing are owner-local.** Insomnia, Dream, Perception, Ego, Freshness, Vectors and their stores remain the named domain owners. Runtime schedules those services but does not duplicate their semantic rules, replay engines or persistent stores. In particular, the in-progress Phase 1–3 Insomnia import-replay correction must be consumed, not reimplemented.
5. **Existing queue and stream contracts are sacred.** Default sequential queue and selectable automatic branching, provisional cancellation until Archive finalization, authoritative ordering, idempotent durable recovery, host delegation semantics, reconnect snapshots, watermark/gap handling, checkpoint-before-delivery, subscription-specific fan-out and revocation must stay unchanged.
6. **External inference cannot hold a long owner lock.** Freeze accepted inputs, release owner/queue locks, call the chosen endpoint, then reacquire a guarded publication path to validate freshness, cancellation, ownership and generation before durable write. Preserve error ordering, at-most-once dispatch and the actual provider cancellation contract.
7. **ConfiguredRuntime is not another Gateway.** It owns endpoint/configuration wiring and explicitly supported offline operations, not mount selection, queue execution, grants, publication, or semantic authority. Switchboard/provider transport is an external capability; it does not decide owner-local semantic policy.
8. **Project Environment is consumed, not redesigned.** Adopt Phase 4's accepted validated `ProjectEnvironment` scope for project file/history operations. Gateway still independently owns `ViewRel`/`ManageProject` checks and post-I/O delivery. Do not authorize a sibling subtree on matching repository ID alone.
9. **Context Engine ownership is narrow in this phase.** Keep current deterministic context/compaction and Echo facts; ensure read-only selection and owner-authorized provider requests. Deferred full Ego/cross-chat assembly and the current-session compaction refinement plan remain separate.
10. **Correction only.** No new queue type, provider, model route, host integration, separate daemon, multi-user semantics, memory format, Cloud sync or generic runtime framework. No gratuitous test multiplication. Current on-disk/wire/public contracts remain supported unless separately approved.
11. **Phase boundaries:** Phase 4 owns Project Environment service and project authority; Phase 5 owns runtime internal execution and dependency directions; Phase 6 owns public Cva/Phylactery facade relocations, external export names and general public API cleanup; Phase 7 owns final campaign graph closure.
12. **Hard-cut migration:** Move internal ownership once to a canonical home; delete obsolete internal wrapper/route when its last consumer moves. An explicitly tracked red window is permitted between coherent slices; no temporary dual-runtime/shadow queue or adapter just to make intermediate builds green.

## Target architecture

### Distinct responsibilities and call direction

| Component | Owns | Must not own / call upward |
| --- | --- | --- |
| **`GatewayRuntime`** | Mounted REL/PHY registry; trust-bound host admission; owner capability tokens, mount epochs and fences; shared work scheduling, provisional queue, host handoff/reconcile; REL feed, stream fan-out, publication/delivery coordination | Memory/Entity/Ego semantic rules, physical record codec, project path proof, Context Engine token policy |
| **`InstanceRuntime`** | One principal, attached PHY identity, exact owner grants/generations, active REL and per-REL conversation/branch selection | Durable REL/PHY ownership, worker execution, separate semantic persistence, mutable global owner selection |
| **`RelRuntime` / `PhyRuntime`** | A single mounted owner's worker lifecycle, guarded owner-local execution, pause/drain/resume, work notifications | Cross-instance global selection, new authority over principals, duplicate durable queue/owner store |
| **`InteractionRuntime`** | Existing REL interaction/session admission boundary and canonical Archive publication delegation | Model/host transport, another durable transcript/queue, general public facade management |
| **`ConfiguredRuntime` and existing provider ports** | Configured routes, endpoint construction, model-auth transport choices, controlled offline invocations | Instance/REL mount maps, semantic processing rules, owner grants, publishing Memory/Graph/Ego on their behalf |
| **Context Engine** | Existing deterministic context-view/compaction policy, read-only Echo/conversation projections, authorized request assembly at current implemented scope | Transcript authority, user grants, provider transport, Ego synthesis, new deferred compaction algorithm |
| **Owner/domain services and stores** | Their accepted read/proof/mutation contracts and semantics | Gateway/Instance handles, runtime host presentation types, host capabilities, upward callback to public facade |
| **Project Environment** | Phase 4 validated repository/project authority, Lore/Git operations | Gateway grants, session state, owner publication, runtime/provider transport |

**Canonical source direction (simplified):** Public consumers (Phase 6) → Gateway/Instance admission → owner execution → domain/owner-lifecycle services → stores → Container. Gateway also invokes narrow Context Engine, Project Environment, External Sources and provider ports. Owner-lifecycle and stores **never** import Gateway or Instance implementation. ConfiguredRuntime supplies the provider capabilities used by those operations; it does not participate in the mounted-owner lifetime.

The **logical architecture contains one `runtime-gateway` component** in the existing 34-component target DAG. This does **not** mean `GatewayRuntime`, `InstanceRuntime`, `RelRuntime`, `PhyRuntime` or `ConfiguredRuntime` should be merged as Rust types. They remain separate, deeper internal modules under that logical orchestration tier. A new physical crate or executable is *not* required.

### Canonical internal interfaces and seam decisions

- **Mounted owner execution access:** Keep Gateway-owned, owner-qualified, epoch-fenced access to REL/PHY execution. Prefer the existing `with_rel`, `with_phy`, token and owner-lifecycle capabilities, reducing exposed state reach-through. A retained private owner handle may carry exact owner identity/guard and a deliberately small read/mutate operation surface; it must not expose `GatewayRuntime` or grant arbitrary `Cva` access to lower domains.
- **Instance admission:** Keep `InstanceAuthorization`, `OwnerGrant`, owner-specific grant generation and attachment incarnation as distinct values; frozen instance tokens are never durable owner authority. Do not collapse current per-owner grant generations into one global version.
- **Worker coordination:** Workers receive owner-local guarded facts and narrow domain inputs from `RelRuntime`/`PhyRuntime`; they cannot reach ambient Gateway instance maps or public `Cva` composition to fetch domain authority. Preserve the existing work-signal and fence behavior unless the source graph proves a specific duplicated coordinator.
- **Host route/cancellation seam:** Retain `GatewayHostAdapter` with capability negotiation, stable operation identity, handoff/status/cancel reconciliation. Do not invent a second adapter per semantic subsystem; use the existing `GeneralEndpoint`, `DecisionEndpoint`, `EmbeddingEndpoint` traits at the actual remote provider seam.
- **Configured endpoint handoff:** Resolve provider routing once for the operation, pass a bounded selected endpoint or endpoint request to the semantic service, and return immutable outcome/evidence for guarded publication. A transient dependency on a configured value is not a reason to move owner stores into ConfiguredRuntime.
- **Context and Echo:** Call Context Engine using explicit authorized conversation/Echo evidence and token-counter inputs; return `ContextRequest`/view/plan, without storing Gateway/Instance references. Keep compaction compaction-checkpoint format and policy unchanged.
- **Project operations:** Phase 4 passes canonical project-scope result and exact owner/publication facts into Gateway. Gateway wraps them with current instance grant admission and post-I/O revalidation, preserving fail-closed behavior.
- **Public boundary:** Public `Cva` and `Phylactery` entrypoints still required by standalone/offline callers can remain public for Phase 6. **No lower owner/service module may depend on them solely to perform a runtime operation after Phase 5.** Where a public `impl Cva` lives in a lower-looking path, classify its *actual owner* before changing it; avoid a Phase 5 mass API move.

### Design alternatives considered

| Alternative | Consequence | Decision |
| --- | --- | --- |
| Merge Gateway, Instance and ConfiguredRuntime into one runtime | Recreates shared selection, mutable service locator and provider/owner coupling; violates ADR 0038 | **Reject** |
| Preserve runtime roles and hard-cut only confirmed cross-boundary responsibilities | Smallest ownership cut; retains existing durable execution behavior and tests | **Choose** |
| Add another generic runtime/provider/owner abstraction layer | Shallow adapters, duplicated transition authority, migration costs without demonstrated substitutability | **Reject**, unless a concrete host boundary already justifies one |

## Phase 4 contract dependency

Phase 5 planning uses the Phase 4 spec's *target* validated project authority and Gateway obligations. The following are **implementation handoff facts**, to be copied from the accepted Phase 4 receipt at Phase 5 B5, not assumptions about filenames:

| Consumed contract | Phase 4 must deliver | Phase 5 changes |
| --- | --- | --- |
| Repository/project scope | Accepted validated backend, repository ID, selected subtree and authorized historical path/revision | Use the service at Gateway's existing project request entrypoints; no direct raw-path bypass |
| Archive correlation publication | Explicit owner-lifecycle/Archive publication evidence and preconditions | Keep Gateway grant and commit/delivery fencing; do not publish project history directly |
| Historical project file bytes | Authorized read that checks accepted REL association and valid historical project path | Preserve pre-I/O + post-I/O authorization across external Lore/Git reads |
| Reassociation and relocation | Explicit Project Environment outcome, fail-closed on ambiguous moves | Do not treat same-repository arbitrary project path as an authorized relocation |
| Project callsite deletion | Register of old `Cva`-dependent internal paths retired by Phase 4 | Do not reintroduce them while cleaning runtime consumers |

Planning-state prerequisite (satisfied by the committed Phase 4 receipt before execution): B5 is blocked until the exact Phase 4 interfaces and final source checkpoint are integrated and accepted. The Phase 4 execution receipt currently reports implementation locally complete, 25/25 focused Project Environment tests and 32/32 CLI tests passing, but broad library/integration verification and final source-graph acceptance pending; its Windows-only host has not compiled Unix-specific write guards. These are Phase 4 integration obligations, not reasons to duplicate their implementations in Phase 5. P5.0 planning may proceed; do not start Phase 5 source work against the uncommitted worktree.

## Implementation strategy — staged hard cut

Only one canonical internal path is permitted after each integration gate. Test the accepted behavior through existing high-level Gateway, live interaction, configuration, and ownership seams; do not create layer-specific scaffolding to artificially increase test counts.

- **Worktree rule:** Work on a dedicated Phase 5 branch/worktree from the integrated Phase 4 checkpoint, *not* today's dirty main or an old Phase 3 worktree. Freeze the exact baseline and preserve any concurrent C-01–C-04 changes separately.
- **Boundary-design window:** Finalize consumer register, call directions and migration DAG before source edits. Compare whether an interface is a value-only contract or an actual capability/semantic callback.
- **Controlled red window:** A coherent internal migration may temporarily break whole-repo builds, but the affected modules, precise worktree and target integration gate must be recorded. Targeted compiles/tests and source dependency checks remain mandatory; don't add throwaway wrappers to fake continuity.
- **Move once:** Introduce only final private execution/owner interfaces, migrate each real consumer directly, and remove obsolete internal methods and old-owner-only fixtures when their final consumer moves.
- **Integration gate:** Build all targets, run mandatory functional suites, replay exact current-format corpus and concurrency/authorization tests, compare component graph before/after and record no unreviewed new forbidden backedges. Worktree/branch changes must be integrated, explicitly superseded or independently archived before declaring P5 complete.

## Execution specification — 45 source-bounded tasks, eight gates

**Execution-only decomposition; no scope expansion.** The original eight responsibility groups and gates (B5, O5, G5, Q5, E5, S5, D5, A5) are unchanged. The earlier 29-item breakdown conflated independent worker migrations, queue/host/stream paths, provider operations, and graph verification. The following **45 tasks** are grounded in *existing* Rust source seams. No new capability or architecture layer is authorized by the increased denominator.

**A task is bounded by its named source/caller family and one observable cut or preservation proof.** A task can close **not applicable—no forbidden edge found** with cited source/checker evidence; it must not manufacture code changes or tests merely to claim progress. A changed task must specify the removed old edge/path, final owner contract, and existing focused regression surface. Each execution entry must record `pending | in_progress | blocked | done | not_applicable`, a commit or no-op justification, and a source/test receipt. The full test suite and full graph scan run at a *gate*, not after every edit. This is not a license to defer local compilation or relevant tests.

**Baseline and mapping ownership:** C-01–C-04 independently own first-time production source → 34-component mapping, historical comparison, and earlier-phase proof. Phase 5 **consumes** their accepted map/checker and records only *incremental runtime-owned changes*; it must not restart or duplicate those audits. B5 still requires C-01–C-04 and accepted Phase 4 handoff before executing source changes.

### P5.0 — Accepted boundary and runtime-specific cut register (B5; 3 tasks)

- **P5.0.1 — Freeze accepted input.** **Depends:** accepted C-01–C-04 and Phase 4. **Scope:** accepted component-map/checker receipt, integration HEAD, current-format corpus, accepted Phase 4 Project Environment read/write/publication APIs. **Done when:** immutable B5 source/graph and consumer-version references recorded; no fresh whole-corpus scan or change to Phase 0 evidence. **Evidence:** Git HEAD, accepted C-01–C-04 receipts, Phase 4 A4 receipt, existing component-checker output.
- **P5.0.2 — Classify runtime-specific violating consumers.** **Depends:** P5.0.1. **Scope:** existing Gateway/Instance/RelRuntime/PhyRuntime, `ConfiguredRuntime`, provider, Context Engine and `runtime_host*` callsites, including actual lower-owner imports. **Done when:** each *runtime-implicated* forbidden edge has a source witness and owner task; public `impl Cva`/value-only/test/analyzer edges are excluded or explicitly deferred. Do **not** re-map unrelated modules. **Evidence:** filtered actual-source component edge register and exact source locations.
- **P5.0.3 — Pin behavioral contracts and implementation seams.** **Depends:** P5.0.1–.2. **Scope:** Gateway/Instance authorization generations, REL/PHY worker lifetimes, host handoff, pending queue, stream/checkpoint, provider lock discipline, configured/offline calls and Phase 4 interfaces. **Done when:** affected callers and existing focused tests are assigned to the numbered tasks below, plus a Phase 6-only export register. **Evidence:** bounded contract/caller matrix with current tests; B5 signed.

**B5 exit:** Accepted source-component mapping, actual runtime-specific bad edges, real Phase 4 APIs and stable test/format baseline. No B5 completion inferred from a valid hand-authored DAG alone.

### P5.1 — Owner execution ownership, one worker family per cut (O5; 9 tasks; depends B5)

- **P5.1.1 — REL/PHY keyed lifetime proof.** **Depends:** B5. **Scope:** `runtime_host_gateway.rs` mount maps and lease/mount-epoch state, `runtime_host_rel_execution.rs` and `runtime_host_phy_execution.rs`. **Done when:** the existing one-live-execution-per-owner, independent instance disposal, stop/drain/reopen and writer lifetime are verified and any genuinely duplicate owner handle is identified. **Evidence:** mounted-owner source proof plus existing mount/PHY tests; no new execution registry.
- **P5.1.2 — REL execution read/mutate boundary.** **Depends:** P5.1.1. **Scope:** `RelShared` guarded reads/mutations and `InteractionRuntime` calls in `runtime_host_rel_execution.rs`. **Done when:** each confirmed owner-state reach-through from lower semantic services is redirected to the current owner-local proof/read/mutate contract, without exposing Gateway or a second Cva wrapper. **Evidence:** before/after source edges and focused REL admission/worker test.
- **P5.1.3 — Insomnia REL worker consumer cut.** **Depends:** P5.1.2 and accepted earlier Insomnia owner replay contract. **Scope:** `runtime_host_rel_insomnia.rs` plus its direct owner admission/receipt callers only. **Done when:** worker scheduling reads owner inputs through the final Insomnia contract; no callback to Gateway/Instance or duplicate import replay remains. **Evidence:** source negative edge and existing Insomnia worker/backpressure tests.
- **P5.1.4 — Dream REL worker consumer cut.** **Depends:** P5.1.2. **Scope:** `runtime_host_rel_dream.rs` and direct Dream publication contracts. **Done when:** scheduling/final publication uses Dream owner evidence, not ambient host state; Dream retry and background-worker behavior are preserved. **Evidence:** Dream-focused existing worker/receipt tests and source edges.
- **P5.1.5 — Perception REL worker consumer cut.** **Depends:** P5.1.2. **Scope:** `runtime_host_rel_perception.rs` and exact Perception queue/publication consumers. **Done when:** all observed owner-service → Runtime dependencies are cut or proven absent; queued perception semantics and accepted mutation ordering remain unchanged. **Evidence:** existing perception/background tests and callsite diff.
- **P5.1.6 — Vectors REL worker consumer cut.** **Depends:** P5.1.2. **Scope:** `runtime_host_rel_vectors.rs`, existing vector state and background retry path. **Done when:** only runtime scheduling owns wake/retry, vectors owns embedding/profile publication; no second writer or premature provider call. **Evidence:** vector worker and route tests plus source edge proof.
- **P5.1.7 — PHY owner execution boundary.** **Depends:** P5.1.1. **Scope:** `runtime_host_phy_execution.rs` shared PHY leases, worker control and owner-local capability. **Done when:** one PHY execution is shared safely by authorized instances, can quiesce/stop/reopen without transferring authority to an Instance, and does not import Gateway presentation below owner lifecycle. **Evidence:** existing PHY execution/instance tests and owner-edge inventory.
- **P5.1.8 — PHY worker family cut.** **Depends:** P5.1.7. **Scope:** `runtime_host_phy_dream.rs`, `runtime_host_phy_perception.rs`, `runtime_host_phy_vectors.rs` as one keyed PHY execution composition; **do not** redesign their semantic producers. **Done when:** each worker uses only the guarded PHY owner read/publication path and no per-instance shared state; maintain existing retry/cancellation. **Evidence:** one PHY lifecycle integration proof with worker-specific source witnesses; separate extra tasks only if a proven independent ownership migration is needed.
- **P5.1.9 — Retire owner-execution leftovers.** **Depends:** P5.1.2–.8. **Scope:** only displaced private owner callbacks, obsolete control fields and old-owner-only test fixtures in the REL/PHY execution modules. **Done when:** no consumers of those paths remain, ownership is singular, existing tests pass and deletions are listed. **Evidence:** exact symbol/caller deletion register, focused owner worker gate.

**O5 exit:** One execution per mounted REL/PHY, no lower-to-Gateway/Instance ownership backedge, no second worker or replay owner; accepted authorization and worker behavior unchanged.

### P5.2 — Gateway and instance separation by authority surface (G5; 6 tasks; depends O5)

- **P5.2.1 — Shared mount/admission authority.** **Depends:** O5. **Scope:** `runtime_host_gateway.rs`, `runtime_host_gateway_mount.rs`, `runtime_host_gateway_admission.rs`. **Done when:** Gateway alone holds durable owner mount maps, admission and fence generations; Instance cannot unmount or create a second execution. **Evidence:** source review and existing mount/admission tests.
- **P5.2.2 — Per-instance REL/conversation selection.** **Depends:** P5.2.1. **Scope:** `runtime_host_instances.rs` and `runtime_host_gateway_conversation_selection.rs`. **Done when:** all identified internal ambient REL/conversation consumers use explicit selected authorized Instance context; no host-global selection fallback persists. **Evidence:** per-instance independent-selection regression and exact old-caller removal.
- **P5.2.3 — Per-instance PHY/privacy resolution.** **Depends:** P5.2.1. **Scope:** attached PHY identity and Gateway semantic/ambient access (`runtime_host_gateway_semantic.rs`, `runtime_host_gateway_ambient.rs`). **Done when:** PHY reads bind to current principal/owner grant, never another Instance's active PHY or a REL-wide broadcast. **Evidence:** existing PHY privacy and multi-principal tests, forbidden-edge witness.
- **P5.2.4 — Dispatch/admission token generation.** **Depends:** P5.2.1–.3. **Scope:** `InstanceAuthorization`, `OwnerAccessToken`, mount-epoch and owner-specific grant generations. **Done when:** every impacted dispatch path validates exact attachment, owner grant and mount epoch; no global generation substitution silently authorizes a revoked owner. **Evidence:** existing stale-token/grant race tests and code path map.
- **P5.2.5 — Publication and delivery reauthorization.** **Depends:** P5.2.4. **Scope:** Gateway operation completion, `deliver_rel`, `with_instance_delivery`, and owner publication. **Done when:** grant/epoch rechecked at durable commitment and subsequent delivery, including after external I/O, without transferring grants into lower stores. **Evidence:** existing post-I/O revocation race tests.
- **P5.2.6 — Remove host-era forwarding for instance authority.** **Depends:** P5.2.2–.5. **Scope:** verified unused private ambient/selection/instance forwarding in `runtime_host.rs` and Gateway. **Done when:** old consumers are gone, private forwarding removed, public `ReliquaryRuntimeHostError`/`Routes` compatibility questions recorded for Phase 6 (not renamed here). **Evidence:** exact deleted path inventory and Gateway API contract tests.

**G5 exit:** Gateway owns shared authority; Instance owns its own attachment and selections; detach/revoke affects only its own rights, not other instances' workers.

### P5.3 — Durable queue, host handoff, feeds and streams (Q5; 8 tasks; depends G5)

- **P5.3.1 — Durable pending journal ownership.** **Depends:** G5. **Scope:** `multiplexing_journal.rs`, `runtime_host_gateway_conversation.rs` registry and owner journals. **Done when:** for each pending-submission transition there is one authoritative journal/controller; no redundant writer, uncommitted Archive turn or duplicate recoverable enqueue. **Evidence:** existing journal/recovery contract tests and transition table.
- **P5.3.2 — Sequential submission/finalization cut.** **Depends:** P5.3.1. **Scope:** `runtime_host_gateway_conversation.rs`, `runtime_host_gateway_conversation_finalization.rs`. **Done when:** queue order, parent/branch version, idempotent Archive finalization and provisional-vs-committed boundary pass through one authority; duplicate dispatch path removed if present. **Evidence:** sequential scheduler/finalization tests.
- **P5.3.3 — Auto-branch policy cut.** **Depends:** P5.3.1. **Scope:** conversation policy and branch continuation modules, no queue redesign. **Done when:** automatic branching retains explicit branch identity/instance selection; concurrent submissions cannot apply contradicting policies or silently retarget other instances. **Evidence:** existing auto-branch and branch-host tests.
- **P5.3.4 — Pending cancellation/commit race cut.** **Depends:** P5.3.2–.3. **Scope:** Gateway cancellation, journal transition and finalization. **Done when:** cancelled pending work never becomes an Archive turn, finalize-vs-cancel has exactly one winner, a restart cannot redispatch cancelled work. **Evidence:** existing cancel/finalization race tests and journal proof.
- **P5.3.5 — Host handoff and unknown-remote outcome.** **Depends:** P5.3.1, P5.3.4. **Scope:** `runtime_host_gateway_host.rs`, `runtime_host_gateway_conversation_generation.rs`; existing `GatewayHostAdapter`. **Done when:** native/host queue handoff and cancellation authority have one owner per stage; ambiguous remote outcome is held for reconciliation, never blindly dispatched twice. **Evidence:** host operation, cancellation and reconnect tests.
- **P5.3.6 — Authorized REL-wide update feed.** **Depends:** G5 and P5.3.2. **Scope:** `runtime_host_gateway_events.rs`, `runtime_host_gateway_snapshots.rs` and subscription registry. **Done when:** authorized REL viewers see committed owner updates in epoch/cursor order but not private PHY payload or uncommitted generation text. **Evidence:** snapshots/events tests.
- **P5.3.7 — Conversation stream and checkpoint delivery.** **Depends:** P5.3.5–.6. **Scope:** `runtime_host_gateway_streams.rs`, `runtime_host_gateway_stream_registry.rs`, `interaction_live.rs`. **Done when:** only active authorized conversation subscribers get generation streams and no byte appears before its durable checkpoint; canceled/revoked viewers are fenced. **Evidence:** stream/live acceptance tests.
- **P5.3.8 — Reconnect/resync and obsolete route retirement.** **Depends:** P5.3.4–.7. **Scope:** `runtime_host_gateway_reconnect.rs`, `runtime_host_gateway_snapshot_reader.rs` and concrete redundant status/stream forwarding if any. **Done when:** restart/epoch rotation, cursor gap, slow consumer, detach/rejoin and unresolved host outcome resynchronize without lost/doubled messages; obsolete internal routes removed only with proven zero consumers. **Evidence:** reconnect, crash/recovery and late subscriber tests.

**Q5 exit:** Exactly one admission/finalization authority, stable host delegation, durable-before-visible and private authorized feeds; no duplicate native/host dispatch or producer.

### P5.4 — Provider configuration and unlocked inference cuts (E5; 7 tasks; depends O5; independent of G5 until integration)

- **P5.4.1 — Configured route/credential ownership.** **Depends:** O5. **Scope:** `configured_runtime.rs`, `configured_runtime_decision.rs`, `model_switchboard.rs` and chosen provider ports. **Done when:** endpoint selection/secret material remain in Configured/Provider capabilities, not Gateway or owner stores; route inheritance and failure ordering are unchanged. **Evidence:** existing switchboard/configured decision tests.
- **P5.4.2 — Lower-value and codec source edge cut.** **Depends:** P5.4.1. **Scope:** `embedding_endpoint.rs` (`EmbeddingMode`, `VectorNormalization`, endpoint error) and Insomnia completion codec. **Done when:** C-01–C-04's actual source mapping classifies value-only vs capability imports, moves **only genuinely misplaced** types/codec directly to an accepted owner if required, and preserves existing tags/bytes. **Evidence:** symbol use graph and existing serialization/codec tests; no whole-tree remap.
- **P5.4.3 — Insomnia long-I/O operation boundary.** **Depends:** P5.1.3, P5.4.1. **Scope:** `runtime_host_rel_insomnia.rs`, configured Insomnia endpoint caller. **Done when:** freeze input/route/cancel identity, release long owner lock for model I/O, then reauthorize and publish via existing Insomnia receipt path; stale result cannot commit. **Evidence:** existing Insomnia backpressure/worker/race tests, lock-and-call source trace.
- **P5.4.4 — Dream long-I/O operation boundary.** **Depends:** P5.1.4, P5.4.1. **Scope:** `runtime_host_rel_dream.rs` and direct Dream model endpoint path. **Done when:** Dream candidate/route frozen before remote call and accepted Dream publication rechecks owner generation; no second state owner or fallback inference loop added. **Evidence:** focused Dream worker/route tests and lock trace.
- **P5.4.5 — Perception long-I/O operation boundary.** **Depends:** P5.1.5, P5.4.1. **Scope:** `runtime_host_rel_perception.rs` and configured Perception endpoints. **Done when:** model execution is outside long owner locks and stale/canceled result cannot publish Entity/Graph evidence. **Evidence:** existing Perception worker/race/endpoint tests.
- **P5.4.6 — Vectors long-I/O operation boundary.** **Depends:** P5.1.6, P5.4.2. **Scope:** `runtime_host_rel_vectors.rs`, configured embedding endpoints and vector publication. **Done when:** embedding request does not retain a long owner lock, and profile/version checks occur before durable vector admission. **Evidence:** existing vector worker/endpoint and profile tests.
- **P5.4.7 — Configured offline operation/public adapter preservation.** **Depends:** P5.4.1–.6. **Scope:** `configured_runtime_historical.rs`, `configured_runtime_insomnia.rs`, `configured_runtime_vectors.rs` and replaced **internal** route forwarders. **Done when:** offline calls keep supported public semantics, use the same provider/domain separation, and obsolete private forwarding is deleted. **Evidence:** configured-runtime integration tests and exact old-caller/source diff. Broad public renaming remains Phase 6.

**E5 exit:** No required provider I/O holds a long owner lock; semantic owners publish results; route/credential selection remains a separate capability, with original endpoint/serialization behavior.

### P5.5 — Current context, Phase 4 calls and external-source integration (S5; 5 tasks; depends G5, E5, accepted Phase 4)

- **P5.5.1 — Context Engine input read boundary.** **Depends:** G5, E5. **Scope:** current `context_engine/evidence.rs`, `context_engine/model.rs` and Gateway context callers. **Done when:** authorized Archive/Echo evidence and explicit scope are passed as values; neither Context Engine nor owner stores import Gateway/Instance to obtain a session. **Evidence:** current context tests and source edge diff.
- **P5.5.2 — Current compaction/checkpoint boundary.** **Depends:** P5.5.1. **Scope:** `context_engine/compaction.rs`, `compaction_budget.rs` and live-runtime checkpoint caller. **Done when:** deterministic current policy and `ContextTokenCounter` remain unchanged, with no new model calls, delayed checkpoint or duplicated conversation owner. **Evidence:** existing context/compaction tests and checkpoint receipt.
- **P5.5.3 — Verify validated historical project-read consumption.** **Depends:** accepted Phase 4, G5. **Scope:** `runtime_host_gateway_ambient_project.rs` historical bytes/read calls. **Done when:** verify the integrated Phase 4 caller already supplies accepted REL scope/correlation history and preserves `ViewRel` plus post-I/O delivery checks. Change source only for a newly demonstrated runtime-to-lower-layer dependency violation; do not re-migrate the Phase 4 read implementation. **Evidence:** Phase 4 accepted caller diff and existing Git/Lore/Gateway read/revocation tests.
- **P5.5.4 — Verify project mutation/lifecycle ownership.** **Depends:** P5.5.3. **Scope:** `runtime_host_gateway_project_lifecycle.rs`, project upload/checkpoint methods and owner publication. **Done when:** verify Phase 4 already separated repository validation from Archive publication, preserved `ManageProject`/post-I/O fencing, and retired implicit same-repository-ID reassociation. Correct only a demonstrated residual runtime ownership backedge, not Phase 4's scope logic. **Evidence:** Phase 4 accepted mutation/lifecycle source diff and Gateway project tests.
- **P5.5.5 — External-source/host boundary.** **Depends:** G5, E5. **Scope:** `runtime_host_gateway_external.rs`, external plan/processing integration. **Done when:** host canonical transcript adapter and REL visibility are explicit; no host transcript ownership duplication, unauthorized PHY source leakage or new External Sources redesign. **Evidence:** existing external-source Gateway tests and source edge diff.

**S5 exit:** Runtime consumes Project Environment, Context Engine and External Sources through their real current contracts; no new compaction, transcript or project feature.

### P5.6 — Deletion and incremental actual-source enforcement (D5; 4 tasks; depends Q5, S5)

- **P5.6.1 — Close obsolete internal forwarding inventory.** **Depends:** O5, G5, Q5, E5, S5. **Scope:** *only* old internal `runtime_host*`, duplicate work/status/route and unused owner reach-through identified in completed tasks. **Done when:** all old internal callers are removed and no adapter/test duplicate is retained to hide a forbidden edge; externally supported exports are explicitly handed to Phase 6. **Evidence:** deleted-file/symbol and surviving-consumer register.
- **P5.6.2 — Validate Phase 5 delta with the accepted source map.** **Depends:** P5.6.1. **Scope:** accepted `scripts/check_production_component_graph.py` and `tools/pitlord/production-components.json` or exact successors **as integrated by C-01–C-04**. **Done when:** map is incrementally updated only for Phase 5 moved/removed production modules; unresolved new source edges are reported, not blanket-suppressed. **Evidence:** before/after changed-edge diff plus mapped/unmapped counts. **Not a second all-source mapping project.**
- **P5.6.3 — Enforce runtime SCC and deferred facade disposition.** **Depends:** P5.6.2. **Scope:** runtime-gateway ↔ owner/service source edge ratchet, target DAG guard and Phase 6 public facade exceptions. **Done when:** no forbidden owner/service → runtime edge and runtime no longer belongs to the physical/owner-store SCC. Actual cycle witnesses, accepted exceptions and negative fixtures are recorded. **Evidence:** compiler-aware component graph/checker and focused architecture negatives, not a new Arcana rescan loop.
- **P5.6.4 — Run bounded subsystem gate.** **Depends:** P5.6.1–.3. **Scope:** accepted REL/PHY worker, queue, instance, host, stream, endpoint, context, Project Environment and integration suites. **Done when:** each changed seam passes its existing focused tests or has a documented non-Phase-5 blocker; no redundant suite loops or gratuitous new test counts. **Evidence:** one consolidated D5 targeted test/source policy receipt.

**D5 exit:** Proven actual runtime hard cut using the accepted production map; no obsolete internal execution path; no unclassified deferred facade backedge.

### P5.7 — Whole integration, canonical docs and worktree closure (A5; 3 tasks; depends D5)

- **P5.7.1 — Final integration verification.** **Depends:** D5. **Scope:** single integrated source checkpoint. **Done when:** `cargo fmt --check`, `cargo check --all-targets`, existing required `cargo test --lib` and applicable multiplexing/live/provider/project integration tests, current-format corpus and architecture checks succeed or expose an explicitly accepted unrelated failure; no re-running broad suites per ticket. **Evidence:** one version-pinned final A5 verification receipt.
- **P5.7.2 — Current-architecture documentation.** **Depends:** P5.7.1. **Scope:** runtime multiplexing, model runtime, Context Engine, architecture, API, maintainer map and current limitations, plus touched ADR links. **Done when:** each description matches the implemented owner and exact runtime call direction, with no obsolete step diaries in canonical docs. **Evidence:** doc diff plus one `ddocs` policy/link pass.
- **P5.7.3 — Integration and Phase 6 handoff closure.** **Depends:** P5.7.1–.2. **Scope:** all Phase 5 commits/worktrees and public facade leftovers. **Done when:** every worktree hunk is integrated, source-proven superseded, or verified archived; temporary wrappers removed, accepted main commit and actual graph recorded; Phase 6 receives exact supported `Cva`/`Phylactery`/`lib.rs` consumer list. **Evidence:** clean acceptance worktree/branch inventory and commit-backed A5 receipt.

**A5 exit:** Current supported behavior passes integration, Runtime is separated from physical/owner SCC in the *actual* source graph, and every parallel implementation change has an explicit disposition. No Phase 5 "complete" claim with unknown unintegrated work.

### Ticket DAG, sequencing and parallel frontier

```text
C-01..C-04 closure + accepted Phase 4 --> B5 (P5.0)
                                         |
                                         v
                             O5 (P5.1: REL/PHY ownership)
                              /                        \
                             v                          v
                   G5 (P5.2)                         E5 (P5.4)
                      |                                  |
                      v                                  |
                   Q5 (P5.3)                             |
                      \                                  /
                       +--------- S5 (P5.5) <-----------+
                                      |
                      Q5 -------------+
                                      v
                                  D5 (P5.6)
                                      |
                                      v
                                  A5 (P5.7)
```

**Hard dependencies:** B5 → O5; O5 → G5/E5; G5 → Q5; G5+E5+Phase 4 → S5; Q5+S5 → D5; D5 → A5. Within groups, the explicit `Depends` on each ticket controls order; no hidden cross-group prerequisites. Q5 and E5 may proceed concurrently after their gates, and P5.1.3–.6 are independent after P5.1.2. A5 is the only final release-quality integration gate.

**Integration bottlenecks:** `src/lib.rs`, `runtime_host_gateway.rs`, module registries, component policy, Cargo manifests and aggregate test registrations have **one designated owner at a time**. Parallel tasks must own disjoint file sets and merge forward at a named checkpoint, not overwrite each other's source. The original Phase 3 unresolved-worktree failure must not recur.

**Test budget and proof hierarchy:** Per change, run only relevant source guards, formatting and targeted existing tests; compile after a coherent cut, not after every file edit. Run each broader O5/G5/Q5/E5/S5/D5 group once at its integration gate, followed by one serial full acceptance run at A5 unless a verified regression specifically requires repetition. Never overlap Cargo builds sharing one target directory; use one agreed locked toolchain/profile and disable incremental compilation only when repairing an observed compiler-artifact collision. Reuse hash-pinned successful unchanged-input receipts and do not rerun historical corpus or unrelated suites without changed contracts. Set explicit per-check timeouts, record incomplete results rather than escalating automatically, and never launch concurrent full Arcana scans/exports. For source-conformance, require direct source ownership/callsite review plus focused policy negatives and a before/after SCC/edge comparison on a consistent production-component map. Classify test-only, immutable-value, analyzer-only and deferred Phase 6 edges separately; a classified edge is not an eliminated edge. The file-node-only Arcana export cannot be claimed as complete symbol-level proof. No new end-to-end suite solely for an intermediate wrapper.

**Denominator audit:** P5.0 **3** + P5.1 **9** + P5.2 **6** + P5.3 **8** + P5.4 **7** + P5.5 **5** + P5.6 **4** + P5.7 **3** = **45 source-bounded tasks**. Gates: B5, O5, G5, Q5, E5, S5, D5, A5 (**8**, not tasks). Status at specification: **0/45 implemented, 0/8 accepted**. Adding a task requires a specific newly discovered source/behavioral responsibility and explicit denominator revision **before** any progress claim; splitting already counted work for convenience does not create new scope.

## Testing decisions and preservation matrix

| Invariant / variation | Primary existing test surfaces | Required result |
| --- | --- | --- |
| Mounts, grants, per-instance selection, owner epoch | `src/runtime_inference/runtime_host_instance_tests.rs`, `runtime_host_gateway_admission_tests.rs`, `runtime_host_gateway_tests.rs`, `tests/multiplexing_gateway_api_contract.rs` | No shared selection, private state leakage, stale grant or unmount by detach |
| Owner execution / worker lifecycle / fenced publication | `runtime_host_gateway_background_tests.rs`, `runtime_host_phy_execution_tests.rs`, `runtime_host_gateway_publication_tests.rs`, `runtime_host_gateway_convergence_race_tests.rs` | One writer/worker per owner; safe drain/restart; no stale publication |
| Queue, host delegation, cancellation, recovery | `runtime_host_gateway_conversation_*tests.rs`, `runtime_host_gateway_host_operation_tests.rs`, `runtime_host_gateway_host_cancellation_tests.rs`, `multiplexing_journal_tests.rs`, `tests/multiplexing_phase0b_*contract.rs` | Durable pending state; atomic cancel vs finalize; idempotent host/native handoff |
| REL update feeds, streams and live checkpoint | `runtime_host_gateway_snapshots_tests.rs`, `runtime_host_gateway_streams_tests.rs`, `runtime_host_gateway_reconnect_tests.rs`, `interaction_live_acceptance_tests.rs`, `tests/live_runtime_adapter.rs` | Durable-before-visible, gap/watermark resync, active-stream fan-out, revocation |
| Model routes, config/provider failures | `model_switchboard_tests.rs`, `configured_runtime_decision_integration_tests.rs`, `runtime_host_route_tests.rs`, `src/runtime_inference/*endpoint*tests.rs` | Inherited routes and endpoint semantics unchanged; no extra inference; secrets protected |
| Context Engine / current compaction | `src/context_engine/tests.rs`, interaction/context integration tests | Deterministic view, token counter and Echo provenance unchanged |
| Project authority from P4 | `runtime_host_gateway_project_tests.rs`, `project_environment_git_tests.rs`, `project_environment_lore_tests.rs` | Only enrolled subtree; pre/post I/O grant checks; no unauthorized bytes/write |
| Structural/format acceptance | `scripts/check_component_dag.py`, current `scripts/check_production_component_graph.py` when accepted, `scripts/check_owner_boundaries.py`, existing convergence source guards and corpus tests | Actual source dependency direction, current-format corpus, and accepted public behavior |

Prefer `cargo test --lib <focused_test_filter>` while cuts are local; the integration gate runs `cargo test --lib` and existing `cargo test --test <name>` suites for relevant contracts. Use project-approved source/policy scripts and the **accepted** current component map; do not hardcode a temporary analyzer workaround in production. The lower-level owner fixes being developed under C-01–C-04 are outside the P5 execution diff.

**No extra inference is required for testing.** Existing fake/controlled endpoints and deterministic fixtures cover model route behavior. A networked provider is not a condition of local phase acceptance unless a specific external contract requires it.

## Out of scope

- Combining `GatewayRuntime`, `InstanceRuntime`, `ConfiguredRuntime` into one type, process or service.
- New multi-instance semantics, a new queue algorithm, a second pending journal, extra scheduler/framework, or expanded ACP/MCP/host capabilities.
- Phase 4's canonical repository/project authorization implementation and additional Lore/Git features.
- Phase 6's broad public `Cva`/`Phylactery` export cleanup, Rust API redesign, renaming `ReliquaryRuntimeHostError` simply for aesthetics.
- Phase 7's final global SCC closeout beyond Phase 5-specific required runtime graph cuts.
- Deferred Context Engine compaction-refinement policy, cross-REL final context assembly, Ego synthesis, Observation transition or external-source overhaul.
- New file format, database migration, compatibility reader/writer branches, or production behavior enhancements.
- Routine code formatting churn, indiscriminate module splits, shallow wrappers, adding tests simply to inflate coverage.

## Stop conditions, risks and evidence policy

1. **C-01–C-04 or Phase 4 not accepted:** source inspection/spec review may proceed; do not run the Phase 5 implementation hard cut.
2. **Runtime authority ambiguous:** resolve whether it belongs to the keyed Gateway mount, one Instance's ephemeral grant/selection, owner-local REL/PHY state, a semantic service, or Configured provider transport **before** moving code.
3. **Real public/external compatibility boundary:** retain it at the external edge; do not proliferate dual internal pathways. Phase 6 decides general public facade disposition.
4. **New in-flight cancellation or queue state machine proposed:** stop; user requested structural correction, not a new scheduling product.
5. **Provider called under long owner lock:** require a frozen-input / guarded-publication plan and corresponding race test at the *existing* contract seam.
6. **Project scope mismatch:** fail closed under Phase 4 contract; do not use Phase 5 runtime orchestration to widen repository identity or path permissions.
7. **Arcana incomplete or stale:** use compiler-aware production source inventory and direct callsite proof; disclose uncertainty. Do not run repeated whole-corpus rescans or treat policy ratchet rebaselining as architectural improvement.
8. **Red migration window:** log broken surface and precise gate restoring it. No generic backward-compatibility scaffolding.
9. **Dirty implementation worktree or uncertain integration branch:** preserve, reconcile and document before phase acceptance. The completed Phase 3 cleanup is the cautionary precedent, not evidence that future dirty branches are safe to discard.
10. **Phase 6 work found:** assign owner and exact consumer list, make only a minimal Phase 5 cut if required to break its runtime SCC. No opportunistic public API overhaul.

## Completion criteria and Phase 6 handoff

Phase 5 is complete when:

- the Gateway/Instance/owner-execution/ConfiguredRuntime responsibilities remain **separate and enforced**;
- each mounted owner has one authoritative execution and all instance/privacy/mount/queue/stream guarantees survive;
- host/model calls occur beyond long owner locks and all writes revalidate owner/cancellation state;
- Gateway consumes the implemented Phase 4 project interface, not raw project authority;
- no lower owner or service imports Gateway/Instance or public host orchestration to implement semantic logic;
- obsolete internal forwarding has been removed; no temporary parallel runtime remains;
- the **actual source-derived production component graph** shows runtime-gateway is not in the physical/owner-store core SCC, with exact source witnesses for remaining Phase 6 public-facade edges;
- source guards, functional/integration/corpus checks and canonical docs meet the A5 gate; and
- all Phase 5 branches/worktrees are integrated, explicitly superseded or verifiably preserved, with an accepted main checkpoint.

**Handoff to Phase 6:** A concrete source-backed register of remaining public facade extension `impl Cva`/`impl Phylactery`, `lib.rs` re-export dependencies and supported offline APIs; no unresolved runtime owner-state backedges transferred without reason.

## Related docs

- [Dependency convergence architecture](dependency-convergence-architecture-refactor-2026-10-21.md) — controlling Phase 5 objective and Phase 6/7 boundaries.
- [Target 34-component dependency DAG](dependency-convergence-target-dag-2026-10-21.md).
- [Phase 1–3 source-conformance closure](dependency-convergence-phase1-3-closure-audit-2026-10-23.md) — C-01–C-04 prerequisites.
- [Phase 4 Project Environment specification](dependency-convergence-phase4-project-environment-spec-2026-10-23.md) — interface and sequencing input.
- [Phase 3 worktree audit](reliquary-phase3-worktree-audit-2026-10-23.md) — completed integration-cleanup precedent.
- [ADR 0037](decisions/0037-reliquary-context-memory-project-environment.md) and [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md).
- [Runtime multiplexing](subsystems/runtime-multiplexing.md), [Model and provider runtime](subsystems/model-runtime.md), [Context Engine](subsystems/context-engine.md).
- [Context Engine compaction refinement](context-engine-compaction-refinement-plan-2026-10-21.md) — deliberately deferred feature work.

## Notes

This document retains the controlling implementation plan. Actual task, gate, test, source-graph and worktree dispositions are recorded in the [execution receipt](dependency-convergence-phase5-execution-2026-10-23.md). The historical status at specification remains distinct from execution acceptance. No new project/host/provider feature, storage format or background inference was introduced.
