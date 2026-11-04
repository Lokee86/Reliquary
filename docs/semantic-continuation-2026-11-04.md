# Semantic continuation evidence — 2026-11-04

- **S01 accepted:** Switchboard and Chronos have no owner-store dependency. Switchboard tests cover dedicated/shared/general decision routing, explicit unavailable selection, schema validation and unresolved decisions. Chronos tests cover deterministic explicit and relative temporal parsing, ambiguity-sensitive detection and the absence of a fabricated relative anchor without a reference time. Both crates passed in the locked offline workspace suite used for G1.
- **S06 accepted:** Freshness tests cover score decay and bounds, deterministic propagation, semantic matching policy and stable due-schedule boundaries. The dispatcher maintains a rebuildable in-memory projection; durable events remain in the Freshness store. The crate passed in the locked offline workspace suite.
- **S05 remains in progress:** deterministic ranking, community routing and bounded traversal are implemented and tested; the turn-scoped agentic query/refine/traverse selection workflow is not yet implemented.
- **S08 remains in progress:** Git and default-feature Lore tests pass, including exact historical bytes and project scope. The current evidence does not yet establish end-to-end source-authorized correlation admission into REL.

G2 remains unaccepted. Native routing, Dream, Entity, Retrieval, vector/context, import/history and external-source vertical workflows retain their own acceptance criteria.
