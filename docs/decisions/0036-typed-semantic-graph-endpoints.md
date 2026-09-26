# ADR 0036: Typed semantic Graph endpoints over Arcana

Parent index: [Architectural decisions](INDEX.md)

Amends [ADR 0024](0024-owner-local-dream-processing.md), [ADR 0025](0025-owner-local-derived-communities.md), [ADR 0026](0026-community-routed-memory-retrieval.md), [ADR 0033](0033-perception-entities-observations-and-ambiguity.md), and the Graph question retained in [Derived semantic nodes exploration](../derived-semantic-nodes-exploration.md).

## Status

Accepted — 2026-09-17. Typed node identity/catalogue, durable Entity owner, and persisted Memory-to-Entity `EntityAssociation` relations are implemented; resolver/bootstrap integration remains Milestone B work.

## Context

Reliquary's Graph is already an independent semantic owner backed by Arcana's repository-agnostic graph kernel. The implemented wrapper historically restricted every durable Graph endpoint to a same-owner `MemoryId` because Dream was the only semantic producer using it.

ADR 0033 adds Entity and Observation identities to the owner-local typed Graph catalogue:

```text
Memory | Entity | Observation
```

The catalogue is a **topology type system**, not a claim that every node kind is propositional. Memories and Observations are propositional nodes. Entities are referential/traversal nodes: they carry durable identity and connect propositions that concern the same referent, but Entity existence is not itself evidentiary authority.

Keeping separate hidden adjacency structures for Memory-to-Entity and Observation support/derivation topology would duplicate the graph machinery that Arcana already supplies and would prevent one coherent typed topology.

The node kinds retain purpose-built authority and lifecycle rules even though they share graph identity/traversal machinery.

## Decision

Graph becomes the owner-local **semantic Graph substrate** for typed semantic-node topology.

A Graph node is addressed by a stable typed reference:

```text
SemanticNodeRef {
    kind = Memory | Entity | Observation
    id   = stable 32-byte owner-local semantic ID
}
```

The typed semantic identity maps to an internal dense `arcana::NodeId`. Arcana continues to own generic adjacency/topology/traversal mechanics only.

The semantic object payload remains owned by its purpose-built domain:

- Memory records remain owned by MemoryStore;
- Entity records are Perception-owned Entity state;
- Observation records are Perception-owned Observation state.

Graph does not become a generalized semantic object database.

## Semantic authority

Graph owns accepted relationship topology and relationship mutation history.

The producer that is allowed to publish a relation family remains explicit:

- **Dream** owns judgment/publication policy for Memory-to-Memory Dream relationship families.
- **Perception** owns judgment/publication policy for relation families involving Entity or Observation nodes.
- **User-authored** graph changes remain explicit user authority where supported.

Sharing Graph storage does not merge those semantic responsibilities.

## Owner boundary

The semantic Graph remains owner-local to one REL or PHY. A bare `SemanticNodeRef` is meaningful only inside that durable owner.

ADR 0034 relational Observations may carry owner-qualified Entity references across mounted owners, but those cross-owner participant references remain payload/provenance references rather than Arcana edges. Relationship is not a fourth Graph node kind or a semantic object sibling to Observation. This ADR does not create cross-owner Arcana edges or allow one owner's Graph to traverse another owner's private state.

## Communities

Dream/Leiden Communities remain a **Memory structural projection** unless a later measured design explicitly changes that contract.

Entity and Observation hubs must not automatically enter the Community projection merely because they are present in the broader semantic Graph. Community construction filters the authoritative Graph to the Memory relationship families it is designed to organize.

## Persistence compatibility

Existing Graph node mappings remain valid as implicit `SemanticNodeKind::Memory` records.

The typed node catalogue may use a new node-mapping record for non-Memory kinds while preserving legacy Memory node records. Existing Memory-to-Memory mutation/version records remain readable and retain their semantics.

Relation-record generalization is a separate implementation step and must preserve existing Dream Graph history.

## Consequences

- Arcana is reused rather than replaced.
- Memory, Entity, and Observation can share one topology/traversal substrate without implying that Entity nodes are propositional evidence or that the node kinds share lifecycle semantics.
- Entity associations do not require a second hidden adjacency database.
- Observation support/topology can later use the same graph substrate where its semantic contract is truly edge-shaped.
- ADR 0034 relational-Observation participant profiles remain distinct from ordinary Graph edges; cross-owner participant references do not create cross-owner Arcana topology.
- Dream can later consume Entity IDs as a deterministic candidate-routing signal without owning Entity resolution.
- Existing Dream behavior remains valid as the Memory-only projection of the broader semantic Graph.

## Implementation sequence

1. Introduce `SemanticNodeKind` and `SemanticNodeRef`.
2. Generalize the Arcana dense-node catalogue from `MemoryId` to typed semantic-node identity.
3. Preserve legacy Memory node mapping compatibility.
4. Define the minimum durable Entity record and Entity owner.
5. **Implemented:** define and validate the first Memory-to-Entity `EntityAssociation` relation family.
6. **Implemented:** generalize relation persistence, traversal projections, migration, and reconciliation to typed endpoints.
7. **Implemented:** wire zero-candidate Admission and calibrated V4 identity resolution into deterministic Entity creation/reuse/split plus `EntityAssociation` publication.
8. **Implemented:** run the frozen zero-Entity bootstrap experiment and verify complete expected convergence/reopen while preserving unresolved lifecycle machinery.

## Verification

Implementation must protect at minimum:

- legacy Memory Graph reopen and traversal;
- distinct typed identity for identical raw IDs of different node kinds;
- stable typed-node to Arcana `NodeId` mapping;
- no cross-owner Graph traversal;
- existing Dream relation semantics unchanged;
- Memory-only Community projection unchanged until explicitly redesigned;
- no Entity/Observation payload authority moved into Graph; and
- no new unresolved retry/TTL policy implied by Graph generalization.
