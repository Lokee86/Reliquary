# Reliquary architecture redesign — foundation and options (2026-11-03)

Parent index: [Documentation index](INDEX.md)

**Status: DRAFT / DESIGN EXPLORATION — not an approved implementation architecture.**
**Baseline:** [Current implementation inventory](current-system-inventory-2026-11-03.md) at `a1039bbe`; [Root dependency audit](crate-first-root-dependency-audit-2026-10-30.md); [Architecture](architecture.md), [invariants](invariants.md), [storage format](storage-format.md), [scope ownership](subsystems/scope-ownership.md), [runtime multiplexing](subsystems/runtime-multiplexing.md), ADR 0001/0034/0037/0038/0042.
**Intent:** Re-design the architecture of the *existing implemented system* before changing its code. Previous 30-crate physical layout and logical component DAG are evidence to evaluate, **not mandatory destinations**. No feature implementation, crate migration, or existing storage-format changes are authorized by this document.

**Stage 1 storage design complete:** [REL/PHY storage architecture](rel-phy-storage-architecture-2026-11-03.md), [ADR 0044](decisions/0044-canonical-rel-phy-owner-session-architecture.md) and [method-level ownership crosswalk](rel-phy-method-ownership-crosswalk-2026-11-03.md). Earlier [owner-session alternatives](rel-phy-owner-session-design-2026-11-03.md) are retained as rationale. **Confirmed naming direction:** [ADR 0043 — REL terminology and wire identity](decisions/0043-rel-terminology-and-wire-identity.md).

## Purpose

Record the product-wide architectural goals and compare materially different storage models before relocating implementation code.

## Overview

This is the initial design exploration. Its storage conclusions were finalized by the [Stage 1 REL/PHY storage architecture](rel-phy-storage-architecture-2026-11-03.md); later service, runtime and dependency designs remain separate.

## 1. Confirmed design objectives and ranking

The project owner selected **maintainability and extensibility as primary goals**, with all existing functionality and performance treated as hard preservation constraints. Reliquary is not feature-complete; the architecture must support substantial semantic expansion before release.

Rank decisions:
1. Coherent state/behavior ownership, understandable interfaces and high locality.
2. Extensibility without repeated rewiring of unrelated modules or large coordinator growth.
3. Preserve all existing supported semantics, recovery/format invariants and current public behavior, except separately approved hard-cut API changes.
4. Preserve runtime performance properties: bounded lock times, owner-local concurrency, bounded model and index work, avoiding gratuitous copies/serialization/dispatch.
5. Avoid unnecessary crates, port/adapter layers, generic frameworks, compatibility wrappers and procedural architecture-evidence machinery.

A smaller crate count is not itself a goal. Neither is maximizing isolation by splitting every store, codec, policy and adapter into its own Rust library. Physical crate boundaries are justified by durable ownership, independent reuse, dependency direction or strong compilation/test leverage.

### Non-negotiable architectural constraints

- Container owns physical file mechanics only; it cannot validate semantic dependencies or own a generalized mutable semantic database. Each domain has one explicit source of truth (ADR 0001).
- A REL and a PHY are **different durable semantic owners**. Owner-qualified references and authorized read composition never transfer mutation authority or disclose private PHY material.
- Each owner has local semantic clocks/history; physical publication is not a replacement for owner-local history. Derived indexes and Communities are not a second semantic authority.
- Native Archive conversation history and reference-only external-host transcripts remain different source-authority modes.
- One canonical mutable owner session/aggregate per mounted durable owner; no shadow state and no exposing the full store bag to arbitrary service modules.
- External model inference and host calls must not retain broad mutable owner locks; cross-owner operations must use explicit bounded steps and durable, idempotent recovery when necessary.
- No lower module depends on Gateway, Root facade, or higher semantic policy to deserialize, reopen, validate, or maintain its own authoritative state.
- A migration preserves current-only REL/PHY format policy. The already known `CVAFRS07` recovery gap is a **separate existing defect**, not an excuse to add legacy-runtime readers.
- No new semantic algorithms or product features are part of this redesign. Planned capability boundaries are included solely to test extensibility.
- Keep modules cohesive and reviewable; use small entry-point coordinators and split implementation by responsibility rather than growing giant files.

## 2. Logical system, independently of current crates

The following names describe *responsibility clusters*, not a one-to-one Cargo plan.

| Capability | What it owns | What it must not own |
| --- | --- | --- |
| **Public product API** | Stable, typed consumer entry points; public projections and errors | Duplicated semantic state, direct cross-domain field access |
| **Gateway / Instance runtime** | Mounted owner lifetime, grants, concurrent execution, per-instance selection, authoritative queue, host handoff, streams, reconnect, worker scheduling | Domain algorithms, owner record codecs, ubiquitous mutation backdoors |
| **Application/workflow coordinators** | Explicit owner-authorized multi-domain actions: ingestion, Insomnia commit, Dream publication, entity resolution, import settlement, historical composition, maintenance/rewrite | General-purpose service locator, unbounded mutable access to all domains, model policy owned by individual services |
| **Semantic services** | Insomnia evaluation, Dream policy/inference, Freshness rules, Retrieval/agentic selection, Entity Resolution, later Observations/Ego, Chronos, Context Engine, embedding/compatibility logic | Physical file management, host transport, universal mutable owner state |
| **Per-owner persistence composition** | One REL/PHY opened session, ordered replay, typed domain ownership and explicit atomic publication/admission seams | Inference, host authentication, general model orchestration |
| **Domain-owned state** | Archive, Memory, Graph/Communities, Entity/Observation lineage, Ego records, vectors, Freshness effects, work/epoch journals and other owner-specific codecs | Global substitute for all domain clocks or one generic semantic schema |
| **Container** | Physical header/frame, atomic frame/publication mechanisms, OS leases, cursors, explicit repair | Semantic validation and cross-domain business rules |
| **External integration boundaries** | Switchboard provider routing, independent Git/Lore project authority, reference-only source adapters, host-supplied capabilities | Reliquary source-of-truth duplication |

Logical dependency direction (not yet a definitive Rust DAG):

```text
Consumers / CLI / Warlock / Hermes
                   |
              Public API
                   |
         Gateway / Instance Runtime ---------> Host/model/source adapters
                   |
        Explicit application workflows
             /          |          \
  Semantic computation   |      Project Context / Engine
             \          |         /
             Authorized owner reads + domain commands
                        |
             Per-owner session/composition
                        |
         Purpose-built domain state owners
                        |
                 Physical Container
```

Pure semantic computation may depend on narrowly scoped domain *value types*, indexed read views or explicit decision/embedding interfaces, **not** on an arbitrary session/Root/Gateway handle. These are semantic contracts; exact Rust crate arrows require a later source-verified design. Cross-service operations belong to named workflow coordinators, not a universal orchestration service.

## 3. Storage architecture — materially different choices

### A. One isolated storage package with internal domain modules

- **Shape:** one storage crate composes Container, Archive, Memory, Graph, Entity, vectors, etc. Internal Rust modules retain separate ownership; callers see a per-owner session facade.
- **Strength:** simplest physical crate graph; minimal cross-crate type translation; efficient internal coordinated writes.
- **Weakness:** a single package can become a new authority hub; changes to any domain impact a broad compilation/test surface; independently reusable domain owners become less visible. Needs strict internal interfaces to avoid recreating Root within the storage crate.
- **Assessment:** viable only if internal domain/module contracts are exceptionally disciplined. Not the recommended default.

### B. One canonical per-owner session/composition boundary over independent purpose-built stores

- **Shape:** Container is independent; durable semantic domains remain independently owned (as crate or internal module according to real leverage). A compact owner-session component owns *the only live mutable collection* of stores for one REL/PHY, ordered reopen and typed transaction gates. Application workflows interact through narrow owner reads/commands, never the concrete store fields.
- **Strength:** strong locality for owner state, explicit recovery and authority, no synthetic semantic root, scales to new persisted domains; retains existing independent store implementations where they are valuable; Rust can enforce meaningful boundaries.
- **Weakness:** changes that introduce a new durable owner family require updating the session's explicit record routing/opens/transaction admission. Overzealous abstraction could create a sprawling accessor interface or a generic `AnyStore` registry.
- **Mitigation:** explicit typed owner operations and bounded **domain-specific** facades; no public `pub(crate)` mutable field bag; no generic all-store trait; only add a crate when the module earns its interface.
- **Assessment:** **recommended direction for detailed design**, not yet an approved decision.

### C. Vertical services that also own their persistent stores

- **Shape:** Insomnia, Dream, Freshness, Ego, Entity Resolution, etc. each own all associated persistence and algorithms. A lower physical Container is shared.
- **Strength:** domain features can be locally cohesive; shallow service/store pairs may disappear.
- **Weakness:** recovery, historical reconstruction, retrieval, import and cross-domain reads need durable facts without requiring inference/model runtimes. Service-to-service imports may produce cycles, implementation-only adapters or broad interfaces. In particular, Dream facts needed by history must not force a historical opener to depend on the Dream classifier/provider.
- **Assessment:** useful **locally** where no independent lower owner is needed, but not a universal packaging policy.

### Initial recommendation

Use **B** as the *structural starting point*, with a selective version of **C** for truly cohesive policy/data modules and no forced one-crate-per-store rule. Treat A as a simplification alternative if the exact interface proof finds current isolation to be mostly shallow overhead. Do not approve B by inertia; test it through real cross-domain writes, historical reads, retained corpus, and an add-a-new-domain exercise.

**Key distinction:** storage as an isolated **subsystem** does not require all persistence to live in one physical crate. Isolation means outside code cannot reach mutable store internals except through owned, typed, explicitly authorized operations.

## 4. Candidate owner-session contract (not yet a frozen Rust API)

**Owner identity and opening:** a typed REL or PHY session binds the exact container file kind, durable owner UUID, replay boundary, semantic domain state, and accepted clocks. It is not tied to an ambient project path or one viewer's selection.

**Reads:** produce immutable, owner-qualified facts or bounded read views (including indexed streaming/iterators where the actual workload requires them). Authorization originates from the runtime/workflow boundary and is revalidated before sensitive hydration/publication. Avoid copying every Memory/vector/graph edge just to cross a software abstraction.

**Mutation:** named typed commands operate on a specific owner and domain: accepted Archive turn, Memory revision, Graph relation, Ego change, Freshness effect, Insomnia completion, etc. A domain validates intrinsic semantics; the owner-session/workflow boundary authenticates foreign references, source proof and publication ordering. There is no general `write_record(bytes)` semantic escape hatch.

**Grouped intra-file writes:** one explicit coordinator prepares a set of domain-validated effects, checks all expected revisions/identities, appends within the established physical grouping, syncs and returns exact receipts; retry/reopen must not partially promote a group. Container is oblivious to the domain names.

**Cross-REL/PHY writes:** do **not** invent a single cross-file atomic database transaction. Preserve current deterministic owner-qualified identity, retry, incomplete-operation handling and publication recovery. Document the actual order and crash cases per workflow.

**Historical reads:** distinguish source-time, transaction/global cut, domain local revision and tombstone/content retention. Historical composition uses immutable owner proofs, not live semantic inference; partial/cut-unavailable is an explicit result.

**Maintenance:** inspect/plan -> owner-validated retention/transform -> write transformed copy -> verify/reopen -> optional separately authorized promotion. Maintenance does not receive unrestricted mutation privileges over a mounted live owner.

**Recovery:** single physical ordered replay into domain owners; owner completion evidence is checked before a write-enabled session becomes visible. Unknown/retired formats fail closed. Repair remains opt-in and authority-bound.

Possible thin public surface (illustrative concepts only): `open_rel`, `open_phy`, `read_snapshot`, `apply_<domain>_command`, `commit_group`, `historical_view`, `verify_recovery`. Specific method signatures, error types, read capability lifetimes and lock/transaction boundaries must be derived from current call sites before acceptance.

## 5. Growth tests — the architecture must support these future capabilities

These are **design scenarios**, not requests to implement the features.

| Future addition | Required extension path | Architectural failure signal |
| --- | --- | --- |
| Observation synthesis (ordinary and relational) | New/evolved semantic policy, owned Observation persistence and bounded Entity/Memory/Graph evidence inputs; runtime schedules lifecycle | Must edit Gateway internals or add a separate universal relationship graph/truth owner |
| Ego Memory-Web and cross-REL context | Ego synthesis consumes authorized immutable facts; Context Engine selects/render context, keeps current-session compaction separate | Ego becomes the global context assembler or gains direct access to all mounted stores |
| New source/provider integration (Hermes, ACP, other) | Implement source normalization/authorized hydration adapter and reuse semantic workflow contracts | Requires a new Insomnia/Dream implementation or duplicates host transcripts into Archive |
| New semantic service without persistence | Implement algorithm over typed facts with explicit bounded model route and a named workflow | Requires adding a fake Store crate, global state field and Gateway mutation entry for pure computation |
| New durable owner-local knowledge type | Add domain owner, explicit record replay/transaction integration, typed service/workflow and history/maintenance policy | Existing domains must be rewritten or Container learns its semantic schema |
| New cross-owner retrieval/context feature | Use owner-qualified authorization and bounded fan-out; compose only permitted REL closure plus requester's PHY | Ambient global active scope or unrestricted cross-PHY graph traversal returns |

**Extensibility test:** one feature should normally require changes in its domain, one bounded owner-composition admission site *if it introduces persisted state*, one explicit workflow, and public API/host wiring where requested. It must not require touching all existing semantic services, changing the whole global storage schema, or duplicating queues.

## 6. Cross-cutting workflows to prove before approving the architecture

1. **Conversation -> Insomnia -> Memory:** accepted Archive provenance, deterministic Episodes, bounded evidence/model stages, destination REL/PHY routing, grouped completion and Freshness birth, retry after crash.
2. **Memory -> Dream -> Graph/Community -> Freshness:** independent bounded decision calls, canonical/duplicate lifecycle, graph publication, first-cycle receipts, consistent history and due transitions.
3. **Retrieval -> agentic decision -> context -> consumption:** immutable candidate queries, scoring, authorized hydration, selected-ID state, successful host handoff, one idempotent accepted-use reinforcement.
4. **Entity / future Observations:** exact body-bound mentions, canonical Entity identity, owner-local/owner-qualified relational evidence, private PHY visibility, future Observation ingestion and reconsideration.
5. **Native import and external references:** separate authority modes, staged owner effects, import completion/reconciliation, no forged transcript source and no data loss from partial epochs.
6. **History/prune/rewrite/reconciliation:** owner-specific clocks, tombstone bytes and unavailable payloads, exact historical views and transformed-copy safety.
7. **Gateway concurrency:** multiple instances/owners, grants and revoke, per-branch queue, host result ambiguity, stream checkpoint visibility, reconnect/recovery with bounded owner locks.
8. **Project Environment:** Git/Lore project revisions remain outside REL semantic history, with exact revision correlation and attachment provenance.

Each workflow needs explicit sequence, read/write ownership, transaction/retry boundary, and lowest-cost canonical integration tests. No new architecture is accepted if a workflow requires importing a higher service into lower storage or reaching inside another domain's mutable state.

## 7. Preliminary dependency rules

- `Container ->` no Reliquary semantics.
- Domain persistence depends on Container and small shared *value/codec* modules only when justified.
- Per-owner composition depends on domain persistence owners; domain owners do **not** import the aggregate.
- Semantic algorithms depend on immutable value/read contracts and explicit endpoint contracts; not on mutable owner sessions, Gateway, Root, or arbitrary other algorithms. Required service-to-service dependencies must be named and acyclic.
- Workflow modules depend on relevant semantic algorithms and owner-session command/read interfaces. They own explicit multi-domain application transactions.
- Gateway depends on owner-session handles and workflow APIs for execution, plus host adapters/Context Engine as needed; workflows do not import Gateway.
- Public facade/CLI/integrators depend downward through canonical entry points. The old Root `Cva` and `Phylactery` names may be reexports or thin APIs, **never second mutable aggregates**.
- Services that need long-lived index access should use scoped, zero-copy/borrowed read capabilities where measured, not push high-volume data through broad serializable DTOs.
- External app/host service abstractions should reflect real deployment/ownership boundaries. Do not create generic ports for every internal helper just to satisfy unit tests.

These rules must be tested against the concrete rust type/call graph before fixing package names and crate boundaries. Cargo DAG is necessary; the still-Root source references are the unresolved proof surface.

## 8. Work required to turn foundation into final architecture

1. **Domain and authority map**: enumerate canonical entities, state machines, all record families, project/REL/PHY authority and expected extension points; reconcile inconsistent names. Source evidence, not speculative architecture.
2. **Owner-session detailed alternatives**: design at least two *concrete* Rust interface shapes (minimally exposed session vs capability-scoped domain commands), map the existing `Cva`/`Phylactery` fields/methods and test actual historical/replay/grouped-publication use cases. Explicitly select after comparison.
3. **Semantic service contracts**: map existing algorithm inputs/outputs and the legitimate dependencies, including when state should stay internal vs independently persisted/readable; apply the deletion test to current service/store pairs.
4. **Workflow/coordinator contracts**: write ordered read/compute/commit/retry contracts for the eight flows above, with crash and authorization scenarios.
5. **Gateway/Instance design**: durable queue/stream/host/worker runtime shape, owner access and lifecycle. No policy algorithm in Gateway; no second per-instance owner mutation context.
6. **Prove combined DAG and feature-extension scenarios**: compile-directed crate design, dependency direction checks, no guessed “allowed” edges, no performance-violating batch/snapshot requirement.
7. **Migration crosswalk**: every production Root/module/operation classified `keep`, `move`, `merge`, `split` or `delete`; no general Root exemption. Order hard cuts and tests without migrating implementation during design.
8. **Acceptance baseline**: separately repair/reverify `CVAFRS07` current corpus; freeze critical behavioral/performance workloads and the public/API/CLI/host tests before code moves.

Deliverables after these steps: one complete **system architecture**, one **ownership/interface specification**, one **actual Cargo/package DAG**, and one **source-to-target migration plan**. Only the first of these can be explored now; none is accepted merely by publishing this draft.

## 9. Decision frontier — recommendations, not approvals

**D1. Physical storage packaging.** Recommend a canonical per-owner session over separate purpose-built domains (Option B), with merging of shallow packages allowed. Needs exact interface/cost proof before approval.

**D2. Root's future role.** Recommend Root become a narrow public/API composition package, not a mutable store owner or execution engine. No autonomous policy/codec implementations unless explicitly justified by the approved design.

**D3. Cross-domain operations.** Recommend explicit, focused application/workflow coordinators, not a universal application manager and not a semantically omnipotent Gateway.

**D4. Service/store pairing.** Recommend case-by-case depth and dependency tests, not “everything split” or “everything combined.”

**D5. Extensibility scope.** Approved direction: support future Observation and Ego capability without implementing them in this refactor. Unknown future features must be possible through documented extension seams without a plugin architecture invented prematurely.

Only D5's **objective** is already selected. D1–D4 are proposed technical defaults and need source-level design confirmation; no approval is implied.

## 10. Verification discipline for this design phase

- This artifact is documentation only. No implementation, format, existing ADR or active migration specification is changed.
- Verify links/indexes and avoid bundling unrelated pre-existing worktree changes.
- Compilation and full regression checks apply to future source changes; this draft itself makes no new passing-runtime claim.
- Keep failed/pending format recovery as an independent verified blocker, not a justification for architectural special cases.
- Avoid broad automatic callgraph/architecture proof machinery: use direct source inspection, Rust/Cargo dependencies, exact behavior tests and targeted structural navigation.

## Related docs

- [Current implementation inventory](current-system-inventory-2026-11-03.md)
- [Final REL/PHY storage architecture](rel-phy-storage-architecture-2026-11-03.md)
- [REL naming decision](decisions/0043-rel-terminology-and-wire-identity.md)

## Notes

This is retained as the alternatives and system-scope exploration. Its storage question is resolved by Stage 1; no implementation or full-system architecture acceptance is implied.
