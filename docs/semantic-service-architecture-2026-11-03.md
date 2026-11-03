# Reliquary semantic service architecture — Stage 2 final design (2026-11-03)

Parent index: [Documentation index](INDEX.md)

**Status: COMPLETE FOR STAGE 2 (logical semantic service design).** This document fixes semantic authority, service behavior boundaries, data flow, extension strategy, and permissible dependency direction. Exact Cargo crate ownership and gateway/workflow scheduling remain Stages 3–4. No Rust implementation or unimplemented feature is claimed complete. Decision: [ADR 0045](decisions/0045-deep-semantic-services-and-effect-contracts.md). Detailed method/evidence mapping: [semantic service boundary crosswalk](semantic-service-boundary-crosswalk-2026-11-03.md).

## Purpose

Replace Root's mixed evaluator/owner/runtime responsibilities with **deep, independently understandable semantic capabilities**. Preserve implemented algorithms, performance, provenance and recovery while making future Observation synthesis, Ego cognition, source adapters and other features local to their own domains.

## Overview

- The **semantic service** evaluates bounded immutable facts with deterministic algorithms and, where required, explicit model/decision/embedding endpoints. It produces typed **proposals/effects**, reasons, requested evidence or refusal; it never holds the canonical mutable REL/PHY database or confers mutation authority.
- An **application workflow** above storage and the service authenticates scope, collects/hydrates proof, invokes semantic evaluation without broad owner locks, rechecks expected owner revisions/grants, submits a **named typed session commit**, and interprets authoritative receipts. The workflow is real, not a generic controller registry.
- **Persistent semantic owner** is defined by [Stage 1 storage architecture](rel-phy-storage-architecture-2026-11-03.md). It validates and retains records even if the algorithm that produced them is offline; that is why the service/store division matters for genuinely durable domains.
- An **execution runtime** schedules bounded work and host operations, keeps request/worker state and invokes workflows. It does not own a second domain database. Gateway-specific design is Stage 3.
- **Reusable semantic primitives** (Chronos, embedding compatibility, pure retrieval ranking, model endpoint contracts) are invoked where needed; they are not independent owner mutation pipelines.
- Distinct **source and decision authority** is essential: Insomnia authors source-grounded Memories; Dream authors Memory-to-Memory semantic relationships and canonical/lifecycle decisions; Perception authors Entity identity and future higher-order Observations; Ego authors personal/project-oriented contextual synthesis. Shared underlying Memory/Graph/Entity/Ego stores do not blur these authorities.

## 1. Implemented capabilities versus future functionality

| Capability | Implemented semantic behavior and source | Final responsibility | Must not own |
| --- | --- | --- | --- |
| **Insomnia** | Compiled extractor/authority ledger/synthesis/enrichment in `crates/reliquary-insomnia/src/`; Root evidence, routing and completion | Episode → evidence-validated Memory candidate drafts, provenance, classification User/Project, exact mention enrichment and bounded evidence rounds | Source Archive, PHY store, durable lease scheduler, grouped write codec |
| **Dream** | `reliquary-dream` classification/verification/canonical/lifecycle; Root `dream_processor.rs`, publisher and Community naming | Owner-local Memory pair decisions, duplicate components, lifecycle intents, derived Community naming proposal and first-pass semantic reasoning | Memory bodies, Graph physical writes, first-cycle journal and Freshness scores |
| **Entity Resolution** | `reliquary-entity-resolution`, Root `src/perception/` and resolution workflows | Evidence-based canonical Entity association/create/reject/ambiguity and guarded merge/reconsideration | Independent Entity state, all Perception reasoning, Memory authorship |
| **Perception — Observations** | **Not implemented**; ADR 0033/0034 and `perception-subsystem-plan.md` define direction | Higher-order propositional and relational Observations, bounded exploitation/exploration, lineage, reconsideration, ambiguity and routing receptors | Dream Memory-Web authority, independent Relationship truth store, universal all-pairs inference |
| **Freshness** | `reliquary-freshness`, `freshness-store` and Root event adapter | Deterministic REL-local relevance, decay, propagation, F8 semantic activity policy and historical calculation; derived due scheduling | Truth, deletion, ownership routing, independent PHY activity clock, inferred use |
| **Retrieval** | `reliquary-retrieval` and Root `src/retrieval/` | Passive owner-local search, lexical/semantic/Community candidates, ranking/dedup and prepared authorized results | Content-handoff proof, mutation, Storage transaction, global PHY knowledge pool |
| **Agentic Memory selection** | Root `agentic_memory_retrieval*` + Gateway integration | Turn-local `memory_query`, `refine_query`, `traverse_graph`, model decision, selected-ID/previously-provided bookkeeping | Persisted Memory authority, accepted-use event, cross-turn global agent state |
| **Chronos** | `reliquary-chronos` and bounded Root decision adapter | Source/valid-time detection, deterministic parse, match and inference verification | Transaction time, new persisted temporal database, Observation lifecycle |
| **Vectors/embedding compatibility** | `reliquary-vectors`, Switchboard adapters | Endpoint contract, compatibility probes, batch packing, embedding correctness and validation | Persistent vector/profile generations, Memory authority, Retrieval ranking |
| **Context Engine** | `reliquary-context-engine` current-session compaction/render/budget | **Final model-facing context composition boundary**: deterministic session compaction and bounded assembly of supplied, independently authorized Ego/Memory content | Canonical Archive/host transcript or Ego synthesis itself |
| **Ego synthesis** | **Not implemented** (`reliquary-ego` placeholder); **Ego Store implemented** | Future Identity/Personality/Anchor/context-relevant synthesis, PHY-relative per-REL Memory-Web summaries and source-grounded projection | Memory/Graph truth, REL ownership, current-session compaction, final context assembly |
| **Switchboard/model runtime** | `reliquary-switchboard`, configured Root runtime adapters | Provider/credential/route/endpoint transport and honest decision/general/embedding capability | Semantic prompts, domain decisions, owner state, workflow policy |

**Adjacent nonsemantic capabilities:** Import normalization is a source ingestion adapter and an exclusive import workflow; History is a read-only owner-proof composition capability; Maintenance is transform/rewrite/reconciliation policy plus exclusive workflow; Project Environment owns Git/Lore authority; external source integration owns transient host hydration. None is silently relabeled as a universal semantic inference service to fill a crate. Their complete orchestration design is Stage 3.

## 2. Service boundaries: input → behavior → output

These are **logical interface shapes**, not new compiled Rust APIs. Current type names can be preserved as values after renaming the legacy REL facade. No generic `SemanticInput`, `SemanticEffect` registry or blanket `dyn Service` layer is permitted.

| Evaluator | Supplied facts/endpoints (not a live store) | Returned result | Authoritative next step |
| --- | --- | --- | --- |
| Insomnia | owner-qualified Episode, accepted source turns, bounded authorized evidence, model route, source/reference chronology | staged `EvidenceRequest` or typed extraction with Memory drafts, rejections, exact provenance/mentions and routing metadata | Insomnia workflow verifies source, runs Chronos, commits REL/PHY records and Insomnia completion |
| Dream | exact Memory/body revisions, bounded candidate contexts, graph/canonical/temporal evidence, profiles, classifier/verifier endpoints | pair classification/verification, duplicate/canonical/lifecycle **intent**, Community name candidate | Dream workflow validates cut/receipt, commits Graph/Memory lifecycle, first-cycle journal and Freshness evidence |
| Entity Resolution | body-bound mentions, candidate Entity facts, authorized Memory/Graph context, bounded decision endpoint | reuse/create/reject/unresolved proposal with evidence and reason; guarded reconciliation intent | Perception workflow confirms identity/body/grants and commits Entity/resolution/Graph admission |
| Future Observation | canonical Entity refs, ordinary/relational propositional anchors, bounded Memory/Observation evidence, Chronos products and decision endpoint | supported Observation proposition/participant/receptor/lifecycle proposal or ambiguity/refusal | Dedicated Observation admission validates provenance, owner scope, visibility and history |
| Freshness | REL accepted-activity cut, current stored records/policy, graph adjacency/Community membership, exact admitted event or semantic matching facts | deterministic affected scores/receipts/next-boundary schedule | Freshness workflow authenticates event source/handoff and commits one idempotent durable effect |
| Retrieval | authorized owner-bound index/routing/vector/graph/Memory metadata + query and limits + current Freshness (where required) | ranked candidate IDs, scores and source references; optional bounded hydration requests | Request workflow revalidates access and hydrates only authorized content; no reinforcement |
| Agentic selection | turn-local controller, eligible candidates, offered IDs, decision capability and budget | validated selected IDs/decline/refinement/traversal and staged controller update | Host/Gateway delivers; **separate confirmed accepted-use** workflow may follow |
| Chronos | original text/body identity, authoritative source/reference time, deterministic alternatives, optional decision response | resolved/partially resolved assessment with exact original spans and verification status | Owning service binds any nondeterministic conclusion to a body/version |
| Vectors | source identity/body/version, endpoint/profile, validation mode and bounded batch | validated embeddings/packed batch/profile compatibility evidence or explicit failure | Typed owner commit validates source/revision and publishes vectors/profile/generation |
| Context Engine | authoritative current-session turns/checkpoint, Echo events, explicit token counter/budget and optional already-authorized Ego/Memory projections | deterministic bounded model-facing messages and compaction plan | Host sends context; Archive/host persists checkpoints and transcript as appropriate |
| Future Ego | authorized REL/PHY evidence/cuts, stored Ego state, controller-side provenance, budget, explicit model endpoints | bounded synthesis candidates/summary proposal/content and budgeted Ego projection | Ego workflow validates source and commits Ego records; Context Engine assembles final context |
| Switchboard | configured capability, route inheritance, credentials, provider request contract | endpoint handle/outcome with typed transport/invalid-output/unavailable errors | Calling semantic service validates domain output; no automatic semantic retry on transport fallback |

## 3. Required processing and control flow

**Ingestion:** authorized native Archive acceptance → deterministic Episode materialization → durable Insomnia work → bounded Insomnia extraction and optional evidence request (owner authorization external to extractor) → Chronos temporal interpretation of prepared draft → REL/PHY routed grouped publication → durable Dream eligibility. Insomnia cannot be replaced by direct caller-authored Memory records for ordinary source extraction.

**Memory organization:** Dream obtains an owner-local candidate set, evaluates verified pairs with bounded concurrency and Chronos, determines duplicate/canonical/lifecycle intents, then the owning workflow publishes Graph and other accepted records. Genuine first-cycle linked state generates eligible Freshness evidence; **zero-link first-cycle settlement still permits Memory-Web admission**. Later graph rewiring is not a new +50 event. Communities are derived from accepted Graph/Memory topology, and naming must bind to the exact snapshot/generation; no live model call under a mutable owner lock.

**Entity/Observation follow-on:** Insomnia's exact Entity mentions are bound to source Memory text, but only Entity Resolution resolves canonical identity. After sufficient post-Dream Memory-Web evidence, Perception resolves/reevaluates Entity mentions. Future Observations consume the organized Web and canonical Entity references; ordinary and relational Observations share **one** Observation semantic authority. Sparse relational Observations require more than Entity co-occurrence and never become a second free-floating global relationship graph. Explicit reconsideration triggers and bounded candidate lanes prevent all-to-all comparison.

**Freshness as a side channel, not a pipeline gate:** REL turns drive lazy decay; first accepted Memory birth creates +100 without propagation; actual Dream first-cycle events and confirmed externally accepted-use receipts may propagate; F8 semantic activity has its own local-only effect. Retrieval, Dream evidence inspection, Ego audits and internal processing must never reinforce merely by reading. Freshness neither dictates truth nor gates Perception/Chronos.

**Retrieval and consumption:** ordered lexical + exact semantic candidate union, semantic-lane Community routing, bounded score/ranking and final Freshness ordering, then optional agentic bounded decision state. User/instance authorization is checked before candidate query and rechecked before hydration. The successful host handoff is **the** point at which an accepted-use event becomes eligible. Existing hybrid weight/config and max-result behavior remain unchanged; do not introduce a third default semantic-only path.

**Context and Ego:** Ego is a future synthesis producer; final context assembly belongs to Context Engine over supplied authorized content and current-session compaction state. Per the active [Ego Web design](ego-web-synthesis-plan.md), its future maximum-detail per-source-REL summary is **PHY-relative** and item/provenance based; full-Web audit when it fits and ordered overflow reduction are retained. Do not reintroduce fixed pre-generated three-size summaries or claim the future controller is implemented. Summary auditing/budgeting uses non-reinforcing reads. Active-PHY and REL-dependency authorization are enforced by runtime/workflow composition, not by a global semantic store.

**Temporal/model routing:** Chronos is an invoked shared primitive, not a permanent Chronos worker. Switchboard resolves the configured endpoint capabilities, but prompts, confidence interpretation, bounded evidence and publication criteria belong to the calling semantic service. The General→Decision adapter cannot pretend uncalibrated confidence exists. Unavailable model route is explicit and cannot authorize publishing speculative data.

## 4. Dependency direction and coupling policy

```text
Gateway / public consumers / named application workflows
                 |               |
       prepare + invoke       authorized read/commit
                 |               |
      semantic evaluators    RelSession / PhySession
                 |               |
     pure shared utilities   purpose-built durable domains
       (Chronos, Vectors,            |
        Retrieval ranking)        Container
                 |
     Switchboard endpoint contracts (only when inference is needed)

Workflow also supplies authorized Ego/Memory projections
to Context Engine for final model-facing context assembly.
```

The drawing shows **logical consumption**, not one enforceable Rust import DAG and not chronological arrows. Exact Cargo arrows are Stage 4.

**Permitted direct semantic dependency:** an evaluator may import a justified *pure reusable primitive* (e.g., Insomnia/Dream→Chronos; Dream→vector similarity/compatibility; Entity Resolution→pure Retrieval candidate utility; all inference consumers→Switchboard endpoint traits). It may import immutable domain **value types** from the owner crate where those types are canonical. That does not authorize importing mutable store access, file open/commit routines, or hidden Root adapters.

**No mandatory service-to-service orchestration dependencies:** Dream does not import Insomnia to receive Memory; Perception does not import Dream to consume accepted Graph/Memory facts; Freshness does not import Dream to accept authenticated first-cycle event facts; Retrieval does not import Freshness to *query* the score store; Ego does not import Gateway or Context Engine for assembly. Workflow stages bridge those needs through exact owner-qualified facts and receipts.

**Specific existing Cargo edges for later audit (not automatically defects):** `reliquary-dream` currently depends on Memory Core, Graph, Switchboard, Chronos and Vectors; `reliquary-entity-resolution` on Perception Store, Memory Core, Retrieval and Switchboard; `reliquary-retrieval` on Archive, Graph, Memory Core, Vectors and Vector Store; `reliquary-insomnia` on Archive, Memory Core and Switchboard; `reliquary-freshness` on Freshness Store, Memory Core and Graph. Inspect **specific uses** during Stage 4: immutable types/pure policy dependencies may remain; any service importing concrete mutable stores is inadmissible. Do not invent 15 new shared-type crates merely to remove a healthy value import.

**Depth/locality rule:** group an algorithm and its directly related policy/validation in one deep semantic service. Split a store when durability/history/independent reopen makes the separation real. The existence of a different `*-store` package does not mandate a new service wrapper, nor must a store move into a semantic service because the pair feels fragmented. Service-to-store pairing is resolved per durable authority, not one pattern for every crate.

## 5. Alternatives, failure modes and final selection

| Option | Benefits | Costs/failure | Decision |
| --- | --- | --- | --- |
| One `SemanticEngine` with routes/algorithms/stores | One call site | Becomes another god-object, feature changes touch everything, cannot independently test deep interfaces | **Reject** |
| Every semantic domain owns its store and calls adjacent services | Locality for simple vertical features | Cyclic history/recovery dependencies, inference imported into file open, duplicated model routing and cross-owner locks | **Reject as blanket policy** |
| **Cohesive service evaluators + canonical per-owner persistence + named workflows** | Deep interfaces, explicit authority, extensibility and preserving current algorithms | Requires deliberate typed evidence/commit edges and coordinator migration | **Select** |
| Generic plugin/trait registry and event bus for all features | Apparent extension flexibility | Runtime indirection and event/owner semantics without actual second provider; hides required ordering | **Reject for now** |

Apply the deletion test to each existing service/store pair. **Keep** Insomnia's semantic evaluator and durable work store as separate concerns; keep Dream algorithms vs recoverable first-cycle evidence; keep Freshness calculations vs authoritative event store; keep Ego storage but do **not** create a fake Ego synthesis runtime; keep Entity Resolution focused rather than rename it into all of Perception; keep Chronos/Context Engine/Vector/Embedding capabilities as deep shared modules. **Reassess** tiny History/Maintenance/Import service crates at the final package design: a helper crate with only pass-through selection utilities does not prove independent service authority.

## 6. Growth, performance and verification gates

**Feature-addition exercise A — Observation synthesis:** add semantic evaluator under Perception, bounded evidence discoverer, typed Observation read/commit, owner-local historical proof and reconsideration registration, and one focused workflow; the Gateway only schedules the new work contract, and no change to Insomnia/Dream/Freshness algorithms is necessary. Existing transitional Relationship data must be explicitly converged, never silently invalidated.

**Exercise B — Ego Memory-Web:** add Ego evaluator/controller over authorized REL evidence, publish maximum-detail PHY-relative summaries with proof/watermarks, and extend Context Engine's supplied authorized content inputs without making it call Ego inference. Cross-REL inclusion is a granted read composition, never a storage mutation or global scope switch.

**Exercise C — new domain or model capability:** new pure reasoning over Memory facts needs an evaluator + workflow and no new store if no persisted state; new persisted knowledge type introduces explicit per-owner domain state/codec/history/commit without forcing the old algorithms to import its type. Model route addition extends Switchboard/config only where needed; semantic prompts stay with the relevant evaluator.

**Performance invariants:** bounded candidate counts, bounded endpoint calls with existing retry/failure semantics, no unbounded pair expansion, no model call under owner locks, no full Memory-Web copy simply for a Rust boundary, no per-user global index, no inference on normal file reopen; preserve batching and community/embedding index leverage. Future benchmarks compare the existing same-corpus latency/memory/recovery/test baselines, not fabricated targets.

**What Stage 2 closes:** each implemented and future semantic capability has one stated decision authority, typed inputs/outputs, ownership boundary, service interaction policy and future extension path. Storage and service authority never overlap. The [source boundary crosswalk](semantic-service-boundary-crosswalk-2026-11-03.md) assigns concrete Root families to their final responsibility layer.

**What remains before any implementation:** Stage 3 Gateway/workflow scheduling and operational recovery design; Stage 4 final packages/Cargo DAG plus ownership ratchets; Stage 5 code-and-wire CVA→REL hard-cut design; Stage 6 move-once source migration map, compilation/regression gates and independent `CVAFRS07` corpus recovery. Future inference feature details left unresolved in their active plans are **not** architecture blockers.

## Related docs

- [Stage 1 storage architecture](rel-phy-storage-architecture-2026-11-03.md)
- [Source/current implementation inventory](current-system-inventory-2026-11-03.md)
- [Semantic service boundary crosswalk](semantic-service-boundary-crosswalk-2026-11-03.md)
- [ADR 0045](decisions/0045-deep-semantic-services-and-effect-contracts.md)
- [Insomnia](subsystems/insomnia.md), [Dream](subsystems/dream.md), [Perception](subsystems/perception.md), [Freshness](subsystems/freshness.md), [Agentic Retrieval](subsystems/agentic-memory-retrieval.md)
- [Chronos](subsystems/chronos.md), [Ego](subsystems/ego.md), [Context Engine](subsystems/context-engine.md), [Model runtime](subsystems/model-runtime.md)
- [ADR 0033](decisions/0033-perception-entities-observations-and-ambiguity.md), [ADR 0034](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md)

## Notes

This completes **logical service design only**, not the full product architecture and not Rust implementation. No final crate count, implementation compilation, newly completed Ego/Observation logic, or accepted provider/Warlock release status is claimed. The `CVAFRS07` six-file current-corpus compatibility defect remains a separate acceptance prerequisite.
