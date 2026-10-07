# Multiplexing P5-B — background workers and host-capability cut

Parent: [P5/6 parallel plan](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)  
Status: **Lane B source implemented; see [acceptance](multiplexing-p5b-acceptance-2026-10-07.md) for actual verification and remaining convergence gates.**

## Purpose

Finish canonical REL/PHY worker execution, configured capability routing, and cross-stage cancellation/reconciliation without retaining legacy-host worker authority. Preserve existing algorithms and owner-local semantic formats.

## Overview

Current final-path owners: runtime_host_rel_execution.rs, runtime_host_rel_insomnia.rs, runtime_host_rel_dream.rs, runtime_host_rel_perception.rs, runtime_host_rel_vectors.rs; runtime_host_phy_execution.rs, runtime_host_phy_dream.rs, runtime_host_phy_perception.rs, runtime_host_phy_vectors.rs; runtime_host_gateway_host.rs and runtime_host_owner_operation_journal.rs.

Old host worker paths runtime_host_owner_execution.rs, runtime_host_insomnia.rs, runtime_host_dream*.rs, runtime_host_vectors.rs, runtime_host_perception*.rs, runtime_host_lifecycle.rs and runtime_host_routes.rs are mixed legacy files for D, not a second B implementation. GatewayHostAdapter currently describes ConversationExecution, Insomnia, Dream, Embedding and ProjectEnvironment. Defined capability contracts do not imply a real stage adapter currently executes each one.

## Target architecture

One mounted REL runs exactly one RelRuntime worker set; each mounted PHY runs exactly one PhyRuntime set regardless of attached instance count. REL Insomnia retains extraction/Episode/REL receipt authority. A frozen durable exact-principal plan handles PHY publications; offline/revoked target is Held with no retarget. Provider/model calls are outside owner locks.

Each stage has exactly one authority: Native, Host, or Held. Gateway controls stable ID/digest, durable host handoff intent, cancellation/status/reconcile and fail-closed uncertain outcome. Host owns provider transport and running-call cooperative cancellation where actually configured; no invented durable pending queue.

## Source ownership

B edits canonical REL/PHY worker modules, gateway host adapter/router/capability implementation, focused worker/host tests, and narrow runtime configuration operations. A owns gateway semantic/project operations; C owns Warlock adapter/credential transport; D owns root runtime_host.rs and deletion. Do not patch old mixed host workers to mirror new worker state.

## Implementation steps

### B0 — capability and worker ledger
Map every legacy worker and configured stage to canonical REL/PHY owner, accepted receipt, retry, wake/status, shutdown/reopen, host or native execution. Mark missing *executable* stages explicitly. Output docs/multiplexing-p5b-capability-ledger-2026-10-06.md.

### B1 — native background parity
Close missing Insomnia/Dream/Perception/vector enable/wake/status/recovery operations on existing RelRuntime/PhyRuntime. Explicit owner ID at every background stage; private generation/embedding never follows active REL. Tests: 2 instances/one PHY/one worker set, 2 principals/one REL/2 private workers, quiesce isolation, receipt replay exactly once, offline PHY Held then exact-destination recovery, Freshness parity.

### B2 — host capability binding
For Insomnia, Dream, Embedding and ProjectEnvironment, implement only real configured host stages that can satisfy their contracts; otherwise explicitly retain Native or report Held/Unavailable. Supply C the precise host adapter payload/result/cancellation boundaries, not a new generic executor registry. Keep finite routes in one canonical config owner.

Tests with deterministic fake host: durable acceptance and payload hash; identical retry; duplicate ID with changed digest denied; capability downgrade; reconnect with ownership debt; unknown status Held; crash after handoff and before receipt; native unrelated stage remains available.

### B3 — staged cancellation, worker lifecycle and lock discipline
Prove cancellation before handoff acknowledgment reaches exact stage owner; cancel vs host dequeue has one winner; once Running, report cooperative interruption not rollback; host Unknown prevents blind dispatch. No two pending queues both own the same transition. Quiesce/reopen fences stale tokens and no duplicate worker starts; provider/model calls never under REL/PHY owner locks.

### B4 — canonical source and compatibility audit
Reject new legacy active REL/PHY reads and duplicated workers. Test new paths through public gateway/owner runtime seam. Provide D complete old-only module manifest and C host capability conformance report. Do not delete old host while Warlock still calls it.

## Test and handoff gates

Run focused REL and PHY worker tests, personal-publication/journal restart tests, host router/generation/fault tests, Freshness and independent-owner lifecycle tests, P2/P3/P4 guards, fmt/check, affected integration and docs checks. Report exact Native/Host/Held matrix; do not call a capability fully supported because an enum contains it.

## Out of scope

Inference algorithm/redesign, PHY/REL schema migration, network protocol, external Warlock app code (C), legacy module deletion (D), speculative throughput tuning (P6).

## Documentation and architecture impact

Update implemented capability/worker ownership truth in architecture, api, maintainer map and limitations. No new store framework, queue owner or compatibility proxy permitted.

## Related docs

- [Parallel execution](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [Phase 2 personal publication](multiplexing-phase2-implementation-plan-2026-10-04.md)
- [Phase 0B lifecycle](multiplexing-phase0b-lifecycle-consumers.md)

## Notes

Capability declarations and fake-host tests do not prove a real external stage conforms.

