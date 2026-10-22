# Runtime multiplexing

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe the implemented Gateway/Instance runtime ownership model, REL/PHY sharing, durable conversation execution, authorization, streaming, and host capability boundary.

## Overview

One Gateway authority owns mounted Reliquary resources. Independently authorized Instance runtimes attach principals and grants to that shared Gateway rather than creating a second mutable runtime per session.

The runtime separates durable owner work from viewer/session attachment. REL/PHY workers are owner-scoped and shared where authorization permits; an instance is an authorization/context view, not another semantic owner.

## Gateway and instance ownership

`GatewayRuntime` owns mounted owner admission, worker identity, durable scheduling, capability negotiation, and owner-level mutation/read routing.

`InstanceRuntime` owns authenticated principal/grant context and selected access. Detaching or revoking an instance cannot silently retarget already accepted work to another owner.

Same-principal instances may share the same authorized PHY worker. Cross-principal attachment is rejected.

## Durable conversation work

Conversation policy and accepted turn history remain Archive-owned. Pending/running work is durable and restart-aware. Gateway does not create a second host queue merely because a provider is external.

Accepted work binds stable owner/conversation/branch/submission identity. Retries and reconnect must not duplicate provider dispatch or durable publication.

## Host execution capability

External provider execution is negotiated through the host adapter/capability boundary. The provider host does not become the REL state owner.

Unknown/ambiguous remote outcomes are held for reconciliation; they are not blindly replayed.

## Streaming and visibility

Generation text is staged behind durable checkpoint synchronization. Authorized viewers receive text only after the corresponding durable checkpoint boundary.

Late join, slow readers, resync, epoch rotation, detach/rejoin, and revocation use server-authoritative state rather than client-side reconstruction. PHY-private payload is not broadcast through broad REL snapshots.

## Authorization

Every operation is evaluated against the current instance/principal grants and owner identity. Authentication and authorization are revalidated at sensitive async boundaries.

Ambient mutable global selection is not semantic authority.

## External sources

Reference-only host-owned transcript integration composes through the external-source core rather than copying canonical host transcripts into Archive.

## Failure and recovery

- revoked/stale instance tokens fail closed;
- owner remount/epoch changes fence stale handles;
- unknown remote provider outcome cannot be replayed as known failure;
- pending-vs-running cancellation respects the negotiated host lifecycle;
- stream overflow/resync returns authoritative prefixes/checkpoints; and
- shutdown joins owner workers without transferring ownership to instances.

## Code Map

- `src/runtime_inference/runtime_host_gateway*`
- `src/runtime_inference/runtime_host_instances.rs`
- `src/runtime_inference/interaction_live*`
- `src/runtime_inference/runtime_host_gateway_conversation*`
- `src/runtime_inference/runtime_host_gateway_stream*`
- `src/runtime_inference/runtime_host*.rs`
- `src/runtime_inference/runtime_shared.rs`
- `src/runtime_inference/interaction_*.rs`
- `src/runtime_inference/multiplexing_journal*.rs`
- `src/runtime_inference/workspace_management*.rs`

## Related docs

- [Architecture](../architecture.md)
- [External sources](external-sources.md)
- [Active multiplexing release plan](../multiplexing-implementation-plan.md)
- [ADR 0038](../decisions/0038-gateway-instance-runtime-multiplexing.md)
- [Behavioral contracts](../behavioral-contracts.md)

## Notes

Current Reliquary Gateway/Instance behavior is documented here. The active plan now tracks only remaining cross-repository Warlock/Hermes adoption and release acceptance.
