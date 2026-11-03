# ADR 0046: One Gateway runtime authority and named application workflows

Parent index: [Architectural decisions](INDEX.md)

**Status: Selected runtime/orchestration architecture — 2026-11-03; implementation remains deferred until the complete architecture, naming migration and move-once plan are accepted.**

Full design: [Stage 3 runtime and orchestration architecture](../runtime-orchestration-architecture-2026-11-03.md). Existing source disposition: [runtime crosswalk](../runtime-orchestration-source-crosswalk-2026-11-03.md). Earlier behavior: [ADR 0038](0038-gateway-instance-runtime-multiplexing.md). Storage and service authority: [ADR 0044](0044-canonical-rel-phy-owner-session-architecture.md), [ADR 0045](0045-deep-semantic-services-and-effect-contracts.md).

## Purpose

Define the canonical execution and host boundary, cross-service workflow authority, concurrency/locking, durable operational coordination and recovery without relocating the existing Root dependency knot unchanged.

## Context

The real Gateway already runs in `src/runtime_inference/runtime_host_gateway*`, not in the placeholder Gateway crate. `GatewayRuntime` holds shared REL/PHY executions, attachments, route/capability state, operational journals, stream distribution and lease gates. `RelShared` currently holds `Arc<Mutex<InteractionRuntime>>` which itself owns `Cva`; PHY uses `Arc<Mutex<Phylactery>>`. These are the real cross-layer dependencies blocking a clean Rust crate DAG. The program also has durable native provisional work, host operation/status and owner-operation journals, which are distinct from REL semantic state.

## Decision

1. **One Gateway authority** per embedding process, with one mounted owner execution/worker set per exact REL/PHY ID and many authenticated Instances. Gateway owns mounts, lifetime, admission, revocation, cancellation, worker scheduling, stream distribution, host capability reconciliation and **its own operational journals**. It is not an inference engine, a general semantic database or an untyped writer.
2. **Instance is an authorized view**, not a second owner. It carries principal, attached PHY, grant/incarnation generations and user selection. Gateway-only mutators stay unforgeable after a crate cut; a public `Arc<InstanceRuntime>` does not grant permission to rewrite grants, detach arbitrary attachments or bypass current authorization.
3. **Canonical Stage 1 owner sessions:** Gateway holds the only mounted mutable `RelSession`/`PhySession` handles. It does not store or expose a duplicate `Cva`/PHY bag. `InteractionRuntime` responsibilities split between the owner session and ephemeral conversation/session execution.
4. **Named workflows, no general workflow engine:** a concrete coordinator obtains authorized immutable proof, invokes bounded semantic evaluation outside long owner locks, revalidates grants/owner/source revisions, and commits through typed owner-session commands. No lower dependency on Gateway, no `with_stores_mut` escape hatch, no universal event bus.
5. **Operational vs semantic durability:** Pending conversation submissions, host phase/generation/checkpoint results, cancellation/finalize receipts and owner-operation attribution are Gateway/operation authority. Archive turns, Insomnia work, Dream first-cycle/Freshness events, Memory/Graph/Ego are per-owner semantic authority. Operational journals cannot independently create Archive history.
6. **Exactly one queue authority per stage:** preserve native REL provisional queue and owner-local durable semantic work. A host can own pending stage admission only under complete durable queue/cancel/dequeue/status/idempotency guarantees; otherwise native Gateway remains authoritative. Execution-only delegation retains native admission. Unknown remote outcome is Held until reconciliation, never blindly redispatched.
7. **Commit/fault ordering:** Durable pending enqueue precedes acknowledgment. Cancellation and finalization serialize at the one stage authority; a winning cancellation produces no Archive turn. Durable finalization intent precedes accepted Archive commit, followed by exact post-commit journal receipt. Crash between stages recovers from immutable IDs and Archive evidence without duplication. Cross-REL/PHY writes are separately committed and idempotently reconciled, never described as atomic across files.
8. **Short owner lock:** no model, host I/O, broad multi-owner locks, or slow-client distribution under semantic owner mutation lock. After inference, reauthenticate, recheck generation, lease and expected data cut before committing. Mount and instance fences remain distinct; physical multi-process writer claims cannot be replaced by in-process quiescence.
9. **Checkpoint visibility:** committed/synced prefix before generation text delivery; REL-wide owner update feed separate from selected conversation stream. Reconnect uses authoritative snapshot plus watermark and reauthentication. Revoked/old mount/cursor tokens fail closed, and PHY content is not broadcast on a shared REL feed.
10. **Recovery/admission before work:** reopen typed owner and exact operational journals, reconcile Archive/host/owner-effect receipts, rebuild ephemeral schedules and only then start workers and admit new writes. Invalid complete frames, unknown host results and unavailable owner proofs remain held/fenced, not silently repaired.
11. **Extensibility:** Future Observation/Ego workers add bounded workflow invocations over existing owner sessions; they do not require a new universal Gateway state field or duplicate source of truth. Model providers integrate via existing host/endpoint contracts and real adapters.

## Alternatives considered

- **Move current Root `GatewayRuntime` plus `InteractionRuntime` and `Cva` wholesale:** rejected; merely transports the existing dependency knot and broad mutability into a lower crate.
- **One worker/owner/queue per Instance:** rejected; creates inconsistent shared conversation ordering, duplicate PHY semantics, and revocation/conflict races.
- **One universal workflow/event bus:** rejected; hides exactly-once and authority transitions in a framework that does not earn its interface complexity.
- **One Gateway, typed owner sessions, named workflow authorities and scoped operational journals:** selected. Preserves existing behavior and makes new services a bounded addition.

## Consequences and required proof

- The old 2026-10-30 G0–G5 Gateway extraction specification is **partial implementation guidance only**, not a final architectural acceptance gate; the complete Gateway must account for conversation, host, instance, owner-operation, stream, lifecycle, worker, admission and recovery families.
- Host/controller trust at a new Rust crate boundary requires an unforgeable Gateway-held instance controller. Former `pub(super)` mutators cannot become freely public merely for call-site compilation.
- Exact queue/journal formats, stable IDs, host capability negotiation, synthetic test interleavings, grant revocation and stream snapshots are preservation contracts. No new protocol or host capability is designed by this ADR.
- Final physically enforceable modules/crates and allowed imports are a Stage 4 decision. CVA→REL naming and wire conversion are separately governed by [ADR 0043](0043-rel-terminology-and-wire-identity.md). Stage 6 must map all call sites, verify integration and compare throughput/latency.
- No implementation code or corpus was changed by accepting this architectural decision.

## Related docs

- [Full Stage 3 runtime architecture](../runtime-orchestration-architecture-2026-11-03.md)
- [Runtime source crosswalk](../runtime-orchestration-source-crosswalk-2026-11-03.md)
- [Current runtime multiplexing](../subsystems/runtime-multiplexing.md)
- [ADR 0038](0038-gateway-instance-runtime-multiplexing.md)
- [ADR 0044](0044-canonical-rel-phy-owner-session-architecture.md)
- [ADR 0045](0045-deep-semantic-services-and-effect-contracts.md)

## Notes

This selects **logical architecture**, not a completed Gateway crate or verified Warlock/Hermes production integration. Later physical package design can refine type placement but cannot silently reintroduce duplicate mutable owner authority or two competing queues.
