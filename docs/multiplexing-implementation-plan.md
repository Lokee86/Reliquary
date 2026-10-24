# Multiplexing consumer and release plan

Phase 7 changes runtime/source ownership only: mounted REL/PHY retain one writer, independent grants, existing publication/cancellation ordering and public method signatures. Durable V2 bytes and corpus fixtures are unchanged. The [execution record](dependency-convergence-phase7-execution-2026-10-24.md) distinguishes source cuts, regression results and final graph acceptance.

Current implementation: [Runtime multiplexing](subsystems/runtime-multiplexing.md)

## Purpose

Track only the remaining cross-repository adoption and release work around the implemented Reliquary Gateway/Instance runtime.

## Overview

Reliquary's Gateway/Instance ownership, worker sharing, durable conversation scheduling, stream/checkpoint machinery, host capability interfaces, external-source core, and management foundations are implemented on current `main`.

The remaining work is primarily:

1. finish and accept the real Warlock consumer;
2. remove/rename residual legacy host-shaped surface where it still leaks into public integration;
3. run release-level multi-instance/provider/UI verification; and
4. implement and accept Hermes integration.

Historical P0–P6/N3 execution diaries are removed from this active plan. Git history and retained tests preserve them.

## Release A — Warlock production adoption

### A1 — Real provider lifecycle and reconciliation

Complete provider execution against the canonical Gateway host contract.

Required:

- stable operation/request identity;
- correct pending versus running cancellation;
- cooperative Stop;
- terminal provider outcomes;
- explicit handling of remotely ambiguous/unknown outcomes;
- reconnect/recovery only where the provider supplies real durable lookup semantics; and
- no blind replay of unknown work.

If a provider cannot expose durable remote recovery, document that limitation rather than fabricating a status protocol.

### A2 — Multi-context product behavior

Prove actual Warlock product surfaces can host independent contexts over one Gateway:

- independent REL/conversation/branch selection;
- same-principal PHY worker sharing;
- cross-principal isolation;
- detach/rejoin;
- grant revocation;
- task-local retrieval/tool attribution; and
- no ambient mutable selection retargeting accepted work.

Backend-only fixture success is not sufficient for desktop acceptance.

### A3 — Durable stream/UI acceptance

Verify the real UI/Tauri path consumes only checkpoint-synchronized text.

Cover:

- late join;
- slow reader overflow/resync;
- reconnect;
- generation/epoch rotation;
- revocation;
- Unicode/prefix replacement;
- duplicate/regressing receipt rejection; and
- no text from uncommitted provider output.

### A4 — Exact attribution

Finish exact task-root and project-file reference propagation through provider/tool/retrieval operations. Do not replace exact references with current-selection lookup at completion time.

### A5 — Dependency convergence

Warlock must pin one accepted Reliquary revision and one compatible direct Arcana revision. Regenerate lock state and prove no unintended duplicate Arcana identity remains.

Run the complete Warlock backend/frontend/provider/context matrix against those exact pins.

## Release A cleanup — legacy surface

The old `ReliquaryRuntimeHost` object is no longer a runtime owner. Residual type names such as `ReliquaryRuntimeHostError` may remain as compatibility naming, but no duplicate host instance/worker/queue/selector authority may survive.

Remaining cleanup should:

- relocate/rename useful shared result/error types when doing so improves the public surface;
- remove truly obsolete forwarding/facade APIs;
- replace tests that assert obsolete owner behavior;
- keep persisted readers required by current data until their separate format-retirement gate; and
- avoid compatibility scaffolding whose only purpose is preserving an abandoned runtime architecture.

## Release A verification

Run release-level verification on the final integrated tree:

- multiple REL/PHY/Instance combinations;
- queue ordering and cancellation/finalization races;
- crash/reopen/no-duplicate work;
- mount/revoke/remount;
- slow stream readers and resync;
- provider interruption and ambiguous outcomes;
- actual Warlock UI/Tauri delivery;
- worker-count/lock/latency/fanout measurements; and
- full locked Reliquary, CLI, Warlock, frontend, documentation, Archivist and Pitlord gates.

Release A is complete only when real Warlock usage passes these gates on exact dependency pins.

## Release B — Hermes integration

Hermes retains canonical transcript ownership. Use the existing external-source core rather than copying sessions into Archive.

### B1 — Source/feed adapter

Implement the normalized Hermes change-feed/snapshot/hydration contract described in [the external host integration plan](conversation-index-integration-plan.md).

### B2 — Session/runtime adapter

Map real Hermes sessions and operations onto Gateway/Instance authorization, durable execution, cancellation, and lifecycle.

### B3 — Multi-session acceptance

Exercise at least:

- two simultaneous sessions;
- same-principal shared PHY;
- different-principal isolation;
- independent REL/branch selection;
- source edit/delete/rewind;
- reconnect/replay;
- Reliquary outage/catch-up; and
- exact transient source hydration.

Release B is complete only after end-to-end real Hermes fixtures/providers pass.

## Separate backlog

The following do not block either release unless explicitly pulled into a product milestone:

- additional import/export formats;
- ACP adapters;
- broader management UX;
- optional provider-specific durable recovery protocols;
- future IPC/service boundaries; and
- generalized organization/team ownership extensions.

## Invariants

- One Gateway owns mounted runtime authority.
- Instance is authorization/context, not another semantic owner.
- No duplicate queue or worker authority.
- Durable text is visible only after checkpoint synchronization.
- Unknown provider outcome is not replay permission.
- REL/PHY privacy is authorization-first.
- External canonical transcripts remain host-owned.
- Consumer convenience never recreates ambient global mutation authority.

Internal runtime convergence retains the implemented Gateway/Instance and mounted-owner execution contracts. It introduces no consumer release capability, host route, scheduler or compaction behavior; the remaining release/product work in this plan still needs its own acceptance. Current source-boundary evidence belongs in the [Phase 5 receipt](dependency-convergence-phase5-execution-2026-10-23.md).

## Related docs

- [Runtime multiplexing](subsystems/runtime-multiplexing.md)
- [External sources](subsystems/external-sources.md)
- [External host integration plan](conversation-index-integration-plan.md)
- [Roadmap](roadmap.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)

## Notes

This plan is intentionally release-oriented. Implemented Gateway/Instance contracts belong in current subsystem/API/architecture documentation.

## Public offline source ownership

The [Phase 6 execution receipt](dependency-convergence-phase6-execution-2026-10-23.md) tracks facade placement of supported offline adapters. Mounted Gateway execution continues through canonical owner execution; the migration introduces no additional runtime or facade middleman.
