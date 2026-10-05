# Multiplexing Phase 4 — P4.8 coverage, lock, and privacy audit

Parent index: [Documentation index](INDEX.md)  
Parent plan: [Phase 4 distribution plan](multiplexing-phase4-distribution-implementation-plan-2026-10-04.md)  
P4.7 convergence: [Production conversation bindings](multiplexing-phase4-convergence-2026-10-05.md)  
Base: `4f88c4b`  
Status: **P4.8 audit complete; P4.9 Phase 4 acceptance remains pending**

## Purpose

Record the P4.8 coverage, lock, privacy, durability, and maintenance-bypass audit of the converged P3/P4 distribution path.

## Overview

The audit confirmed that P4's sequencer owns subscriber registries, queues, and cursors; subscriber handles are not stored in semantic stores or the P3 coordinator, though callers may hold returned handles. P3 remains the durable conversation source. Two source issues found during review were corrected: checkpoint reconstruction now locates the exact generation/message and copies only a validated durable prefix, and journal status is captured once and bucketed per conversation with sequence checks, removing repeated full-journal status clones. This does not make all observation construction linear or eliminate all history work; Archive branch enumeration remains unchanged. The former path cloned every full stream body before applying the journal size bound. The prefix fix bounds newly copied delivery text while locks and the publication gate are held; it does not bound the underlying persisted Archive content size or avoid unrelated Archive branch enumeration.

Privacy assertions permit shared REL titles and stable IDs, and exclude private PHY bodies, principal IDs, and handoff digests from broad REL snapshots/events. No production sanitization fix was needed: source boundaries and classification were correct.

## Six-criterion audit

| Criterion | Finding and evidence |
| --- | --- |
| Every gateway instance-visible mutator is classified | Passed the expanded coverage guard: 28 isolated positive/negative fixtures, 112 public APIs, all five REL worker files. This structural heuristic guard does not prove indirect arbitrary Rust calls or all runtime behavior. |
| No private PHY body enters a REL event or snapshot | Passed strengthened privacy assertions. Shared REL titles/IDs are allowed; private PHY bodies, principal IDs, and handoff digests are excluded. |
| Subscriber state stays in P4 | Registries, queues, and cursors are owned by the mounted P4 sequencer. Subscriber handles are not stored in semantic stores or the P3 coordinator; callers may hold returned handles. |
| No network/backpressure wait under semantic-owner or journal locks | Source review verified runtime → coordinator → journal acquisition order, no journal-held runtime acquisition, provider/model work outside short persistence stages, and bounded nonblocking fanout. The corrected checkpoint path copies only the validated durable prefix; the bound applies to copied delivery text, not persisted Archive content size. |
| Events follow durability | Source review and tests confirm publication follows the owner/Archive synchronization point or the corresponding durable journal receipt/checkpoint. A safe generic ArchiveChanged invalidation may follow a synchronized branch-head write before an ArchiveCommitted journal receipt exists; typed ArchiveCommitted follows the exact receipt. Later capture/publication failure invalidates distribution continuity while preserving an already successful durable mutation result. |
| Maintenance bypass preserves continuity rules | Reconnect regression verifies the maintenance helper rejects invocation while the REL is live; quiescence closes old handles and rejects old subscriptions. It then exercises a synchronized Archive branch write through the drained bypass and reopens with a new epoch and authoritative snapshot containing the branch. Old cursors/reconnects are rejected; no incremental replay or body is exposed. |

## Verification status

The final P4.8 matrix passed 1,026 library and 95 integration tests with zero failures or ignored tests, including three new tests and two strengthened privacy tests. Runtime `cargo check`, formatting check, and the coverage guard (28 fixtures/112 APIs, all five REL worker files) passed. No CLI code or public exports changed; the CLI suite was not rerun for P4.8.

The 120-second architecture refresh timed out while Lexicon analyzed Rust/Python; Arcana/Pitlord did not run, so no refreshed architecture pass is claimed. The unrestricted `cargo test` example path retains the inherited allocator conflict in `archive_open_profile`; the passing matrix uses `cargo test --tests`. An initial build against a reused target failed before changed-source compilation because dependency artifacts were missing; the fresh P4.8 target resolved that environment issue and the final matrix passed. Both structural and changed-from documentation-policy checks passed.

## Documentation impact

- Inspected: P4 plan, P4.7 convergence record, architecture, invariants, maintainer map, behavioral contracts, limitations, coverage map, roadmap, and documentation standards.
- Updated: this audit record, index, P4 plan, architecture, invariants, maintainer map, behavioral contracts, limitations, coverage map, and roadmap.
- Not affected: public API, durable encodings, P3/P4 ownership boundary, or external CLI.
- Compliance check: structural and changed-from documentation-policy checks passed.
- Known documentation gaps: architecture refresh remains incomplete as stated above.
- Status: P4.8 audit findings and current verification are recorded; P4.9 acceptance remains pending.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: P3 retains durable conversation source authority; P4 retains all subscriber and distribution authority.
- Pitlord enforcement impact: 120-second Lexicon timeout before Arcana/Pitlord; no refreshed architecture result.
- Other verification impact: 1,026 library and 95 integration tests, runtime check, format check, and 28-fixture/112-API coverage guard passed.
- Known architectural gaps: P4.9 remains; P3.8/P3.9 remain separate pending gates.
- Status: two bounded source-capture defects were corrected; no new owner or public API was introduced.

## Related docs

- [P4 distribution plan](multiplexing-phase4-distribution-implementation-plan-2026-10-04.md)
- [P4.7 convergence record](multiplexing-phase4-convergence-2026-10-05.md)
- [Architecture](architecture.md)
- [Behavioral contracts](behavioral-contracts.md)
- [Current limitations](current-limitations.md)

## Notes

P4.8 audit and its available code checks are complete. P4.9 is the remaining Phase 4 acceptance gate; this record does not claim overall Phase 4 acceptance.
