# Gateway and host lifecycle

Parent index: [Reliquary architecture](./INDEX.md)

## Purpose

Describe runtime owner mounting, principal-scoped access, worker lifecycle, durable conversation operations, and the integration contract with a trusted external host.

## Overview

`reliquary-runtime-gateway` is the runtime boundary between callers/hosts and product workflows. It mounts owner sessions, binds authenticated principals to owner grants, creates scoped `Instance` handles, starts and supervises owner workers, and coordinates queue, generation, retrieval, processing, and delivery operations. An `Instance` carries immutable grant/binding context; mutable store handles remain inside Gateway-owned workers and owner-session.

Gateway does not own physical/semantic store encoding, Application publication policy, provider credentials or model selection, or a deployed service process. The external integration is a Rust trait contract consumed by a host application; Reliquary does not claim to implement the host transport or process deployment.

## State and lifecycle

Mount admission checks owner identity and REL/PHY kind before registering an owner. Principal grants scope Instance methods, including the read-only `has_active_vectors` summary used by hosts to bind non-secret metadata before inference-backed writes. Callers cannot reuse an Instance's prepared retrieval or generation subscription across another owner, principal, stream, or Gateway binding. Worker queues serialize mutations for each mounted owner and shut down with the Gateway lifecycle.

A direct Memory summary read uses the mounted REL or PHY covered by the Instance's read grant. Application checks the requested cut, and Gateway rechecks the credential before returning the summary.

For Community recompute, Gateway captures owned input under the REL read and releases the lock for calculation. Apply reacquires the mounted writer and rechecks the credential before the owner session validates the captured versions and commits.

Conversation submission state lives in a Gateway-owned append/replay journal, separate from REL semantic storage. The flow captures a submission, arbitrates finalize/cancel, commits the accepted turn through the mounted REL writer, and records an Archive receipt. A host handoff has a stable operation id and payload digest. Gateway syncs a handoff intent before calling the host; after an ambiguous result or restart, it queries host status and reconciles instead of blindly issuing the operation again. Generation delivery/checkpoints and memory-delivery intents are similarly tracked with stable identities and bounded replay surfaces.

Trusted host control releases a read-only PHY under the same mount guard as quiescence. Callers drain owner operations and update their admission grants at the host boundary.

Gateway coordinates REL dependency edits with mount changes under one topology guard. It validates each proposed reachable graph and resolves context in dependency-first order with an explicit read grant at each traversed owner. The prepared context carries weak mount bindings, the declaration generations and synthesis cuts. Gateway rechecks those bindings on delivery and finalization. Token counting and inference run after the locks are released.

## External host contracts

The public `TrustedHostAdapter` contract supplies configured capabilities, authenticated `connect`, declared per-capability queue guarantees, reconciliation, stable-id handoff/status/cancel, and optional confirmed memory delivery/status. `HostQueueGuarantees::complete` requires durable acceptance, stable submission IDs, linearizable cancel/dequeue, queryable operation status, and idempotent handoff before Gateway treats the capability as executable. Host-reported capability declarations are not proof that an adapter honors them; production host conformance requires independent integration verification.

These are library integration contracts, not wire protocol/deployment documentation. Exact public signatures are re-exported through [`src/lib.rs`](../../src/lib.rs) and [`crates/reliquary-runtime-gateway/src/lib.rs`](../../crates/reliquary-runtime-gateway/src/lib.rs); the CLI and operator guides own user-facing invocation details.

## Failure, recovery, and invariants

Authorization is checked at the Instance boundary and again for sensitive operations such as owner mutation, queue cancellation, and external memory delivery. Stale prepared retrieval is rejected against current owner state. Unknown host status leaves an operation held for later reconciliation; it is not interpreted as success or failure. Queue and stream journals replay bounded local state after restart; external recipient replay requires a fresh authorized snapshot. A real host's durability, status, idempotency, and cancellation behavior is outside the proof boundary of Gateway unit tests.

## Code map and tests

- Mounts and grants: [`crates/reliquary-runtime-gateway/src/gateway_mounts.rs`](../../crates/reliquary-runtime-gateway/src/gateway_mounts.rs), `instance_owner.rs`, and `instance_management.rs`.
- Durable queue: [`crates/reliquary-runtime-gateway/src/conversation_queue.rs`](../../crates/reliquary-runtime-gateway/src/conversation_queue.rs); host calls/reconciliation: [`host_runtime.rs`](../../crates/reliquary-runtime-gateway/src/host_runtime.rs); lifecycle/capabilities: `host_lifecycle.rs`; stream replay: [`generation_stream.rs`](../../crates/reliquary-runtime-gateway/src/generation_stream.rs).
- Tests exercise stable host ids, uncertain handoff/reconcile, cross-Gateway router rejection, queue reopen, stale retrieval rejection, principal/grant checks, and bounded generation replay in these modules and `crates/reliquary-runtime-gateway/src/tests.rs`.
- External host-facing public surface: [`crates/reliquary-runtime-gateway/src/lib.rs`](../../crates/reliquary-runtime-gateway/src/lib.rs) and the retained consumer integration test at [`tests/external-consumer`](../../tests/external-consumer).

## Extension seams

Add owner/domain semantics to Application or the relevant semantic package, not to Gateway. Add external capability behavior behind `TrustedHostAdapter` and make stable idempotency/reconciliation requirements explicit. Keep all mutable store operations in Gateway workers and owner-session; expand public host contracts only when a concrete host integration requires the capability.

## Related docs

- [Application workflows](application.md)
- [Owner sessions](owner-session.md)
- [Switchboard](switchboard.md)

## Notes

The adapter trait defines obligations for an integration. It cannot establish that an external host implementation satisfies them.
