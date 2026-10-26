# REL and Phylactery scope ownership

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe current durable scope ownership for Reliquary (REL) and Phylactery (PHY), including Memory placement, dependency composition, privacy, and cross-owner references.

## Overview

REL and PHY are distinct durable semantic owners.

- A REL owns project/conversation-scoped semantic state and its Archive-backed source history when operating natively.
- A PHY owns user-private semantic state that should survive independently of any one project REL.

Neither is merely a namespace inside the other.

## Memory ownership

Routing can classify durable Memory placement as project/REL or user/PHY. The destination owner then owns Memory identity, revisions, lifecycle, Graph/vector state allowed by that owner, and relevant local semantic processing.

A PHY Memory does not require a live pointer to an originating REL. Source references may preserve identifier/provenance evidence without making the source owner a runtime dependency.

## Authority separation

Moving or referencing semantic evidence does not transfer source authority. A REL cannot mutate PHY-private state merely because it can reference it, and a PHY does not inherit REL conversation history.

Cross-owner Graph/relationship evidence is typed and authorization-aware. Payload privacy remains enforced separately from relationship visibility.

## REL dependency composition

REL dependency topology is a host/runtime scope graph. Opening one REL does not implicitly grant every related REL. Authorized dependency closure can inform context/retrieval composition without copying all source state into the active REL.

User-relative cross-REL summaries, where implemented in Ego, remain keyed to the active PHY plus source REL rather than becoming an implicit shared organization synopsis.

## PHY limitations

PHY does not own the REL accepted-turn activity clock. Features such as Freshness that depend on an owner-local activity source require a separately defined PHY policy rather than borrowing another REL's activity.

## File and identity model

REL and PHY are typed file kinds with durable owner identity. Rewrites/rebuilds can preserve or intentionally regenerate owner identity according to explicit policy; physical file paths are not semantic owner identity.

## Code Map

- `src/facade/**`
- `crates/reliquary-memory-core/src/**`
- `src/memory/{processing_epoch_*,memory_source_time*}.rs`
- `src/retrieval/**`
- `src/retrieval/**/*.rs`
- `src/semantic_graph/**`
- `src/runtime_inference/runtime_host_gateway*`

## Related docs

- [Architecture](../architecture.md)
- [Storage format](../storage-format.md)
- [Runtime multiplexing](runtime-multiplexing.md)
- [ADR 0018](../decisions/0018-reliquary-and-phylactery-naming.md)
- [ADR 0020](../decisions/0020-reliquary-and-phylactery-file-kinds.md)
- [ADR 0021](../decisions/0021-typed-reliquary-scopes-and-connections.md)
- [ADR 0029](../decisions/0029-active-rel-hierarchy-and-deferred-connection-scope.md)
- [ADR 0034](../decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md)

## Notes

The former scope design plan is retired. ADRs retain rationale; this document owns the current operational scope model.

Memory Core is shared compiled code, with separate REL and PHY owner sessions.
It owns terminal and retained revision facts; root composition owns source
permission, cross-owner import and maintenance decisions. External Source window
receipts remain root-owned. Processing epochs stay at root until Owner Lifecycle
extraction; they do not allocate Memory revisions.
