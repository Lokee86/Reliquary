# ADR 0034: Cross-owner relational Observations and active-PHY privacy boundary

Parent index: [Architectural decisions](INDEX.md)

Implementation planning: [Perception subsystem plan](../perception-subsystem-plan.md) and [Roadmap](../roadmap.md)

## Status

Accepted — 2026-09-08; semantic model amended 2026-09-25; relational candidate/synthesis constraints and shared Exploitation/Exploration discovery model amended 2026-09-26.

The owner-local Relationship persistence foundation implemented on 2026-09-23 remains valid storage work, but **Relationship is no longer a sibling semantic object beside Observation**. Relationship semantics are now defined as a specialized **relational Observation**: an Observation whose proposition describes a durable connection among two or more resolved Entities.

The existing `RelationshipStore`, `RelationshipId`, participant indexes, evidence references, migration/reconciliation support, and active-PHY privacy work are therefore treated as a provisional structured persistence/indexing foundation to be converged with the Observation owner before semantic synthesis is implemented. No new independent Relationship lifecycle or synthesis authority should be built on top of it.

This ADR continues to amend ADR 0029 by reactivating relationship-specific semantic state without reintroducing Connection REL classes, and it amends ADR 0033 by defining the relational specialization of Observation plus its cross-owner reference/privacy rules.

## Context

ADR 0021 explored a dedicated Connection Reliquary whose durable state belonged to a relationship. ADR 0029 correctly rejected Connection as a behavioral REL class and made current RELs homogeneous, while leaving the underlying relational-knowledge problem unresolved.

That problem is real, but a separate third Perception semantic category is unnecessary.

These propositions have the same fundamental semantic shape:

```text
Postgres handles all data through the Rails API.
Bob is Sarah's father-in-law.
Sarah has hiring authority at the Vancouver office.
```

Each is a proposition inferred or retained from evidence. The latter two happen to describe relationships among durable Entities and benefit from structured participant/role metadata, but that does not make them a different kind of knowledge from other Observations.

The required abstraction is therefore:

```text
Observation
├── ordinary higher-order proposition
└── relational Observation
    ├── participant EntityRefs[]
    ├── optional participant roles
    └── optional relation classification
```

A relational Observation can still require specialized routing, indexing, privacy composition, and structured fields. Those are representation and processing specializations, not separate semantic authority.

## Decision

### Relationship is a specialized Observation

A **Relationship** is the structured relational form of an Observation whose proposition concerns a durable connection among two or more Entities.

Conceptually:

```text
Observation {
    id
    owner
    proposition
    support / derivation
    lifecycle / chronology
    receptors / ambiguity
    ...
}

RelationalObservationProfile {
    participants: EntityRef[]
    participant_roles[]
    classification
}
```

"Relationship" remains acceptable shorthand for this relational profile in APIs, indexes, migration code, and user-facing presentation where useful. It does **not** denote an independent semantic object family with its own truth, provenance, reconsideration, or lifecycle model.

Relational Observations use the same Observation semantics for:

- proposition authority;
- exact support/derivation lineage;
- contradiction, qualification, ambiguity, and supersession;
- mutation-driven reconsideration;
- Observation lifecycle/currentness policy;
- user authorship/correction;
- Chronos valid-time interpretation; and
- routing receptors where future evidence may materially bear on the proposition.

Relational classes may specialize **whether wall-time staleness is applicable** without creating a separate lifecycle system. Current-state relationships may need wall-time currentness checks, while durable/historical relationship classes may reasonably be immune to staleness-by-time alone. The exact class/policy mapping remains calibration/design work; valid-time closure and semantic staleness must remain distinct concepts.

### Structured participants do not replace the proposition

Entity participants, roles, and classification make relational Observations easier to route and inspect, but they do not constitute the semantic claim by themselves.

For example:

```text
participants = [Bob, Sarah]
classification = family
roles = [father-in-law, child-in-law]
proposition = "Bob is Sarah's father-in-law."
```

The structured profile can support deterministic lookup and privacy composition. The proposition plus support lineage remains the semantic knowledge.

### Relational Observations are sparse and semantically materialized

Entity existence does not imply a relational Observation. Reliquary must not materialize one for every Entity pair or possible participant set, and an Entities-only context can never establish a proposition because Entities are referential/traversal nodes rather than evidence.

Relational synthesis uses the same Exploitation and Exploration discovery modes as ordinary Observation synthesis, but it has a specialized candidate/output contract:

- the propositional anchor must reference at least one durable Entity;
- the selected propositional context must introduce at least one other distinct durable Entity;
- a published relational Observation must carry at least two distinct canonical Entity participants; and
- a relational Observation is not eligible as evidence/anchor for synthesizing another relational Observation.

Memories and ordinary Observations may provide relational evidence. Relational Observations may still contribute to ordinary higher-order Observations and may themselves receive new evidence through contribution/reconsideration.

The two-participant rule is a deterministic shape invariant only. Whether the evidence actually establishes a useful relationship remains semantic judgment.

A cheap Decision/Jev-style relatedness gate may reject clearly unpromising relational candidate contexts before heavier synthesis. Its score is routing metadata only and cannot establish the relational proposition.

This is a specialized candidate-generation and model-call path inside unified Observation processing, not an all-Entity-pairs inference pass and not an independent semantic authority.

### Participants use owner-qualified Entity identity

A relational Observation may refer to Entities that live in different mounted REL/PHY Memory Webs.

Participant references are owner-qualified:

```text
EntityRef {
    owner_id
    entity_id
}
```

The Observation itself is still owned by exactly one REL or PHY. Cross-owner Entity references do not transfer Entity ownership and do not authorize cross-owner Dream Memory edges or cross-owner Community structure.

Where ordinary owner-local Graph topology can represent same-owner Observation/Entity relations, Perception may publish those typed edges under ADR 0036. Cross-owner participant references remain explicit payload/provenance references rather than Arcana edges spanning owners.

### Observation ownership provides semantic disentanglement

The same real-world Entities may participate in distinct relational Observations owned by different scopes.

For example:

```text
Sarah PHY
    "Sarah and Brother are siblings."          private personal Observation

Vancouver Office REL
    "Sarah and Brother work together."         shared organization Observation

Project Phoenix REL
    "Sarah reviews Brother's Phoenix changes." project-specific Observation
```

These are not competing copies of one universal Relationship. They are owner-local propositions backed by the evidence legitimately available to each owner.

The owner answers **which semantic world knows this proposition**. Participant references answer **which Entities the proposition concerns**.

### REL-owned relational Observations are portable shared context

A REL carries relational Observations that legitimately belong to that REL.

If Bob and Sarah both open the Vancouver Office REL, both may access Vancouver-owned relational Observations according to the normal access policy for that REL. The REL must not depend on Sarah's PHY to explain shared business knowledge.

A mounted active PHY may contribute additional private relational Observations at runtime without transferring them into the REL.

### PHY-owned relational Observations are private to the active PHY

Only the currently active PHY may contribute private relational Observations to normal runtime composition. A non-active user's PHY must never be traversed, queried, or used as a bridge merely because it references Entities also visible in an open REL.

The governing rule is:

> **Relational-Observation visibility follows Observation-owner visibility. Entity visibility does not grant visibility into another owner's Observations.**

For PHY state specifically:

> **Exactly one active PHY contributes private relational Observations to normal runtime composition.**

### Effective relational topology is a runtime-composed view

There is no single persisted universal Relationship graph spanning every user's PHY and every REL.

The runtime may derive an **effective relational-Observation view** from currently permitted Observation owners:

```text
EffectiveRelationalObservations =
    ActivePHY.relational_observations
    union
    PermittedActiveRELs.relational_observations
```

`PermittedActiveRELs` follows the normal active-REL/dependency authorization rules. Sibling or inactive RELs do not participate merely because they contain matching Entities.

This composed view does not transfer ownership, merge underlying Memory Webs, canonicalize private state across users, or publish cross-owner Dream/Arcana edges.

### References may cross ownership; visibility never does

A relational Observation owned by one permitted owner may contain owner-qualified Entity references that resolve into another mounted/permitted owner. Such references may remain temporarily unresolved when the referenced owner is not mounted.

Cross-owner references do not imply reciprocal access. Opening or seeing a participant Entity cannot reveal an otherwise inaccessible Observation owner.

### Relational Observations do not become nested Memory Webs

Relational Observations may carry structured participant metadata and compact presentation state, but they do not acquire:

- Archive/source payloads;
- raw Memory ownership;
- an independent Dream graph;
- Leiden Communities;
- nested Relationship graphs; or
- an independent Perception scheduler.

Their evidence remains ordinary owner-qualified semantic evidence and their lifecycle remains Observation lifecycle.

### Perception owns synthesis and maintenance

Perception owns relational Observation synthesis because it depends on resolved Entity identity and higher-order semantic judgment.

Relational candidate discovery shares the same high-level discovery modes as ordinary Observation synthesis:

- **Exploitation** follows strongly justified Entity, source, Web-structure/regional, and derivation routes;
- **Exploration** may use Exploitation overflow, semantic/lexical similarity, cross-region sampling, and random comparison under bounded recent-comparison history.

The Entity bridge is particularly important for relational synthesis because canonical Entity identity supplies participant routing without making the Entity itself evidence.

The actual semantic output is still an Observation proposition with support lineage. Specialized participant extraction/classification may accompany that proposition, but no independent Relationship truth object is created.

Existing-Observation contribution uses the ordinary pairwise `propositional anchor <-> Observation` contract, including relational Observations as targets. Memories and ordinary Observations may support, challenge, qualify, or otherwise mutate a relational Observation and thereby trigger its reconsideration. Relational Observations may contribute to ordinary Observations but are excluded from new relational-Observation synthesis.

## Existing Relationship persistence foundation

The implemented REL/PHY substrate currently provides:

- stable owner-local `RelationshipId` and revisions;
- dense `relationship_version`;
- open classification strings;
- n-ary owner-qualified `EntityRef` participants with optional roles;
- owner-qualified `MemoryRef` evidence;
- bounded summaries;
- reopen/global-version validation;
- migration and divergent reconciliation;
- physical reclamation;
- deterministic query-by-participant; and
- local Entity-merge participant retargeting.

That work remains useful because relational Observations need structured participant indexing, owner-qualified references, and robust persistence/reconciliation.

However, this storage shape predates the semantic unification in this amendment. Before relational synthesis ships, implementation must decide how it converges with the Observation owner. Acceptable implementation shapes include folding these fields into Observation persistence or retaining a tightly coupled relational-profile record keyed by one Observation identity. What is no longer acceptable is treating the existing Relationship record as a separate semantic authority with an independent proposition/lifecycle/support model.

Historical replay may continue to preserve existing Relationship records during the transition. New semantic synthesis must target the unified Observation model.

## Consequences

Perception has one derived propositional family plus durable referential identity:

```text
Entity                         = referential/traversal node
Memory                         = source-grounded propositional node
Observation                    = derived propositional node
    └── relational specialization ("Relationship")
```

This removes duplicated lifecycle, provenance, ambiguity, reconsideration, and temporal semantics between Relationships and Observations while preserving Entity identity as traversal/participant structure rather than proposition authority.

Relational knowledge still gets the structure it needs for participant queries, role/cardinality validation, cross-owner references, portable REL state, and active-PHY privacy.

Ego may later use the effective relational-Observation view as a routing/context signal while composing normal Memory/Observation evidence from authorized owners.

The existing Relationship persistence foundation becomes migration/convergence work rather than justification for a separate semantic layer.

## Rejected alternatives

### Keep Relationship as a sibling Perception semantic object

Rejected. A Relationship is itself a proposition about a connection among Entities. Giving it a separate semantic owner duplicates Observation authority, provenance, lifecycle, reconsideration, ambiguity, and temporal semantics.

### Restore Connection as a special REL class

Rejected. ADR 0029's homogeneous REL decision remains correct. Relational Observations belong to ordinary REL/PHY owners.

### Create one Relationship record per Entity pair

Rejected. Entity identity and relational knowledge are different concerns. A relational Observation is created only when evidence establishes a useful proposition.

### One global persisted relationship graph across all PHYs

Rejected. This would make private user state discoverable through shared Entities and couple unrelated owners' semantic worlds.

### Copy private relational state into a shared Organization REL

Rejected. Shared REL knowledge and private PHY knowledge remain independently owned even when they concern the same Entities.

### Keep relational state as Entity metadata

Rejected. A proposition about two or more Entities is not metadata belonging to any one participant.

### Treat every relationship as an independent nested Memory Web

Rejected. Relational Observations need structured semantics and provenance, not recursive Reliquaries.

## Open implementation questions

- unified Observation persistence schema and stable Observation identity;
- migration/convergence of the implemented `RelationshipStore` and `RelationshipId` foundation into relational Observation representation;
- participant-role and cardinality vocabulary;
- sparse relational candidate-generation/materialization thresholds and per-lane budgets;
- exact support/derivation representation for relational Observations, including ordinary-Observation evidence while excluding relational-Observation-to-relational-Observation synthesis;
- receptor generation and contribution routing for relational Observations;
- relationship-class wall-time-staleness applicability, including durable/historical classes that should not decay merely because time passes;
- active-PHY plus authorized-REL composition APIs;
- authorization policy beyond the owner-visibility baseline; and
- Ego retrieval/context rules over the effective relational-Observation view.

## Verification

Implementation must protect at minimum:

- no semantic Relationship authority independent of Observation;
- no relational Observation creation solely because Entities exist;
- at least two distinct canonical Entity participants for every published relational Observation;
- no relational Observation used as evidence/anchor for synthesizing another relational Observation;
- no all-Entity-pairs inference path;
- both Exploitation and Exploration may derive relational Observations under the same relational gates;
- exact Observation support/derivation lineage for relational propositions;
- durable participant identity through owner-qualified Entity references;
- separate personal, organization, and project relational Observations for the same participants;
- REL portability without a dependency on another user's PHY;
- shared REL Observation visibility for authorized users;
- strict exclusion of non-active PHY private Observations;
- no traversal from a visible Entity into an inaccessible Observation owner;
- runtime composition from active PHY plus permitted active RELs only; and
- no cross-owner Dream Memory edges or Community authority introduced by relational Observation support.

## Related docs

- [ADR 0021 — Typed Reliquary scopes and Connection state](0021-typed-reliquary-scopes-and-connections.md)
- [ADR 0029 — Homogeneous Reliquaries and dependency-based context inheritance](0029-active-rel-hierarchy-and-deferred-connection-scope.md)
- [ADR 0033 — Perception entities, observations, and ambiguity handling](0033-perception-entities-observations-and-ambiguity.md)
- [ADR 0036 — Typed semantic Graph endpoints over Arcana](0036-typed-semantic-graph-endpoints.md)
- [Reliquary and Phylactery scope design record](../reliquary-phylactery-memory-scope-plan.md)
- [Perception subsystem plan](../perception-subsystem-plan.md)
- [Roadmap](../roadmap.md)
