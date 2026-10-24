# Perception

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe the implemented Perception foundations: Entity mention state, Entity ownership/resolution, graph association, scheduling, and the boundary to future Observation synthesis.

## Overview

Perception turns semantic evidence into longer-lived derived semantic state. Its implemented production foundation is Entity-centered: Insomnia records exact Entity mentions, Perception resolves or admits Entities, persists mention-resolution state, associates typed graph evidence, and schedules bounded reconsideration.

Observation synthesis and broader propositional reasoning remain active future work in the Perception plan.

## Public composition boundary

Public `Cva`/`Phylactery` audit, candidate, resolution and reconciliation adapters belong to `src/facade/`. Perception retains audit calculation, candidate/evidence policy and pure preparation/commit plans over concrete owner readers. Public adapters gather existing owner borrows; mounted workers use the same lower plans and concrete owner-lifecycle admission/publication without calling facade helpers. Public Resolver/Engine bindings own private inference cores, which mounted workers use directly. [Phase 6 evidence](../dependency-convergence-phase6-execution-2026-10-23.md) records the migration and its verification limits.

## Memory mention input

Insomnia persists exact Entity mentions against the authoritative Memory body. Mention records retain field/span/verbatim evidence and are validated against the exact body.

Mention extraction does not itself create or merge an Entity.

## Entity ownership

Entity is a durable semantic owner with stable identity, metadata, aliases/evidence, lifecycle, and merge/tombstone behavior. Perception may propose/resolve identity; Entity storage owns accepted durable state.

Zero-candidate mentions can use the existing generative Admission path. Non-empty candidate sets use deterministic evidence plus configured decision machinery, while preserving hard merge/rejection guards.

## Resolution state

Each mention key has explicit persisted resolution lifecycle rather than an implicit "try again forever" loop. Current states distinguish unresolved/dormant/resolved/rejected behavior and exact evidence/retry identity.

Wake sources are bounded deterministic events such as new post-Dream evidence or relevant Entity changes. Ordinary reads do not manufacture retries.

## Decision routing

Entity resolution uses its operation-specific decision route, then shared `general_decision`, then adapted `general`. Decision output without calibrated confidence cannot force a merge that requires a confidence threshold. The existing generative resolver remains a distinct fallback mechanism where its contract permits it.

## Graph association

Entity/Memory and cross-owner association uses typed semantic graph endpoints. Privacy and owner authorization remain explicit; a graph edge does not transfer payload ownership.

## Chronos boundary

Chronos supplies temporal interpretation primitives. Perception owns any resulting Observation validity or multi-evidence semantic conclusion. Chronos does not own Observation lifecycle.

## Failure and recovery

- Invalid mention spans/body bindings fail closed.
- Resolution retries are identity/evidence bounded.
- Terminal Entity lifecycle cannot be resurrected by later mention processing.
- Ambiguous evidence remains unresolved rather than forcing identity.
- Authorization boundaries apply before cross-owner evidence access.
- Persisted resolution state survives reopen and scheduling recovery.

## Code Map

- `src/perception/**`
- `src/runtime_inference/*perception*`
- `src/semantic_graph/**`
- `src/chronos/**`

## Related docs

- [Entity resolution state](../entity-resolution-state.md)
- [Perception future work](../perception-subsystem-plan.md)
- [Chronos](chronos.md)
- [ADR 0033](../decisions/0033-perception-entities-observations-and-ambiguity.md)
- [ADR 0034](../decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md)
- [ADR 0036](../decisions/0036-typed-semantic-graph-endpoints.md)

## Notes

This document intentionally stops at implemented behavior. The target Observation/receptor/reconsideration architecture remains in the active plan with enough context to understand the intended continuation.
