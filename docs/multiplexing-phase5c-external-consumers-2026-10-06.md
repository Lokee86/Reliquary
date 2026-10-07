# Multiplexing P5-C — external consumers: Warlock and detachable CLI

Parent: [P5/6 parallel plan](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)  
Existing audit: [Phase 0B G6](multiplexing-phase0b-consumer-verification.md)  
Status: **Implementation-ready; actual upgraded external integration not yet verified.**

## Purpose

Move the real external application from host-global REL/PHY/session authority to a single GatewayRuntime with authenticated, independently selected InstanceRuntime contexts. Integrate conversation queue, generation streams and provider execution/cancellation without manufacturing an external durable pending queue. Audit the detachable CLI without forcing irrelevant rewriting.

## Overview

Warlock-v2 is the primary known consumer. Its workspace.rs currently stores ReliquaryRuntimeHost; workspace_open/session/set, conversation and chat command/stream code assume one shared active workspace/session. Reliquary direct imports were inventoried in the Phase 0B G6 audit, which previously found 58 Warlock Rust source files. Warlock Cargo still pins Reliquary 83b3ac87 and direct Arcana 66b4e96; new Reliquary main is newer. **Refresh that inventory** before migration rather than assuming the old count or pins still describe any branch in progress.

Warlock owns provider transport, credentials, route selection, cancellation of running inference and Tauri UI channels. Its inspected inference lifecycle is not a durable pending-chat queue. Reliquary owns accepted native queue order/cancellation, Archive finalization/checkpoint and REL distribution.

The currently checked-out Warlock main has unrelated uncommitted graph/UI edits (including workspace_graph_render*, KnowledgeGraph, WorkspaceShell and an Arcana panel). Create a separate clean Warlock worktree from its committed base. Never stash/reset those changes or reuse that dirty checkout for C.

Reliquary CLI currently calls ConfiguredRuntime for operator Insomnia/vector workflows and has no confirmed direct ReliquaryRuntimeHost reference. Keep an operator-only CLI on its legitimate standalone/quiesced seam; migrate only if a real new gateway authority or public export change requires it.

## Source and repo ownership

C edits **Warlock** src-tauri workspace*, conversation*, chat*, inference_lifecycle, reliquary_* and provider adapters, Warlock Cargo.toml/Cargo.lock and required app tests/TypeScript callers. Avoid graph/Arcana UI files unless a proven consumer mismatch requires it. A/B own missing low-level Reliquary operations, E owns the transport-neutral runtime/adapter contract, F owns the workspace-management facade, G owns cross-framework import/interoperability contracts, and D/convergence owns root public exports. If CLI changes, C owns only the affected cli/src modules/tests. Do not commit untested production pins until an actual Reliquary integration commit exists.

## Target architecture

One Warlock application gateway per mounted owner set. A distinct operational context/device/agent gets its own verified InstanceRuntime attachment with one exact PHY binding and independent active REL/conversation/branch selection. The gateway owns shared REL execution, durable journal and generation; viewer attach/detach is observation and does not start a writer or cancel accepted work.

Warlock implements a real GatewayHostAdapter only for supported configured provider/external stages. It returns exact capability contracts; never advertises durable Pending queue ownership without proving acceptance/ID/cancel-dequeue/status/handoff guarantees. Native pending admission stays Reliquary-owned when those guarantees are unavailable.

Tauri output channels carry exact P4 authorized subscription/checkpoint delivery after sync, not raw unsynchronized provider output as if committed. Rel-wide safe updates go to all authorized listeners; active text only to subscribers of exact authorized conversation/operation.

## Implementation steps

### C0 — fresh consumer/capability inventory
Regenerate real source call graph and pins (script verify_multiplexing_phase0b_consumers.py is an *old-shape drift oracle*; replace its manifest after final consumer cut). Produce an exact table: old Warlock call → final GatewayRuntime method/trait, permissions, state ownership, Tauri surface, test. Flag missing A/B/E/F/G APIs early; do not invent proxy wrappers.

### C1 — workspace and independent attachments
Replace WorkspaceService old host-global runtime field and mount/session selection with one gateway plus instance registry keyed by actual operating contexts; preserve owner mount fencing and quiescence. Ensure reconnect/refresh reauthorizes. Tests: two contexts select different RELs/branches; same REL two viewers one generation; same PHY two instances one owner; guessed PHY denied; detach does not cancel durable accepted work.

### C2 — conversation admission, generation and frontend streams
Move chat commands to durable gateway submit/status/cancel and finalization path. Connect provider handoff/result to stable operation identity and P3 generation controls; preserve attached content/identity and archive activity exactly once. Render Tauri events from authorized P4 stream/checkpoint join. Reconnect uses snapshot+watermark, epoch validation, resync and exact stream prefix; no duplicate text or silent gap. Tests: cancel pending before dispatch, cancel racing dequeue, interrupted running operation, branch conflict, late viewer, slow Tauri listener, reconnect/revocation.

### C3 — other workspace/retrieval/maintenance callers
Port Archive/Memory/Graph/Entity/echo/provenance/project/status/reconciliation operations to A's explicit owner methods. No implicit global active REL, no direct mutable runtime handle, no foreign private PHY reads. Preserve Warlock legitimate display/navigation semantics, not old host authority.

### C4 — real host/provider conformance
Using fake adapter and actual Warlock provider integration, test durable handoff digest, stable ID retry, Unknown Held, capability downgrade, reconnect/cancel reconciliation, running interruption and cross-stage cancellation until downstream acknowledgment. Provider network operations stay outside REL/PHY owner locks. Never claim Warlock has a host pending queue without live source/tests proving its stronger contract.

### C5 — dependency hard cut and compile
After A/B/E/F and applicable G core work have an accepted Reliquary integration revision, update Warlock's Reliquary pin, direct Arcana pin and Cargo.lock as one bounded change. Require cargo tree locked to show **one intended Arcana package identity**. Rework imports/source drift inventory directly to the new API; do not retain ReliquaryRuntimeHost as a forwarding façade.

Run Warlock Rust locked check/tests and npm production build; exercise Tauri command/integration fixtures with two instance contexts. Verify this C worktree did not absorb unrelated dirty Warlock main edits.

### C6 — detachable CLI audit
Reinspect cli/src for real moved gateway/host APIs. If present, adapt to canonical interfaces and run CLI locked fmt/check/test. If absent, explicitly record no change and pass existing tests; do not instantiate a user-facing multiplexing gateway just to replace ConfiguredRuntime for standalone operator activity.

## Acceptance and handoff

Deliver new Warlock branch commits with an upgraded pinned Reliquary/Arcana lockfile, real compiled/tested provider/queue/stream scenario results, updated direct-import/consumer manifest, source negative check (no ReliquaryRuntimeHost, host-global active session or direct writable owner), and any bounded known build exceptions. The accepted C revision must be consumed by D before old exports are deleted.

A missing public operation is a repair request to its A/B/E/F/G owning track, not permission to add a Warlock-local mutable authority or old-host shim.

## Out of scope

Warlock graph renderer/Arcana panel redesign, new Tauri protocol, distributed process gateway, provider algorithm changes, and unspecified ACP adapters without identified source. Do not overwrite unrelated work.

## Documentation and architecture impact

Update Warlock integration/current state with true runtime ownership, provider capability honesty and UI event scopes. Change Reliquary public docs only through convergence owner after real upgraded build evidence.

## Related docs

- [Parallel execution](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [Phase 0B G6 consumer inventory](multiplexing-phase0b-consumer-verification.md)
- [P4 exact stream](multiplexing-phase4-streams-2026-10-05.md)

## Notes

Use a clean Warlock branch and preserve its currently dirty main. Recheck the pinned dependencies and callers at execution.

