# Semantic service source and contract crosswalk — 2026-11-03

Parent index: [Documentation index](INDEX.md)

**Status:** Stage 2 source-to-target design evidence; no Rust code moved or compiled. Baseline: `65ac07a6`. This groups current source families; a complete function-by-function implementation map belongs to migration Stage 6.

## Purpose

Identify where the implemented semantic algorithms, store state, application coordination, and worker orchestration belong. Preserve existing code and avoid rebuilding the Root aggregate inside another crate.

## Overview

Compiled evaluators already own substantial logic, but Root still owns evidence hydration, multi-domain publication, Gateway, and historical composition. The destination classification below distinguishes evaluator, storage, workflow, and runtime without assuming a one-to-one Cargo crate split.

## Implemented source families

| Existing source | Implemented behavior | Target responsibility |
| --- | --- | --- |
| `crates/reliquary-insomnia/src/{extraction,ledger,ownership,enrichment}*.rs` | `InsomniaExtractor`, evidence request/response, candidate and mention policy | Keep deep Insomnia evaluator; return typed candidates and requests |
| `src/runtime_inference/insomnia/{extraction,evidence,product_adapter}.rs` | Authenticated source Episode and requested evidence hydration | Insomnia application workflow, outside evaluator |
| `src/runtime_inference/insomnia/{processor,processor/application}.rs`, `insomnia/completion.rs` | Temporal evaluation, REL/PHY routing, grouped Memory/Freshness completion and codecs | Split evaluator policy, multi-owner workflow, and storage-owned grouped carrier |
| `crates/reliquary-insomnia-store/src/**` | Durable work/attempt/lease/completion state | Domain persistence in REL session |
| `src/runtime_inference/insomnia/{worker,runtime_step}*.rs` | Backlog scheduling, bounded worker lifecycle, owner commit invocation | Gateway worker scheduling and named Insomnia workflow |
| `crates/reliquary-dream/src/**` | Pair evaluator, classifier/verifier, duplicate/canonical/lifecycle rules and Community naming | Keep deep Dream semantic evaluator; owner-supplied facts |
| `src/runtime_inference/dream_processor.rs` | `DreamProcessor` gathers candidates, evaluates, publishes pairs and settles lifecycle | Named Dream workflow; no mutable owner in evaluator |
| `src/runtime_inference/dream_{candidates,temporal,lifecycle,publisher,freshness,cooldown,pair_history}.rs` | Hydration, Chronos adaptation, graph/lifecycle/first-cycle handling | Split pure Dream policy, owner records, Freshness effects and workflow |
| `crates/reliquary-dream-store/src/**` | First-cycle Begin/journal codec and replay proof | Durable Dream evidence owner; no inference on reopen |
| `src/semantic_graph/**` and Community facade modules | Derived graph topology and Community naming | Graph/Community domain and Dream naming evaluator; bound to generation |
| `crates/reliquary-entity-resolution/src/**` | Candidate ranking, `EntityResolutionEngine`, match/admission/reconciliation | Keep focused Entity Resolution evaluator under Perception |
| `src/perception/**` and runtime Perception modules | Body-bound mention evidence, owner admission, re-evaluation and scheduling | Perception application workflow plus runtime scheduling |
| `crates/reliquary-perception-store/src/**` | Entity and transitional Relationship and resolution state | Durable owner-local records; Observation converges via explicit future migration |
| `crates/reliquary-freshness/src/**` | Deterministic score, propagation, F8 policy, historical math, due dispatcher | Keep semantic Freshness computation; dispatcher rebuildable, not durable |
| `crates/reliquary-freshness-store/src/**` | Accepted-use, score, settlement receipts and history | Durable Freshness owner, REL-specific |
| `src/facade/cva_freshness*.rs`, `src/runtime_inference/freshness_access.rs` | Birth/turn/Dream and accepted-handoff integration | Source proof + named workflow; service computes; store commits |
| `crates/reliquary-retrieval/src/**` | Pure hybrid ranking, lexical/semantic/Community and graph candidate rules | Keep Retrieval service, non-mutating |
| `src/retrieval/**` | Live Archive/Memory/vector hydration, ranking and public search | Authorized Retrieval read workflow; no reinforcements |
| `src/runtime_inference/agentic_memory_retrieval*.rs` | Turn-scoped `AgenticMemoryController` query/refine/traverse state | Agentic selection evaluator/controller; no global state |
| `src/runtime_inference/runtime_host_gateway_agentic_memory*.rs` | Auth and host delivery of selected Memory | Gateway execution and separate confirmed accepted-use workflow |
| `crates/reliquary-chronos/src/**`, `src/chronos/**` | Temporal parsing, candidate verification and bounded decision adaptation | Chronos shared semantic primitive, no store |
| `crates/reliquary-vectors/src/**`, `src/runtime_inference/runtime_vector_step.rs` | Embedding endpoint/compatibility/packing, source validation and publication | Vectors pure/endpoint capability plus typed vector commit workflow |
| `crates/reliquary-context-engine/src/**`, `src/echo/**` | Deterministic session compaction and evidence render | Context Engine final assembly; Echo evidence independently durable |
| `crates/reliquary-ego-store/src/**` and `src/facade/*_ego.rs` | Ego identities/personality/anchors and history | Keep durable Ego authority in REL/PHY sessions |
| `crates/reliquary-ego/src/lib.rs` | Semantic service **placeholder only** | Future Ego synthesis, not claimed implemented |
| `crates/reliquary-switchboard/src/**`, `src/runtime_inference/configured_runtime*.rs` | Model routes, transports, credentials and endpoint composition | Switchboard provider infrastructure and runtime adapters |

## Adjacent non-semantic capabilities

| Existing source | Final boundary |
| --- | --- |
| `crates/reliquary-import/src/**`, `src/imports/**` | Source normalization plus exclusive import/epoch workflow, not a new semantic author |
| `crates/reliquary-history/src/**`, `src/facade/historical_*.rs` | Immutable historical owner-proof composition; no inference dependency |
| `crates/reliquary-maintenance/src/**`, `src/rewrite/**`, reconciliation | Exclusive plan/copy/verify/promote workflows; test whether tiny helper crate earns separate interface |
| `crates/reliquary-project-environment/src/**` | Git/Lore external project authority and exact REL revision correlation |
| `src/external_source/**`, external-index store | Host transcript authority, normalized transient hydration and disposable index |

## Behavioral verification crosswalk

| Service | Critical preservation tests |
| --- | --- |
| Insomnia | Bounded evidence, exact source provenance, zero-result completion, user/project routing, malformed model output |
| Dream | Pair evaluation and canonical ordering, zero-link first pass, genuine new-edge Freshness event, no retry/rewire duplication |
| Entity Resolution / Perception | Mention body/span binding, ambiguous rejection, guarded merge, cross-owner privacy, future Observation design |
| Freshness | Deterministic scoring across workers, explicit accepted-use receipt only, bounded F8, no read reinforcement |
| Retrieval / agentic | Frozen weights/candidate limits/ranking, previous selected IDs, turn state, no pre-handoff mutation |
| Chronos | Deterministic-first temporal analysis, unresolved-only decision, source/valid/transaction separation |
| Vectors | Exact profile/probe/pack, source revision check, no endpoint inside owner lock |
| Context Engine / Ego | Accurate token budgets, checkpoint source lineage; future PHY-relative summary without private leakage |
| Switchboard | Route inheritance, unavailable/invalid response, no fabricated confidence or credential exposure |

## Related docs

- [Stage 2 semantic architecture](semantic-service-architecture-2026-11-03.md)
- [Stage 1 owner-session architecture](rel-phy-storage-architecture-2026-11-03.md)
- [Existing-system inventory](current-system-inventory-2026-11-03.md)
- [ADR 0045](decisions/0045-deep-semantic-services-and-effect-contracts.md)

## Notes

This is a **family-level crosswalk**, not an AST of every supporting free function. Stage 6 requires a complete move/keep/merge/delete map and actual compilation/test proof. No future Observation/Ego inference functionality is implemented by this documentation.
