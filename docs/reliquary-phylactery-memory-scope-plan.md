# Reliquary and Phylactery scope design record

Parent index: [Documentation index](INDEX.md)

## Purpose

This document retains the durable-scope design rationale for Reliquary and Phylactery: ownership boundaries, typed file identity, authority separation, and context-composition constraints. It is not the future implementation owner. Current behavior belongs to architecture/reference docs; future scope work belongs to [Roadmap](roadmap.md).

## Overview

Durable state is partitioned by semantic owner rather than stored in one global memory pool. User-global state belongs to Phylactery; non-user state belongs to typed Reliquary scopes. The active composition problem is hierarchy among multiple RELs—especially Organization ownership of Projects—without project cross-contamination. Ownership, retrieval visibility, and mutation authority remain separate concerns.

## Status

Design record. **Reliquary** and **Phylactery** are accepted names under [ADR 0018](decisions/0018-reliquary-and-phylactery-naming.md). [ADR 0020](decisions/0020-reliquary-and-phylactery-file-kinds.md) establishes the `.rel`/`.phy` product file split, and [ADR 0021](decisions/0021-typed-reliquary-scopes-and-connections.md) established Organization/Project/Connection typed Reliquary identity.

ADR 0029 replaced behavioral Organization/Project/Connection REL classes with homogeneous RELs and explicit dependency-based context inheritance. ADR 0034 subsequently reactivated relationship-specific semantics as relational Observations over structured owner-qualified Entity references. Legacy typed Connection file identity remains compatibility-only; relational Observations are not separate REL classes, a third semantic object family, or independent Memory Webs.

## Problem

A single omni-memory pool creates both retrieval pollution and state-ownership ambiguity. Durable state does not all belong to the same thing.

The retained typed owner identities and active ownership boundaries are:

- **User / Phylactery** — user-global Identity state that persists across work;
- **Organization / Reliquary** — company/organization state that persists across projects;
- **Project / Reliquary** — bounded work state specific to one project;
- **Relational Observations** — ADR 0034's specialized Observations over structured owner-qualified Entity participants. Each Observation is owned by a REL or PHY, may reference Entities across mounted owner Memory Webs, and does not create a separate `.rel`/`.phy` owner.

These are ownership boundaries first. Retrieval and context composition are separate concerns.

A company policy does not become Project state because it was used while working on a project. A project delivery delay does not become Organization state because company personnel observed it. Relational Observations follow the same ownership rule: organization-visible supplier/coworker propositions belong to the relevant REL, while user-private personal relational propositions belong to that user's PHY.

## Product file direction

### Phylactery

User-global state remains **Phylactery `.phy`**, a distinct semantic file kind rather than a Reliquary with fields removed.

Its implemented core is durable user Memory plus same-file Graph state, Packed Vectors, Memory Vectors, and Compatibility Profiles. Archive/history, Episodes, Files/attachments, Insomnia state, Archive Vectors, Vector Generations, interaction-stream checkpoints, and the current Archive-specific lexical index are not Phylactery owners.

A Phylactery Memory does not require project source turns or a live pointer to an originating Reliquary. Current direct publication requires the existing REL-local Episode/node/conversation provenance fields to be absent. Future source export may retain permitted provenance only through an explicit cross-file lineage/snapshot representation; source absence must remain valid.

### Reliquary family

Non-user durable scopes use the **Reliquary `.rel` family** over shared storage/container mechanics.

New human-facing files should use a typed double extension:

```text
<name>.<scope-type>.rel
```

Current accepted type abbreviations:

- Organization: `.org.rel`
- Project: `.prj.rel`
- Connection: `.con.rel`

Examples:

```text
acme.org.rel
tower-a.prj.rel
willyswidgets.con.rel
```

The final `.rel` identifies the Reliquary family. The penultimate extension is a human-visible scope hint. The internal file-kind/scope-type discriminator is authoritative; renaming a file must not change its semantic type.

Hierarchy is not encoded in filenames.

No Person Reliquary scope is currently established. It must not be added without an independent durable state-ownership requirement.

### Shared mechanics, distinct semantics

Reliquary and Phylactery should share low-level framing, checksums, immutable object storage, versioning, vector/graph primitives, recovery, compaction, and migration machinery where those mechanics are genuinely common.

Their semantic validation and permitted owners remain distinct.

Within Reliquary, Organization, Project, and Connection identities may share physical mechanics while retaining scope-specific validity. Active context-composition work currently targets Organization and Project; Connection semantics beyond the existing typed identity are deferred.

Existing `.cva` files remain legacy Project Reliquary data for migration purposes; new Reliquary creation uses typed `.rel` headers without removing any of the current storage owners.

## Ownership versus authority

Scope ownership does not imply that every kind of state may be inferred or mutated automatically.

This distinction is especially important for Organization scope.

An Organization may own both:

- learned operational state/history; and
- explicitly governed policies, permissions, procedures, compliance rules, and institutional instructions.

ADR 0021 applied the same authority distinction to its provisional Connection design. That remains a valid constraint if Connection semantics are ever reactivated, but it is not an active publication model under ADR 0029.

Insomnia/Dream must never convert repeated observed behaviour directly into authoritative policy, permission, contract terms, or governed instructions merely because the observation belongs to an active scope.

The eventual schema must therefore preserve at least the conceptual distinction between **learned/synthesized state** and **explicitly governed state**. Mutation authority is a separate policy from scope ownership.

## Organization scope

Organization owns durable state belonging to the organization independently of individual projects.

In construction this includes, for example:

- day-to-day company operations;
- estimating and bidding practices;
- purchasing and supplier practices;
- scheduling and crew conventions;
- accounting/administrative conventions;
- approval paths;
- safety/security/compliance policy;
- institutional terminology and defaults;
- recurring operational history and learned company knowledge.

Project-specific implementation state remains outside Organization even when the project is owned by that organization.

Organization is therefore a genuine state scope, not merely a persistent instruction addendum.

## Project scope

Project owns durable state specific to one bounded body of work.

Project state includes project source/history, files, decisions, constraints, schedules, deliveries, issues, Memories, provenance, vectors, Graph state, owner-local Dream-derived lifecycle/relationship state, and later Ego-derived project context.

A Project may inherit applicable Organization context without transferring ownership of that inherited state into the Project. Relational-Observation context composes from the active PHY plus authorized active RELs under ADR 0034; it does not transfer Observation ownership into the Project.

## Relational Observation design amendment

ADR 0021 established the original typed Connection identity and explored durable relationship state, participant roles, classifications, and graph edges. ADR 0029 correctly retired Connection as a behavioral REL class. ADR 0034 now reactivates the useful relational semantics without restoring a Connection REL and without creating an independent Relationship semantic object.

The active design treats a Relationship as a specialized Observation whose proposition concerns two or more Entities. Structured owner-qualified participants, roles, and relation classification support routing, inspection, validation, and privacy composition; proposition authority, provenance, lifecycle, ambiguity, Chronos interpretation, and reconsideration remain ordinary Observation semantics. The existing `RelationshipStore` is transitional persistence to be converged with the Observation owner before synthesis ships.

The same participants may therefore support separate personal, organization, and project relational Observations. REL-owned Observations are portable/shared with that REL; PHY-owned Observations are private and only the active PHY contributes them to normal runtime composition. Extensible classification and directional participant roles remain preferred over rigid `VendorConnection`/`ClientConnection` subclasses.

## Scope hierarchy and relational-Observation view

The active near-term topology is a hierarchy among simultaneously open RELs, with Organization able to own or provide inherited context to Projects while each Project remains isolated from sibling Project state by default.

Conceptually:

```text
Organization
├── Project A
├── Project B
└── Project C
```

Hierarchy controls default context inheritance; it does not transfer durable ownership. Project A state must not become visible to Project B merely because both share an Organization parent.

REL dependency topology remains separate from ADR 0034 relational-Observation composition. Dependencies answer ambient context inheritance among REL owners; relational Observations answer propositions about Entity participants. The effective relational view is composed at runtime from relational Observations owned by the active PHY plus authorized active RELs. It is not a separately persisted universal Relationship graph and does not become a second REL dependency graph.

## Context composition

Normal context must be composed from applicable scopes rather than by searching every Reliquary.

Conceptually, the active near-term model for work inside a Project may assemble:

```text
current interaction
+ relevant Phylactery/User state
+ relevant Project state
+ applicable Organization state
```

Sibling/inactive Projects do not participate automatically. Relational-Observation participation follows owner visibility instead: authorized active REL Observations may participate, while only the active PHY contributes private Observations.

Context composition must preserve three separate questions:

1. **Ownership:** what scope durably owns this state?
2. **Access:** is the current actor/runtime permitted to inspect it?
3. **Retrieval/composition:** should it participate in this execution context now?

Do not collapse those into a single namespace/filter decision.

## Nested scope discovery and evidence extraction

Reliquary and Phylactery files may physically appear inside directories observed by another Reliquary. They must be recognized as independent scope boundaries rather than recursively ingested as ordinary source artifacts.

The default ingestion behaviour is:

```text
other .rel/.phy encountered
        ↓
recognize scope/container
        ↓
do not ingest its container bytes as ordinary evidence
        ↓
resolve permitted scope identity/edge
        ↓
query through scope interface when evidence/state is needed
```

This is not equivalent to ignoring subordinate scopes. A parent, ancestor, or associated scope may deliberately consume permitted evidence or state from another scope.

The cross-scope operations should remain distinct:

- **discover/reference** — identify the scope and relationship;
- **read/extract** — inspect permitted evidence/state while ownership remains with the source;
- **copy/export** — preserve selected evidence or derived state in the destination scope when policy requires independent durability.

Example for the active Organization/Project direction:

```text
Tower A Project
  └── repeated workflow evidence
          ↓ permitted extraction/publication
Acme Organization
  └── learned operational Memory
      provenance -> permitted Tower A evidence
```

Depending on export policy, the destination may retain only a stable source reference, a bounded evidence snapshot, or a full permitted evidence copy. It must not recursively embed the source `.rel`/`.phy` container.

The invariant is:

> **Scopes may consume permitted evidence from other scopes, but they never acquire another scope merely by containing its file. Cross-scope state movement is semantic extraction/publication, not recursive repository ingestion.**

This keeps physical repositories independent while allowing explicit composition across User and REL boundaries. ADR 0034 adds relational-Observation composition over those owners without recursively ingesting or merging their Memory Webs.

## Interaction/event boundary

Ordinary meetings, messages, conversations, and interactions are primarily durable evidence/history, not automatically independent scopes.

One interaction may eventually feed more than one authorized owner, for example separate Project and Organization propositions. Relational-Observation synthesis is not another Insomnia Memory-publication destination; Perception derives relational propositions after Entity resolution under ADR 0034.

This is preferable to creating an Interaction scope for every event.

A new scope kind should be created only when the thing has durable state and lifecycle that cannot be assigned correctly to surrounding scopes.

## Insomnia scope classification

The first persistence classifier is now implemented as `user | project` under ADR 0023. It is deliberately conservative and remains only the first boundary, not the complete long-term ownership model.

The current classifier/router supports `user | project`. Organization is the next plausible non-user Memory destination only after its learned-state authority and hierarchy rules are explicit. Relational Observation is not an Insomnia Memory-owner destination: ADR 0034 assigns relational synthesis/maintenance to Perception over already-owned semantic evidence.

Organization classification must not imply permission to create governed policies, permissions, contractual terms, or standing instructions. Scope classification answers **where a proposition belongs**; authority/policy answers **whether and how it may be written**.

Atomicity still matters. A mixed interaction may need separate durable propositions when its content belongs to different authorized owners.

## Pass-boundary direction

The implemented `v3-4` extraction/routing sequence is:

`semantic authority/disposition -> fixed groups -> optional metadata classification -> optional ownership classification -> wording -> Entity enrichment -> owner publication`

Lexical Memory routing sits outside that model sequence: REL and PHY derive it deterministically from complete current Memory title/content through the disposable Memory lexical index.

Persistence ownership is a separately testable classification/routing stage. For the current User/Project slice it runs after groups are fixed (and after metadata when configured) but before wording, and may change only the destination owner. It cannot change the durable proposition, authority/provenance, category/type/lifecycle, group membership, candidate identity, or later routing metadata. Post-wording enrichment is owner-neutral: it is bound to the final Memory body and travels with whichever owner receives that Memory.

A later governance/export-policy stage may still be required before learned Organization publication. Observation synthesis, including relational Observations, remains separate from Memory ownership classification and occurs later in Perception. That is separate from the now-resolved placement of User/Project ownership classification.

Any future Organization ownership classifier should be evaluated independently from authority policy: a correct ownership prediction can still result in `do not publish` or `require explicit authorization` for the proposed state type.

## Publication and export boundary

Scope ownership, export permission, and source-export permission are separate decisions.

A User Memory extracted from a Project may classify correctly as Phylactery-owned while project policy forbids exporting it. Likewise, future Organization-owned learned state may be semantically well-placed while confidentiality or authorization policy still blocks publication. The same constraint would apply to Connection only if that scope is reactivated.

The current implementation has no general export-policy engine. Supplying an explicit PHY routing target enables User publication; Project-only paths do not run the ownership classifier. A routed User Memory strips REL-local provenance fields but retains their owner-qualified identifiers in `MemorySourceRef` without copying source records, while also retaining resolved `source_time_ns` semantic chronology; the REL completion records `MemoryRef { owner_id, memory_id }` to the resulting PHY object.

A richer future policy may still distinguish `memory_export = allow | deny` and `source_export = allow | deny`, especially for Organization or confidential Project material. A destination Memory must not require a live cross-file source pointer unless that source dependency is itself an explicit product contract. Connection-specific export policy is deferred with the rest of that scope's semantics.

## Retrieval implication

Retrieval is compositional rather than an omni-memory search.

The near-term resolver should:

- identify the active Project and its explicit Organization ancestry;
- query each permitted active owner independently;
- query Phylactery for relevant user-global state;
- exclude sibling/inactive Projects by default; and
- combine/rank the resulting evidence within a context budget.

This allows Projects and Organizations to grow independently without forcing every Memory into one global candidate set. Relational-Observation retrieval/composition follows ADR 0034: compose authorized active-REL relational Observations with active-PHY relational Observations, never a non-active PHY.

## Validation requirements

Active/near-term scope fixtures should eventually include:

- obvious User-only Memories;
- obvious Project-only Memories;
- obvious Organization-only learned operational Memories;
- mixed interactions that must split across authorized active owners;
- project-specific preferences that must not leak to Phylactery or Organization;
- organization-wide practices observed during one project that should route to Organization only when authority policy permits learned operational state;
- sibling Project cases proving Organization hierarchy does not cause project cross-contamination;
- NDA/confidential cases where ownership is clear but publication/export is forbidden;
- source-export-denied cases where destination Memory can remain valid without source content; and
- inactive/mothballed scope cases verifying unrelated state does not enter ordinary retrieval.

Relational-Observation fixtures must prove sparse materialization, participant-role/cardinality behavior, separate personal/organization/project propositions over the same Entities, REL portability, and strict exclusion of non-active PHY private Observation state.

Evaluate ownership accuracy separately from extraction coverage, semantic metadata, wording quality, authorization, export-policy enforcement, and context-resolution accuracy.

## Open decisions

- whether later Phylactery capabilities justify additional purpose-built owners beyond the implemented Memory/Graph/vector/profile core;
- richer source-export/resolution policy around the implemented identifier-only `MemorySourceRef`;
- exact Organization versus Project learned-state owner sets;
- whether bare `.rel` remains creatable or only supported for compatibility/migration;
- representation and validation of learned versus governed Organization state;
- automatic publication/authorization policy for Organization learned state;
- exact Organization→Project hierarchy representation and context-resolution algorithm;
- source-copy versus lineage representation across active scope boundaries;
- Memory/source export policy representation;
- correction, supersession, deduplication, and retirement across each active owner kind;
- cross-owner context composition belongs to retrieval/Ego rather than Dream; Dream remains strictly owner-local and does not federate REL/PHY candidates or persist cross-file Graph relationships;
- Ego/context assembly budget and conflict resolution across User/Organization/Project context;
- exact convergence/versioning of the transitional ADR 0034 Relationship persistence into relational Observations, owner-qualified Entity references, relational materialization thresholds, and runtime authorization beyond the active-PHY baseline;

## Related docs

- [ADR 0021 — Typed Reliquary scopes and Connection state](decisions/0021-typed-reliquary-scopes-and-connections.md)
- [ADR 0029 — Active REL hierarchy and deferred Connection scope](decisions/0029-active-rel-hierarchy-and-deferred-connection-scope.md)
- [ADR 0034 — Cross-owner relational Observations and active-PHY privacy boundary](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md)
- [ADR 0020 — Reliquary and Phylactery file kinds](decisions/0020-reliquary-and-phylactery-file-kinds.md)
- [Insomnia semantic validation — 2026-08-24](insomnia-semantic-validation-2026-08-24.md)
- [Roadmap](roadmap.md)
- [Architecture](architecture.md)
- [ADR 0012 — deterministic Episodes and Insomnia Memory authority](decisions/0012-deterministic-episodes-and-insomnia-memory-authority.md)
- [ADR 0017 — CVA workspace and Warlock host application](decisions/0017-cva-workspace-and-warlock-host-application.md)

## Notes

This is a retained design record, not the active planning owner. Current homogeneous REL/dependency behavior is governed by ADR 0029; relational-Observation semantics and privacy/composition are governed by ADR 0034; implementation sequencing belongs to the roadmap.
