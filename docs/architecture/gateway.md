# Gateway and host lifecycle

Parent index: [Reliquary architecture](./INDEX.md)

## Purpose

Describe runtime owner mounting, principal-scoped access, worker lifecycle, durable conversation operations, and the integration contract with a trusted external host.

## Overview

`reliquary-runtime-gateway` is the runtime boundary between callers/hosts and product workflows. It mounts owner sessions, binds authenticated principals to owner grants, creates scoped `Instance` handles, starts and supervises owner workers, and coordinates queue, generation, retrieval, processing, and delivery operations. An `Instance` carries immutable grant/binding context; mutable store handles remain inside Gateway-owned workers and owner-session.

Gateway does not own physical/semantic store encoding, Application publication policy, provider credentials or model selection, or a deployed service process. The external integration is a Rust trait contract consumed by a host application; Reliquary does not claim to implement the host transport or process deployment.

## State and lifecycle

Mount admission checks owner identity and REL/PHY kind before registering an owner. Principal grants scope Instance methods, including the read-only `has_active_vectors` summary used by hosts to bind non-secret metadata before inference-backed writes. Callers cannot reuse an Instance's prepared retrieval or generation subscription across another owner, principal, stream, or Gateway binding. Worker queues serialize mutations for each mounted owner and shut down with the Gateway lifecycle.

A leased REL writer can restore a verified owner-session checkpoint during the mount. Gateway gives the owner session an opportunity to refresh derived state at its serialized writer boundary. The existing principal, owner, and cut checks continue to govern reads from preserved history.

A direct Memory summary read uses the mounted REL or PHY covered by the Instance's read grant. Application checks the requested cut, and Gateway rechecks the credential before returning the summary.

For Community recompute, Gateway captures owned input under the REL read and releases the lock for calculation. Apply reacquires the mounted writer and rechecks the credential before the owner session validates the captured versions and commits.

An Entity identity comparison can start from automatic discovery or an explicitly selected pair. Root requires `ManageRel` on the recording REL and separate read access to the peer. Gateway rechecks both owner grants against the active Instance and captures both mounted sessions. Evaluation runs outside the owner locks. Before publication it rechecks credentials, mount identity, captured cuts and the expected predecessor; only the recording REL writer appends the receipt. Inspection also requires both read grants and classifies the selected receipt against the observed evidence.

The identity worker admits pairs of mounted RELs only when one live Instance can manage a writable recording owner and read the other owner. It builds an in-memory index from pages of 32 Entities per owner and scans eight local index rows at a time. An index is reused for a particular mount and evidence cut. Changing either invalidates the index and subsequent work rechecks both versions. Eligible owner pairs take turns. Automatic publication prefers the lower owner ID when both directions are writable and checks both directions for an existing receipt under the same two owner locks. This also prevents a foreground comparison in the other direction from being duplicated by discovery.

Core automatically revisits recorded pairs in bounded batches while an authenticated Instance can manage the recording REL and read the peer. The grants must belong to the same active Instance. Reopening the workspace under those grants resumes reconsideration from the stored receipts. Foreground and automatic comparisons use the same Gateway inference binding and publication checks. Replacing the binding invalidates an in-flight response from the old endpoint. A missing endpoint leaves model-dependent work pending. The scheduler backs off after transport failures and waits for a change before resubmitting evidence that produced an invalid judgment.

The trusted host's separation control binds a five-minute review to the authenticated operator session, the directional pair, both mounts, and the expected separation head. Confirm reviews also bind the currently applicable evaluator receipt, route binding, and exact endpoint revisions. Undo reviews bind the current confirmation and can proceed after an endpoint retires. The runtime rechecks both REL grants under the credential publication fence while the pair mounts are pinned and their writer locks are held in owner order. Commit revalidates the review without invoking inference and appends only to the recording REL. Authorized retries return the original durable action. Inspection keeps the evaluator and operator decisions visible together and applies a confirmation only when both endpoints remain active and their retained histories establish continuity. Both read grants cover the receipts, event history, and operator audit details.

The Archive embedding operation admits an explicitly named staged import under the mounted REL operator grant.
Gateway prepares the Archive cut and publishes through the same owner writer as ordinary Archive embedding.
Provider calls run outside the writer lock. Publication rechecks the grant, open import epoch, and Archive cut.

Archive vector search captures the mounted REL's active generation, packed matrix, and fragment mapping under a single authorized read. The retained view can finish scoring after the writer moves to a newer generation. Hybrid search uses active Archive profiles for query embedding selection and captures lexical evidence with the matrix. Inference and ranking run outside the owner writer lock. Archive index publication captures from an authorized writer, prepares the record outside the lock, then obtains the writer again to recheck authorization and publish. Gateway checks the REL grant and credential again before returning search hits from the captured source cut.

Agentic temporal search binds its window to the query generation and includes it in the prepared delivery manifest. Recovery restores it with the associated frontier. If a newer query is active, recovery reconciles confirmed delivery IDs into that query and preserves its current window.

For a copied Hermes range, Gateway checks the authenticated REL grant and the copy's principal before reading source receipts, Archive rows and attached Echo. The read uses the requested Archive version and exact ancestry. The Root chat controller also checks the current inspection cut and records which references the host disclosed in the turn.

Conversation submission state lives in a Gateway-owned append/replay journal, separate from REL semantic storage. The flow captures a submission, arbitrates finalize/cancel, commits the accepted turn through the mounted REL writer, and records an Archive receipt. A host handoff has a stable operation id and payload digest. Gateway syncs a handoff intent before calling the host; after an ambiguous result or restart, it queries host status and reconciles instead of blindly issuing the operation again. Generation delivery/checkpoints and memory-delivery intents are similarly tracked with stable identities and bounded replay surfaces.

A trusted path mount prepares the owner state under its file lease and checks the requested kind and any expected ID before publishing it. The Root registry and the runtime mount update together after preparation. A quiesced reopen checks the registered owner on the actual new handle.

Trusted host control releases a read-only PHY under the same mount guard as quiescence. Callers drain owner operations and update their admission grants at the host boundary.

Gateway coordinates REL dependency edits with mount changes under one topology guard. It validates each proposed reachable graph and resolves context in dependency-first order with an explicit read grant at each traversed owner. The prepared context carries weak mount bindings, the declaration generations and synthesis cuts. Gateway rechecks those bindings on delivery and finalization. Token counting and inference run after the locks are released.

Authentication with `ManageRel` admits Perception work for that owner's writable REL. The lifecycle takes one owner-worker lease per poll and binds the credential, mount and configured routes. It performs provider work outside the writer lock and validates the same binding before publication. A progress record retains separate cursors for routine discovery, older samples and evidence that settles between audits. Pending retrieval and new delivery check exact current Observation revisions. Confirmed delivery recovery reads its earlier accepted manifest and provenance. [Managed Perception worker](../../crates/reliquary-runtime-gateway/src/instance_perception.rs)

## External host contracts

The public `TrustedHostAdapter` contract supplies configured capabilities, authenticated `connect`, declared per-capability queue guarantees, reconciliation, stable-id handoff/status/cancel, and optional confirmed memory delivery/status. `HostQueueGuarantees::complete` requires durable acceptance, stable submission IDs, linearizable cancel/dequeue, queryable operation status, and idempotent handoff before Gateway treats the capability as executable. Host-reported capability declarations are not proof that an adapter honors them; production host conformance requires independent integration verification.

Typed retrieval delivery distinguishes a newly completed exact operation from a replay. Observation-only delivery confirms consumption through the trusted host and advances the turn frontier without a Memory-use receipt. Mixed delivery records the selected Memory use once through the REL writer and retains the selected Observations in the same turn frontier.

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

- [Reliquary source and binary distribution terms](../../README.md#license)
- [Application workflows](application.md)
- [Owner sessions](owner-session.md)
- [Switchboard](switchboard.md)

## Notes

The adapter trait defines obligations for an integration. It cannot establish that an external host implementation satisfies them.
