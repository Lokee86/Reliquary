# Phase 5 — Runtime and Orchestration Dependency Convergence

**Status:** Specified for review; **not implemented**.  
**Date:** 2026-10-23.  
**Repository:** Reliquary, `main` (planning baseline `7059e4c`; concurrent C-01–C-04 source-conformance work is present in the worktree and must not be overwritten).  
**Campaign:** [Dependency Convergence Architecture Refactor](dependency-convergence-architecture-refactor-2026-10-21.md), Phase 5.  
**Execution prerequisite:** Phase 1–3 closure C-01–C-04 verified; Phase 4 Project Environment accepted and its concrete interfaces recorded. **Phase 5 specification does not depend on Phase 4 implementation.**  
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

Known actual reverse dependencies warrant special review: vector/profile values stored under Runtime's `embedding_endpoint.rs` are consumed by lower Retrieval; Insomnia owner/store/codec files are physically under `runtime_inference`; the runtime tree has `Cva` and `Phylactery` composition code. Treat each as **value-only**, **owner store**, **service**, **runtime execution**, or **public extension** before deciding the cut. The full source inventory may change when the concurrent C-01–C-04 verification is accepted: revalidate it at B5; do not overwrite that work now.

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

B5 is blocked until these exact interfaces are accepted. P5.0 planning and design may run in parallel with Phase 4 implementation; no Phase 5 source implementation begins against a speculative Phase 4 seam.

## Implementation strategy — staged hard cut

Only one canonical internal path is permitted after each integration gate. Test the accepted behavior through existing high-level Gateway, live interaction, configuration, and ownership seams; do not create layer-specific scaffolding to artificially increase test counts.

- **Worktree rule:** Work on a dedicated Phase 5 branch/worktree from the integrated Phase 4 checkpoint, *not* today's dirty main or an old Phase 3 worktree. Freeze the exact baseline and preserve any concurrent C-01–C-04 changes separately.
- **Boundary-design window:** Finalize consumer register, call directions and migration DAG before source edits. Compare whether an interface is a value-only contract or an actual capability/semantic callback.
- **Controlled red window:** A coherent internal migration may temporarily break whole-repo builds, but the affected modules, precise worktree and target integration gate must be recorded. Targeted compiles/tests and source dependency checks remain mandatory; don't add throwaway wrappers to fake continuity.
- **Move once:** Introduce only final private execution/owner interfaces, migrate each real consumer directly, and remove obsolete internal methods and old-owner-only fixtures when their final consumer moves.
- **Integration gate:** Build all targets, run mandatory functional suites, replay exact current-format corpus and concurrency/authorization tests, compare component graph before/after and record no unreviewed new forbidden backedges. Worktree/branch changes must be integrated, explicitly superseded or independently archived before declaring P5 complete.

## Execution specification — 29 bounded tasks, eight gates

All entries are architectural correction tasks. IDs and scope are fixed at specification review; if a source fact forces extra work, record a named correction and update the denominator explicitly **before** calling that work complete. Do not turn a long migration into an arbitrary growing task count.

### P5.0 — Verified baseline and contracts (B5 gate; 3 tasks)

- **P5.0.1:** Confirm accepted Phase 4 HEAD, C-01–C-04 completion receipts, actual compiled production-component graph, current-format corpus, target 34-component policy, and canonical docs. Snapshot Phase 5's exact before graph without replacing Phase 0 evidence.
- **P5.0.2:** Inventory *each* producer/consumer of Gateway, Instance, REL/PHY execution, ConfiguredRuntime, Context Engine and `ReliquaryRuntimeHost*` symbols. Classify it as provider port, immutable value, owner store, domain service, runtime orchestration, public facade extension, test, or analyzer uncertainty. Include `src/lib.rs` exports and supported external consumers; do not assume pathname ownership.
- **P5.0.3:** Record the accepted Phase 4 interface handoff, a source-backed list of **specific** forbidden runtime ↔ lower-component edges, preexisting tests and lock/cancellation invariants; assign cuts to P5 rather than P6. Stop if an unverified Phase 1–4 lower-boundary defect remains.

**B5 exit:** Exact executable source baseline and all Phase 4 contracts verified, explicit current cycle/backedge witnesses, no unclassified impacted runtime entrypoint, and no uncommitted worktree work silently assumed to be integrated.

### P5.1 — Owner-scoped execution boundary (O5 gate; 4 tasks; depends B5)

- **P5.1.1:** Confirm the canonical owner lifetime around `RelRuntime` and `PhyRuntime`: one mounted REL/PHY execution, owner ID, mount claim, lease fence, worker stop/drain/restart, durable owner handle and lifecycle direction.
- **P5.1.2:** Refactor only proven mixed owner/runtime state access into deep private owner execution operations using existing owner-local read/proof/mutation contracts. No new generic handle registry and no new semantic owner.
- **P5.1.3:** Migrate REL Insomnia/Dream/Perception/vectors and PHY worker consumers directly; check that provider inference never takes place under long owner locks. Preserve receipt/status and pending cancellation identities.
- **P5.1.4:** Delete all internal old-owner execution callbacks/forwarders rendered unused by these cuts, with exact old-caller inventory and targeted REL/PHY worker lifecycle tests.

**O5 exit:** Owner and lower services know no Gateway/Instance representation; single owner execution authority preserved; no duplicated workers, replay engine, adapter, or write admission. Record actual source edge deletions.

### P5.2 — Gateway versus instance boundary (G5 gate; 4 tasks; depends O5)

- **P5.2.1:** Constrain Gateway shared mount/owner maps, lifecycle, grants, topology, streams and authorization to Gateway; use existing `InstanceRuntime` as instance-local principal/selection only.
- **P5.2.2:** Remove any surviving ambient/global active REL or PHY assumption in internal runtime consumers. Every instance request resolves exact owner, principal, attached PHY and independent grants before owner access.
- **P5.2.3:** Preserve independent token versions: attachment incarnation, general grant generation, owner-specific grant generation and mount epoch; reauthorize at dispatch, owner mutation, publication and delivery.
- **P5.2.4:** Delete redundant old `ReliquaryRuntimeHost` *ownership* forwarding entrypoints and test-only duplicates once callers use Gateway/Instance authority; retain documented public error/route names until Phase 6 review.

**G5 exit:** Multiple instances can share the same authorized owner without sharing selection or extra workers; revoke/detach fences exact identities and cannot stop unrelated owner work. No owner/service depends on Gateway/Instance implementation.

### P5.3 — Durable operation, queue and stream coordination (Q5 gate; 4 tasks; depends G5)

- **P5.3.1:** Trace the existing authoritative durable pending-submission journal and `ConversationRegistry` ownership; eliminate any genuine duplicate transition controller, not a merely separate host and native *stage* with explicit handoff.
- **P5.3.2:** Preserve ordered sequential queue and automatic branching behavior, stable idempotent submission/operation identity, pending cancellation and exactly one winner at dequeue/finalization; prove host/native cancellation handoff boundaries.
- **P5.3.3:** Reduce stream/reconnect/snapshot read paths to Gateway-owned subscriptions over authoritative REL facts. Preserve checkpoint-before-visible delivery, epoch/watermark gap recovery, slow-reader resync and authorized REL-wide versus active-conversation feeds.
- **P5.3.4:** Remove obsolete duplicate queue/status/stream routing only where source proves an unused internal path; verify restart, crash-gap, late subscriber and revocation races against existing high-level tests.

**Q5 exit:** One authoritative state transition per submission; no doubled dispatch or semantic publication; cancellation and restart recovery remain consistent; stream delivery privacy/durability intact.

### P5.4 — Configured provider and long-I/O seam (E5 gate; 4 tasks; depends O5; can overlap P5.2–P5.3)

- **P5.4.1:** Separate actual endpoint/route/credential selection from Gateway/Instance and owner/domain state. Keep `ConfiguredRuntime`, Switchboard, `GeneralEndpoint`, `DecisionEndpoint`, `EmbeddingEndpoint` and `GatewayHostAdapter` distinct.
- **P5.4.2:** Classify the lower-layer imports of `EmbeddingMode`, `VectorNormalization`, `EmbeddingEndpointError` and Insomnia completion codec under `runtime_inference`; move only misplaced owner/value definitions directly to the owning component where necessary. Keep supported serialized numeric tags unchanged.
- **P5.4.3:** For Insomnia, Dream, Perception, Vectors and configured offline calls, freeze authenticated evidence, release long owner locks before provider I/O, then validate generation/cancellation/owner admission on commit. Existing endpoint selection/failure fallback semantics remain unchanged.
- **P5.4.4:** Retire replaced internal route wrappers and prove endpoint auth, unavailable routes, retry/no-fallback rules, secret redaction, offline ConfiguredRuntime behavior and no provider-side store mutation.

**E5 exit:** Model configuration/transport remains external capability; no provider invocation holds a long owner lock; lower stores do not import runtime owner implementations and current persisted endpoint tags remain readable.

### P5.5 — Context Engine, external sources and Project Environment consumers (S5 gate; 3 tasks; depends G5, E5 and accepted Phase 4)

- **P5.5.1:** Make Gateway obtain authorized current Conversation/Echo facts via Archive/owner read contracts and pass explicit views to the **current** Context Engine projection/compaction policy. Remove lower → runtime callbacks; preserve checkpoint and token-counter semantics.
- **P5.5.2:** Consume Phase 4's accepted validated Project Environment scope for Gateway lifecycle, attachments and file reads; retain `ViewRel`/`ManageProject`, post-I/O reauthorization and exact project subtree checks. Remove old direct raw-path Project Environment call paths that are demonstrably replaced.
- **P5.5.3:** Place existing external-source integration and configured provider handoff at owned runtime/external integration seams, without redesigning transcript authority or Import. Prove no shared owner state or private PHY evidence escapes through REL-wide feeds.

**S5 exit:** Context/Echo inputs flow downward, project authority is not duplicated, cross-source access remains authorized, and no new Context Engine or external-source feature has been added.

### P5.6 — Targeted deletion and actual-source ratchet (D5 gate; 4 tasks; depends Q5, S5)

- **P5.6.1:** Remove superseded internal RuntimeHost forwarding, duplicate test scaffolds, dead route bridges and unused owner reach-through; preserve actual public contracts for Phase 6 with an explicit consumer list.
- **P5.6.2:** Map **all** compiled production Rust modules to the accepted 34 components, including internal paths and re-exported `impl` ownership; compare imports, capability calls, typed/value-only imports and known analyzer artifacts. List every unmapped or uncertain source instead of suppressing it.
- **P5.6.3:** Ratchet *actual source* against the Phase 5 target rules. Require no newly forbidden owner/service → runtime edge, no Runtime or physical/owner-store core SCC, and an explicit fingerprint/source witness disposition for each remaining Phase 6/7 deferred facade/public edge.
- **P5.6.4:** Run existing targeted worker, queue, multiplexing, stream, endpoint, context, Project Environment and policy checks with negative cases. Classify failures by Phase 5 versus documented unrelated baseline; do not add hundreds of internal implementation tests.

**D5 exit:** Source-verified cycle reduction, no unaccounted runtime backedges, obsolete internal paths deleted. A passing *target* DAG checker alone does not pass.

### P5.7 — Integration, docs and handoff (A5 gate; 3 tasks; depends D5)

- **P5.7.1:** Whole-repo `cargo fmt --check`, `cargo check --all-targets`, relevant `cargo test --lib` and integration suites, current-format corpus tests, source/architecture ratchets and documentation checks; preserve required deterministic fixture snapshots. Gate any known unrelated failure explicitly rather than silently treating it as green.
- **P5.7.2:** Update `docs/architecture.md`, `docs/subsystems/runtime-multiplexing.md`, `docs/subsystems/model-runtime.md`, `docs/subsystems/context-engine.md`, `docs/api.md`, `docs/maintainer-map.md`, `docs/current-limitations.md` and touched ADR references to describe *implemented* ownership. Update execution register/receipts, not feature roadmap.
- **P5.7.3:** Verify every Phase 5 branch/worktree has been integrated, explicitly superseded with proof, or independently archived; remove transient shims and dead tests; commit acceptance evidence. Hand Phase 6 an exact remaining list of public `Cva`/`Phylactery` and `lib.rs` facade imports without preemptively rewriting supported APIs.

**A5 exit:** Entire accepted Phase 5 implementation is in a single integration checkpoint, all mandated checks pass or have approved evidence-based exceptions, production-source graph demonstrates the runtime hard cut, and zero unreviewed parallel changes remain.

### Ticket dependency graph and parallel frontier

```text
B5 (P5.0)
  -> O5 (P5.1)
      -> G5 (P5.2) -> Q5 (P5.3) ----+
      -> E5 (P5.4) ------------------+--> S5 (P5.5) --> D5 (P5.6) --> A5 (P5.7)
                     [G5 + E5 + accepted Phase 4 required for S5]
                     [Q5 + S5 required for D5]
```

After O5, the **Gateway/Instance** and **provider/endpoint** cuts may proceed on independently owned file sets. They must not concurrently edit shared root modules (`src/lib.rs`, `runtime_host_gateway.rs`, test module registrations, ownership policy or `Cargo.toml`) without a deliberate integration seam. Stage those in one owner task. `Q5` follows `G5`, `S5` follows both `G5` and `P5`. No loop in the ticket graph.

**Denominator audit:** P5.0 = 3, P5.1 = 4, P5.2 = 4, P5.3 = 4, P5.4 = 4, P5.5 = 3, P5.6 = 4, P5.7 = 3 = **29 bounded tasks**. **Eight gates:** B5, O5, G5, Q5, E5, S5, D5, A5. They are gates, not extra implementation tasks.

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

This is a **planned** implementation and source map, not evidence any Phase 5 task has run. No new project/host/provider feature, Rust source edit, storage-format change or background inference is authorized by writing the specification. The task and gate denominator is not a proxy for progress: each item needs source-backed integration evidence and a verified end-to-end gate.
