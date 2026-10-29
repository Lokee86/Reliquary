# Dream

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe the implemented Dream ownership boundary, first-cycle Memory processing, relationship and duplicate settlement, Community derivation, and downstream event contracts.

## Overview

Dream organizes owner-local Memories after publication. It evaluates Memory pairs, publishes semantic relationships, resolves duplicate components, settles Memory lifecycle, derives Communities, and emits the first-cycle evidence consumed by Freshness and later semantic layers.

Dream does not own Memory bodies, accepted conversation history, Freshness scores, Entity identity, Observation synthesis, or model-route configuration.

## First-cycle processing

A newly published Memory is eligible for one authoritative first Dream lifecycle pass. The pass may:

- establish zero or more relationships to existing Memories;
- identify a duplicate component and canonical representative;
- promote an active Memory into settled `knowledge` or `canonical` state;
- archive a duplicate according to Dream lifecycle rules; or
- complete with no relationships.

Retries must reconstruct or retain the same first-cycle authority rather than turning already-published edges into new semantic events.

## Relationship authority

Graph stores typed relationship publications; Dream owns the semantic decision to create them. Relationship direction/kind remain semantic facts even when other subsystems derive read-only projections, such as Freshness's undirected relevance adjacency.

Dream never persists a generic numerical relationship strength merely for Freshness.

## Duplicate components

Duplicate recognition is component-oriented, not a parallel identity store. Dream merges component evidence, chooses the active/canonical representative under its lifecycle policy, and may rewire the physical duplicate chain. Rewiring existing component structure is not a new first-cycle event.

Consumers must resolve through Dream's current representative/component authority rather than infer canonical state from raw chain position.

## Communities

Dream derives owner-local Memory Communities from the current graph. Community membership and semantic names are derived state; Graph relationships remain the durable semantic topology.

A stale Community snapshot cannot be treated as exact historical membership. Consumers that need historical or uncertain membership must use their own conservative contract rather than rewriting Dream history.

## Temporal semantics

Dream delegates temporal parsing and matching to Chronos. It retains pairwise semantic relationship authority and classifier context, while Chronos owns shared temporal mechanics.

## Freshness integration

Successful first settlement is the ordinary Memory-Web admission boundary for extracted active Memories. A completed zero-link pass still admits an active Memory for decay.

Actual newly established first-cycle relationships may emit the +50 Freshness linkage event toward existing connected information. Creation alone, retries, later rewiring, and pre-existing links do not create that event.

## Failure and recovery

Dream work is durable enough that crash/reopen cannot silently accept a partial first-cycle outcome as complete. Import completion and other coordinators may treat outstanding import-owned first passes as blockers even if later lifecycle changes make the Memory inactive.

## Durable first-cycle evidence boundary

The Dream Store support crate now contains the `CVDREAM1` journal and
`CVDREAM2` Begin record codecs, canonical population digest, and deterministic
replay projection for passes, roots, intents, births and accepted graph cuts.
It does not choose or authorize the Memory population: the higher owner
reconstructs the exact physical prefix and proves the compact Begin digest
before materializing the Begin for the journal. Freshness validates its
semantic proof and Graph/Community publication authority above the store.
Physical replay continues to establish the prefix strictly before Begin.

This storage boundary does not move pair evaluation, duplicate ranking,
settlement, Graph mutation, Community derivation or Freshness reinforcement
into the store. The compiled crate integration is complete; see the
[Dream Store specification](../dream-store-crate-extraction-spec-2026-10-27.md)
for the boundary and acceptance requirements. Lower, workspace, formatting,
focused Dream, full library and CLI checks pass. See the
[implementation evidence](../dream-store-crate-extraction-evidence-2026-10-27.md).

## Service and owner boundary

`crates/reliquary-dream/` evaluates supplied candidate facts, pair classification and verification, bounded inference, duplicate plans and lifecycle intents. The Root `DreamProcessor` coordinates authorized Memory, vector and Graph reads, publication and first-cycle settlement. The Root Community namer retains snapshot checks, representative hydration and name publication. Dream Store and the Root pair journal continue to own their existing durable evidence; the service does not open either store.

## Code Map

- `crates/reliquary-dream/src/` - semantic policy and inference

- `src/runtime_inference/dream*`
- `src/semantic_graph/**`
- `src/chronos/**`

## Related docs

- [Architecture](../architecture.md)
- [Freshness](freshness.md)
- [Chronos](chronos.md)
- [Dream classifier v2 design/evidence](../dream-classifier-v2-design.md)
- [ADR 0024](../decisions/0024-owner-local-dream-processing.md)
- [ADR 0025](../decisions/0025-owner-local-derived-communities.md)
- [ADR 0030](../decisions/0030-provenance-anchored-dream-maintenance.md)
- [ADR 0031](../decisions/0031-dream-derived-community-semantic-names.md)
- [ADR 0032](../decisions/0032-community-lineage-and-name-continuity.md)

## Notes

The former Dream implementation plan is retired. Git history preserves staged implementation chronology; this document and the ADRs own current behavior.
