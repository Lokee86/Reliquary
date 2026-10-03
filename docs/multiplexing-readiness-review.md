# Multiplexing readiness review: pre-implementation gates

Parent index: [Documentation index](INDEX.md)
Related plan: [Multiplexing implementation plan](multiplexing-implementation-plan.md)
Baseline: [Multiplexing Phase 0 baseline](multiplexing-phase0-baseline.md)
Target authority: [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)

## Purpose

Prevent implementation against an architectural outline that has not resolved the hard multi-instance, multi-principal and owner-lifecycle interactions. This is a targeted source-backed design review, not a claim that the defects are currently reachable in the existing single-PHY host.

## Overview

**Readiness verdict, 2026-10-02: architectural direction agreed; hard-cut implementation NOT READY.** Keep the GatewayRuntime/InstanceRuntime split, existing owner-local REL executions, one authoritative PHY execution per owner, REL-wide authorized updates and conversation-only active streams. Resolve the gates below before handing a Phase 1 patch plan to an implementation agent. A harmless type sketch or isolated fixture does not constitute approval for behavioral extraction.

The first plan placed conversation coordination in Phase 3, after instance managed-session extraction in Phase 1. Source inspection shows that order is unsafe. The plan also underestimated the semantic impact of routing user-owned Insomnia results in a shared multi-principal REL and treated REL-wide update distribution as a simpler layer than its required mutation integration.

## Blocking source-backed findings

### G1 — Distinguish conversation, branch, open session, selected conversation and generation

**Evidence:** `interaction_session.rs` uses `InteractionRuntime.sessions: HashMap<session_id, SessionState>` and `open_session` rejects an already-open ID. `runtime_host_session.rs` owns one `ManagedSessionState` per REL, calls `close_managed_session_for` when switching, and finalizes the session/changes durable active metadata on close. Moving only the managed selector into an instance does not allow two instances to open the same conversation and may let instance A terminate instance B's shared operation.

**Required design:** state the durable conversation/branch identities versus per-instance view/selection and one shared gateway-owned execution per active continuation; specify how multiple viewers attach without opening duplicate writable sessions, and what finalization, inactivity and stream ownership mean under simultaneous viewing. User-facing policy agreed: default sequential chronological queue for pending messages (a randomly chosen, once-fixed order for genuine ties), with clearly visible automatic branching as an option. Define the exact branch/writer admission, verified timestamp and pending-order contract before extracting managed sessions; queued submissions are cancellable provisional state **outside canonical conversation history**, and must only become Archive turns when individually finalized. Gateway restart survival is required: define a separate durable pending-submission journal for native-mode work, stable IDs, durable enqueue acknowledgment, persistent cancellation and restart reconstruction in the chosen pending order. Immediately initiate configured host connection/reconciliation at startup before work crossing that integration boundary resumes. Retain native machinery required by Reliquary; evaluate Warlock and other hosts by actual capabilities and integrate only overlapping stages where conflicting authority would arise. Prefer the host queue for host-executed work if it guarantees effective pending cancellation and status reconciliation, because preventing cancelled work from executing is the central concern. Host-owned and Reliquary-owned queues may coexist for different stages only with stable ID-based handoff, cancellation propagation, and authoritative cancellation/dequeue serialization; never duplicate authority for the same transition or Archive commitment. The journal is not conversation history. Specify idempotent reconciliation if Archive commit succeeds but journal finalization has not been recorded; define how cancellation races finalization and how queued messages interact with a running generation. These mechanics remain implementation gates. Existing `conversation_active` is durable Boolean metadata but active viewers are transient and many-to-many: choose separate semantics and preserve historical file compatibility.

**Pass criteria:** state-transition table for create/attach/switch/reopen/enqueue/cancel/finalize/generate/complete/interrupt/detach/reconnect/last-viewer/gateway-loss, including exact leaf ancestry and Episode scheduling. Tests must prove queued or canceled submissions never appear as conversation turns or advance activity, queue-status updates are distinct from committed-message updates, queued cancellation is acknowledged to every relevant connected instance, cancellation/finalization races have exactly one outcome, idempotent cancellation/duplicate submission does not republish history, and one viewer's switch/detach does not close another's shared continuation.

### G2 — Define principal attribution and multi-principal background publication

**Evidence:** `runtime_host_interaction.rs::begin_message` takes the host's sole PHY ID. `interaction_stream.rs::begin_message_with_principal` may copy an agent's `principal_id` from the parent turn. `insomnia/ownership.rs` classifies `User` versus `Project` only, and `runtime_host_insomnia.rs::process_claim` publishes every prepared user-owned draft into whichever single PHY is currently active. `runtime_host_owner_execution.rs` constructs REL-owned workers sharing that mutable PHY slot; Dream, vector and Perception worker paths also gate their user work on `phylactery_active`. Additionally, `InteractionStreamRecord` does not persist a `principal_id`: `InteractionRuntime::append_stream_records` can project the actor from matching live in-flight state, but cannot reconstruct it from the checkpoint alone after a restart.

**Required design:** define `actor_principal` for durable source authorship, explicit generation initiator/authorization, and separately resolved *destination PHY owner* for each personal synthesis output. Parent message identity must never automatically grant another user's identity. Define the durable checkpoint/operation attribution needed for safe late join, replay and recovery without silently changing established on-disk compatibility. For mixed-principal episodes, owner routing must be provenance-supported and deterministic; if attribution is ambiguous, do not publish to an arbitrary user's PHY. Document partial-failure and idempotent retry across REL receipt and potentially several PHY writes: no cross-file atomicity is currently guaranteed. Treat this as a cross-owner processing-contract change, not merely moving worker threads.

**Pass criteria:** fixtures with two principals and shared conversation turns, source-anchored personal facts for each, ambiguous personal facts, REL-only facts, endpoint/retry failures between PHY and REL publication, and multi-instance shared-PHY de-duplication. No resulting fact may enter another user's PHY.

### G3 — Prove ordered update-feed coverage at authoritative mutation boundaries

**Evidence:** `InteractionRuntime::accept_turn` synchronizes after ingestion; `interaction_stream_persistence.rs` synchronizes a streaming checkpoint separately; many other runtime_host paths mutate Archive metadata, Memories, Graph, project revision state, derived indexes and background completion through distinct seams. A check of one Archive/version counter cannot observe clock-neutral stream checkpoints or all derived/operational status changes.

**Required design:** enumerate event classes (including provisional native/delegated queue status, cancellation, host attachment/reconnection and external operation reconciliation), and specify each class's authoritative source, durability requirement, ordering, relevant owner-local watermark and recipient set. Establish gateway-owned post-commit publication for *every class a connected instance must know about*, not merely chat commits. A broad REL update feed reaches every connected authorized instance accessing the REL, regardless of current conversation; carry only authorized REL-owned state or invalidation identifiers, never arbitrary private PHY contents. Streaming text and detailed generation events reach only active authorized conversation subscribers, after successful durable checkpoint. Define atomic subscribe/snapshot/cursor handshakes and bounded overflow/restart recovery. **Both disconnected instances and disconnected/remounted RELs require resynchronization on reconnection:** reauthorize instances; reconcile REL durable state and advance mount epoch on lost continuity; restore authorized REL/branch/queue/operation status; selectively reattach generation streams at checkpoint/cursor. Use incremental replay only when epoch/cursor continuity is proven; otherwise atomic authoritative snapshot/watermark must prevent misses while subscribing. Distinguish durable semantic versions from ephemeral feed sequence and clock-neutral stream checkpoints.

**Pass criteria:** a mutation/event coverage matrix and adversarial tests for concurrent subscription transition, mutation while opening an inactive conversation, worker publication, missed events, late stream join, slow subscriber overflow, instance reconnect, REL disconnect/remount and stale-epoch invalidation, missing/expired cursor, concurrent mutation during resubscription, restart and revocation. Zero silent stale-cache conditions.

### G4 — Make authorization a real, inspectable capability and revocation model

**Evidence:** `runtime_host_semantic_access.rs::semantic_owner` permits addressing any mounted REL explicitly and checks the single PHY slot; ambient `visible_semantic_owners` follows one host-global active REL closure. These are valid host-only reachability semantics but not a per-principal security boundary.

**Required design:** trusted mapping of principal to PHY, instance attachment identity and REL grants; distinguish viewing a REL from controlling a shared generation or mutating its project state. Decide whether access is REL-wide (including all REL conversations) or conversation-level ACL exists; update payload granularity must match that choice. Dependencies do not transitively grant otherwise unauthorized owners. Define revocation effects on active reads, streams, pending commands and queued personal-context work. Public direct owner-qualified APIs need trusted internal/maintenance classification or authenticated context; mounting is never a user permission grant.

**Pass criteria:** cross-principal negative tests for inactive conversations, provenance/Echo, dependency owners, PHY retrieval, update payloads, live stream join, generation cancellation, stale attachments and post-revocation operations.

### G5 — Quiescence, cross-owner locking and single-writer scope

**Evidence:** `runtime_host_reconciliation.rs::reconcile_rel_file` independently opens/compares/promotes by pathname; it does not first fence mounted owner operations. REL operations use `Arc<Mutex<InteractionRuntime>>`, while Insomnia currently acquires REL and PHY locks during cross-owner publication. `Cva::open` and `InteractionRuntime` are public lower-level interfaces, and another process can bypass an in-process gateway.

**Required design:** define whether a gateway can only claim authority over resources explicitly handed to it and what happens on conflicting external file access; provide mount identity/canonical path fencing and explicit offline maintenance policy. Do not advertise cross-process protection without a proven lock. Establish owner-lock ordering or non-nested staged commits, bounded inference lock intervals and cross-owner idempotent recovery. Quiesce must stop admission, drain/cancel/wait on active operations per defined policy, stop or fence owner workers, reconcile/promote, reopen, advance mount epoch, publish invalidations, and then re-enable access. A stale instance token cannot write into replaced state.

**Pass criteria:** tests for two mounted aliases of one owner, quiesce with in-flight generation/Insomnia and subscribing instances, partial cross-owner publication, concurrent unrelated REL activity, restart and stale token rejection.

### G6 — External consumer and verification baseline

**Evidence:** `ReliquaryRuntimeHost` and `InteractionRuntime` are publicly exported, and current test/support code and repository integrations call them directly. Source audit covered this Reliquary checkout; it did **not** comprehensively inspect all downstream Warlock/ACP/agent callers. The newly added Phase 0 characterization test asserts two *different* conversations in one REL, not the shared-conversation requirement. Cargo tests were blocked by another process's build-directory lock; documentation structural/change-impact checks had unrelated findings in `docs/insomnia-explicit-memory-commit-plan.md`.

**Required design:** enumerate actual downstream imports and operational deployment paths before choosing a public API break; define in-process versus future cross-process singleton authority and any real external compatibility boundaries. Establish an isolated test build output directory or an agreed window after the other Cargo job ends; freeze existing behavior, then add *failing target-behavior tests* against the final interface prior to broad migration. Record the pre-existing failure baseline and documentation findings, not assumptions of full greenness.

**Pass criteria:** external-consumer impact list identifying Warlock's and other frameworks' actual capabilities and a per-stage authority/delegation matrix, startup gateway/host handshake and idempotent cross-stage handoff; initial compile/test baseline, per-phase contract tests and whole-repository/CLI/host integration gate. Restart and reconnect tests must assert immediate startup connection attempt, cancellations reconciled before replay, host queue preference where supported, legitimate parallel stage queues without conflicting authority, cancellation versus host dequeue and cross-stage handoff races, no cancelled pending item dispatched, no duplicate execution/Archive finalization, honest already-running interruption outcomes and safe handling of unreachable/unknown host state.

## Dependency correction

The safe sequence is:

1. **Phase 0B: design hardening (required before a broad hard cut).** Resolve G1-G6 into a concrete ownership/type diagram, state machines, event matrix, authorization matrix, worker and mutation ownership matrix, lock/quiescence design, external-consumer impact manifest and test oracle. Use the agreed sequential-queue default and optional visible automatic branching, and resolve their durable queue, timestamp, operation and branch state machines before Phase 1.
2. **Phase 1A: foundational authority.** Create final gateway-owned keyed resource/attachment identities, trustworthy instance context, authorization and owner lifecycle. Do not migrate writable shared-conversation selection to duplicate `open_session` objects.
3. **Phase 1B: minimal conversation coordinator prerequisite.** Establish the shared conversation state/attachment and writer admission contract *before* moving managed selection, then route instance-local view/selection through it. The broader conflict-policy implementation and UI can expand later, but core correctness cannot.
4. **Phase 2: personal-owner isolation** only after multi-principal Insomnia and background publication contracts are proven; tests must include shared REL/mixed principals, not merely multiple PHY mounts.
5. **Further phases:** comprehensive REL write-concurrency policy, mutation-integrated REL broadcasts and subscriber-only streams, consumer hard cut, recovery/load testing and measured optimization. Event publications must be added at the correct ownership boundary as the mutation paths are migrated, not retrofitted from a blind global version poll at the end.

This review identifies **known blockers**, not every downstream defect. It is deliberately stricter than the earlier eight-phase overview.

## Phase 0B follow-up (2026-10-03)

The [integrated design package](multiplexing-phase0b-design.md) supplies concrete G1–G6 mechanisms, mutation/consumer matrices and executable reference models. These settle the design proposals for review; they do not prove production enforcement. The broad hard-cut verdict remains **NOT READY** while recorded production baseline and downstream compile gates are red or incomplete. Use the package's verification record for executed outcomes rather than the earlier unrun baseline.

## Related docs

- [Multiplexing implementation plan](multiplexing-implementation-plan.md)
- [Multiplexing Phase 0 baseline](multiplexing-phase0-baseline.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
- [Current architecture](architecture.md)

## Notes

The user-facing policy is set: sequential queue by default with visible automatic branching as an option. Queued submissions remain provisional and cancellable; they become canonical conversation history only at finalization. The implementation still must specify the exact queue, recovery, branch and cancellation state machine. It must be implemented as genuine gateway coordination, not an instance-global lock or a second REL session object. No Phase 0B gate is satisfied solely by writing prose; state-machine proofs and executable multi-instance fixtures must support the final Phase 1 handoff.
