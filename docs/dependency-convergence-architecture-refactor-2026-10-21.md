# Dependency Convergence Architecture Refactor

Date: 2026-10-21
Status: Phases 0–1 implemented and accepted locally; Phases 2–7 remain proposed
Parent: [Roadmap](roadmap.md)

## Overview

This specification defines the dependency-convergence campaign target, scope, phase boundaries, and acceptance conditions. Phase 0 establishes the evidence and enforcement baseline; later phases change implementation boundaries while preserving current behavior except for explicitly identified corrections.

## Purpose

Define the broad architecture refactor required to remove Reliquary's current dependency knot, restore clear ownership direction, retire obsolete architectural surfaces, and incorporate a limited set of already-identified corrective work that is materially cheaper or safer to perform while those boundaries are being changed.

This specification does **not** define new product or semantic features. It does not replace the roadmap and does not schedule general roadmap implementation.

The implementation plan derived from this document must preserve current behavior except where this specification explicitly identifies an existing architectural defect, duplicated responsibility, obsolete compatibility surface, or already-planned correction.

## Problem

Reliquary is decomposed by responsibility but not sufficiently by dependency.

The current implementation contains a large strongly connected dependency region spanning major parts of:

- physical/storage coordination;
- Archive and historical state;
- Memory and semantic owners;
- Freshness and Dream;
- Perception/Relationship state;
- retrieval;
- configuration/security;
- runtime inference;
- Gateway/Instance orchestration;
- Project Environment integration; and
- the public facade.

This creates several practical problems:

1. contained changes acquire unrelated dependencies and expand in scope;
2. owner-local behavior is frequently coordinated through broad facade/runtime surfaces;
3. lower-level owners can become coupled to orchestration or public presentation concerns;
4. obsolete transitional layers remain expensive because many consumers still know about them;
5. correction work duplicates authoritative state rather than depending on narrow owner contracts;
6. documentation becomes difficult to keep current because execution history and compatibility routing leak into architecture descriptions; and
7. future changes risk deepening the knot unless the desired dependency direction is mechanically enforced.

The goal is not merely to produce an acyclic graph. The refactor must increase locality and module depth so future changes remain bounded.

## Accepted foundational target and invariants

The full logical component DAG has now been designed and adopted: [Target Component DAG](dependency-convergence-target-dag-2026-10-21.md), with the canonical [34-component edge allowlist](../tools/pitlord/convergence-component-dag.json). **Every permitted edge is explicitly identified and justified.** The target requires no cycles, no upward/ancestral backreach even where acyclic, no unlisted sibling dependencies, owner-local encapsulation, and downward shortcuts only where explicitly permitted. This is the permanent project architecture target, not optional design guidance. The current 19-area source capture is a migration/evidence view, not a conflicting target.

The completed logical design is separate from the **locally verified Phase 0 source-baseline gate**: all captured findings have been classified and the existing 19-area ratchet passed. Code mapping and finer 34-component source enforcement remain distinct acceptance work for the migration phases. Later phases must transform the current implementation to the DAG without adding future roadmap features.

## Target dependency direction

The intended architectural direction is:

```text
physical storage / persistence mechanics
        ↓
owner-local stores and durable owner contracts
        ↓
domain services and owner composition
        ↓
cross-owner orchestration / runtime coordination
        ↓
public product facade / host integration
```

External dependencies such as model providers, Lore/Git, filesystem services, and host adapters enter through explicit boundaries appropriate to their real ownership. They must not become upward callbacks from semantic owners into orchestration or product layers.

This direction is a constraint, not a requirement to split the repository into crates. Module and ownership boundaries should be proven before any crate split is considered.

## Refactor principles

### 1. Ownership before layering

Every durable fact and mutation path must have one authoritative owner.

A dependency may be inverted or removed only after its responsibility is assigned to the correct existing owner. The refactor must not create generic managers, service locators, or shared mutable registries merely to make the graph acyclic.

### 2. Replace, do not layer

When an obsolete boundary is replaced:

- migrate callers to the new owner/seam;
- move protecting tests to the new interface;
- delete obsolete forwarding APIs, compatibility wrappers, and duplicate ownership;
- do not retain permanent bridge abstractions solely to keep old call shapes alive.

Temporary migration machinery is permitted only inside a bounded implementation step and must have an explicit deletion gate.

### 3. Deep seams only where ownership varies

Introduce traits/interfaces only where they represent a real ownership or deployment seam.

Do not create one-interface-per-module dependency inversion. Prefer direct owner-local dependencies where ownership is stable.

### 4. Shared types must be dependency-neutral

Types used across layers must live with their semantic owner or in a genuinely neutral contract module.

Moving DTOs into a generic shared package solely to break cycles is not sufficient if the ownership dependency still exists conceptually.

### 5. Hard cuts are preferred during development

Reliquary's current development policy remains in force: when internal or persisted architecture is replaced, migrate controlled development state as required, verify the replacement, then remove temporary compatibility paths.

No new long-lived backward-compatibility burden should be introduced without an explicit reason.

### 6. Preserve behavior while changing structure

This campaign is primarily structural.

New behavior is out of scope unless it is required to correct an existing defect identified in this document or to preserve existing behavior after ownership moves.

### 7. Documentation changes with architecture

Every phase must leave the architecture it touches correctly documented at HEAD.

There is no separate later pass whose purpose is to reconstruct what the refactor changed.

## Scope

### Included

The campaign includes:

- dependency convergence and SCC elimination;
- removal of upward/backward dependencies that violate ownership;
- retirement of obsolete RuntimeHost/facade architecture;
- Project Environment / project-VCS responsibility simplification already established by existing ADRs;
- the existing semantic import-settlement minimization correction;
- project-path/repository-scope authorization correction at the proper Project Environment boundary;
- selective Container ownership/recovery corrections when required to establish the storage boundary;
- removal of obsolete compatibility/forwarding surfaces encountered by the refactor;
- architecture enforcement/ratchets needed to prevent cycles from returning; and
- documentation repair for areas touched by the refactor.

### Explicitly excluded

The campaign does **not** implement or plan new feature work, including:

- cross-scope Entity capability expansion;
- Perception Observation implementation or relational-Observation feature work;
- Ego synthesis, cross-chat context, or cross-REL summary features;
- explicit Memory commit;
- Hermes/provider/import adapter expansion;
- the deferred External Sources redesign and Warlock-default source-duplication posture (recorded in the roadmap and external-host integration plan); retain the current contract for this correction-only campaign;
- Warlock product/UI adoption work;
- new Chronos behavior;
- new version-history/rollback features;
- new REL/PHY synchronization behavior;
- new file-management/product usability features;
- organization/team scope features;
- agentic retrieval quality improvements;
- archive-vector row/block optimization;
- new storage compression projects;
- new model/provider/security product capabilities beyond boundary corrections required by the refactor.

Existing future features may constrain seam design, but they are not implementation deliverables of this campaign.

## Existing corrective work incorporated into this campaign

### Semantic import-settlement minimization

The current `CVAFRS06` semantic import settlement is materially over-materialized. The existing follow-up plan identifies duplicated Freshness, Memory, and import state that should instead be authenticated through authoritative owner evidence.

This correction belongs in the convergence campaign because its solution depends on clean owner boundaries.

The campaign must:

- establish narrow owner read/proof contracts required by settlement validation;
- prevent the settlement from becoming a second copy of authoritative semantic state;
- preserve semantic attribution and corruption detection; and
- execute the actual compact settlement hard cut only after the required owner seams are stable.

The detailed settlement format remains owned by its existing follow-up plan. This architecture specification only places that correction in the convergence sequence.

### Project Environment / project-VCS responsibility simplification

Existing ADRs already establish:

- Lore/Git owns project-file tree/history authority;
- Reliquary owns semantic state and correlations to exact repository state;
- project repository revisions and REL semantic cuts are correlated, not nested; and
- Reliquary should not maintain a second project VCS.

The convergence campaign must remove or simplify remaining Reliquary machinery whose primary purpose duplicates:

- project/blob ancestry;
- project-file revision history;
- filesystem reconstruction history;
- project-file merge/replay behavior; or
- other repository-history responsibilities already owned by Lore/Git.

The campaign must preserve:

- exact repository revision/file references used by semantic provenance;
- ProjectRevisionCorrelation;
- repository identity validation;
- repository-backed historical reads required by Reliquary semantics; and
- the distinction between semantic history and project history.

No new repository-management feature is part of this campaign.

### Obsolete RuntimeHost/facade retirement

The Gateway/Instance architecture is current. The historical `ReliquaryRuntimeHost` architecture is no longer an independent runtime owner.

The convergence campaign must remove residual architectural dependence on obsolete host/facade layers, including:

- obsolete forwarding APIs;
- old owner assumptions in tests;
- duplicate queue/worker/selector ownership;
- compatibility naming when retaining it creates real coupling; and
- internal calls that route owner behavior upward through the product facade.

Useful result/error types may be relocated to their actual owner.

### Project path and repository authorization

The existing architecture audit identified a boundary defect where repository identity may be validated while path-scoped historical reads can bypass the intended project scope.

The Project Environment convergence phase must establish one canonical validated project/repository path authority and require all repository-backed reads/mutations to consume that validated authority.

This is a correction to an existing boundary, not new authorization functionality.

### Container ownership and recovery corrections

The architecture audit also identified two storage-boundary concerns:

- lack of a general explicit writer-ownership/exclusion contract; and
- open/scan recovery behavior that may mutate/truncate storage while attempting inspection.

These corrections belong in this campaign only to the degree required to establish a clean physical-storage boundary.

The refactor should prefer:

- explicit write ownership/lease semantics at Container;
- non-mutating inspection;
- explicit repair/recovery operations; and
- semantic owners deciding semantic reachability above Container.

Broader storage hardening remains separate roadmap work.

## Architectural accommodation without feature implementation

Some future systems already have accepted ownership direction and therefore constrain this refactor.

The convergence design must leave clean extension points for them without implementing them.

### Context Engine and Ego — target dependency ownership (2026-10-21)

Context Engine is the independent owner of **all final model-facing context management and assembly**. Its current implemented scope is conversation compaction and bounded conversation views; future planned context assembly additionally consumes authorized outputs from Ego, Retrieval, Archive/Echo and other producers. Context Engine selects material, allocates the final combined context budget, manages current-session and cross-chat context, includes authorized cross-REL summaries, and constructs the final model-facing request. It does **not** own semantic synthesis, source truth, or provider transport.

Ego owns semantic generation and persisted Identity, Personality, Anchors and Memory-Web Summaries, but **does not own runtime injection, final context selection, or final input assembly**. Ego may perform its own bounded *generation-time* selection/budgeting; the Context Engine alone arbitrates *inference-time* selection and budgeting across producers. Dependency direction: **Runtime/Gateway → Context Engine → Ego (read contracts), Archive/Echo, Retrieval and other authorized context producers**. Ego and lower-level owners must not import Context Engine. Gateway continues to own session execution, mounted authorization and provider coordination, not context-content assembly.

For this correction-only campaign, establish or preserve that acyclic ownership boundary where existing dependencies are touched; do **not** implement future Ego synthesis, cross-chat, cross-REL inclusion, or comprehensive final-assembly features. The separate compaction-refinement plan remains scoped to existing current-session compaction; future assembly is tracked by the roadmap and relevant Ego plans.

### Relationship / Observation transition

Relationship is transitional under the accepted Perception architecture.

The refactor must avoid making Relationship a more central shared dependency. It should isolate current Relationship persistence/consumption behind Perception-owned boundaries so later relational-Observation replacement does not require another system-wide dependency inversion.

No Observation feature work is included.

### Ego

Ego remains an upper semantic/context-composition consumer.

The refactor must not introduce lower-level dependencies on Ego or use Ego as a generic cross-owner state service.

No Ego synthesis/context feature work is included.

## Campaign structure

Each phase below is a broad architecture phase, not a detailed implementation step list.

A later linear execution plan may split a phase into many small commits and may interleave narrowly independent work where safe.

### Phase 0 — Freeze, graph, and enforcement baseline

Purpose: establish the exact current dependency problem and prevent it from worsening during the campaign.

Required work:

- capture the current module/domain dependency graph from current `main`;
- identify the principal SCCs and the concrete edges that create them;
- classify edges by responsibility: durable ownership, read dependency, mutation dependency, construction/init, orchestration callback, shared type, configuration, test-only, or obsolete compatibility;
- record current public/runtime/facade entrypoints that cross ownership boundaries;
- define the target allowed layer/domain direction in machine-checkable form where practical;
- preserve existing functional baselines; and
- introduce ratchet-style architecture checks that can tighten as cycles are removed.

Do not require the final DAG before implementation begins. Enforcement should ratchet progressively.

Documentation:

- add the target dependency model to Architecture/Invariants;
- record the campaign ownership map;
- remove no feature documentation merely because its implementation depends on a future phase.

Exit condition:

- every major SCC edge has an explicit disposition;
- the target dependency rules are concrete enough to drive implementation;
- no new unclassified back-edge may be introduced.

### Phase 1 — Physical storage and owner-store foundation

Purpose: establish a lower layer that has no semantic dependency on facade/runtime orchestration.

Required work:

- make Container and physical framing depend only on physical/storage contracts;
- remove semantic/facade/runtime callbacks from physical storage paths;
- retain owner-aware semantic reachability above Container;
- establish explicit writer ownership/exclusion as needed for a coherent storage boundary;
- separate non-mutating inspection from explicit repair/recovery where the current path conflates them;
- ensure owner stores can replay/open through storage contracts without requiring public facade construction;
- relocate dependency-neutral physical types only where necessary.

Do not:

- redesign storage formats for size alone;
- implement new compression;
- turn Container into a semantic owner.

Documentation:

- update Storage Format, Architecture, Invariants, semantic-maintenance docs, and protecting contract references for any changed recovery/write boundary.

Exit condition:

- Container/physical storage is outside the core semantic/runtime SCC;
- semantic reachability and policy remain outside Container;
- owner stores do not require runtime/facade dependencies to open/replay.

### Phase 2 — Owner-local semantic boundaries

Detailed execution specification: [Phase 2 owner-local boundary plan](dependency-convergence-phase2-owner-local-boundaries-spec-2026-10-22.md) — 91 bounded tasks with per-owner, physical-preservation, privacy, ratchet and corpus gates. The accepted DAG is authoritative; Phase 2 is specified, **not implemented**.

Purpose: make each durable semantic owner independently usable through a narrow, owner-correct interface.

Primary areas include:

- Archive / Echo / Episodes / interaction state;
- Memory;
- Entity and current Perception-owned transitional Relationship state;
- Graph;
- Freshness;
- Dream durable owner state;
- vector/profile/generation owners;
- Ego persisted owner state where currently implemented;
- import epoch/settlement owner state;
- external-source durable state.

Required work:

- remove cross-owner implementation reach-through where an owner contract should exist;
- separate owner-local persistence/validation from cross-owner coordination;
- move shared lifecycle/proof/read types to their actual owner or a neutral contract only where justified;
- prevent owner stores from calling upward into facade/runtime orchestration;
- preserve shared `owner_lifecycle` as vocabulary without turning it into a generic persistence owner;
- isolate transitional Relationship dependencies behind Perception ownership;
- expose only the read/proof seams actually needed by higher coordination layers.

This phase should also establish the owner seams later used by import-settlement minimization.

Documentation:

- current subsystem docs must describe actual owner boundaries after each cut;
- stale lifecycle/owner claims encountered in canonical docs are corrected immediately.

Exit condition:

- major durable owners can be opened, replayed, read, and mutated through owner-local contracts without facade/runtime dependencies;
- cross-owner logic is visibly separate from owner-local storage;
- the central SCC is materially reduced.

### Phase 3 — Historical, maintenance, import, and reconciliation composition

Detailed correction-only execution spec: [Phase 3 History/Maintenance/Import convergence](dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md). It defines 98 bounded tasks with hard-cut gates, the accepted Phase 2 owner proof seams, read-only History, explicit Maintenance/Import coordination, and the preapproved CVAFRS06 semantic-settlement minimization. The spec **does not** alter the accepted DAG or implement the phase.

Purpose: move cross-owner historical/maintenance behavior into explicit domain services above owner stores instead of allowing the facade to become the integration layer.

Required work:

- separate historical reconstruction/oracles from public facade plumbing;
- isolate pruning, rewrite, reclamation, reconciliation, and import completion coordination behind explicit composition modules;
- make cross-owner proof acquisition depend on owner read/proof contracts rather than internal store structure;
- remove obsolete compatibility/normalization paths that survive only because coordination code still knows about them;
- keep development hard-cut policy intact.

This phase includes the semantic import-settlement correction after its required owner seams are available:

- audit every persisted settlement field against its authoritative owner;
- remove reconstructible duplicated semantic state;
- retain only irreducible semantic authority or compact authenticated commitments;
- perform the normal development-format hard cut for the corrected settlement;
- delete temporary conversion/compatibility machinery after corpus verification.

Do not:

- implement new historical reconstruction features;
- implement new sync/conflict policy;
- reopen the completed 377-step compact-history refactor except for this explicit correction.

Documentation:

- Storage Format and semantic-maintenance docs become authoritative for the corrected settlement;
- superseded settlement descriptions are retired/archived;
- canonical docs must no longer depend on historical execution plans to explain current behavior.

Exit condition:

- historical/maintenance/import/reconciliation coordination no longer forces owner stores or storage to depend upward;
- semantic settlement no longer duplicates reconstructible owner state;
- temporary correction machinery is removed.

### Phase 4 — Project Environment and repository boundary convergence

Purpose: make Project Environment a clean integration boundary over repository authority rather than a duplicate project-history system.

Required work:

- enforce Lore/Git as project-file history authority;
- remove/simplify duplicate Reliquary project/blob ancestry and repository-history machinery;
- retain only semantic correlations and exact repository-backed references required by Reliquary;
- establish canonical validated repository/project-path authority;
- require historical file reads and repository-backed operations to use that validated authority;
- separate machine-local checkout/mount location from durable repository semantic identity;
- remove stale Warlock-owned naming/compatibility surfaces when they create architecture coupling.

Do not:

- add new repository management modes;
- add new file usability features;
- design new cloud synchronization behavior.

Documentation:

- reconcile Architecture, Project Environment subsystem docs, ADR references, API and current limitations with the final ownership boundary.

Exit condition:

- Reliquary no longer contains a second project VCS;
- repository integration is a bounded dependency of Project Environment;
- path/repository authorization is enforced at one canonical boundary;
- project integration no longer participates unnecessarily in the core SCC.

### Phase 5 — Runtime/orchestration convergence

Purpose: make runtime coordination depend downward on domain services instead of sharing ownership with them.

Required work:

- isolate owner execution from Gateway/Instance coordination;
- ensure Gateway owns mounted orchestration rather than owner-local semantic behavior;
- retire residual `ReliquaryRuntimeHost` ownership assumptions;
- remove obsolete forwarding/facade APIs and old-owner tests;
- preserve current worker, queue, scheduling, stream, and mounted-owner behavior;
- keep provider/model calls outside long owner locks;
- ensure configuration/runtime routes are consumed through narrow runtime contracts;
- prevent domain owners from depending on Gateway, Instance, or host presentation types.

Context Engine should end this phase with clean conversation/Echo/inference dependencies, but its compaction-refinement feature plan remains deferred.

Documentation:

- Architecture and runtime subsystem docs describe only the current Gateway/Instance/owner-execution model;
- phase/P-step execution diary material encountered in canonical docs is removed or moved to evidence.

Exit condition:

- runtime orchestration is above owner/domain services in the dependency graph;
- obsolete host architecture is deleted rather than wrapped;
- runtime no longer belongs to the same core SCC as physical storage and owner stores.

### Phase 6 — Public facade and integration surface convergence

Purpose: make the public facade a composition/export boundary rather than an internal dependency hub.

Required work:

- eliminate internal dependencies on `Cva`, `Reliquary`, or public facade types where lower/domain contracts suffice;
- keep public convenience composition at the top of the graph;
- relocate shared errors/results/types to their true owner when the facade is currently their accidental home;
- remove obsolete public forwarding APIs where hard-cut retirement is justified;
- preserve supported current behavior and stable semantics;
- reduce `lib.rs`/public registry coupling where this follows naturally from proven ownership.

Do not:

- perform broad naming churn solely for aesthetics;
- add new public capabilities;
- turn the campaign into an API redesign unrelated to dependency convergence.

Documentation:

- update API, behavioral contracts, maintainer map, and subsystem links for every changed supported surface.

Exit condition:

- no lower layer depends on the public facade;
- public facade composes downward through stable domain/runtime contracts;
- accidental public surface uncovered by the refactor has an explicit disposition.

### Phase 7 — Graph closure, ratchet, and documentation closeout

Purpose: prove the architectural objective and finish only the documentation debt exposed or created by the campaign.

Required work:

- recompute the complete dependency graph;
- verify the giant core SCC is gone;
- verify all remaining cycles are intentionally local and justified, or remove them;
- tighten architecture policy from ratchets to the final allowed graph;
- remove temporary adapters, compatibility seams, migration modules, dead tests, and stale routing introduced only for convergence;
- run full functional/architecture verification;
- update canonical documentation so it stands without execution diaries.

Documentation closeout in this phase is limited to campaign-related truth:

- Architecture;
- Invariants;
- subsystem ownership docs;
- Storage Format for changed persistence;
- API/behavioral contracts for changed surfaces;
- Maintainer Map;
- Current Limitations entries touched by the campaign;
- roadmap disposition of incorporated corrective plans.

Independent documentation-audit work that is unrelated to the refactor may remain outside this campaign.

Exit condition:

- the intended dependency direction is mechanically enforced;
- the core architecture is no longer one giant SCC;
- no temporary convergence seam remains;
- canonical docs accurately describe the resulting architecture;
- incorporated corrective work is either complete or explicitly returned to its original separate plan if it proved independent.

## Standard structure for each implementation phase

The later linear plan should give each phase the same internal structure.

### A. Baseline

- identify exact modules/edges owned by the phase;
- capture focused tests and architecture checks;
- record current public/persistent contracts that must survive.

### B. Boundary design

- identify authoritative owners;
- compare materially different seam designs where the boundary is uncertain;
- choose the smallest interface with sufficient leverage;
- define prohibited dependencies after the cut.

### C. Migration

- move consumers in small coherent slices;
- preserve working tests;
- avoid cross-phase opportunistic feature work;
- hard-cut obsolete internal paths once all callers move.

### D. Deletion

- remove superseded modules/wrappers/types/tests;
- remove temporary adapters;
- remove stale compatibility code not justified by current data/product contracts.

### E. Ratchet

- tighten dependency/ownership checks so the removed edge cannot return;
- add or move tests to the new interface.

### F. Documentation

- update canonical docs for the exact architecture now present;
- graduate durable facts from any consumed planning document;
- retire stale statements and execution-diary routing.

### G. Acceptance

- focused owner/domain tests;
- cross-boundary integration tests;
- architecture graph/policy checks;
- formatting/build/static gates;
- current-format retained-corpus checks where persistence changed.

## Target architectural properties

The campaign is successful when the resulting system has these properties.

### Physical layer

- knows bytes, framing, identity headers, allocation, synchronization, and physical recovery;
- does not know semantic reachability, import policy, runtime routing, or public product behavior.

### Owner stores

- own durable semantic state and validation;
- expose narrow owner-local read/mutation/proof contracts;
- do not depend on Gateway, public facade, product UI, or unrelated owners.

### Domain services

- compose owner-local state for one semantic responsibility;
- contain cross-owner rules only where that domain actually owns the composition;
- do not become generic coordination frameworks.

### Historical/maintenance services

- compose explicit owner evidence;
- do not obtain authority by reaching through facade internals;
- distinguish semantic transformation from physical reclamation.

### Project Environment

- integrates repository authority without duplicating it;
- owns semantic correlation and repository validation;
- treats checkout paths as host-local bindings, not durable identity.

### Runtime

- owns scheduling, mounted-owner coordination, worker lifecycle, stream delivery, and host capability execution;
- does not own semantic truth already owned by domain stores.

### Public facade

- is a top-level composition and usability surface;
- is not imported by lower layers;
- does not act as the hidden service locator for internal behavior.

## Acceptance criteria for the whole campaign

The refactor is complete only when:

1. the current giant core SCC is eliminated;
2. the target dependency direction is mechanically enforced;
3. physical storage has no semantic/runtime/facade dependency;
4. owner stores have no Gateway/public-facade dependency;
5. runtime orchestration depends downward through domain contracts;
6. the public facade has no lower-layer consumers;
7. duplicate Project/VCS responsibilities identified by existing ADRs are removed;
8. obsolete RuntimeHost ownership/forwarding architecture is removed;
9. project repository/path authorization is enforced at one canonical boundary;
10. the semantic import settlement no longer duplicates reconstructible authoritative state;
11. temporary migration/compatibility seams introduced by the campaign are deleted;
12. current behavior remains covered by owner/interface-level tests rather than obsolete implementation-shape tests;
13. architecture policy prevents reintroduction of the removed cycles; and
14. canonical documentation accurately describes the final current architecture.

## Relationship to the roadmap

This campaign consumes only existing corrective/architectural roadmap work that directly intersects dependency convergence.

It does not absorb the general semantic/product roadmap.

Feature tracks remain independently deferred until the convergence campaign is complete or until a separately justified dependency-independent feature is intentionally prioritized.

## Relationship to detailed execution planning

This document defines the target and broad phase structure.

It intentionally does not specify:

- file-by-file edits;
- commit count;
- exact test commands;
- worktree layout;
- sub-step numbering;
- parallel-agent assignments; or
- detailed migration choreography.

Those belong in a later linear implementation plan derived from a fresh dependency-edge inventory against the chosen implementation baseline.

## Notes

The campaign should optimize for deletion, locality, and clear authority rather than for preserving current module names.

An acyclic graph alone is insufficient. If a change still requires callers to understand many shallow interfaces or route through several coordination layers, the architecture has not meaningfully converged.

## Related docs

- [Roadmap](roadmap.md)
- [Dependency convergence Phase 0 inventory](dependency-convergence-phase0-2026-10-21.md)
- [Architecture](architecture.md)
- [Architectural invariants](invariants.md)
- [Maintainer map](maintainer-map.md)