# Multiplexing P5-E — live runtime and public adapter surface

Parent: [P5/P6 parallel execution](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)  
Roadmap owner: [Complete the live runtime seam](roadmap.md)  
Status: **implementation-ready; not implemented.**

## Purpose

Finish the transport-neutral host-facing runtime surface that remains above the accepted GatewayRuntime/InstanceRuntime core. This track owns adapter/session/event/status behavior that should be shared by Warlock, ACP, Hermes, or another host without giving any transport semantic authority.

## Overview

P3/P4 completed durable queue/branch/generation, checkpointing, reconnectable REL distribution, and exact conversation streams. P5-A/B expose lower-level owner and worker capabilities; P5-C migrates Warlock. What is still not independently owned is the reusable host-facing runtime seam: reconnect/resume around durable cursors, normalized tool/session/artifact events, bounded host controls, capability caching, cancellation/shutdown semantics, and diagnostics.

Existing source includes InteractionRuntime, interaction_session.rs, interaction_background.rs, the gateway generation/stream APIs, RuntimeCapability and host reconciliation. InteractionRuntime currently owns transport-neutral session coordination but still exposes a direct Cva and does not by itself represent the final multiplexed public adapter surface.

## Target architecture

A host attaches through a small transport-neutral adapter API backed by one GatewayRuntime and one or more verified InstanceRuntime contexts. The adapter translates host lifecycle/events into existing durable queue/generation/stream operations and returns typed status/results. It never owns Archive semantics, REL/PHY selection globally, pending queue authority, or P4 subscriber storage.

Normalized event vocabulary may cover session lifecycle, user/assistant turn lifecycle, tool invocation/result, generated artifact reference, operation/checkpoint/status and explicit errors. Event bodies retain exact provenance and do not become a generalized semantic event store. Visible stream data crosses the existing P4 durability boundary.

## Source ownership

Primary ownership: interaction_runtime.rs, interaction_session.rs, interaction_background.rs, narrowly scoped new interaction adapter/event/status modules and focused tests. E does not edit A-owned gateway semantic/project modules, B-owned workers/host router, C's Warlock source, or D's root exports/deletion. Root lib/runtime_host exports are requested from convergence.

## Implementation steps

### E0 — public runtime surface inventory
Map current InteractionRuntime/GatewayRuntime session, generation, stream, cancellation, reconnect and status methods into one final adapter-facing contract. Identify direct Cva/mutable owner escape hatches that cannot survive as host integration APIs.

### E1 — normalized session and event contract
Implement transport-neutral session/event types for durable session open/resume/close, turn/generation status, tools and generated artifacts. Protocol-specific ACP/Tauri/Hermes envelopes stay in adapters. Preserve stable operation/message IDs and checkpoint-before-visible-output.

### E2 — reconnect/resume and adapter migration
Expose durable resume tokens/cursors composed from existing mount epoch, conversation/generation identity and stream cursor. Reconnect reauthorizes the instance and obtains authoritative snapshot/checkpoint before future deltas. Adapter replacement cannot fabricate a second writer or replay Unknown host work.

### E3 — cancellation/shutdown and bounded controls
Give hosts explicit cancel/status/shutdown semantics for long-running work using accepted P3/B authority. Bounded memory/runtime control operations may request wake/quiesce/reconcile/maintenance through existing owners; never return a raw mutable store. Running external cancellation remains cooperative.

### E4 — capability cache and diagnostics
Cache only stable negotiated capability facts with generation/reconnect invalidation. Add typed runtime status sufficient to expose owner mount state, held host work, resync requirement, worker availability and recoverable errors without user-content/secrets. Diagnostics read existing state; they do not become authority.

### E5 — acceptance
Test two adapters/instances on one REL, adapter reconnect/migration, stale cursor/grant, tool/artifact event ordering, cancellation at Pending and Running stages, capability downgrade, diagnostics privacy, no direct owner handle, and no duplicate session/writer. Run fmt/check/integration/docs guards.

## Dependencies

E can begin against accepted P3/P4 and existing A/B APIs. Missing owner operations are requests to A/B. C should consume E's final surface instead of implementing Warlock-only equivalents. G adapters later bind to E.

## Out of scope

Protocol-specific ACP/Hermes/Tauri encoding, workspace management UI, provider algorithms, persistent generalized event log, legacy host deletion, distributed service deployment.

## Related docs

- [ADR 0016 shared interaction runtime](decisions/0016-native-product-surface-and-shared-interaction-runtime.md)
- [P5-C external consumers](multiplexing-phase5c-external-consumers-2026-10-06.md)
- [P6 verification](multiplexing-phase6-parallel-verification-plan-2026-10-06.md)

## Notes

This track closes a real public-runtime ownership gap in the A-D rewrite. It must deepen the existing interaction/gateway seams rather than introduce a second runtime façade with independent state.
