# Multiplexing P5-A — REL/runtime consumer migration

Parent: [P5/6 parallel plan](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)  
Status: **Implementation present on `feat/multiplexing-p5-rel-consumers`; verification/acceptance incomplete**. See [P5-A local acceptance](multiplexing-p5a-acceptance-2026-10-06.md) and [consumer matrix](multiplexing-p5a-consumer-matrix-2026-10-06.md).

## Purpose

Move remaining Archive, Memory, Entity/Graph, Echo/provenance, retrieval, repository/project, status and maintenance callers to the *existing* owner-qualified gateway. Remove the need for ambient REL/session/PHY state without moving semantic ownership or duplicating algorithms.

## Overview

Canonical paths already exist in runtime_host_gateway_ambient.rs, runtime_host_gateway_ambient_extended.rs, runtime_host_gateway_ambient_project.rs, runtime_host_gateway_semantic.rs, runtime_host_gateway_semantic_search.rs, runtime_host_gateway_snapshots.rs and runtime_host_gateway_mutation_events.rs.

Legacy inventory, not A-owned deletion targets: runtime_host_archive_search.rs, runtime_host_owner_archive.rs, runtime_host_knowledge*.rs, runtime_host_memory_*.rs, runtime_host_semantic_*.rs, runtime_host_project_environment.rs, runtime_host_status.rs, runtime_host_interaction.rs and runtime_host_session.rs. New gateway offers exact owner reads, archive pages/transcripts, semantic search, private PHY profile, project operations and authorized owner lifecycle. Confirm exact current signatures before changing them.

## Target architecture

A caller carries a verified InstanceRuntime and requested REL/PHY owner identity. Gateway validates current grant, attachment, owner mount epoch and fence, dispatches a narrow semantic owner operation, commits/syncs and publishes an appropriate P4 event. Separate authorization is needed for dependency RELs and UseOwnPhy. Caller cannot obtain mutable Cva/PHY ownership or consult active_execution, host-global active_rel_id or phylactery_owner_id.

## Source ownership

Edit gateway ambient*, semantic*, project/retrieval helpers and their tests only. New narrowly scoped helper modules may be added under the canonical gateway. Do not change runtime_host.rs root, delete mixed legacy host files, modify B worker files or edit Warlock. Provide D a deletion manifest and C an API contract table.

## Implementation steps

### A0 — real consumer matrix
Enumerate legacy method and real caller → existing gateway method / missing operation / trusted quiesced maintenance / obsolete. Record permissions, durable boundary, event class and exact tests. Output: docs/multiplexing-p5a-consumer-matrix-2026-10-06.md.

### A1 — Archive/conversation/semantic REL reads
Port exact transcript/branch search, Echo/file provenance, dependency Archive search and page summaries. No fallback to another selected REL; no implicit grant from a dependency closure. Tests: instance A/B different selections, granted/denied dependency, inactive conversation, remount stale grant and scope.

### A2 — Memory/Graph/Entity/private PHY
Use existing owner-polymorphic semantic reads and exact UseOwnPhy. Preserve private result lane through retrieval and embedding. Do not leak PHY content through REL snapshots or create missing vector profiles on demand. Tests: same REL/two principals/two PHYs, guessed PHY denied, revoked grant during retrieval, wrong dependency denied, principal A profile not B, REL-only memory graph isolation.

### A3 — project/status/reconciliation/maintenance
Migrate metadata/project correlation, repository operations, owner status, quiesce/reopen and safe reconciliation. External Git/filesystem calls run outside owner locks; authoritatively synced REL changes emit correct P4 class. Tests: ManageProject denial, one REL quiesced while others run, stale epoch, status and safe invalidation.

### A4 — ownership hard-cut audit
Run direct source searches and P2/P3/P4 guards; classify every new gateway public mutator and update P4 coverage. The legacy host may remain physically present until D but the new caller surface cannot route to it. Refuse a wrapper merely to pass an intermediate compile.

## Test and handoff gates

Required: focused gateway Archive/semantic/project/auth tests, negative privacy/grant cases, applicable integrated library/integration tests, public API contract where changed, fmt/check, docs structural/change-impact, diff check. A's final report supplies source/API matrix, accepted test results, missing-public-export requests to D, and old-only file/caller deletion candidates. Source guard is supporting evidence, not proof of correctness.

## Out of scope

Dream/Insomnia/vector worker ownership (B), Warlock and CLI implementation (C), deletion/root exports (D), benchmark tuning (P6).

## Documentation and architecture impact

Update the canonical API/maintainer and behavioral contract descriptions only for implemented behavior; avoid claiming external consumer completion. One owner, one authorization decision and post-durable publication per write remain mandatory.

## Related docs

- [Parallel execution](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [Phase 0B consumer inventory](multiplexing-phase0b-consumer-verification.md)
- [Phase 4 mutation coverage](multiplexing-phase4-mutations-2026-10-05.md)

## Notes

Legacy source paths are inventory only. P5-D owns deletion and final root wiring.