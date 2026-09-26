# ADR 0029: Homogeneous Reliquaries and dependency-based context inheritance

Parent index: [Architectural decisions](INDEX.md)

## Status

Accepted — 2026-09-04. Supersedes ADR 0021's behavioral distinction between Organization, Project, and Connection Reliquaries. Legacy typed REL headers remain readable for compatibility, but new RELs are homogeneous. The relationship-specific semantic-state mothballing in this ADR is superseded by ADR 0034; ADR 0034 does not restore Connection as a REL class.

## Context

ADR 0021 introduced Organization, Project, and Connection as distinct Reliquary scope kinds. Subsequent Ego and multi-REL design exposed two problems with that model.

First, real ownership structures are not reliably described by a fixed Organization → Project hierarchy. Large organizations commonly contain programs, products, projects, subprojects, client scopes, repositories, and other nested or cross-cutting structures. A REL may legitimately depend on more than one higher-level REL.

Second, the labels themselves do not need to control storage, repository capability, retrieval, or context inheritance. What Ego needs is an explicit answer to a narrower question: which RELs provide ambient context to this REL? Encoding organizational labels into file identity turns presentation taxonomy into behavior and makes future structures unnecessarily rigid.

## Decision

### All current RELs are homogeneous

A current Reliquary has one file kind and one durable owner identity. New REL owner IDs use the generic `rel-<uuid>` namespace.

Organization, Project, Connection, Program, Product, Subproject, Repository, Client, or any other category is not a Reliquary class. Existing typed REL headers remain readable as legacy compatibility input and preserve their historical owner prefixes while open or reconciled.

### `type` is optional metadata only

Each REL may carry an optional human-facing type label. The value is unrestricted organizational metadata subject only to size/validity limits.

Examples include `Organization`, `Division`, `Program`, `Product`, `Project`, `Subproject`, `Client`, `Research`, or `Repository`.

No Reliquary or Warlock behavior may branch on this label. It may be used for display, filtering, grouping, icons, search, and other organizational UX only.

### REL dependencies define context inheritance

Each REL may declare zero or more directed dependencies by durable REL owner ID.

If REL A depends on REL B, B is an ambient-context dependency of A. Ego may therefore include B's synthesis when assembling context for A. The dependency edge does not transfer ownership: memories remain owned by the REL in which they were published.

Dependencies form a directed graph rather than a fixed tree. Arbitrary depth and multiple dependencies are allowed so real project/program structures can be represented without adding new REL classes.

Context resolution must reject cycles. A deterministic dependency closure is ordered with dependencies before the dependent/primary REL. Sibling RELs are not ambient context unless an explicit dependency path connects them.

### Repository capability is independent of type

Project-file history, Lore/Git correlation, file bindings, and related repository features are REL capabilities established by repository association. They are not privileges of a `Project` type label.

A REL without a repository association remains a valid REL. A REL with a repository association may use repository-backed file behavior regardless of its metadata type.

### PHY remains distinct

The Phylactery remains the separate user-owned durable scope. This ADR removes classes within REL; it does not collapse user and REL ownership into one file kind.

### Relationship/Connection REL semantics remain retired

The earlier idea of a special relationship/Connection **REL owner** is not part of the active model. Old Connection-typed files remain readable as legacy RELs.

ADR 0034 subsequently reactivates relationship-specific semantics as **relational Observations** with structured owner-qualified Entity participants. Those Observations may span mounted REL/PHY Entity identities while remaining owned by one ordinary REL or PHY; they do not reintroduce Connection as a REL class, create a third Perception semantic object family, or give each relationship an independent Memory Web. The previously implemented `RelationshipStore` is transitional persistence to be converged with Observation.

## Consequences

Ego can resolve ambient context from graph topology instead of hard-coded Organization/Project rules.

Large organizations can model projects within projects within programs, shared platform dependencies, cross-cutting initiatives, or other structures without schema changes.

`type` can evolve freely as UI metadata without migration or behavioral compatibility concerns.

Repository features must test actual repository association/capability rather than type labels.

Legacy typed RELs can be opened without forcing immediate destructive migration, while newly created RELs use the generic identity model.

Dependency metadata is durable state owned by the REL. Multi-REL hosts may additionally validate the mounted graph and must fail closed on cycles or unresolved dependencies when computing a complete ambient-context closure.

## Alternatives considered

### Fixed Organization → Project → Subproject classes

Rejected. It merely postpones the taxonomy problem and still cannot naturally represent multiple parents or cross-cutting dependencies.

### Keep typed REL classes but add arbitrary dependency edges

Rejected. Once dependencies carry the actual inheritance semantics, behavioral REL classes become redundant and risk divergent rules between topology and labels.

### Infer dependencies from repository/package graphs

Rejected as authority. Code or package dependencies may suggest a context relationship, but they do not necessarily mean the entire dependent REL should inherit another REL's memory synthesis. REL context dependencies are explicit durable metadata.

### Flatten all mounted RELs into ambient context

Rejected. It creates sibling/project cross-contamination and makes mounting a REL equivalent to granting it ambient influence.

## Related docs

- [ADR 0021 — Typed Reliquary scopes and Connection state](0021-typed-reliquary-scopes-and-connections.md)
- [Reliquary and Phylactery scope design record](../reliquary-phylactery-memory-scope-plan.md)
- [Roadmap](../roadmap.md)
- [ADR 0034 — Cross-owner relational Observations and active-PHY privacy boundary](0034-cross-owner-relationship-graph-and-active-phy-privacy.md)
- [Architecture](../architecture.md)

## Notes

Dependency topology answers context inheritance, not memory ownership, authority, or automatic upward promotion. Those remain separate concerns. A child/project REL does not rewrite a dependency/parent REL merely because it inherited that REL's context.