# Multiplexing P5-F — workspace management surface

Parent: [P5/P6 parallel execution](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)  
Roadmap owner: [Complete the workspace management API](roadmap.md)  
Status: **next-wave critical-path track; specified and ready to start in parallel with E and C closure. F0–F4 can proceed immediately; F5 selective import/export waits on G1. No implementation is yet accepted.**

## Purpose

Provide the product-facing management layer required by Warlock and other hosts without exposing raw REL/PHY internals or creating a generalized semantic object store. This closes the gap between P5-A's low-level owner-qualified operations and P5-C's real application needs.

## Overview

GatewayRuntime already exposes many concrete operations: conversation summaries/transcripts, Archive search, Echo/files, Memory/provenance, semantic owner reads, retrieval, project correlation, owner status, quiesce/reopen and reconciliation. P5-A verifies/migrates those primitives. The roadmap separately requires a coherent management surface for conversations, files, Memories, health/statistics, import/export and safe maintenance. That composition was not owned by A-D.

## Target architecture

Workspace management is a thin, typed application surface over existing semantic owners and E's runtime status. It accepts verified instance/owner context, composes narrow operations and returns immutable DTO/results. It never stores independent semantic truth, bypasses grants, returns mutable Cva/Phylactery handles, or reimplements Archive/Memory/Graph policy.

## Source ownership

F owns a dedicated management module/facade and tests, plus management-specific DTOs. A owns primitive gateway semantic/project operations; E owns live runtime/session/status; C owns Warlock presentation. D owns root exports. F does not modify store codecs or worker ownership.

## Implementation steps

### F0 — management capability matrix
Map every roadmap/Warlock management need to existing A/E operation or a missing primitive. Record grant, owner, pagination/bounds, mutation semantics, audit/event behavior and whether operation is safe for live use or requires quiescence.

### F1 — conversation/session management
Expose list/page/open/select/branch/session inspection and permitted management actions beyond raw transcripts, using P3 coordinator authority. Selection remains instance-local; no management call becomes a writer implicitly.

### F2 — file/source/project management
Expose file inventory, source provenance, project-file references, standalone add/import/export where current persistence supports it, and safe resolved reads. Rename/move/unlink are implemented only when retention/history contracts are defined; otherwise return an explicit unsupported capability rather than destructive best effort.

### F3 — Memory/semantic inspection and permitted lifecycle
Expose Memory inventory/provenance/lifecycle state, Graph/Entity/Community summaries, exact owner stats and only those manual lifecycle actions that existing owners define safely. Private PHY inspection requires exact personal authorization. No generic mutate-any-object endpoint.

### F4 — health, verification and statistics
Compose owner/runtime diagnostics, journal/worker/reconciliation state, version/repair status and bounded statistics into a host-facing health result. Do not leak secrets, prompts, private owner bodies or unrelated dependency data.

### F5 — selective import/export
Provide a typed management workflow for supported user-owned semantic state using existing import/export codecs and owner checks. Stable IDs/idempotency and provenance are mandatory. Protocol/provider-specific transcript conversion belongs to G.

### F6 — acceptance
Test grant boundaries, pagination/bounds, live-vs-quiesced operations, same REL/two principals, private PHY isolation, unsupported destructive operation behavior, health privacy, import/export retry/reopen and P4 invalidation for mutations. Warlock C integration must use F rather than raw stores for these surfaces.

## Dependencies

F can design/implement composition immediately; exact primitive gaps are delegated to A. E supplies runtime/session diagnostics. C consumes F. D exports only the final accepted surface.

## Out of scope

UI layout, new semantic owner types, generalized object CRUD, speculative file VCS, provider-specific transcript import, external canonical transcript indexing, destructive retention policy not already defined.

## Related docs

- [ADR 0016](decisions/0016-native-product-surface-and-shared-interaction-runtime.md)
- [P5-A REL consumers](multiplexing-phase5a-rel-consumers-2026-10-06.md)
- [P5-C Warlock](multiplexing-phase5c-external-consumers-2026-10-06.md)

## Notes

F deliberately composes existing owners. If an operation requires semantic knowledge absent from the facade, add the narrow primitive to its real owner rather than teaching management to inspect internal storage.
