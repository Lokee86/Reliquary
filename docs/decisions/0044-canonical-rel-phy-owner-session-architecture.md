# ADR 0044: Canonical typed REL/PHY owner sessions with purpose-built domain state

Parent index: [Architectural decisions](INDEX.md)

**Status: Selected storage architecture — 2026-11-03; implementation not authorized until full-system design approval.**

Detailed contract: [REL/PHY storage and owner-session architecture](../rel-phy-storage-architecture-2026-11-03.md). Source evidence and complete method classification: [current inventory](../current-system-inventory-2026-11-03.md) and [method crosswalk](../rel-phy-method-ownership-crosswalk-2026-11-03.md).

## Context

The existing Root `Cva` holds 35 `pub(crate)` mutable fields and `Phylactery` holds 19. The Root lifecycle opens many concrete owners, but extracted `reliquary-owner-lifecycle` mainly offers ordered replay helpers. Insomnia, Dream, Retrieval, Gateway and maintenance modules can reach inside these Root aggregates. Splitting physical crates did not solve this internal ownership knot. Reliquary must remain maintainable and extensible as Observations, Ego and further capabilities are implemented, while preserving existing semantics and performance.

## Decision

1. **One canonical mutable session per mounted durable owner:** `RelSession` for a REL, `PhySession` for a PHY. Each privately composes the correct typed `Container` and concrete domain state. A public `Rel` or `Phylactery` facade may own exactly one session, never duplicate it.
2. **Independent semantic authority:** Container has only physical responsibilities. Archive, Memory, Graph, Entity/Observation, Ego, Freshness, vectors, Insomnia receipts, Dream evidence, import/maintenance and other actual persisted domains own their records, admission rules and history. REL and PHY have different allowable owner domains, enforced on open/replay.
3. **Typed bounded session operations:** Domain-specific reads/proofs and validated write commands; named owner-local grouped publication commands. No general mutable store bag, untyped writes, dynamic registry, all-service context, or generic cross-database manifest.
4. **Workflow outside storage:** Semantic services compute, workflow coordinators authorize/prepare/commit multi-domain or multi-file operations, Gateway handles runtime/grants/workers. None is required for physical open/replay/validation. Higher services never hold the domain mutex during model/provider execution.
5. **Durability and recovery:** Preserve physical sync/receipt contracts, owner-specific clocks, grouped carrier semantics, source identity, idempotent retry and fail-closed reopen. A post-durable projection error requires replay before further mutation. No invented cross-file atomic transaction: REL-to-PHY routing commits each owner independently with deterministic retry/recovery.
6. **History and exclusive maintenance:** Use owner-specific immutable revision/proof views and supported cuts; import/rewrite/prune/reconcile/repair are explicitly authorized offline/exclusive workflows, not unrestricted direct store mutation. Historical unavailability remains visible.
7. **Derived runtime state outside storage:** Freshness due scheduling/notifications, model execution and Gateway subscriptions remain operational/rebuildable. Persisted independent work/journal evidence stays in its relevant durable domain.
8. **Package boundaries:** Separate semantic authorities are **logical ownership**, not an obligation for a Rust crate per field. The exact Cargo packaging will be selected with the full system DAG; merge shallow packages only when this authority boundary is retained.
9. **Naming:** New target code uses REL rather than CVA per [ADR 0043](0043-rel-terminology-and-wire-identity.md). Persisted magic/domain identifiers require a separately proven hard-cut migration; this ADR itself changes no wire format.

## Alternatives considered

- **One monolithic storage package with internal semantic modules:** simplest imports but risks recreating the Root-wide mutable god-object and broad change impact. Not selected as the starting structure; physically consolidating specific shallow modules remains permitted if private ownership survives.
- **One large public `Rel` receiver with all workflow methods:** mimics the present facade and forces new capabilities to extend it. Rejected.
- **Each semantic service completely owns its persistence:** locally attractive but makes independent historical reads, import and replay depend on inference/model layers, driving cycles/backreach. Rejected as the universal rule.
- **Canonical session over concrete domain owners:** selected for clear authority, deep interfaces, scoped borrowing and long-term extensibility without a compatibility framework.

## Invariants and consequences

- No additional mutable aggregate or permanent `Cva` alias; no lower-to-higher dependency back-edge.
- Every operation must validate exact owner identity, file kind, expected revision and write admission at the receiving boundary; authorization cannot be inferred from simply being mounted.
- Owner-local concurrency is preserved; cross-owner reads may fan out with grants, but no broad two-owner lock or provider work inside owner locks.
- Some typed cross-domain commits must change callers, and the grouped carrier codecs currently in higher Root modules must be assigned a lower storage-owned representation.
- The known six-file current-corpus Freshness mismatch (`CVAFRS07`) is a separate prerequisite for implementation acceptance, not a justification to keep legacy normal readers.
- The final Rust methods and crate packaging must be compiled/proven during the later migration, not claimed verified by this design decision.

**Reversal requires a new architecture decision** demonstrating a smaller, more maintainable and equally extensible authority model under the same correctness/performance constraints.
