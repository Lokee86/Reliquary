# Accepted target component DAG

Date: 2026-10-21  
Status: **Accepted logical target** for the correction-only dependency-convergence refactor. Phase 0's **coarse source baseline and ratchet passed locally**; the finer target is partially mapped for the physical boundary; the remaining owners are not yet source-separated.  
Parent: [Dependency Convergence Architecture Refactor](dependency-convergence-architecture-refactor-2026-10-21.md)  
Machine-readable authority: [Logical DAG allowlist](../tools/pitlord/convergence-component-dag.json)

## Governing decision

Reliquary's target is **not merely an acyclic source tree**. It is a **closed, explicitly permitted component dependency graph**, directed from each consumer to the component it uses. Every new production edge must satisfy all of these independently:

1. **Acyclicity:** no direct or transitive cycles, including those hidden by reexports, generic types, generated code, construction, or callbacks. In-process and cross-crate dependencies are included.
2. **No upward or ancestral backreach:** a lower-authority component cannot depend on a higher/coordinating component **even where adding that dependency would remain acyclic**. For example, `memory-store → retrieval` and `archive-store → runtime-gateway` are forbidden before they can complete any cycle.
3. **Default deny:** a component depends only on its **listed direct dependencies** in the machine-readable target. A possible edge is not approved merely because the destination is lower in the order. Every addition requires a concrete purpose, accepted ownership, and ratchet update.
4. **Deep and independent components:** sibling processors have no implicit dependency on each other. One may invoke another only when its own responsibility requires that exact accepted API and the edge is in the allowlist. Avoid fragmenting a coherent module into micro-components to inflate independence.
5. **Encapsulated authority:** the called component retains exclusive ownership of its durable state, validation, policy and publication. A caller's read access does not authorize mutation or direct struct/field access.
6. **No lifecycle inversion:** low-level stores and capabilities never import Runtime, Gateway, public facade, host adapters, or a high-level callback. Owner events can be returned or published through owner-local contracts; Runtime can react without reverse structural dependencies.
7. **Direct downward shortcuts are legitimate:** Runtime may call Retrieval and owner read APIs directly; Context Engine may independently call Retrieval; Retrieval may read Memory, Archive and Graph. An artificial one-hop-per-layer rule is **not** an invariant.

`rank` in the logical model is a **topological authority order**, not a promise that there are exactly fourteen deployed layers. Every permitted edge must go to a **strictly lower** rank. Equal-rank peers are independent; justified peer invocations are given an explicit order rather than granted unrestricted sibling access. The rank is necessary but **never sufficient** for permission: the exact allowlist is authoritative.

These are **permanent project invariants**, not temporary Phase 0 cleanup techniques. Production code, tests, Cargo crate boundaries and future new components must preserve the intent; test-only exceptions must be identified, not misreported as production architecture. New substantial components require updating the model before shipping code that depends on them.

## Exact logical dependency register

The following table is generated from the accepted JSON target and lists **every logical component and every allowed immediate target**. It has 34 components and 163 directed edges. The JSON additionally records for **every edge** whether it is a read, write, construction or invocation, along with its justification. **No unlisted logical edge is permitted.**

| Component | Order | Role | Direct dependencies (not transitive) | Main extraction phase |
| --- | ---: | --- | --- | --- |
| `pure-codec` | 0 | foundation | — | P1 |
| `review-clocks` | 0 | foundation | — | P1 |
| `config-security` | 0 | foundation | — | P5 |
| `container` | 1 | physical | `pure-codec`, `review-clocks` | P1 |
| `archive-store` | 2 | store | `container`, `pure-codec`, `review-clocks` | P2 |
| `memory-store` | 2 | store | `container`, `pure-codec`, `review-clocks` | P2 |
| `graph-store` | 2 | store | `container`, `review-clocks` | P2 |
| `perception-store` | 2 | store | `container`, `pure-codec`, `review-clocks` | P2 |
| `ego-store` | 2 | store | `container`, `pure-codec`, `review-clocks` | P2 |
| `freshness-store` | 2 | store | `container`, `review-clocks` | P2 |
| `vector-store` | 2 | store | `container`, `pure-codec` | P2 |
| `insomnia-store` | 2 | store | `container`, `review-clocks` | P2 |
| `dream-store` | 2 | store | `container`, `review-clocks` | P2 |
| `import-store` | 2 | store | `container`, `review-clocks` | P2 |
| `external-index-store` | 2 | store | `container` | P2 |
| `maintenance-store` | 2 | store | `container`, `review-clocks` | P2 |
| `owner-lifecycle` | 3 | composition | `container`, `archive-store`, `memory-store`, `graph-store`, `perception-store`, `ego-store`, `freshness-store`, `vector-store`, `insomnia-store`, `dream-store`, `import-store`, `external-index-store`, `maintenance-store` | P1 |
| `switchboard` | 3 | capability | `config-security` | P5 |
| `chronos` | 4 | capability | `review-clocks`, `switchboard` | P5 |
| `history` | 4 | capability | `owner-lifecycle`, `archive-store`, `memory-store`, `graph-store`, `perception-store`, `ego-store`, `freshness-store`, `vector-store`, `insomnia-store`, `dream-store`, `import-store`, `external-index-store`, `maintenance-store` | P3 |
| `project-environment` | 4 | capability | `archive-store`, `owner-lifecycle` | P4 |
| `vectors` | 5 | service | `switchboard`, `vector-store`, `archive-store`, `memory-store` | P2 |
| `freshness` | 5 | capability | `freshness-store`, `memory-store`, `graph-store`, `review-clocks` | P2 |
| `retrieval` | 6 | service | `memory-store`, `archive-store`, `graph-store`, `perception-store`, `vectors`, `vector-store`, `freshness`, `switchboard` | P2 |
| `perception` | 7 | service | `perception-store`, `memory-store`, `archive-store`, `graph-store`, `retrieval`, `vectors`, `chronos`, `switchboard` | P2 |
| `insomnia` | 8 | service | `insomnia-store`, `archive-store`, `memory-store`, `perception`, `chronos`, `switchboard`, `freshness` | P2 |
| `dream` | 8 | service | `dream-store`, `memory-store`, `graph-store`, `vectors`, `chronos`, `switchboard`, `freshness` | P2 |
| `ego` | 8 | service | `ego-store`, `memory-store`, `graph-store`, `retrieval`, `freshness`, `archive-store`, `switchboard` | P2 |
| `external-sources` | 9 | integration | `external-index-store`, `vectors`, `insomnia`, `memory-store`, `freshness` | P5 |
| `maintenance` | 9 | service | `history`, `owner-lifecycle`, `container`, `archive-store`, `memory-store`, `graph-store`, `perception-store`, `ego-store`, `freshness-store`, `vector-store`, `insomnia-store`, `dream-store`, `import-store`, `external-index-store`, `maintenance-store` | P3 |
| `import` | 10 | service | `import-store`, `archive-store`, `insomnia`, `dream`, `perception`, `memory-store`, `freshness`, `history`, `owner-lifecycle` | P3 |
| `context-engine` | 11 | service | `archive-store`, `ego`, `retrieval`, `freshness`, `switchboard` | P5 |
| `runtime-gateway` | 12 | orchestration | `owner-lifecycle`, `context-engine`, `retrieval`, `insomnia`, `dream`, `perception`, `ego`, `import`, `external-sources`, `project-environment`, `maintenance`, `history`, `vectors`, `freshness`, `chronos`, `switchboard`, `archive-store`, `memory-store`, `config-security` | P5 |
| `public-consumers` | 13 | consumer | `runtime-gateway`, `maintenance`, `history`, `import`, `project-environment` | P6 |

### What the components mean

- **Physical:** `container` owns framing, file identity, ordered allocation, bounded I/O, writer exclusion, read-only inspection and explicitly authorized physical repair. Pure codecs and typed review clocks are downward vocabulary, never semantic managers.
- **Owner-local durable stores:** `archive-store` owns source conversation/Echo/interaction material and compaction checkpoints; `memory-store` owns Memory revisions/provenance/lifecycle; `graph-store` owns typed Memory topology and Community projections; `perception-store` owns Entity, mention-resolution and the current transitional Relationship persistence (later Observation, **not implemented here**); `ego-store` owns Identity/Personality/Anchors and semantic summary outputs; `freshness-store`, `vector-store`, `dream-store`, `insomnia-store`, `import-store`, `external-index-store` and `maintenance-store` own their named durable evidence. These are **state boundaries**, not twelve standalone daemons.
- **Owner-lifecycle:** existing REL/PHY owner opening, replay, physical-handle ownership and validated aggregation currently dispersed through Cva/Phylactery and facades. This is a real deep composition responsibility, **not** a new mutable global service locator. It must not call into Dream, Insomnia, Gateway, public facade or semantic policy.
- **Shared capabilities:** `switchboard` executes model routes/provider transport but owns no semantic decision; `chronos` owns shared temporal reasoning and calls the provider endpoint when needed; `vectors` owns embedding generation/profile compatibility and the distinct vector store; `freshness` owns triggered-only score policy/effects and its store, without an autonomous execution lifecycle; `history` reconstructs old state **read-only**. A capability is separately owned even when its use is synchronous.
- **Semantic services:** `retrieval` directly reads Memory, Archive, Graph, vector and Freshness evidence. `perception` accepts Entity/mention semantics; `insomnia` authors extracted Memories; `dream` authors pair and Community semantics; `ego` authors semantic Identity/Anchor/summary artifacts. Their durable outputs remain owner-local; they do not own Runtime or each other's worker lifecycle.
- **Coordinators/integrations:** `import` manages historical ingestion and its existing exceptional settlement/completion boundary; `external-sources` currently integrates host-canonical conversation evidence and must remain distinct from Import and Project Environment; `project-environment` validates Lore/Git project/file/revision access; `maintenance` contains reconciliation, pruning, reclaim, explicit repair and rewrite, invoking the lower-authority `history` reconstruction machinery.
- **Context Engine:** a separately owned service that manages **all model-facing context selection, token budgets, current-session compaction, cross-chat and cross-REL inclusion, and final input assembly**. Ego produces Identity, Personality, Anchors and summarized Memory-Web material; **Ego does not perform final budgeting or injection**. Context Engine can consume these Ego-owned read contracts without an Ego → Context Engine backreference.
- **Runtime/Gateway:** schedules/coordinates mounted REL/PHY work, session/branch state, authorization, instance execution, handoff and provider calls. It may depend **directly** on Retrieval, other services and narrow owner read contracts; it must never turn lower-level components into ambient Runtime clients.
- **Public consumers:** ordinary facades, Warlock/Hermes integrations and CLI use Runtime/Gateway for normal live work, with explicitly supported offline Maintenance/History/Import/Project Environment entrypoints. They cannot reach private stores merely because Rust makes the type accessible.

### Mandatory authority and execution boundaries

**History versus Maintenance:** History performs nonmutating reconstruction, cut inspection and shared replay/proof mechanics; Maintenance alone authorizes historical reconciliation, prune/reclaim, repair and rewrite. Historical reads do not depend on Maintenance. Historical comparison proposals remain disposable until an authoritative owner admits publication. No second replay engine or project VCS is created.

**Service-to-owner writes:** source/store entrypoints own validation and mutation. Services may invoke owner publication and compose transactions (for example Insomnia → Memory store, Dream → Graph store), but stores must not call the service back to complete validation/replay. Durable multi-owner or grouped transactions are coordinated above owner stores and checked against exact owner evidence. The Container knows only physical record mechanics.

**Freshness:** Dream/link publications, new Memory admission, accepted external retrieval handoff and imports trigger Freshness through an explicit operation/event. Freshness maintains its own scores and receipts. Ordinary internal retrieval, History reads and context selection must be **non-reinforcing**. Only the authorized accepted external handoff records access reinforcement.

**Vectors and inference:** Vector generation is separately owned from vector storage and Switchboard provider transport. Retrieval requests embeddings through Vectors; Dream/Perception may do the same. Switchboard does not select semantic candidates, write vectors or manage sessions.

**Context and conversation:** Archive retains authoritative conversation/Echo and checkpoint state; Context Engine constructs model-facing projections only. Runtime supplies authorized owner scope and executes the resulting request; Context Engine does not obtain or expand permissions on its own. Ego produces semantic artifacts, and neither Context Engine nor Retrieval may rewrite Ego semantic truth to fit a token budget.

**Project versus external sources:** Project Environment and Lore/Git own project-file/revision access and correlation, not host transcript indexing. External Sources currently handles host-owned conversation provenance/indexing and feeds existing Insomnia semantics. The **Warlock-default source duplication and substantial External Sources redesign are explicitly deferred roadmap work** and are not part of this correction-only implementation. The current target does not pretend that the future transcript authority contract is settled.

**External ports:** Lore/Git repositories are reached solely through Project Environment; model providers through Switchboard; canonical external transcript hosts through External Sources/Gateway integration. External parties are **ports/adapters**, not lower semantic owners and not an excuse to add callbacks into upper runtime modules. Any host-supplied credential, file or authorization capability must be narrowed to a typed input rather than storing an upward Rust dependency.

## Current-source to target crosswalk (migration routing, not a claim of completion)

| Existing source path/area | Logical components to separate | Campaign phase |
| --- | --- | --- |
| `src/compact_delta.rs`, `src/staleness/**`, `src/container/**` | Pure codecs, review clocks, Container | 1 |
| `src/archive/**`, `src/memory/**`, `src/semantic_graph/**` | Archive, Memory, Graph durable owners; extract accidental facade/Cva orchestration | 1–2 |
| `src/perception/**` | Perception service versus Entity/Resolution/Relationship store | 2 |
| `src/freshness/**` | Triggered Freshness policy vs Freshness store; imported history settlement remains owner local | 2–3 |
| `src/retrieval/**` | Retrieval service vs Vectors generation and vector/profile/generation store | 2, 5 for endpoint wiring |
| `src/ego/**` | Durable Ego store, Ego synthesis producer; never Context Engine assembly | 2, 5–6 |
| `src/runtime_inference/**` | Gateway/Instance orchestration; Insomnia + work receipts; Dream + journal; Switchboard/provider routes; vector generation adapter | 2, 5 |
| `src/imports/**` | Import service + import store; any divergent-history reconciliation moves to Maintenance | 2–3 |
| `src/external_source/**` | External Sources integration + durable derived index store | 2, 5 |
| `src/facade/**`, `src/owner_lifecycle.rs`, `src/lib.rs` | Owner-lifecycle, History, Maintenance, and public facade exports; eliminate lower-to-facade imports | 1–3, 6 |
| `src/rewrite/**` | Maintenance-owned historic transformation; use read-only History and owner proofs | 3 |
| `src/project_environment/**` | Project Environment only; Lore/Git own project VCS history | 4 |
| `src/context_engine/**`, `src/context_engine.rs` | Context Engine: current compaction now; future full context assembly, not in this campaign | 5 |
| `src/chronos/**`, `src/config_security/**` | Chronos, provider-neutral execution seams and configuration/secret owners | 5 |
| Consumer `cli/**`, `tests/**`, host adapters | Public/host interfaces; no domain owner residence | 6–7 |

Any imported type currently housed in `lib.rs` or a Cva inherent `impl` must be assigned by **semantic ownership**, not by its current source file. This crosswalk is intentionally many-to-many because the present 19 Pitlord areas do not represent the eventual 34 logical components. Do not blindly rewrite the 19-area baseline as if there were already 34 physical source areas.

### Phase 1 physical source mapping

| Source | Target owner | Permitted lower contracts |
| --- | --- | --- |
| `src/container/container.rs`, `container_scan.rs`, `container_lease.rs` | `container` | Physical identity/framing, standard filesystem/OS APIs, local clock and envelope values |
| `src/container/container_version.rs`, `publication_envelope.rs`, `grouped_clock.rs` | `container` physical clock/encoding seams | Pure scalar parsing, SHA-256 and physical errors; no semantic owner decoders |
| `src/facade/{cva,phylactery}_lifecycle.rs` | Higher owner replay composition (retained until P2/P6) | Pull Container frames and feed concrete owner states |
| `src/facade/semantic_maintenance.rs` | Explicit owner maintenance composition | Validate owner prefix before authorizing physical repair |

The `container-no-upward-dependency` source rule and physical control-flow guard
apply now to the physical rows. They do not claim the remaining 34-component
mapping is complete. Phase 0's original capture remains retained.

## Phase closure and enforcement

| Phase | DAG-specific exit |
| --- | --- |
| **0 — Evidence and target** | Keep original graph/baseline unchanged. Adopt and validate this complete logical component allowlist; review/classify actual source-level findings, separate real/test-only/analyzer artifacts, and map them to removal/relocation steps. Phase 0's 19-area capture, reviewed debt ledger and source gate have passed locally. Source-to-component mapping and the final 34-component enforcement ratchet remain work for migration and Phase 7 closure. |
| **1 — Physical/store foundations** | Container and pure primitives have no upper imports; owner-lifecycle/store open and explicit repair route through narrow physical contracts. |
| **2 — Owner-local separation** | No owner store imports another semantic service, Runtime, facade or foreign owner implementation; isolate vectors, Freshness, Dream/Insomnia work and Ego/Perception state. |
| **3 — History/Maintenance/Import** | Historical replay is read-only; reconciliation/prune/rewrite is Maintenance-owned; import settlement depends downward on owner evidence with no cross-owner persistence loops. |
| **4 — Project Environment** | Lore/Git remain project-VCS authority; no path/scope bypass or duplicate file ancestry. |
| **5 — Services/Runtime/Context** | Independent operational services, provider endpoint seams and full logical Runtime downward direction are enforced. **Do not implement deferred Ego synthesis, broad Context assembly or External Sources redesign** to satisfy a graph edge. |
| **6 — Facade and contracts** | Delete obsolete upward reexports and RuntimeHost/facade dependencies; public surfaces consume supported component APIs. |
| **7 — Convergence closure** | Rebuild source-derived graph, verify all source edges satisfy the new logical allowlist and no production SCC/upward/unlisted edges remain; retire transitional baseline/debt/suppressions and retain permanent enforcement. |

**Machine checks today:** `python scripts/check_component_dag.py --self-test` validates target acyclicity, strict dependency order, explicit edge mode/reason, unique identities, store independence and History read-only behavior. `python scripts/check_dependency_convergence.py --check-target` runs the integrated fast check. The usual `scripts/check_architecture.py` continues to run the existing Pitlord source analysis and Phase 0 debt gate.

**Enforcement limitation:** the new JSON is the authoritative **logical target**, not yet a Rust symbol/path-level filter. New source dependency edges are currently guarded by the existing 19-area ratchet and the **reviewed** Phase 0 debt ledger: 4,609 records, of which 790 are source-confirmed, 3,683 test-only and 136 analyzer artifacts. Mapping every source module to its target component and tightening Pitlord against the *component* allowlist belongs to the migration phases. A passing logical-target validator **does not mean that the Rust graph is clean, that the new component rules already govern source code, or that the completed Phase 0 19-area source gate enforces the new fine-grained component rules**.

## Acceptance examples

| Edge | Outcome | Why |
| --- | --- | --- |
| `runtime-gateway → retrieval` | Allowed | Direct interactive search without forced intermediate layer |
| `context-engine → retrieval` | Allowed | Context assembly requests supporting evidence independently |
| `retrieval → memory-store`, `retrieval → archive-store` | Allowed | Owner-local read contracts |
| `context-engine → ego` | Allowed | Consume Ego-produced read-only outputs |
| `ego → context-engine` | **Forbidden** | Upward inversion even if no cycle currently exists |
| `memory-store → retrieval`, `container → owner-lifecycle` | **Forbidden** | Ancestral backreach irrespective of SCC presence |
| `dream → insomnia` | **Forbidden by default** | Unlisted same-level dependency; Runtime/Import coordinate their lifecycles |
| `history → maintenance` | **Forbidden** | Historical inspection must not gain mutation/reconciliation authority |
| `retrieval → freshness` | Allowed for passive reads | Does **not** authorize accepted-use reinforcement on internal lookup |
| `external-sources → project-environment` | **Forbidden by default** | Unrelated host transcript versus project VCS authority |

## Change procedure

For a new service or edge, first decide whether it belongs within an existing deep component; then name its owner, exact capability, direct edges and accepted rank, prove it does not add a cycle or backreach, and review the migration/verification cost. Update this graph, the implementation crosswalk and its Pitlord enforcement together. **An edge that is technically acyclic but violates direction or the allowlist must fail**; a service cannot import a high-level facade merely to obtain a convenient DTO or error type.

This is an architecture **hard target**. It authorizes refactoring seams and deleting redundant code, **not** new roadmap capabilities or currently unimplemented semantics.

## Purpose

Define the accepted logical dependency destination for the convergence campaign.

## Overview

The preceding contract defines scope, ownership and verification requirements.

## Related docs

[Architecture](architecture.md), [Phase 1 specification](dependency-convergence-phase1-storage-foundation-spec-2026-10-21.md).

## Notes

The V2 encoding remains unchanged. Current behavior and unfinished acceptance are recorded separately.
