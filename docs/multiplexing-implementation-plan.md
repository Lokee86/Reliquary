# Reliquary multiplexing implementation plan

Parent index: [Documentation index](INDEX.md)
Architectural decision: [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
Current-source audit: [Phase 0 baseline](multiplexing-phase0-baseline.md)

## Purpose

Specify the GatewayRuntime/InstanceRuntime migration across Reliquary, including shared REL/PHY execution, instance-specific context, conversation synchronization, correctness gates and consumer migration.

## Overview

Status: Phase 1 is complete locally on feat/multiplexing-phase1 after the completed Phase 0B design package. The [Phase 1 finish record](multiplexing-phase1-finish-plan-2026-10-04.md) documents F1–F6 ownership/recovery, accepted-work lifetime, routing, lifecycle/fencing, authorization, final serial verification and packaging. The [earlier remaining-work plan](multiplexing-phase1-remaining-work-2026-10-03.md) remains the historical audit. This accepts the in-process authority foundation; Phase 2 multi-principal PHY ownership, exact-owner publication and private access are now verified locally; see the [Phase 2 finish record](multiplexing-phase2-implementation-plan-2026-10-04.md). Phase 3 conversation coordination is accepted locally; event-feed convergence and consumer/other-capability migration remain Phase 4–5. It is an independent whole-stack project, not an Ego workstream.

## Problem statement

ReliquaryRuntimeHost combines long-lived shared resource authority with one operating instance's active REL, attached PHY, managed session and background-worker activation. It mounts multiple RELs but cannot safely host independent concurrent users/devices/agents. It has no broad REL update feed, no explicit active-conversation stream subscriptions, and no branch-level write-coordination interface for several instances viewing the same conversation.

The goal is **one authoritative GatewayRuntime with many InstanceRuntimes**, not many processes or stores running a single-user runtime independently. Retain existing REL/PHY semantic owners, multi-REL infrastructure, per-owner workers, Archive persistence and model/provider boundaries.

## Final architecture and contracts

### Lifetime and ownership

```text
GatewayRuntime (long-lived, one authority for each mounted durable owner)
├── rel_executions: REL ID -> REL execution / Archive / per-REL workers
├── phy_executions: PHY ID -> PHY execution / personal processing
├── instances: instance ID -> authorized InstanceRuntime context
├── rel_update_feeds: REL ID -> attached authorized instance subscribers
└── conversations: (REL ID, conversation/branch) -> operation coordination
    └── generation streams -> only active authorized subscribers

InstanceRuntime (short-lived attachment)
├── verified principal / one attached PHY
├── one active REL + authorized access to other RELs
├── managed conversation selection per REL
└── attached REL update feed(s), active conversation stream subscription(s)
```

Gateway owns physical owner handles, operation authority, resource shutdown and real mutation. Instance stores only its own context, attachment references and observed event cursors. REL owns conversations; PHY never owns a conversation. More than one instance may share a PHY and/or REL but may not duplicate their authoritative execution.

### Interfaces to implement

Names are illustrative Rust-level contracts; Phase 1 defines exact types without introducing an intermediate forwarding host.

- Gateway lifecycle: mount/unmount REL or PHY with duplicate-owner rejection; create authenticated instance attachment; revoke/drop attachment; query authorized owner topology; quiesce/reconcile owner; gateway shutdown. On startup, immediately initiate configured host reconnection and reconcile queue/operation state before host-dependent dispatch.
- Instance context: current principal/PHY, select REL, select or create/reopen managed conversation, read authorized ambient/explicit owner data, acquire/release conversation stream subscription; access to PHY is bound at attachment, never chosen through untrusted request parameters.
- REL update feed: attach to selected/authorized REL and obtain an atomic snapshot plus mount epoch/cursor; deliver **all relevant post-commit REL changes** to every connected instance accessing that REL, including inactive conversations, index changes and status. No conversation-level update filtering. **Every disconnected instance and REL must resynchronize on reconnection**, not only on restart or a detected event gap: revalidate instance grants, restore the authorized REL snapshot/current branch/queue/operation status, then reattach active streams using checkpoint-plus-cursor. On REL remount/lost continuity, reconcile owner durable state and advance mount epoch, invalidating affected instance caches. Resume incremental delivery only if epoch/cursor continuity is provable; otherwise atomic authoritative snapshot-and-watermark must close the reconnect race.
- Conversation live stream: explicitly attach only when the conversation is active for that instance; return authorized durable checkpoint and current operation cursor, then deliver future synchronized deltas and terminal events for that specific generation; detach without terminating the underlying generation.
- Native runtime / host integration: retain Reliquary's native queue, coordination, cancellation, recovery and synchronization for its own processing and correctness. Integrate **per capability**, prioritizing **host queue ownership for host-executed work where the host supports effective pending cancellation and status/reconciliation**. Avoid a second Reliquary queue competing for the same host dispatch; keep native machinery where needed for Reliquary correctness or the host's cancellation/reconciliation contract is insufficient. Separate host and Reliquary stage queues may coexist with explicit ownership handoff, cancellation propagation and a cancellation check immediately before every dispatch. Map who owns admission, pending order, dispatch, cancellation, generation and completion for each integration, using stable IDs to prevent cancelled work being replayed or duplicate execution/Archive commitment. Evaluate Warlock's actual capabilities instead of assuming an integration mode. Immediately attempt host connection at startup, exchange capabilities and reconcile cross-boundary pending/in-flight/committed work before resuming that boundary; preserve unrelated native processing. Hold unknown external operation status for reconciliation rather than duplicate execution; reconcile cancellation before replay and retry unavailable hosts automatically. For already-running external operations, distinguish cooperative interruption from the strict guarantee that cancelled *pending* work does not execute.
- Conversation command: accept idempotent submissions with expected current leaf/revision and enforce the conversation's gateway-coordinated policy (**default: sequential chronological queue; option: visible automatic branching**). Queue entries are provisional, independently cancellable, and entirely outside canonical conversation history. A submission becomes an Archive turn only when its finalization/commit wins the queue-item state transition; cancellation while queued must eliminate it without a turn, Archive activity, Episode, Insomnia work or branch. Reliquary-admitted native queued work **must survive gateway restarts** through a separately durable pending-submission journal (not Archive history); acknowledge native enqueue only after journal durability. Host-owned upstream queued work remains under its host's persistence/recovery contract and is reconciled at handoff. Stable submission IDs, persisted cancellation and finalization reconciliation against Archive must prevent resurrection or duplicate commits after a crash. Reserve or join one authorized generation, checkpoint output durably before publishing, and complete/cancel with authorization. Queued messages arriving during an in-flight generation join at the next safe continuation, never retroactively change the generation context. Preserve the final committed order and original Archive-local parent identity when branching; determine verified timestamp/tie handling and the pending journal's concrete storage, idempotent replay and cancel-versus-finalize fencing in Phase 0B.
- Owner-local work: REL owner queue/scheduler independent of any client selection; PHY owner work de-duplicated by PHY ID; personal-context work captures an explicit immutable principal/visibility context.
- Explicit authorization: every instance-facing operation enforces grants, including explicit owner calls, REL dependencies, historical Echo/provenance and subscriptions. Internal maintenance does not implicitly become user access.

No new persistent event log is required for the first in-process version: an ephemeral per-mount ordered event sequence plus authoritative revision/checkpoint reconciliation is sufficient. Distinguish Archive semantic versions, clock-neutral streaming checkpoints and ephemeral notification sequence; do not conflate them.

### Invariants

1. Exactly one authoritative live execution per mounted durable REL or PHY. No instance-owned clone of a mutable store.
2. One bound PHY and one active REL per instance; independent per-instance managed selection, even for instances using the same PHY and REL.
3. REL-wide changes are broadcast to every connected authorized instance accessing that REL; live generation output is restricted to active subscribers of that conversation.
4. The durability rule remains: no visible committed streaming delta before its checkpoint has synchronized.
5. An instance joins a stream without restarting/stealing its operation; viewers do not acquire cancellation authority by subscribing.
6. REL dependency closure intersects instance authorization; one PHY's private state is invisible to another principal despite shared REL, shared Entity identities or background work.
7. Every final conversation mutation has explicit expected branch parent/revision; stale writes cannot silently overwrite a newer head. Enqueued submissions are cancellable provisional state, not conversation history, and cancellation racing finalization has one atomic, observable outcome.
8. All owner-local semantic invariants, durable IDs, transaction clocks, Archive history and project repository boundaries survive unchanged.
9. A client disappearing never implies the shared REL, PHY, other sessions or owner workers should shut down.
10. One REL's slow work or lock must not serialize unrelated RELs; inference holds no long-lived owner-store locks.

## Implementation decisions and migration strategy

Use a **staged internal hard cut**. Establish canonical target owners and interfaces first; move responsibilities once; migrate callers directly; delete obsolete ambient source-of-truth fields/APIs when their callers have moved. Do not maintain synchronized old/new runtime state or add permanent compatibility shims. The Rust public and external consumer compatibility obligations must be checked before a breaking surface is removed; retain interoperability only at a demonstrably required external boundary.

The old ReliquaryRuntimeHost is a source of behavior to migrate, **not** a permanent facade around GatewayRuntime. No new generalized physical storage framework, model provider redesign, standalone daemon requirement, semantic Ego change or REL/PHY format migration is part of this plan.

## Phases and actionable steps

### Phase 0 — Baseline and specification (this document set)

- **0.1** Inspect the current ownership, routing, session, PHY, worker, retrieval, Archive and external-consumer boundaries. Record source/test inventory and known active worktree changes in the [baseline](multiplexing-phase0-baseline.md).
- **0.2** Record target authority and privacy model in [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md); clarify that ADR 0037's single active REL/PHY is current behavior superseded for the final target, while ADR 0034's PHY privacy invariant becomes instance-relative.
- **0.3** Establish the characterization matrix from existing tests. Run existing targeted tests without modifying concurrent unrelated implementation; record results and missing tests.
- **0.4** Specify integration order, failure/recovery semantics, tests and unresolved policy gates. Validate documentation index/structural checks.
- **Exit:** target boundaries, audit and testing matrix are explicit; no runtime implementation committed.

### Phase 0B — Required design hardening before implementation

Resolve the six source-backed blocking gates in [the readiness review](multiplexing-readiness-review.md): shared conversation execution/attachment, multi-principal Insomnia routing and recovery, mutation coverage for REL events, enforceable authorization/revocation, quiescence and lock/single-writer scope, and actual downstream consumer/baseline verification. Produce state machines, matrices, concrete interface contracts and adversarial fixture definitions, not just diagrams. **Gate: no broad implementation until these are settled and baseline tests actually run.**

### Phase 0B integration record (2026-10-03)

The [Phase 0B design package](multiplexing-phase0b-design.md) now specifies the six gates with five executable reference fixtures. Review its cross-contract rules and verification record before implementation. Production migration remains pending; existing baseline and consumer build failures keep the broad hard-cut gate closed. The canonical dependency order is authority foundation, coordinator, then instance selector routing.

### Phase 1 — Gateway/instance authority extraction

- **1.1** Define exact public/internal GatewayRuntime and InstanceRuntime ownership contracts and errors, including native queue semantics, per-capability host integration/authority negotiation, explicit stage handoffs and immediate startup reconnect/reconciliation hook. Gateway retains the existing keyed REL executions, routes and policy; introduce keyed PHY execution/instance attachment shapes at their **final owner**, even if first implementation supports a narrow test configuration.
- **1.2** Establish a minimal gateway-owned conversation coordinator with shared writable continuation, multiple read/view attachments, explicit writer admission, gateway-owned durable provisional queue for Reliquary-admitted work (available to Warlock where appropriate), serialized cancel-versus-finalize and correct detach/finalization; this is **a prerequisite**, not deferred Phase 3 machinery. Keep existing interaction/session primitives beneath it, but do not create duplicate writable `open_session` objects for the same conversation.
- **1.2a** Only after 1.2, relocate active REL selection, per-instance managed conversation selection and reopen-pending state from host/execution singletons into instance context. Keep explicit owner operations as gateway internals.
- **1.3** Make all user-facing ambient semantic operations resolve through the initiating instance; no hidden active_execution() path may determine an operation's target in the completed architecture.
- **1.4** Convert instance lifecycle into attach/detach independent of shared REL/PHY lifecycle. Establish an explicit quiesce/fence interface for global owner maintenance.
- **Tests:** two instance selections cannot affect each other; instance detach never unmounts an owner; same-instance existing reopen/active-conversation behavior holds; explicit owner operations cannot bypass instance authorization.
- **Gate:** only one authoritative selection source (instance); no synchronized legacy active REL, active PHY, or managed-session shadow state. If Phase 1 is not yet multi-PHY capable, do not advertise general multi-user operation before Phase 2's gate.

### Phase 1 implementation and verification record (2026-10-03; finish scope updated 2026-10-04)

**Status: Phase 1 complete locally, 2026-10-04.** GatewayRuntime/InstanceRuntime provides trusted identity/path admission, process-local gateway/legacy owner exclusion, REL-wide durable provisional conversation work, shared continuation, instance-authorized ambient/project APIs, lifecycle fencing and ConversationExecution host-stage ownership. Accepted work outlives its attachment; client cancellation/delivery retains current grants. Reopen failure is closed/stopped and external-work races reauthorize before commit/delivery.

Final-source verification passed: gateway 69/69, journal 15/15, instances 7/7, legacy isolation 3/3, public gateway API and baseline contracts 1/1 each, routing guard 2/2, library 862/862, every integration target, CLI formatting/locked check and 19/19 tests, and both documentation checks. The unrestricted Cargo suite retains only the inherited archive_open_profile/lore_base allocator conflict. Refreshed architecture indexing timed out after 180 seconds; current source guards passed and no refreshed Pitlord pass is claimed. See the [final execution record](multiplexing-phase1-finish-plan-2026-10-04.md) for command-level evidence and packaging.

The [Phase 1 finish plan](multiplexing-phase1-finish-plan-2026-10-04.md) is now the completion record. Production host transfer for Insomnia, Dream, Embedding and ProjectEnvironment, real Warlock conformance and legacy-host deletion remain Phase 5; Phase 1 integrates ConversationExecution ownership only and does not synthesize host assistant results.

### Phase 2 — PHY registry, private access and work ownership

**Status: R2.1–R2.7 complete and verified; committed locally on feat/multiplexing-phase2.** Exact PHY workers, durable User/explicit Agent attribution, frozen multi-principal publication and withdrawal, restart recovery, private retrieval/profile isolation, pending-work lifecycle and the expanded hard-cut guards pass production acceptance and the final library/integration/CLI matrix. The completed R2.7 gate in the [Phase 2 implementation plan](multiplexing-phase2-implementation-plan-2026-10-04.md) owns broad regression and commit evidence.

- **2.1** Deepen the existing gateway PHY registry into one execution/worker owner per durable PHY; same-principal instances share it and different principals cannot alias it.
- **2.2** Remove Phylactery, personal profile and personal worker state from REL `OwnerExecution`; split vectors/Dream/Perception into REL-local and PHY-local execution.
- **2.3** Persist instance-derived actor/generation attribution so restart and shared conversations never infer a principal from ambient selection or parent ancestry.
- **2.4** Implement G2 frozen multi-principal Episode routing and crash recovery: exact principal→PHY destinations, idempotent external writes, REL semantic receipt, no ambiguous personal fallback.
- **2.5** Migrate private semantic/retrieval access to instance-authorized owner sets: granted REL closure plus only the bound PHY.
- **2.6** Make principal profile projections and worker wakeups explicitly owner/principal keyed.
- **2.7** Add PHY quiesce/fencing/recovery equivalent to its new execution responsibility.
- **2.8** Hard-cut remaining ambient personal ownership from the new gateway path; legacy external host deletion remains Phase 5.
- **2.9** Run final multi-principal privacy, recovery, worker-deduplication, library/integration/CLI/docs verification and package the Phase 2 branch.
- **Gate:** two principals safely share a REL while personal writes/retrieval/workers remain isolated; same-principal instances share exactly one PHY execution; no background job consults mutable global active PHY.

### Phases 3 and 4 — parallel implementation after Phase 2

**Execution model:** after Phase 2 reaches its acceptance gate, freeze the final P3/P4 observation/event seam once, then fork Phase 3 and Phase 4 from the same integration base. Phase 3 owns conversation queue/branch/generation semantics. Phase 4 owns REL sequencer/subscriber/resync infrastructure. They proceed concurrently; only the production conversation-stream adapter and final conversation event hooks wait for convergence.

See:
- [Phase 3 conversation coordination implementation plan](multiplexing-phase3-conversation-implementation-plan-2026-10-04.md)
- [Phase 4 REL distribution implementation plan](multiplexing-phase4-distribution-implementation-plan-2026-10-04.md)

### Phase 3 — REL conversation coordination

**Status: P3.0–P3.9 accepted locally; P4.0–P4.9 and production distribution binding are accepted with documented verification limits.** [P3.9 evidence](multiplexing-phase3-local-acceptance-2026-10-05.md) and [P4.9 evidence](multiplexing-phase4-acceptance-2026-10-05.md) record the lane gates. Final P3/P4 integration is verified for main ([integration evidence](multiplexing-p34-main-integration-2026-10-06.md)); Phase 5 consumer migration remains separate.

- Deepen the existing `ConversationRegistry` rather than creating another coordinator.
- Make the durable REL journal order the automatic scheduler authority.
- Persist and enforce `SequentialChronological` and `VisibleAutoBranch`.
- Move from one conversation-wide processing flag to branch-local continuation/generation ownership.
- Preserve durable cancel/finalize/Archive idempotency and accepted-work lifetime.
- Expose an authoritative conversation observation snapshot for Phase 4; do not own delivery cursors/subscribers.
- **Local gate:** queue/branch/generation/recovery semantics pass without requiring Phase 4 transport.
- **Convergence gate:** P4 consumes P3 post-durability observations and checkpoint state without moving subscriber authority into the coordinator.

### Phase 4 — REL distribution, stream subscription and resynchronization

**Status: P4.0–P4.9 implemented, audited and locally validated; P4.9 local/convergence functional acceptance passed with documented verification exceptions.**

- Add one `RelSequencer` per mounted REL with mount epoch, checked cursor, typed watermarks and bounded subscribers.
- Add atomic authorized snapshot+subscribe and explicit ResyncRequired overflow handling.
- Cover every instance-visible gateway mutation family with safe REL summaries/invalidation events.
- Add reconnect/remount/revoke behavior with epoch/grant revalidation.
- Bind the production conversation stream hub to P3's authoritative snapshot, receipt, and checkpoint sources; keep P4 subscriber/cursor ownership separate. P4.7 is implemented and verified; see the [convergence record](multiplexing-phase4-convergence-2026-10-05.md). P4.8 coverage/lock/privacy audit is complete; see the [audit record](multiplexing-phase4-audit-2026-10-05.md).
- Keep generation text out of broad REL feeds and private PHY content out of all REL-wide payloads.
- **Local gate:** sequencer/snapshot/overflow/reconnect and generic stream hub pass against deterministic providers.
- **Convergence gate:** inactive conversation summaries, late stream join, branch events and checkpoint delivery pass against final P3 semantics. P4.7 verification and P4.8 audit are complete; P4.9 local/convergence functional acceptance passed; see the [P4.9 acceptance record](multiplexing-phase4-acceptance-2026-10-05.md) for repository-wide verification exceptions.

### Phases 5 and 6 — concurrent consumer migration and verification

**Status: execution-ready parallel plan, implementation not started.** P3/P4 have converged onto main. Rather than serialize all of P5 followed by P6, run P5-A, P5-B, P5-C, and P6-H concurrently against one accepted planning base. A/B own separate Reliquary source areas, C owns the real Warlock/CLI consumers, and P6-H builds independent stress/host/fault/measurement harnesses. Root API integration and the final old-host deletion occur only when real consumers have migrated; P6-R is the post-deletion release gate.

Authoritative handoffs:
- [P5/P6 parallel execution and convergence plan](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [P5-A REL/runtime consumer migration](multiplexing-phase5a-rel-consumers-2026-10-06.md)
- [P5-B background workers and host capabilities](multiplexing-phase5b-background-host-2026-10-06.md)
- [P5-C Warlock and detachable CLI](multiplexing-phase5c-external-consumers-2026-10-06.md)
- [P5-D legacy-host deletion after caller convergence](multiplexing-phase5d-legacy-deletion-2026-10-06.md)
- [P6 concurrent verification harness and final release](multiplexing-phase6-parallel-verification-plan-2026-10-06.md)

### Phase 5 — full-stack consumer migration, parallel lanes then deletion

- **P5.0 shared API freeze:** map real consumers to existing gateway/instance operations; add only genuinely missing final interface contracts, not compatibility shims.
- **P5-A REL/runtime lane:** Archive, Memory, Entity/Graph, provenance, retrieval, project/repository, owner-qualified lifecycle/status and private PHY access.
- **P5-B background/host lane:** Dream, Insomnia, Perception, vectors, REL/PHY worker parity, stage capability negotiation, stable handoff/reconciliation and cancellation.
- **P5-C external lane:** Warlock workspace/chat/session/provider/stream migration and truthful dependency-pin upgrade; inspect CLI actual usage and migrate only real affected callers. Use a new clean Warlock worktree, preserving currently dirty graph/UI changes.
- **P5-D final cut:** after A/B/C acceptance, integrate root exports once, delete obsolete ReliquaryRuntimeHost and old-only helpers/characterization, replace old tests with gateway tests, update architecture/docs/ADR.
- **Exit:** one gateway-owned runtime with every legitimate consumer migrated, no ambient alternate owner and real upgraded Warlock/CLI acceptance.

### Phase 6 — harness in parallel; final release after P5-D

- **P6-H concurrently:** deterministic multi-instance topology, queue/branch/finalization crash faults, exact REL feed/stream reconnect and privacy, host queue and cancellation/reconciliation fakes, reproducible lock/fan-out/memory profiling.
- **P6-R after P5-D:** real post-deletion adversarial stress, measured optimization if needed, locked Rust/CLI/Warlock tests and builds, docs/source/architecture guards, current state review and final release evidence.
- **Exit:** validated cancellation and exactly-once local effects, safe multi-principal synchronization, no old host authority, measured scalability and explicit status for any still-blocked architecture refresh or platform gate.

## Decision register

| Topic | Resolution / gate |
| --- | --- |
| Gateway/instance split | Agreed and accepted in ADR 0038. |
| Instance PHY and REL selection | One verified PHY and one active REL per instance; independent managed conversation selection per REL. |
| Same PHY in many instances | Allowed; one authoritative gateway PHY execution and one personal processing owner. |
| Same REL in many instances | Allowed; owner execution shared; scope/visibility instance-authorized. |
| REL updates | Broadcast to every connected authorized instance accessing the REL, regardless of current conversation. |
| Active generation output | Deliver only to actively subscribed instances of that conversation, with late-join checkpoint catch-up. |
| Concurrent message policy | **Agreed:** default sequential queue ordered chronologically among pending submissions, randomly chosen, permanently fixed order for genuinely simultaneous messages; selectable automatic branching with clear visible branches. Phase 0B must settle verified clock/admission details, mandatory durable pending journal and idempotent restart recovery, atomic cancel-versus-finalize, in-flight generation behavior and preference storage. |
| Host integration | Preventing cancelled work from executing is paramount. Prefer the host's queue for host-executed work when pending cancellation and status reconciliation are reliable; preserve Reliquary's own machinery wherever its guarantees require it. Distinct stage queues may coexist with cancellation-safe handoff and a cancellation check before dispatch; one authority per transition. Evaluate Warlock's real capabilities. Startup connects promptly and reconciles cancellations before replay. |
| Global durable conversation_active | **Phase 3 storage/lifecycle investigation:** preserve current on-disk decoding, separate runtime presence, document any semantic change before writing. |
| Runtime event durability | Initially ephemeral ordered mount-local notifications plus authoritative snapshot/reconciliation; no new persistent log without measured need. |
| Cross-process gateway | Not required initially; establish an interface and lifecycle suitable for a future single-service authority. |

## Out of scope

Ego synthesis algorithms, Perception/Chronos/Dream semantic changes, new REL/PHY file formats, universal cross-owner Graph authority, distributed multi-writer storage, live cross-process lease arbitration, provider transport redesign, and automatic new UI features. Consumers may require interface migration without semantic redesign.

## Completion gates and documentation impact

The final repository must have one canonical gateway-owned REL/PHY execution path and one instance-local selection path, with obsolete internal host sources deleted. Existing persisted REL/PHY files must remain readable. Updated public behavior must have owner/instance contract tests and appropriate current-state docs; documentation checker and architecture policy must pass against the integration snapshot.

Work on this roadmap should not silently absorb or overwrite the separately modified REL activity-clock, Ego, Chronos, Archive or Perception changes present during the Phase 0 audit.

## Related docs

- [Multiplexing Phase 0 baseline](multiplexing-phase0-baseline.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
- [Architecture](architecture.md)
- [Roadmap](roadmap.md)

## Notes

Phase ordering is a dependency plan, not evidence that any proposed interface or feature currently exists. Both user-facing policies are selected; the Phase 0B gate is their exact queue/cancel/finalize, ordering and branch state machine, which must precede writable shared-conversation extraction.
