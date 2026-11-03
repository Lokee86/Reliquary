# REL/PHY storage and owner-session architecture — Stage 1 final design (2026-11-03)

Parent index: [Documentation index](INDEX.md)

**Design status: COMPLETE FOR STAGE 1; pending whole-system architectural approval and implementation.** This resolves the storage ownership, session interfaces, publication, historical, recovery and isolation choices. It does not assert that the architecture is implemented or that the proposed Rust types were compiled. Existing functional/performance behavior remains the preservation baseline. The final Cargo package graph, semantic services, Gateway and comprehensive code-migration tickets are later design stages.

**Inputs:** [current implementation inventory](current-system-inventory-2026-11-03.md), [design options](architecture-redesign-foundation-2026-11-03.md), [preliminary owner-session comparison](rel-phy-owner-session-design-2026-11-03.md), [full inherent-method crosswalk](rel-phy-method-ownership-crosswalk-2026-11-03.md), [ADR 0001](decisions/0001-purpose-built-database-ownership.md), [ADR 0034](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md), [ADR 0037](decisions/0037-reliquary-context-memory-project-environment.md), [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md), and [ADR 0043](decisions/0043-rel-terminology-and-wire-identity.md). Decision record: [ADR 0044](decisions/0044-canonical-rel-phy-owner-session-architecture.md).

## Purpose

Fix the storage design and REL/PHY owner-session contracts before any implementation extraction, preserving existing semantics, durability and performance while enabling new capabilities.

## Overview

Stage 1 selects canonical typed per-owner sessions, independently owned durable domains and named workflows above storage. It specifies identity, access, commit, replay, history, repair and recovery without prescribing a premature full Cargo DAG.

## 1. Selected architecture and rationale

**Selected: a canonical, opaque, owner-local storage session composed from independent purpose-built semantic persistence owners (Option B), with selective consolidation of shallow physical modules allowed after interface proof.**

- One in-process mutable `RelSession` per mounted REL identity; one `PhySession` per mounted PHY identity. They have **different persisted authority sets**.
- Each session privately holds the typed physical `Container` lease and only its own durable domain projections and derived indexes. Public code never sees a mutable store bag.
- Each semantic domain owns its durable records, validation, historical facts, mutation identities and local clock. The **session owns file-kind enforcement, ordered replay, cross-domain admission and the physical publication step**, not the domain's business semantics.
- **Named workflows** above sessions do source authorization, evidence hydration, external/model calls, semantic decisions, multi-owner routing and maintenance planning. They use typed read/proof/commit operations; they cannot mutate `Container` or peer domain internals directly.
- Gateway owns attached instances, principal/grants, mounted owner handles and workers. It is neither a durable data owner nor a required dependency of a file opener.
- The public `Rel` and `Phylactery` APIs are thin owning facades over exactly one session instance for offline/standalone use. Gateway may hold the sessions directly behind owner-local execution locks. No duplicate `Cva` state or `Cva` aliases survive the eventual naming cut.

**Alternatives rejected as defaults:** (A) a single general-purpose REL object with 35+ domain methods and unlimited internal access; (C) placing all persistence underneath the semantic services (History/Recovery would then depend on Dream/Insomnia inference and Switchboard). A storage **subsystem** is desirable, but its isolation does not require merging all stores into one monolithic crate. See [options](architecture-redesign-foundation-2026-11-03.md) for tradeoffs.

**Not selected:** a generic `Store` interface/registry, all-owner service locator, universal write-manifest framework, trait per store, mutable `with_stores` callback, one physical lock for all RELs/PHYs, or a semantic super-database. Only add an adapter at a real owner/deployment seam.

## 2. Exact responsibility and call direction

```text
App / CLI / Host
      |
  public Rel / Phylactery API          Gateway + Instance (grants, queues)
      |                                   |
      +------> named application workflows <---- semantic services
                        |
              typed authorized owner operations
                        |
              RelSession / PhySession
                   |     |
             domain records / owner-specific proofs
                   |
               Container (bytes, lease, clock, frame, sync)
```

This is a *logical dependency diagram*, not permission for lower storage to import a workflow's type. A workflow passes owned, validated domain values and revision/identity expectations to a named session operation. The session calls the concrete domain store's admission/codec methods. The workflow receives an exact receipt. Lower storage never imports the workflow or Gateway. Shared data contracts must be placed at the lowest **legitimate** owner, not a generic `common` dumping ground.

**Authority split:**

| Responsibility | Final owner | Others may |
| --- | --- | --- |
| REL/PHY identity, physical frame, writer lease, global physical reservation/time/sync | Container + typed session open | Ask for an owner-qualified immutable snapshot or typed commit |
| Archive source turns, Episodes, branches, file refs, conversation metadata (REL only) | Archive and its associated durable evidence owners | Read permitted facts; workflow submits validated turn command |
| Memory body/revision/lifecycle, owner-local provenance | Memory | Submit typed mutation; History reads immutable revision proof |
| Graph typed relations; derived Communities | Graph / Community derived projection | Submit validated relation effect; read cut-bound snapshots |
| Entity, mention resolution and future Observation evidence | Respective purpose-built persistent semantic owner | Compose authorized evidence through session reads |
| Ego Identity, Personality, Anchors and synthesis records | Ego durable state | Semantic Ego evaluator prepares effects; session commits |
| Packed/archive/memory vectors, profile and generation bindings | Vector/profile domain owner(s) | Embedding service prepares, runtime verifies endpoint, session commits |
| Freshness accepted receipts/events/settlement/score histories (REL) | Freshness durable owner | Service computes effects; workflow proves accepted external use |
| Insomnia work/attempt/receipt and Dream first-cycle proof | Their durable evidence owners | Service evaluates; workflow authorizes grouped publication |
| Import epochs, rewrite lineage, maintenance/retention evidence | Their respective durable owners | Exclusive offline coordinator plans/commits verified operations |
| Source-owned external index and publication receipts | Separate derived-index/accepted-publication owners | Source integration manages authorization, hydration, rebuild |
| Future processing queue, model calls, lease expiry, notifications, due scheduling | Gateway/service runtime (operational and rebuildable), except independently specified durable operational journals | Consult owner facts/receipts, never claim semantic history |
| Project files/revision ancestry | Git/Lore project repository, through Project Environment | REL stores exact correlations, not a copy of VCS authority |

**REL-only data:** Archive, Episodes, conversation compaction/stream evidence, Insomnia work, Archive vectors/vector generations, import epoch, project history/file references, REL activity-driven Freshness. **PHY-shared state:** Memory, Graph/Community, Entity/Relationship transitional evidence, Ego, appropriate packed/memory vectors, profiles, Dream state/processing state and its private user profile. PHY has no REL Archive history or independent REL turn-clock. Existing PHY external-publication receipt handling does not grant it external-host transcript authority. This exact scope distinction is enforced in the **typed open/replay state**, not merely by filename.

## 3. Concrete Rust ownership and interface decisions

These are **interface contracts** with representative Rust shapes; names and exact parameter structs can be refined in the subsequent dependency/migration stages without changing their authority.

```rust
// Internal owner-session component. All fields private to its implementation.
// Struct/method visibility is illustrative: a separate owner-session crate
// requires pub entrypoints, while same-crate placement can restrict to pub(crate).
pub struct RelSession {
    container: Container,
    domains: RelDomains, // concrete typed states; no trait-object registry
}
pub struct PhySession {
    container: Container,
    domains: PhyDomains,
}

// Public wrapper owns exactly one session. It is not a second state aggregate.
pub struct Rel { session: RelSession }
pub struct Phylactery { session: PhySession }

// Existing hot read uses its own Container for body materialization.
impl RelSession {
    pub fn memory(&mut self, id: MemoryId) -> Result<Memory, MemoryError> {
        self.domains.memories.memory(&mut self.container, id)
    }
}
```

The example follows existing compilable borrowing topology in `src/facade/cva.rs::memory` and `src/facade/phylactery.rs::memory`: distinct mutable fields borrow separately, and the result is an **owned** Memory. No mandatory `MemoryView<'_>` or copy of the whole Memory Web. This is a **source-feasibility witness**, not a claim that the proposed code was compiled.

**Permitted session operations (shape):**
- `create_rel / open_rel / open_phy / open_read_only`, kind-checked by physical header and durable owner ID; no generic reinterpretation of a PHY as a REL.
- `identity / head / local_clock / accepted_source_clock` and bounded read queries for Archive/Memory/Graph/Freshness/Ego/vector/owner receipts.
- `read_historical(key, cut)` and immutable owner history/proof snapshots; exact unavailable/terminal states.
- Domain-specific *bounded* `commit_memory`, `commit_graph`, `commit_ego`, `commit_vector_batch`, etc. for already authorized and appropriately validated effects.
- **Named cross-domain owner-local** operations such as `commit_accepted_turn`, `commit_grouped_insomnia_completion`, `commit_accepted_use`, and `commit_initial_dream_settlement`. They validate/recheck current owner state and perform the actual grouped physical admission.
- Restricted exclusive import, transformation, explicit repair and promotion commands with stateful admission fences, not unlimited store access.
- `sync` with clearly specified durability; return exact typed receipt, expected cut and newly accepted/no-op status.

**Forbidden interfaces:** `stores()`, `container_mut()`, `with_any_store_mut`, untyped `commit_bytes` across the application boundary, an arbitrary service callback inside a held owner lock, or a public mutable reference to an entire `Archive`, `MemoryStore` or `Graph`. Read-only borrowed access may be used internally in a domain-specific implementation if it cannot outlive owner lock/epoch and cannot bypass authorization.

**No high-level model/provider objects in sessions.** In particular, move `FreshnessDueDispatcher` and `freshness_notifications` out of `RelSession` into the Freshness/runtime scheduling component. Rebuild these from durable owner facts at reopen; their notification state is not file truth. A parsed record decoder also must not depend on Switchboard, an Insomnia extractor, the Dream classifier, Gateway or an ambient authenticated instance.

**Error model:** keep domain errors at a domain command boundary, then wrap with a typed `RelOpenError`, `PhyOpenError`, `OwnerCommitError`, `OwnerHistoryError` or `OwnerAdmissionError` **only where error scope changes**. Preserve distinctions among corruption, unsupported current format, unavailable historical payload, wrong owner, stale revision/cut, write fenced, receipt conflict and interrupted recovery. No catch-all string `Error` and no unnecessary one-variant wrappers.

## 4. Data access, isolation and transaction semantics

**Owner-local access:** Gateway selects an owner by durable UUID and validates principal/grants before exposing any data. The session additionally binds file kind, expected owner identity, physical revision, owner-specific semantic expectations and operation admission. Permission checks are revalidated after long asynchronous/model phases. Owner-local readers can use a borrowed, short-lived indexed view; a model call gets a bounded owned selection, not an open lock/borrow.

**Physical publication:** Container owns framing, physical global coordinate/reservation, optional transaction time, append and sync. It does **not** understand the semantic domain. Each owner owns mutation identity, semantic versions and persisted/rebuildable history. A physical global version must not become universal Graph/Memory/Freshness ancestry.

**Preflight:** prepare immutable effects outside the owner lock when possible. At commit, obtain that one owner session, revalidate expected durable head, identities, source/revision/route permissions and all participating domain admission. If a preflight fails, append nothing and consume no versions.

**Single-domain append:** call the concrete domain's validated store publication through the session, preserving existing write ordering and exactly one owner-clock change. An identical retry must return the prior result without duplicate durable records; contradictory identity reuse rejects.

**Grouped intra-REL publication:** `InsomniaCompletion` is the canonical example in `src/runtime_inference/insomnia/processor/application.rs`:
1. Insomnia evaluation and source hydration happen outside the durable mutation path. The workflow prepares validated project/user drafts, source receipts, body spans, routing and Freshness birth information.
2. On the destination owner, revalidate Episode evidence, lease/attempt, original source identity, expected Memory births/revisions, routing and Freshness current facts. The **domain stores** own their component validation.
3. A **lower storage-owned grouped carrier codec** encodes the approved Insomnia/Memory/Freshness publication without importing Insomnia's inference service. PHY open may decode the routed Memory portion without acquiring a PHY Insomnia work queue. Existing grouped external-source carrier proof similarly belongs below the host-integration layer.
4. Append the one established carrier and its exact embedded physical versions, sync according to the current receipt contract, and then apply all domain projections. This matches the observed append -> `commit_embedded_version_range_at` -> sync -> apply sequence.
5. A failure after append/sync but before all in-memory projections are updated must **not** allow further writes through a divergent session. Mark the live session unavailable for mutation and recover by ordered reopen/replay; no partial success is acknowledged.
6. Identical retry after replay resolves from retained source/mutation/attempt evidence. Graph/Community and Dream downstream work remains separately scheduled as in current behavior; do not fabricate a larger atomic transaction.

The session owns the **atomic storage write boundary**, not Insomnia's candidate extraction or approval policy. It may privately borrow disjoint concrete fields at commit. This is a narrow, named deep operation, not a generalized database batch framework.

**Memory birth and tombstone:** current `Cva::publish_memory` crosses Memory and Freshness birth receipts; current REL/PHY `tombstone_memory` crosses Memory terminal state and Graph derived exclusion. These become explicit named workflows or named owner-local combined operations. Their state owners remain distinct; retries and replay must preserve existing revision/cut semantics and avoid publishing a stale Community snapshot.

**Cross-file REL → PHY routing:** no cross-file atomic commit is claimed or introduced:
1. An authorized workflow extracts user-targeted Memory candidates after all required inference and source validation without holding either owner lock.
2. Bind destination PHY UUID, source REL/Episode identity and deterministic mutation identity; recheck access/owner incarnation.
3. Commit PHY-owned Memory and its owner-qualified source reference first, using a short owner-local transaction; preserve existing mutation-conflict/retry behavior (see `publish_user_application` in `insomnia/processor/application.rs`).
4. Then commit REL Insomnia completion containing the resulting external `MemoryRef`s and exact existing owner-local evidence; a crash between commits leaves a valid PHY record and incomplete REL completion, not a fake global rollback.
5. On retry, verify the exact previously published PHY semantics by deterministic identity and reconcile/finish REL completion. Conflict or missing permission fails closed.
6. Hold no simultaneous long-lived REL/PHY locks, no provider call across any owner mutex, and do not leak PHY content through REL-wide subscription events.

**Accepted-use Freshness:** retrieval candidates or internal hydration alone never reinforce. Only the trusted host's confirmed handoff/consumption evidence permits the `record_accepted_memory_use` workflow. It validates receipt/owner/turn/identity, computes effects without holding broad locks, commits one idempotent REL Freshness receipt, and then updates derived due scheduling. The session cannot infer external use from a read.

## 5. Open, recovery, history and maintenance

**Session open sequence:**
1. Verify current 2.0 typed header, kind, durable owner UUID, writer/read mode and lease. A PHY rejects REL-only records and unsupported source provenance.
2. Build independent temporary replay projections for the domain owners. Read accepted physical frames in exact stream order through Container's replay cursor; classify to the correct domain codec. Nested physical publication envelopes and grouped carriers must be validated at their designated lower storage contract.
3. Enforce unique accepted physical publication coordinates, owner-local semantic version/revision identities, retained history, cross-domain references and grouped-source proofs. No inference/model calls, ambient Gateway state or automatic destructive tail truncation during open.
4. Complete each domain's `finish`/validation and compare cross-domain closure, retained Dream/Insomnia/Import/Freshness readiness evidence, vector/profile backing and owner-specific topology. Invalid accepted complete frames fail closed. Unavailable historical payload is returned as explicit unavailability, not reconstructed into fabricated truth.
5. Only after all validations succeed publish the assembled session as write-admissible. Rebuild derived indexes, due schedules and runtime workers **above** or alongside session creation without turning them into durable authority.
6. Interrupted write or a replay/validation error yields a failed/unavailable session, not a partially live owner. Tail repair is a separate explicit authorized operation under a lease and bounded repair plan.

**Historical reads:** accepted Archive turn activity, physical global publication cut, Memory/Graph/Entity local revision and transaction/valid time are independent axes. A query explicitly chooses its supported cut. It may return current, retained terminal, payload unavailable, invalid cut or corruption; no invented unified history clock. Cross-domain history composition reads cut-bound immutable owner proofs, then validates available reference closure. No Dream/Insomnia inference or provider dependency in history. Existing `historical_memory_revision` calls `reliquary_history::evaluate` over owner facts; that already demonstrates the correct direction.

**Exclusive import:** ImportStart/Complete, source receipt, explicit write fencing, readiness and Freshness settlement remain durably owner-authenticated. The Import workflow must be allowed to plan and verify outside the session lock, then use a narrow exclusive commit/admission command. An open epoch disallows ordinary writes; completion requires exact evidence/settlement and synchronization. Routed PHY effects remain explicitly unsupported where existing import cannot authenticate the other owner.

**Rewrite/prune/repair/reconcile:** run as explicit owner operations on exclusive/offline owner snapshots and **transformed copies**, with domain-specific eligibility/closure proofs. Preserve owner identity/history generation, retained tombstone/provenance/source/offset semantics and publication form. Verify copy via current-only normal open before promotion. Do not silently mutate the mounted original or allow a low storage opener to import higher maintenance strategy.

**Current-format prerequisite:** the six-file promoted current corpus still contains a previously reported REL Freshness `CVAFRS07` reopen failure in this branch (three PHY open, three REL fail); `tests/current_format_boundary.rs` has an ignored whole-corpus gate. Re-establish the canonical current semantic implementation and verify on copies independently. The later **CVA → REL wire/header/identity migration** belongs to the separate naming/format stage, not to this owner-session architecture. Do not add retired readers.

## 6. Source-feasibility proofs (existing code, not new Rust builds)

| Boundary being designed | Concrete current source witness | Feasibility result |
| --- | --- | --- |
| Private owner collection and typed kind | `src/facade/cva.rs` (35 fields), `phylactery.rs` (19), `cva_lifecycle.rs`, `phylactery_lifecycle.rs` | Exactly one concrete session can enclose the existing fields; two typed domain sets already exist. Must eliminate `pub(crate)` exposure, not clone the stores. |
| Hot Memory read and borrow separation | `Cva::memory` and `Phylactery::memory`: `self.memories.memory(&mut self.container, id)` | Existing borrowed-field pattern returns owned `Memory` and requires neither a global lock nor a `dyn Store`; direct source proof. |
| Bounded data/semantic separation | `src/retrieval/memory_search.rs`; `src/runtime_inference/dream_candidates.rs`; `insomnia/processor.rs` | Inputs can be owner-qualified facts to semantic algorithms; proof of every call conversion is an implementation gate, not asserted here. |
| Grouped post-sync publication | `src/runtime_inference/insomnia/processor/application.rs`, especially appended completion, version-range commit, sync then Memory/Insomnia/Freshness projection | One named internal admission operation can borrow exact disjoint session fields. Codec must move below inference and replay must reproduce grouping; not yet compiled in new layout. |
| Cross-file authority | `src/runtime_inference/insomnia/processor.rs::process_claimed_insomnia_episode_routed` + `processor/application.rs::publish_user_application` | Current code performs PHY publication before REL completion and verifies deterministic conflicts; preserves multi-owner retry without global atomicity. |
| Historical cut/query | `src/facade/cva.rs::historical_memory_revision`, `cva_historical_freshness.rs::historical_freshness_timeline`, `historical_manifest.rs` | Existing historical core consumes immutable owner facts; can move above lower stores without inference. |
| Typed replay and admission | `crates/reliquary-owner-lifecycle/src/lib.rs::replay_open`, `ingest_rel_core_frame`; `src/facade/{cva,phylactery}_lifecycle.rs` | Ordered replay exists; missing part is **canonical owner-composition ownership**, not a generic database framework. |
| Runtime lock and activity | `src/runtime_inference/interaction_runtime.rs::accept_turn`; `runtime_host_rel_execution.rs` | Existing owner-local runtime mutex permits short lock for append/sync. Separate due scheduling and model execution must respect this boundary. |

The [method crosswalk](rel-phy-method-ownership-crosswalk-2026-11-03.md) classifies **393 REL-side and 126 PHY-side inherent methods**, including existing API functions currently implemented outside `facade/`. Every inherent method has a target responsibility; no existing method is automatically considered part of a new `RelSession` simply because it currently has `&mut self`.

**Feasibility is source-grounded, not compiler-proven.** The final actual Rust signature, internal module factoring and Cargo topology belong to the dependency architecture and staged migration design. This does not reopen the storage ownership or transaction decisions.

## 7. Extension tests and performance obligations

- **New Observation persistence:** introduce one concrete owner-local type, codec/replay proof, typed commit/read, historical/maintenance rules and a semantic service/workflow. No change to Gateway's internal storage fields or a new universal `Relationship` database. Preserve ADR 0034's relational-Observation design.
- **Ego synthesis:** consume scoped Memory-Web/Entity evidence through authorized owner reads, publish Ego records through a typed commit and let Context Engine assemble model context. Ego never imports the owner aggregate or becomes a second semantic history.
- **New pure service:** needs no new store or physical owner; consumes bounded immutable facts and returns decisions. No phantom crate or global state field.
- **New provider/external transcript:** implements a boundary adapter and can reuse source normalization/authorized hydration; host authority remains outside Archive for reference-only sources.
- **New physical record domain:** requires explicit replay/transaction admission and relevant historical/maintenance treatment at **one owner composition boundary**. It must not force unrelated semantic algorithm changes.
- **Performance:** owner-local locks only, no multi-owner lock during inference, no arbitrary memory/graph clones, high-volume reads use bounded indexed scans or borrow-scoped iteration, cached indexes have exact invalidation, and vector work stays batch-oriented. Collect before/after latency/memory and recovery measurements during implementation, not invented design-time benchmarks.

## 8. Stage-1 completion and follow-on acceptance gates

**Design decisions closed here:** Option B storage shape; typed REL/PHY aggregate ownership; scope-specific domain membership; private mutable fields; session query/command contracts; application-vs-store authority; single/grouped and cross-file publication; idempotency; current-format/repair rules; historical cuts; import/rewrite/reconcile exclusivity; derived scheduling placement; session lock limits; extension strategy.

**Proof still required when code changes (not outstanding Stage-1 design choices):**
- Build and test current-only REL/PHY reopen, including six-file corpus recovery; wrong-kind, complete-frame, incomplete-tail and writer-exclusion cases.
- Verify idempotent grouped Insomnia Memory+Freshness births, embedded global coordinate, retry, invalid source and crash after sync; PHY routing incomplete/duplicate/conflict behavior.
- Retain historical exact cuts, tombstone/payload-unavailable/retained revision, import settlement and transform comparisons; source/VCS provenance.
- Ensure bounded owner locks, no provider work under locks, authorized REL dependency closure, PHY privacy, concurrent instances, stream checkpoint durability/revocation.
- Run `cargo fmt --check`, locked workspace/CLI checks and focused/full tests at integration gates. Compare existing performance baselines where recorded.
- Use source and compiler tests to verify no `pub(crate)` mutable state bag, Root back-edge, duplicated aggregate, generic all-store framework or unfinished method disposition survives.
- Complete CVA naming/wire migration under ADR 0043 at its **separate** proof gate; current corpus incompatibility is tracked independently.
- Validate final physical crates and semantic-workflow DAG as **stages 2–4**; these later stages may refine API names and module package grouping but cannot silently reverse this approved storage authority.

**Out of scope:** introducing Observations/Ego algorithms, changing retrieval scoring, optimizing vector schemes, new provider integrations, new format bytes, actual crate extraction and release-level Gateway/Warlock/Hermes qualification.

**Result:** Stage 1 is architecturally specified; implementation readiness remains conditional on completion of the entire system design and migration plan.

## Related docs

- [Stage 1 architectural decision](decisions/0044-canonical-rel-phy-owner-session-architecture.md)
- [Complete method-ownership crosswalk](rel-phy-method-ownership-crosswalk-2026-11-03.md)
- [Architecture redesign foundation](architecture-redesign-foundation-2026-11-03.md)
- [Current implementation inventory](current-system-inventory-2026-11-03.md)
- [REL naming and wire-identity decision](decisions/0043-rel-terminology-and-wire-identity.md)

## Notes

Only the storage authority and typed owner-session design is complete. This document deliberately does not claim new compiled interfaces, current corpus recovery, final crate layout or overall architecture approval.
