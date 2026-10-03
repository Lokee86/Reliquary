# Phase 0B design: REL update coverage and authorization

Parent index: [Documentation index](INDEX.md)  
Implementation plan: [Multiplexing implementation plan](multiplexing-implementation-plan.md)  
Readiness gates: [Multiplexing readiness review](multiplexing-readiness-review.md)  
Target authority: [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)  
Source inventory: [Phase 0 baseline](multiplexing-phase0-baseline.md)

## Purpose

Resolve Phase 0B gates G3 and G4 with a source-backed mutation/event coverage matrix, ordered feed and reconnection contracts, an enforceable grant model, revocation semantics, and a std-only adversarial contract fixture. This document specifies target behavior. It does not claim that the gateway, feed, grant registry, or fixture is production implementation or proof.

## Overview

The first in-process gateway uses one sequencer per mounted REL. Every instance operation is authorized against its current attachment and grant generation. Every committed event is ordered after its authoritative mutation and before the sequencer admits the next mutation. A REL update feed contains only REL-owned summaries, statuses, cursors, and invalidation keys. It never carries PHY-private content. Conversation stream data has a separate subscription and cursor, available only to authorized instances that selected that conversation.

The initial access policy is **REL-wide**: a principal with `ViewRel` may read every conversation and conversation-level REL state in that REL. Conversation ACLs are not assumed. Controlling a generation, submitting, cancelling another principal's work, or changing project state requires separate grants. A REL dependency contributes ambient context only when the instance independently has a grant to that dependency.

## Source evidence and present boundary

The current host is not an authorization boundary for independent principals. `runtime_host_semantic_access.rs::semantic_owner` resolves any mounted REL by explicit owner ID and the single active PHY; `visible_semantic_owners` derives ambient visibility from one host-global active REL dependency closure plus that PHY. These reachability rules are valid for the present single-principal host, but mounting is not an instance grant.

Mutation and synchronization are distributed across distinct seams:

- `runtime_inference/interaction_runtime.rs::accept_turn` calls `ingest_turn_with_receipt`, then synchronizes before returning its receipt. Episode scheduling and finalization use separate mutation paths and synchronization.
- `archive/interaction_stream_persistence.rs` writes a stream checkpoint and synchronizes it separately from a completed turn. A stream checkpoint has no Archive semantic version.
- `runtime_host_interaction.rs` separately mutates and syncs conversation title/activity metadata. Session selection/close also changes durable active metadata through `runtime_host_session.rs`.
- Public `Cva` facade methods expose append-node/branch, conversation metadata, memory publication, graph relations, project history, vector publication, and other owner mutations. Some stores update derived retrieval/index state as a side effect; invalidation cannot be inferred from the Archive version alone.
- `runtime_host_insomnia.rs::process_claim` crosses an REL/PHY boundary after extraction and preparation. Worker status and partial cross-owner publication are not one Archive transaction.
- `archive/interaction_stream_record.rs` persists stream message, parent, role, timestamp, content and status, but not principal attribution. Current interaction append/recovery can consult live in-flight state; a checkpoint alone is not durable initiator/actor authority.

These facts mean feed correctness must be established at the gateway's owner mutation seams. Watching only one clock or only completed conversation turns misses durable metadata, clock-neutral stream checkpoints, non-Archive owner changes, and operational status transitions.

## G3: authoritative event and mutation coverage

### Event envelope and ordering

Each REL execution owns a single `RelSequencer` with a mount epoch, checked monotonically increasing `u64` cursor, owner revision watermarks, attached authorized subscriber queues, and an admission state. An epoch is a fresh opaque identity for each authoritative mount lifetime; it is never reused after an unmount, replacement, reopen that loses continuity, or cursor exhaustion.

An event envelope contains:
`{ rel_id, mount_epoch, cursor, class, durable_watermarks, invalidation_keys, visibility }`.
The cursor orders notifications only within one epoch. It is not a semantic clock. Existing source-visible watermarks include the container/global, Archive, Memory, Entity, Graph, Ego mutation, vector, and project-correlation versions/sequences where those APIs expose them. Conversation checkpoints and operation cursors are independent of Archive semantic versions. A durable queue/worker revision is a proposed typed status watermark where no current owner clock exists; do not imply that such a clock is already implemented. A missing watermark means that class has no corresponding durable clock. Do not synthesize a durable version for transient delivery state.

Publication follows one linearized path:

1. Verify the mount epoch, current authorization token, expected owner revision, and any expected branch head.
2. Perform the authoritative owner mutation through the gateway-owned REL execution or its typed REL/PHY operation coordinator.
3. Complete that operation's existing durability requirement (including sync where its owner contract requires it).
4. While retaining the REL sequencer's write/admission lock, allocate the next cursor and enqueue the event to every currently authorized REL-feed subscriber. Event payloads are summaries/status/invalidation identifiers, not copied mutable owner records.
5. Release the sequencer. A client acknowledgement or successful socket write is not the mutation's commit point.

Every gateway-writable path must either use this path or be classified as non-user-visible maintenance whose completion advances mount epoch and forces resynchronization. Phase 1 must remove/contain direct mutable handles that let instance-facing callers bypass this seam. Low-level public owner facades may remain useful to trusted standalone/maintenance clients, but they cannot be treated as gateway-authenticated instance APIs.

The event queue is bounded. If adding an event would exceed capacity, replace the pending ordinary queue with one latched `ResyncRequired { epoch, latest_cursor }` marker. Do not append ordinary events while latched. The consumer must atomically obtain a current authorized snapshot and cursor, discard older queued state, and acknowledge the marker before incremental events resume. Overflow is observable and never means “drop one event and continue.”

### REL-wide event coverage matrix

| Authoritative mutation or transition | Feed class and recipient | Durability/watermark before publication | Payload policy |
|---|---|---|---|
| Turn/node ingestion, branch creation, turn attachment, file-to-memory link, conversation title/activity metadata | `ArchiveChanged`; every connected authorized instance with `ViewRel` | Archive/global version after the current Archive sync boundary | conversation/branch IDs and changed-field summary; no transcript bodies in broad update |
| Streaming checkpoint, interruption, durable generation completion, generation terminal status | `ConversationOperationChanged`; REL-feed all viewers receive status only; text/checkpoint detail goes to active subscribers for that exact conversation+operation | checkpoint sync completes before any checkpoint is visible; operation/checkpoint cursor is independent of Archive clock | broad feed carries operation ID/status and conversation ID; stream feed carries checkpoint sequence and content only after authorization |
| Memory, Entity, relationship/graph, community, Ego, or other semantic owner mutation inside REL | `SemanticOwnerChanged` with owner-local class/watermark | respective owner write committed; publish invalidation after any required sync | stable IDs, type and watermarks; no private PHY projection |
| Retrieval/lexical/vector generation or index rebuild/invalidation affecting REL-visible queries | `RetrievalInvalidated` | publish after source owner mutation; derived index may rebuild asynchronously | affected index/profile/version keys; derived state is never the semantic authority |
| Episode creation, Insomnia claim/status/retry/receipt/completion, Dream/Perception/vector worker status or owner work becoming visible to REL clients | `ProcessingStatusChanged` | durable receipt/queue transition synchronized if persisted; otherwise label explicitly ephemeral and recover it from authoritative status snapshot | job/episode IDs, state, progress and safe error summary; exclude prompts, PHY drafts and private inference payloads |
| Native provisional submission enqueue/order/cancel/finalization; delegated host queue attach/handoff/cancel/reconcile | `SubmissionQueueChanged` / `HostWorkChanged` | native enqueue/cancel journal durable before acknowledgement; delegated state is acknowledged by the authoritative host before reporting transition | stable submission/operation ID, status, order token and authority side; do not expose private PHY body or unrelated owner data |
| REL project revision/correlation, repository attachment/binding or project-management policy change | `ProjectStateChanged`; require `ViewRel` to observe and `ManageProject` to mutate | Project Environment operation and REL correlation finish their declared commit boundary | repository/revision identifiers and invalidation; paths/content are returned only by separately authorized project APIs |
| Owner mount, unmount, recovery, reconnect/reopen, host capability reconciliation, gateway degraded/healthy transition | `OwnerLifecycleChanged` or epoch transition; current authorized subscribers only | reconcile authoritative state first; rotate epoch whenever cursor continuity is not provable | availability, epoch, watermarks, pending/in-flight operation status; never revive cancelled work |
| PHY memory/profile/Ego/graph/processing change | **No REL-wide event containing the change**; notify only the principal's separately authorized private channel | PHY owner commit and private grant check | REL may receive a safe principal-keyed invalidation only when a REL-owned projection actually changes; never publish the private PHY payload |

Mutation families must be enumerated against the current facade and runtime-host call graph during Phase 1 implementation. A newly added instance-visible mutator has no implicit feed coverage: it must add a row to this matrix and an adversarial coverage test before it can be exposed through GatewayRuntime.

### Atomic snapshot and cursor handshake

`subscribe_rel(instance, rel, known_cursor?)` runs under the same sequencer lock used by mutation publication:

- Revalidate attachment, principal, live grants and owner mount epoch.
- Build an authorized snapshot covering REL-wide state, all conversation/branch heads, per-submission queue status (stable ID, state, and gateway order only), current safe operation statuses, and typed watermarks. Queue text/body is never part of a broad feed or snapshot; any submitter-specific draft read uses a separate, explicitly authorized operation.
- Capture `(mount_epoch, cursor_watermark)` and register the subscriber before releasing the lock.
- Return the snapshot plus watermark. Every subsequently admitted event has the same epoch and a cursor strictly greater than the returned watermark.

The snapshot itself must not expose generation transcript text unless the instance separately joins that conversation stream. A caller may request replay from `known_cursor` only when the same epoch is live, the cursor is retained without a gap, and its current grant generation matches. Reconnect always revalidates and obtains a fresh authoritative snapshot; a cursor is only a possible bandwidth optimization and never authority.

Conversation stream join is a different atomic handshake. Under the conversation-operation coordinator it validates `ViewRel`, captures durable transcript/checkpoint plus operation identity/status and stream cursor, then registers the subscriber before releasing the coordinator. Checkpoints are delivered strictly after successful persistence. Stream identity includes REL epoch, conversation ID, operation ID/generation and stream cursor. Joining is observation only; it grants no cancellation right.

### Instance and REL reconnection

- **Instance disconnect/reconnect:** discard transport-bound subscriber handles. On reconnect, validate the principal binding and current grant generation again; obtain a new atomic snapshot and watermark for each authorized REL, including branch heads, queue/cancellation state and operation status; then rejoin only the instance's selected authorized conversation streams using checkpoint-plus-cursor. Never apply a stale client cursor to a different grant generation.
- **REL disconnect/remount/reopen:** stop new admissions, reconcile owner state and pending/in-flight work, create a new epoch when feed continuity is lost, invalidate old snapshots/subscription tokens, then issue snapshots to authorized instances. A prior-epoch cursor is unusable. A disconnected instance does not cancel shared work.
- **Host reconnect:** initiate configured host connections immediately and in parallel with restoring Reliquary-owned durable pending state. Hold only work that crosses that host boundary until local restore and host capability/operation reconciliation both complete. Exchange stable operation IDs, reconcile cancellation before replay, and establish one dispatch authority before dispatch. Unknown host status is held for reconciliation; no blind redispatch. Host-unavailable status is visible in the safe REL operation snapshot where relevant, while independent native work can continue.
- **Revocation:** is an authorization event, not a reconnect. Revoke before revalidating subscriptions; a revoked instance receives no snapshot or further feed/stream content.

### G3 adversarial exit tests

The fixture in `tests/multiplexing_phase0b_events_auth_contract.rs` models (it does not implement) the contracts below with `rustc --edition=2024 --test`:

1. A publication racing with two subscribe-and-snapshot callers is either included in the snapshot watermark or delivered once at a later cursor, never absent from both.
2. Stream checkpoint cursor and Archive revision remain distinct; a checkpoint is invisible before its durable checkpoint flag.
3. Queue overflow yields a latched resynchronization marker; no stale incremental event follows until snapshot acknowledgement.
4. A new mount epoch rejects old cursors; instance reconnect takes a fresh snapshot even when a cursor is offered.
5. Inactive-conversation changes appear in REL feed summaries, while stream text is emitted only to authorized active conversation subscribers.
6. The update envelope contains only REL-owned identifiers/status/invalidation keys, never private PHY content.

## G4: inspectable authorization and revocation

### Trusted identity, attachment and grants

The transport/host supplies a trusted `PrincipalId`; caller-provided PHY IDs and arbitrary principal strings are not credentials. Gateway resolves the principal to its canonical `phy-<uuid>` identity and attaches that PHY from the mounted PHY registry. An instance attachment has a non-reusable `InstanceId`, an attachment incarnation, a bound principal/PHY, and a monotonically increasing grant generation. Sharing one PHY across devices for the same principal points to the one authoritative PHY execution; another principal cannot select it by ID.

Each owner grant is explicit and capability-scoped:

| Capability | Meaning |
|---|---|
| `ViewRel` | Read all conversation histories and REL-owned state in one REL, including inactive conversations and authorized provenance/Echo/project metadata |
| `SubmitConversation` | Submit against a selected REL conversation through the gateway writer/queue |
| `CancelOwnPendingSubmission` | Cancel only this principal's still-pending submission before finalization |
| `CancelSharedGeneration` | Request cooperative cancellation of an already-dispatched shared generation; grant is separate from stream/view permission |
| `ManageProject` | Change project/repository state and attachments on the REL |
| `ManageRel` | Perform explicitly privileged REL lifecycle/metadata operations |
| `UseOwnPhy` | Read/write the PHY resolved from the trusted principal binding; cannot name another PHY |

The default REL ACL is owner/host configured and REL-wide. Conversation-level ACLs are not supported in this design; no per-conversation filtering is promised. Read-only viewers may see the same REL-wide conversation metadata and safe operation statuses. Detailed streaming output requires `ViewRel` plus an active stream selection. `SubmitConversation`, `CancelOwnPendingSubmission`, `CancelSharedGeneration`, and `ManageProject` are independently checked. A stream subscription never implies submission or cancellation authority.

REL dependencies do not grant authorization. Ambient context is the active REL dependency closure intersected with the instance's explicit `ViewRel` grants, plus its bound PHY only for a capability explicitly permitted to use personal context. Explicit owner-qualified calls check that exact owner's capability even when the owner is mounted or appears in the dependency closure. Provenance/Echo and repository-backed reads authorize both the target REL and every referenced owner/content source before returning it. Internal maintenance runs through a distinct trusted maintenance capability, never through an instance attachment.

An authorization token is an unforgeable gateway-issued tuple:
`{ instance_id, attachment_incarnation, principal_id, grant_generation, rel_mount_epoch }`.
It is scoped to operation/resource and checked at request admission, again immediately before worker dispatch or external handoff, at owner commit/finalization, and before every outbound feed/stream delivery. For PHY-bound work, include the resolved PHY owner ID and verify it still equals the instance's trusted binding. Queue admission alone does not preserve future authorization.

### Revocation state transition

Revocation is serialized through the Gateway authority gate and takes effect at a defined linearization point:

1. Close lease admission for the affected grant/attachment under the authority gate. Existing bounded leases may finish; no new dispatch, owner-stage commit or outbound-enqueue lease can start.
2. Drain the already-issued bounded leases. Owner locks are acquired only after a lease is acquired; release each owner lock and lease before moving to another owner. Inference runs without an authority or owner lock. An external operation already dispatched is cancellation-requested but is not falsely claimed to have stopped.
3. Advance the grant generation, mark the grant revoked, remove REL/conversation subscribers, clear queued payloads, and replace the attachment feed with a terminal `Revoked` result containing no owner snapshot or private data. The outbound-enqueue lease makes a racing delivery either linearize before this clear or fail after admission closes.
4. Cancel that principal's queued personal-context jobs before dispatch. For already-dispatched work, suppress later publication if its current lease cannot be acquired.
5. Cross-owner processing uses the exact principal/PHY route intent established by its domain owner. A stage acquires a short owner-operation lease, commits under that owner's lock, then releases the lease before acquiring another owner lock. If revocation fences a later stage, preserve any already committed stage, keep the durable route intent bound to its original destination, and mark it held/reconciliation-required in its authoritative work store. Retry requires current authorization or explicit privileged repair; never redirect the output to another PHY. This design does not impose a REL-first or PHY-first commit order.
6. Do not roll back data already durably committed, and do not claim that content already seen by a client can be withdrawn. A revoked host operation that cannot be stopped remains visibly in-flight/reconciliation-required but its result cannot be delivered or attributed through the stale instance.

Attach, grant, revoke, detach, and remount are serialized through one Gateway authority gate. Each short instance-facing dispatch, owner commit, cross-owner stage, or outbound-enqueue step atomically acquires a lease only if the attachment incarnation, principal, grant generation, REL mount epoch and admission state still match. The lease protects that bounded step until it releases; it does not hold the authority gate while taking an owner lock, and no lease spans inference. Revocation/quiescence closes lease admission under the gate, drains already-issued bounded leases, then increments the grant generation or mount epoch and clears affected outbound queues. Thus a commit already admitted under a lease linearizes before revocation; no new dispatch, commit or delivery lease can begin after closure. No nested authority/REL/PHY locks are required. A stale attachment incarnation or token from an earlier REL epoch is rejected even if its IDs are otherwise valid. A newly granted attachment receives a new token and snapshot; it does not inherit queued private work admitted under another principal.

### G4 adversarial exit tests

The fixture checks:

- A principal cannot choose another PHY, or read it by naming its owner ID.
- A mounted sibling/dependency REL is denied until explicitly granted, and dependency closure does not imply permission.
- View permission does not authorize submission, generation cancellation or project mutation.
- Conversation-level ACLs are not silently implied: a granted REL viewer sees inactive conversation summaries, while non-granted principals see no payload.
- A token is rejected after grant-generation change, detach, or REL epoch change.
- Revocation blocks further dispatch/commit/delivery checks and removes feed access; stream subscription does not preserve privilege.
- REL feed events contain no PHY-private body fields.
- Stale cross-owner publication is suppressed after revocation and surfaced as reconciliation-required, not silently redirected to another PHY.

## Integration and readiness boundary

This design closes the **specification** for G3 and G4. Before either gate is marked implemented, Phase 1 must:

1. Route every instance-visible mutation and subscription through the sequencer and grant checks; inventory the actual mutation call graph against the matrix.
2. Add unit tests at real Archive, Memory, Graph, project, worker, queue and stream publication seams; add concurrency tests around publication and subscribe/reconnect.
3. Ensure no public host operation accepts a caller-selected principal/PHY as its authority.
4. Recheck grant tokens at dispatch, commit and outbound delivery paths, including handoffs to a connected host.
5. Run the full established project verification and the documentation structural/change-impact checks required by `AGENTS.md`.

This document and its std-only fixture do not satisfy or claim those implementation checks. G1, G2, G5 and G6 remain independent gates; broad Phase 1 work remains blocked until the root integration review confirms all Phase 0B gates and the baseline requirements.

## Related docs

- [Readiness review](multiplexing-readiness-review.md)
- [Implementation plan](multiplexing-implementation-plan.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
- [Phase 0 baseline](multiplexing-phase0-baseline.md)

## Notes

The event classes and capabilities here are target contracts for GatewayRuntime. Current `ReliquaryRuntimeHost` methods, public Cva/Phylactery facades, raw checkpoint records and process-local locks do not provide these contracts on their own. Reconcile this draft with the remaining G1/G2/G5 decisions and the root integration before treating Phase 0B or implementation as ready.
