# ADR 0021: Typed Reliquary scopes and Connection state

Status: Superseded for REL typing and context topology by ADR 0029; retained as historical design record
Date: 2026-08-25
Owners: historical typed-scope design and rationale
Amends: ADR 0020 Reliquary semantics and filename convention
Amended by: none
Superseded by: ADR 0029 Homogeneous Reliquaries and dependency-based context inheritance

## Context

ADR 0020 established Reliquary `.rel` as project/workspace state and Phylactery `.phy` as user-global Identity state. Further design work exposed a broader requirement: a project is not the only non-user thing that owns durable state.

An Organization has durable state that is neither user-global nor project-local. In construction, for example, company-wide operations, practices, policies, recurring business state, suppliers, approvals, and institutional history remain distinct from the details and execution history of any one project.

Connections between parties also have durable state of their own. A client, vendor, partner, or personal relationship can accumulate standing terms, communication norms, shared history, unresolved issues, reliability history, negotiated exceptions, and other state that belongs to the relationship rather than to either participant or to a single project.

Connections are not uniformly hierarchical. A client Connection may sit structurally above projects undertaken for that client, while a vendor Connection may participate laterally in many projects. The scope model therefore cannot be a fixed tree based only on scope type.

Using bare `.rel` for every non-user scope would also make files opaque in ordinary file management. Conversely, creating unrelated top-level formats for Organization, Project, and Connection would falsely imply distinct storage engines.

## Decision

### Scope is a state-ownership concept

A **scope** is a durable state owner with its own identity and lifecycle. Scope ownership is distinct from retrieval, access, and context composition.

A fact should be persisted according to what durable thing it belongs to, not merely according to which active context made it visible.

Examples:

- company operating practice -> Organization scope;
- Tower A delivery state -> Project scope;
- recurring reliability history with a supplier -> Connection scope;
- stable user-global Identity state -> Phylactery.

The same interaction may provide evidence for more than one scope and may therefore produce separate durable propositions for different owners.

### Phylactery remains the User scope

The User scope remains the distinct **Phylactery `.phy`** file kind.

`.phy` is deliberately exceptional. It is the user's root/global Identity state and is not renamed into the Reliquary family merely because both file kinds may share low-level container primitives.

### Reliquary becomes the family for non-user durable scopes

Reliquary `.rel` is no longer defined as project/workspace-only. It is the shared container family for durable non-user scopes that use Reliquary semantics.

The currently accepted scope kinds are:

- **Organization** — `org`
- **Project** — `prj`
- **Connection** — `con`

No **Person** scope is established by this decision. A Person scope must not be introduced merely because a Connection has human participants; it requires an independent state-ownership case and a separate decision.

Additional scope kinds may be added later only when a real durable state owner requires them.

### Typed double-extension filename convention

Human-facing Reliquary filenames should use:

`<human-name>.<scope-type>.rel`

Examples:

```text
acme.org.rel
tower-a.prj.rel
willyswidgets.con.rel
```

The final extension identifies the shared Reliquary family. The penultimate extension advertises the scope kind for ordinary file management, filtering, shell use, backup inspection, and attachments.

The scope kind stored **inside the file is authoritative**. Renaming `acme.org.rel` to `acme.prj.rel` must not change the scope's semantic type. Warlock/Reliquary should eventually warn when the filename hint and internal scope kind disagree.

Bare `.rel` may remain valid where compatibility, migration, or generic tooling requires it, but new user-facing files should prefer a typed filename once the typed scope format is implemented.

Hierarchy is not encoded in filenames. `acme.tower-a.prj.rel` must not be required to express parentage; the scope graph owns relationships between scopes.

## Organization scope

Organization owns durable state belonging to the organization rather than to any one project or user.

Representative state includes:

- company-wide operating practices and procedures;
- policies, approval rules, security/compliance rules, and terminology;
- estimating, purchasing, scheduling, accounting, and administrative conventions;
- recurring operational history and institutional knowledge;
- organization-level supplier/client patterns that are not specific to one project;
- other durable company state required across projects.

Organization state is not limited to manually authored instruction text. However, **authority remains separate from ownership**: an observed pattern must not silently become an authoritative policy, permission, contractual term, or compliance rule. The eventual schema/pipeline must preserve the difference between learned operational state and explicitly governed instructions.

## Project scope

Project owns durable state specific to one bounded body of work.

Representative state includes project decisions, constraints, source history, schedule and delivery state, project-specific files, participants, issues, provenance, Memories, graph state, and other work history that should not automatically contaminate other projects.

A Project commonly receives inherited Organization context but retains ownership of its own project-specific state.

## Connection scope

A **Connection** is durable state about a relationship between parties/scopes. The term replaces `Relationship` in the user-facing scope vocabulary to avoid collision with Reliquary's `.rel` extension and with generic graph/database relationship terminology.

Connection is not merely a graph edge. It is a state owner that may have substantial durable history of its own.

Representative state includes:

- standing communication norms and paths;
- shared or transactional history;
- negotiated operating expectations and exceptions;
- recurring reliability or behavioural patterns;
- unresolved relationship-level issues;
- standing commercial terms where policy permits persistence;
- durable commitments or expectations that span individual projects.

A Connection should carry **classification plus participant roles**, rather than requiring a rigid subclass for every business relationship.

Conceptually:

```text
Connection
├── class
├── participants
├── directional participant roles
├── durable Connection state
└── typed edges to other scopes
```

Candidate broad classes include `commercial`, `professional`, `organizational/internal`, and `personal`. These are provisional and should remain extensible.

Candidate directional roles include `client`, `vendor`, `supplier`, `subcontractor`, `general_contractor`, `consultant`, `partner`, `employee`, and `employer`. A participant may hold multiple roles and roles may change over time.

Role and class are descriptive/contextual metadata. They may select default graph/context policy, but they must not be treated as immutable object subclasses.

## Scope graph

Scopes form a **typed graph/DAG**, not a universal tree.

Edges must distinguish at least the conceptual difference between:

- **structural/contextual edges** — establish containment, ancestry-like context, or default inherited context;
- **associative edges** — establish relevance/participation without unconditional context inheritance.

Examples:

```text
Organization --structural--> Project
Client Connection --structural/contextual--> Project
Project --associative--> Vendor Connection
Project --associative--> Consultant Connection
```

The exact edge vocabulary is not fixed by this ADR.

Whether a Connection is hierarchy-like is therefore a property of its role and graph edges in a particular situation, not a property of the `Connection` scope type itself.

## Context composition

Context resolution should operate over active scopes and typed edges rather than by searching one omni-memory pool or walking every neighboring node.

Conceptually:

```text
active Project
  + applicable Organization ancestors/context
  + applicable structural Connection context
  + task-relevant associated Connections
  + relevant Phylactery/User state
  -> composed execution context
```

Not every connected scope is automatically injected. Edge semantics, authorization, current task/context, and retrieval relevance determine composition.

This preserves the distinction:

- **ownership** — where state durably belongs;
- **access** — who/what is permitted to inspect it;
- **retrieval/composition** — when it should participate in the current execution context.

## Nested Reliquary/Phylactery boundary

Reliquary and Phylactery files may appear physically inside directories observed by another Reliquary. They must not therefore be treated as ordinary source artifacts or recursively ingested as opaque evidence.

The substrate rule is:

> **A scope may discover and consume permitted evidence/state from another scope, but it must not acquire that scope merely because the other scope file is located inside an observed directory.**

When an observed directory contains another `.rel` or `.phy` file, ingestion should:

1. recognize it as an independent durable scope/container;
2. avoid ingesting the container bytes as ordinary evidence;
3. resolve its stable scope identity and graph relationship where permitted; and
4. access its evidence/state only through explicit scope interfaces and authorization/export policy.

Conceptually:

```text
Organization Reliquary watches Acme/
  |
  +-- ordinary files          -> normal evidence ingestion
  +-- tower-a.prj.rel         -> discover/mount Project scope
  +-- vendor.con.rel          -> discover/mount Connection scope
  +-- user.phy                -> discover User scope if authorized
```

Cross-scope evidence use must distinguish at least:

- **discover/reference** — know the other scope exists and how it relates;
- **read/extract** — query permitted evidence or state without taking ownership of the source scope;
- **copy/export** — deliberately preserve selected evidence or derived state in the destination scope when policy requires it.

For example, repeated supplier delivery failures evidenced in several Project Reliquaries may support a Connection- or Organization-owned Memory. The destination scope may retain a reference, a bounded evidence snapshot, or a permitted full copy depending on export/retention policy. The source Reliquary itself is never recursively embedded.

Cross-scope extraction must preserve provenance to the originating scope and evidence identity wherever policy permits. A destination assertion must not silently erase the fact that its evidence originated elsewhere.

This keeps the **physical scope boundary flat** while allowing the **logical scope graph** to remain hierarchical/DAG-like. It also prevents recursive repository containment and duplicate ownership while still allowing higher-level scopes to learn from subordinate work.

## Interaction/event boundary

Ordinary interactions, meetings, messages, and events are not automatically first-class scopes.

They are primarily evidence/history that may update one or more durable owners:

```text
interaction
├──> Project state
├──> Organization state
└──> Connection state
```

A separate scope should exist only when the thing has an independent durable state/lifecycle that cannot be represented correctly as evidence within its surrounding owners.

## Memory/write routing implication

The existing `user | project` Insomnia persistence decision is no longer sufficient as the complete long-term ownership model.

The memory pipeline must eventually distinguish at least:

- User/Phylactery-owned learned state;
- Project-owned learned state;
- Organization-owned learned operational state where automatic synthesis is permitted;
- Connection-owned learned relational state where automatic synthesis is permitted.

This does **not** authorize automatic mutation of governed policy, permissions, contracts, or standing instructions. Ownership classification and mutation authority are separate decisions and should be represented separately.

The scope router should prefer atomic propositions. A single interaction may legitimately yield one Project Memory and one Connection Memory rather than forcing the entire interaction into one destination.

## Consequences

- `.rel` remains one product/container family without making every Reliquary file opaque in the filesystem.
- `.phy` remains a distinct User/Phylactery semantic file kind.
- Organization is elevated from an instruction addendum to a real durable state owner.
- Connection is elevated from a relationship edge/instruction addendum to a real durable state owner.
- Project remains a durable state owner but is no longer synonymous with Reliquary itself.
- Connection classification is modelled through extensible class and directional roles rather than an exploding subclass hierarchy.
- Scope hierarchy becomes graph policy rather than filename structure or fixed type inheritance.
- Reliquary/Phylactery files encountered inside observed directories are recognized as independent scopes rather than recursively ingested as opaque evidence.
- Cross-scope evidence extraction remains possible through explicit read/export interfaces with provenance and policy enforcement.
- Context composition and persistence ownership can evolve independently.

## Open decisions

- exact internal scope-kind field, IDs, and versioning;
- exact accepted filename abbreviations beyond `org`, `prj`, and `con`;
- whether bare `.rel` remains creatable or only readable/migratable;
- exact Organization, Project, and Connection semantic owner sets;
- representation of learned versus explicitly governed state inside Organization and Connection scopes;
- automatic-write authorization rules for Organization and Connection learned state;
- final Connection `class` vocabulary and directional role vocabulary;
- temporal handling for changing participant roles;
- concrete structural versus associative edge types;
- context traversal/inheritance rules for each edge type;
- how scopes shared by multiple organizations/users are authorized and mounted;
- whether any additional durable scope kind is justified by real ownership requirements;
- how legacy project-only `.rel` assumptions migrate into the generalized Reliquary scope model.

## Related docs

- [ADR 0020 — Reliquary and Phylactery file kinds](0020-reliquary-and-phylactery-file-kinds.md)
- [Reliquary and Phylactery scope design record](../subsystems/scope-ownership.md)
- [ADR 0029 — Active REL hierarchy and deferred Connection scope](0029-active-rel-hierarchy-and-deferred-connection-scope.md)
- [Roadmap](../roadmap.md)
