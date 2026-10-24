# Agentic Memory retrieval

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe the implemented agent-facing Memory retrieval controller, its authority boundaries, turn state, decision routing, graph exploration, and accepted-use handoff.

## Overview

Agentic Memory retrieval is a REL-scoped, turn-scoped controller layered over existing Memory, vector, graph, Freshness, and Gateway primitives. It does not replace passive search and it does not give a model direct mutation authority.

The controller exposes three operations:

1. `memory_query` selects from the complete authorized lexical/exact-semantic candidate union.
2. `refine_query` reformulates a declined query while preserving turn state and authorization.
3. `traverse_graph` performs bounded one-hop exploration around selected anchors.

The model-facing controller owns only selection/refinement state. Memory bodies remain owner data, Graph remains relationship authority, Freshness remains relevance authority, and accepted use is recorded only after confirmed content handoff.

## Public composition boundary

Agentic freshness grading gathers the public owner read in `facade/agentic_memory_grading.rs`; controller state and deterministic ordering remain Retrieval-owned. The [Phase 6 evidence](../dependency-convergence-phase6-execution-2026-10-23.md) tracks that source boundary without changing query, authorization or accepted-use behavior.

## Current behavior

Each requesting turn owns one controller state containing the current query, searched Memory IDs, selected Memory IDs, and bounded refinement/traversal bookkeeping. State never floats globally between turns or instances.

Normal query execution:

```text
authenticated REL scope
  -> lexical + exact-semantic candidate collection
  -> preserve complete raw match identity
  -> exclude already selected content
  -> Freshness-order the full remaining union
  -> send at most ten owner-qualified candidates to the decision route
  -> validate selected IDs against the offered set
  -> hydrate only approved content
```

If a query produces no newly approved content but raw results intersect already selected IDs, the controller reports `already_provided`; an empty raw set reports `no_matches`. It does not fabricate a new search result to satisfy the model.

Refinement is bounded and cannot erase authorization, selected IDs, or the original turn identity. Graph traversal is one hop per request, explores incoming and outgoing relationships independently, and applies bounded candidate limits rather than recursively walking the graph.

## Decision routing

The Memory Retrieval decision capability uses the configured runtime decision hierarchy:

```text
models.memory_retrieval_decision
  -> models.general_decision
  -> adapted models.general
  -> unavailable
```

Selection happens during runtime composition. Runtime failure of a selected route does not silently retry through a lower tier. The General adapter validates structured choices and never invents calibrated confidence.

## Freshness boundary

All candidate collection, graph inspection, provenance reads, and Freshness reads are non-reinforcing.

A +25 accepted-use event can occur only after the host confirms successful handoff of newly selected Memory content. The existing `Cva::record_accepted_memory_use` boundary owns the durable event. Search hits, model-visible candidate metadata, declined results, internal hydration, and retries are not accepted use.

The host must bind handoff acknowledgement to the authenticated turn/controller identity so replay cannot double-reinforce.

## Ownership and failure behavior

- Gateway/Instance authorization is checked before owner reads.
- Controller state is request/turn scoped and is never a second durable semantic store.
- Candidate IDs are owner-qualified and revalidated before hydration.
- Model answers outside the offered candidate set are rejected.
- Missing decision capability returns an unavailable outcome rather than falling back to arbitrary general inference.
- Authorization loss invalidates continued traversal or hydration.
- Accepted-use replay is idempotent through the Freshness receipt/event identity.

## Validation

Current source contains focused functional coverage for full-pool ranking, selected-ID exclusion, exact/lexical union behavior, query refinement, graph traversal, authorization, controller isolation, decision schemas, and accepted-use completion. Retained-corpus fidelity evidence remains evaluation evidence rather than a runtime contract.

## Code Map

- `src/runtime_inference/agentic_memory_retrieval*`
- `src/runtime_inference/agentic_memory_frozen_fixture*`
- `src/runtime_inference/runtime_host_gateway_agentic_memory*`
- `src/runtime_inference/general_decision*`
- `src/runtime_inference/configured_runtime_decision*`
- `src/retrieval/**`
- `src/freshness/**`

## Related docs

- [Architecture](../architecture.md#freshness-retrieval-and-consumption)
- [Rust API](../api.md)
- [Configuration](../configuration.md)
- [Freshness](freshness.md)
- [Behavioral contracts](../behavioral-contracts.md)
- [Agentic retrieval verification](../agentic-memory-retrieval-verification-2026-10-16.md)
- [Host handoff evidence](../agentic-memory-host-handoff-seam-2026-10-16.md)

## Notes

The earlier implementation and linear execution plans are retired. Git history and the retained verification documents preserve implementation chronology.
