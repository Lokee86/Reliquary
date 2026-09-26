# ADR 0037: Reliquary owns the Context/Memory/Project Environment

Parent index: [Architectural decisions](INDEX.md)

Supersedes the Warlock-owned repository/runtime portions of [ADR 0017](0017-cva-workspace-and-warlock-host-application.md), [ADR 0027](0027-warlock-project-repositories-and-reliquary-storage-boundary.md), and [ADR 0028](0028-project-folder-and-repository-bootstrap-contract.md). Amends [ADR 0029](0029-active-rel-hierarchy-and-deferred-connection-scope.md) by assigning mounted REL topology and dependency execution to Reliquary itself.

## Status

Accepted — 2026-09-24. The ownership migration is complete: runtime-host topology, REL reconciliation/recovery, Project Environment mechanics, managed conversation/session lifecycle, cross-owner semantic access, deterministic Context Engine policy, and the corresponding Warlock cleanup are implemented. Remaining work is new semantic capability construction (cross-scope Entity identity, Observation composition including relational Observations, then Ego), not further ownership evacuation.

## Context

Reliquary began as a purpose-built semantic database embedded by Warlock. It now owns Archive history, Memories, Echo, Ego state, Perception Entities, the transitional Relationship persistence substrate, semantic Graph state, vectors, Dream/Insomnia processing, provenance, project-revision correlations, and repository-backed project-file references. ADR 0034 now assigns the eventual relational semantics to Observation rather than to an independent Relationship object family.

Warlock subsequently accumulated the multi-REL runtime topology and the other half of the Project environment: mounted REL/PHY state, active-REL selection, dependency traversal, cross-owner retrieval fan-out, repository discovery and validation, Lore/Git mutation mechanics, repository-backed attachment ingestion, historical project-file reads, reconciliation orchestration, and substantial conversation/context policy.

That split makes Reliquary semantics depend on a particular host application. Ego, cross-scope Entity resolution, relational-Observation composition, provenance, and contextual retrieval all require graph-aware access to neighboring REL owners. Implementing those capabilities in Warlock would make the host application the semantic owner and would prevent Reliquary from functioning as a coherent standalone Rust library.

The repository correlation already persisted by Reliquary is not incidental metadata. A Project REL records exact associations between a REL semantic cut and a Lore/Git revision, and ProjectFileRef records identify exact historical repository content. The code that consumes and maintains those semantics belongs with the environment that owns them.

## Decision

Reliquary is the owner of the complete **Context/Memory/Project Environment**.

The public long-lived Reliquary host becomes graph-aware and owns:

- mounted RELs and the mounted PHY;
- active REL identity and REL dependency topology;
- independent per-owner execution workers, queues, locks, and lazy semantic/index state;
- ambient visibility and deterministic dependency closure;
- parallel cross-owner Memory, Archive, provenance, Entity, Relationship, Ego, and other semantic operations;
- conversation/Archive session semantics and durable interaction state;
- deterministic Context Engine and compaction semantics;
- REL/PHY reconciliation, recovery, and derived-state rebuilding;
- Project repository discovery, identity validation, bootstrap, management policy, Lore/Git revision operations, historical file reads, repository-backed attachment ingestion, and REL/repository correlation; and
- semantic lifecycle rules for all Reliquary-owned background processing.

Warlock remains a host and coordinator. It may provide:

- local REL/PHY file paths and host-local owner-to-project-directory mount associations;
- provider/model endpoint adapters, credentials, cancellation, transport, and provider-specific token/context-limit behavior;
- application process lifecycle and explicit calls to mount, unmount, activate, or shut down Reliquary environments;
- Tauri/UI presentation and native Files presentation; and
- coordination between Reliquary and other independent capabilities such as Lexicon, Arcana, agents, Cantrips, Rituals, ACP, and MCP.

Warlock does not implement REL dependency traversal, contextual visibility, cross-owner semantic fan-out, repository-history semantics, conversation semantics, provenance expansion, or other Reliquary domain behavior merely because it invokes or displays those capabilities.

### Owner-local execution remains independent

A graph-aware host does not imply one serialized Cva execution lane.

Each mounted REL retains its own owner-local execution state and worker machinery. Cross-owner operations may fan out concurrently and compose deterministically. Neighbor REL access must not require serially locking every mounted Cva.

The current single-REL ReliquaryRuntimeHost implementation is therefore first extracted behind an internal owner-execution component. The public graph-aware host then coordinates multiple such components.

### Project repository ownership

Lore or Git continues to own project-file tree/history authority. Reliquary owns the Project environment integration that consumes that authority.

Reliquary therefore owns repository discovery and validation, exact revision identity, repository-management policy, historical reads, attachment snapshots/checkpoints, and correlation with REL semantic cuts. This does not move REL/PHY physical semantic storage into Lore/Git and does not make repository revisions replace REL semantic clocks.

A host may persist the machine-local location of a project checkout and supply that path when mounting a Project REL. The local checkout path is not durable semantic identity.

### Ambient scope

Ambient multi-owner context follows the active REL dependency closure plus the active PHY according to the relevant capability's privacy rules.

Mounted siblings are not automatically ambient. Explicit owner-qualified operations may address another mounted owner where the capability contract permits it.

## Consequences

### Ownership and dependencies

- Reliquary can implement Ego, cross-scope Entities, relational Observations, context assembly, and provenance without calling upward into Warlock.
- Warlock becomes a thinner coordinator/presenter instead of a second Reliquary semantic owner.
- Lore becomes a Reliquary dependency for the managed Project environment; Git integration likewise moves with that environment implementation.
- Provider implementations remain outside Reliquary behind existing typed endpoint traits.
- Lexicon, Arcana, and other independent capabilities remain separate domain owners even when Warlock coordinates them with Reliquary.

### State, lifecycle, and operations

- One graph-aware Reliquary host may mount multiple RELs plus one PHY.
- Mounted RELs retain independent worker execution and may keep lazy derived/index state.
- Activation changes semantic primacy; it does not require destroying neighboring owner execution.
- PHY state is mounted once at the graph host rather than physically transferred between isolated per-REL hosts.
- Project environment bootstrap/adoption, repository discovery and identity validation, management policy, Lore checkpoints, Git snapshots, repository-backed attachment ingestion, exact historical reads, and repository-root resolution execute within Reliquary after the host supplies the local project mount path.
- Legacy on-disk identifiers such as `warlock.repositoryId`, `.warlock/`, and `warlock/uploads` remain compatibility names; they do not imply Warlock implementation ownership.
- `ReliquaryRuntimeHost::reconcile_rel_file` owns same-owner sibling discovery, compare/reconcile/promote orchestration, conflict reporting, retired-vector detection, derived-vector rebuild, and post-rebuild reopen validation. Warlock may trigger the operation around mount transitions and retain a presentation projection of its transient report.
- Warlock may quiesce interactive provider work around mount transitions, but Reliquary owns internal semantic worker behavior and shutdown correctness.

### Compatibility and migration

- This decision does not change REL/PHY physical formats, durable owner IDs, Memory/Entity identifiers, ProjectRevisionCorrelation, ProjectFileRef, Archive history, or existing provider endpoint traits.
- Migration is incremental: introduce a Reliquary primitive and tests, adapt Warlock to it, then delete the duplicate Warlock implementation.
- The existing public single-REL host behavior remains compatible while its implementation is extracted into an internal owner-execution component.
- Repository and semantic tests move with their authoritative implementation rather than being discarded.
- Current Project REL filesystem placement remains independent of repository identity.

## Alternatives considered

### Keep graph/repository semantics in WorkspaceService

Rejected. It leaves Reliquary dependent on Warlock for its own cross-owner semantics and forces every new graph-wide feature to cross an application boundary.

### Let Reliquary call back into Warlock for neighboring RELs or repository operations

Rejected. The semantic owner would depend upward on one particular host and would no longer be a coherent standalone library.

### Collapse all RELs into one serialized runtime

Rejected. Mounted RELs need independent execution, locks, workers, and lazy stores. Cross-owner operations must support concurrent fan-out.

### Move REL/PHY storage into Lore/Git

Rejected. Project repository history and Reliquary semantic storage remain distinct purpose-built histories correlated at explicit points.

## Verification

The staged migration must protect at minimum:

- existing REL/PHY reopen and storage compatibility;
- current single-REL runtime-host behavior during extraction;
- independent worker creation/shutdown per mounted REL;
- graph cycle rejection and dependencies-before-dependent closure;
- sibling exclusion from ambient context;
- one mounted PHY with correct privacy semantics;
- concurrent cross-owner retrieval without a global Cva serialization lock;
- exact Lore/Git repository identity and historical file verification;
- ProjectRevisionCorrelation and ProjectFileRef compatibility;
- reconciliation and vector-recovery behavior;
- conversation interruption/reopen durability; and
- standalone Reliquary tests for every semantic capability before the corresponding Warlock implementation is removed.

## Risks and debt

- Moving several currently mixed Warlock modules requires splitting semantic operations from serde/UI projection rather than copying files wholesale.
- Multi-owner execution introduces lock-order and shutdown hazards; owner-local locks must remain bounded and cross-owner inference must not hold multiple long-lived Cva locks.
- Lore and its compatibility-constrained dependency graph are owned by Reliquary. Cargo `[patch]` directives are root-only, so downstream application roots may need to repeat Reliquary's patched `quinn-proto` override as build plumbing even though they own no Lore semantics.
- Hot/cold/lazy-loading and eviction policy for inactive mounted REL derived state remains a later operational decision.
- Host-local project-directory persistence remains a Warlock configuration concern until a more general mount-provider contract is justified.

## References

- [ADR 0001 — Purpose-built database ownership](0001-purpose-built-database-ownership.md)
- [ADR 0016 — Native product surface and shared interaction runtime](0016-native-product-surface-and-shared-interaction-runtime.md)
- [ADR 0027 — Warlock project repositories and Reliquary storage boundary](0027-warlock-project-repositories-and-reliquary-storage-boundary.md)
- [ADR 0028 — Project folder and repository bootstrap contract](0028-project-folder-and-repository-bootstrap-contract.md)
- [ADR 0029 — Active REL hierarchy and deferred Connection scope](0029-active-rel-hierarchy-and-deferred-connection-scope.md)
- [ADR 0034 — Cross-owner relational Observations and active-PHY privacy boundary](0034-cross-owner-relationship-graph-and-active-phy-privacy.md)
