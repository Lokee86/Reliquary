# Reliquary multiplexing implementation plan

Parent index: [Documentation index](INDEX.md)
Architectural decision: [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
Current-source audit: [Phase 0 baseline](multiplexing-phase0-baseline.md)

## Purpose

Specify the GatewayRuntime/InstanceRuntime migration across Reliquary, including shared REL/PHY execution, instance-specific context, conversation synchronization, correctness gates and consumer migration.

## Overview

Status: architectural roadmap only, revised after [the readiness review](multiplexing-readiness-review.md) on 2026-10-02. **Not implementation-ready.** Complete Phase 0B's blocking designs and executable baselines before the broad Phase 1 hard cut. No multiplexing implementation has shipped. This is an independent whole-stack project, not an Ego workstream.

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

### Phase 1 — Gateway/instance authority extraction

- **1.1** Define exact public/internal GatewayRuntime and InstanceRuntime ownership contracts and errors, including native queue semantics, per-capability host integration/authority negotiation, explicit stage handoffs and immediate startup reconnect/reconciliation hook. Gateway retains the existing keyed REL executions, routes and policy; introduce keyed PHY execution/instance attachment shapes at their **final owner**, even if first implementation supports a narrow test configuration.
- **1.2** Establish a minimal gateway-owned conversation coordinator with shared writable continuation, multiple read/view attachments, explicit writer admission, gateway-owned durable provisional queue for Reliquary-admitted work (available to Warlock where appropriate), serialized cancel-versus-finalize and correct detach/finalization; this is **a prerequisite**, not deferred Phase 3 machinery. Keep existing interaction/session primitives beneath it, but do not create duplicate writable `open_session` objects for the same conversation.
- **1.2a** Only after 1.2, relocate active REL selection, per-instance managed conversation selection and reopen-pending state from host/execution singletons into instance context. Keep explicit owner operations as gateway internals.
- **1.3** Make all user-facing ambient semantic operations resolve through the initiating instance; no hidden active_execution() path may determine an operation's target in the completed architecture.
- **1.4** Convert instance lifecycle into attach/detach independent of shared REL/PHY lifecycle. Establish an explicit quiesce/fence interface for global owner maintenance.
- **Tests:** two instance selections cannot affect each other; instance detach never unmounts an owner; same-instance existing reopen/active-conversation behavior holds; explicit owner operations cannot bypass instance authorization.
- **Gate:** only one authoritative selection source (instance); no synchronized legacy active REL, active PHY, or managed-session shadow state. If Phase 1 is not yet multi-PHY capable, do not advertise general multi-user operation before Phase 2's gate.

### Phase 2 — PHY registry, private access and work ownership

- **2.1** Mount PHYs by durable owner ID exactly once in gateway registry. Instances attach through verified principal binding and explicit grants; many instances may reference the same PHY.
- **2.2** Separate REL processing from PHY processing: move user queue, memory profile and personal Perception/Dream/vector scheduling to per-PHY ownership where semantically applicable. Keep REL-owned project processing per REL. Eliminate active-REL toggles used merely to redirect global PHY work.
- **2.3** Bind user message attribution and personal work requests to their initiating instance/principal. Principal-profile projections into a REL are keyed by principal, not overwritten by whoever most recently selected it.
- **2.4** Migrate knowledge/retrieval/provenance/Echo/Entity and future context consumers to instance-authorized owner sets. Explicit REL access is not automatically granted just because a REL is mounted.
- **Tests:** same/different principal across same/different REL; PHY privacy under concurrent retrieval; exactly one personal worker set per PHY; no private state in REL-wide results.
- **Gate:** independent instances and PHY executions pass authorization/privacy and processing deduplication tests. No background job consults mutable global active PHY.

### Phase 3 — REL conversation coordination

- **3.1** Implement and test the two agreed conversation policies over the single coordinator: **sequential chronological queue by default**, with a **clearly visible automatic branching** option on actual conflicts. Phase 0B must resolve verified timestamp ordering and genuinely simultaneous ties, the durable pending-submission journal and its restart replay without introducing entries into conversation history, atomic queued cancellation versus Archive finalization, queue processing around in-flight generation, branch identities and per-conversation preference persistence. Live synchronization is mandatory for either policy; no silent stale-write rejection, reordering of committed history or last-write-wins.
- **3.2** Extend the minimal gateway conversation coordinator established in Phase 1 with the chosen full branch-conflict policy, stronger generation controls, idempotent command retry and multi-viewer lifecycle. Do not introduce a second coordinator or duplicate writable InteractionRuntime session objects.
- **3.3** Move managed conversational selection out of REL execution. Preserve explicit resume-from-leaf and Archive branches; resolve global durable conversation_active flags so one instance switching away cannot incorrectly mark a conversation inactive while another is viewing it.
- **3.4** Make writer and generation lifetimes independent of initiating instance connection; cancel only through authorized operation controls; checkpoint/complete/recover through existing durable seams.
- **Tests:** simultaneous submissions, cancel-before-finalize, cancellation racing finalization, no canceled/queued turns in Archive or derived activity, queue recovery without accidental publication, multi-viewer single generation, instance switch/detach during stream, two independent conversations, exact durable recovery and episode finalization.
- **Gate:** no shared-branch lost updates, double generation, last-writer-wins or cross-instance cancellation.

### Phase 4 — REL distribution, stream subscription and resynchronization

- **4.1** Publish REL-wide post-commit update events through a gateway-owned ordered feed; subscribers are all connected, authorized instances accessing that REL, not only viewers of its currently active conversation.
- **4.2** Add conversation/generation subscriptions restricted to instances actively viewing the authorized conversation. Joining an active generation returns an atomic checkpoint/cursor before future deltas. Broadcasting output follows durable checkpoint synchronization.
- **4.3** Implement mount-epoch and event-cursor handshake, deduplication, bounded fan-out and gap/reconnect/restart reconciliation for **both reconnecting instances and reconnecting/remounted RELs**. Reauthorize every resumed instance, refresh its REL/branch/queue/operation state, rejoin only authorized selected streams from durable checkpoint/cursor and invalidate caches when a REL epoch changes. Where event continuity cannot be proven, replace incremental replay with an atomic authoritative snapshot-and-watermark. An overflow must force resync; no unbounded per-instance backlog.
- **4.4** Distinguish operation lifecycle events from large text deltas; REL updates are broad and compact, while output is stream-scoped. Ensure a subscription transition cannot miss a concurrent commit.
- **Tests:** inactive conversation updates visible; no stream leakage; late stream join reconstructs exactly once; instance disconnect/reconnect, REL disconnect/remount with epoch invalidation, concurrent mutation during snapshot-and-subscribe, expired cursor fallback, gap, slow reader, restart, revocation, queue-cancellation reconciliation, multi-instance ordering.
- **Gate:** updates and streams satisfy separate recipient rules; reliability and privacy pass concurrent stress fixtures.

### Phase 5 — Full stack and host-consumer migration

- **5.1** Migrate remaining runtime_host modules: Archive, memories, Graph, Entity/Perception, Dream, Insomnia, vectors, runtime status, repository operations, reconciliation and configuration entry points to explicit gateway/instance call direction.
- **5.2** Adapt public library facade and actual external consumers (Warlock, detachable CLI, ACP/agents as applicable) directly to canonical interfaces. Inventory their actual queue/session/stream/scheduling/cancellation/reconnect capabilities; **prefer the host queue for host-executed work when it can reliably cancel pending items**. Retain native Reliquary machinery for internal work and guarantees an external queue cannot meet. If separate stage queues are necessary, prove cancellation reaches every not-yet-dispatched stage and a cancelled item cannot execute on either side. No duplicate commitment authority. Audit configured finite/runtime routes; do not create a redundant permanent product-host authority.
- **5.3** Remove obsolete ReliquaryRuntimeHost ambient-state fields, host-global PHY APIs and obsolete session forwarding once every legitimate dependent is migrated. Preserve existing on-disk and authenticated external contracts explicitly where proven.
- **5.4** Update current-state architecture.md, api.md, maintainer-map.md, behavioral-contracts.md, documentation-coverage.md and current-limitations.md when implemented; update ADR 0037 status/singleton language without deleting its ownership rationale.
- **Tests:** all migrated consumer entry points enforce per-instance scope and use the same gateway authority; no internal path reads implicit global active state.
- **Gate:** one canonical runtime implementation, no shadow singleton or dual-owner worker set, no unintended temporary adapters.

### Phase 6 — End-to-end verification and measured optimization

- **6.1** Run unit/integration/stress fixtures covering same-PHY and different-PHY instances across one/many RELs; event loss/reconnect; subscriber churn; branch conflicts; shutdown/reconciliation; worker deduplication. Include gateway restart with host immediately available/unavailable, host reconnect with divergent queued/in-flight/committed state, offline cancellation, cancellation racing host dequeue, cancellation during cross-stage handoff, host-owned queue suppression of cancelled dispatch, adapter capability downgrade and idempotent replay. Verify already-running host interruption reports its real outcome.
- **6.2** Profile per-REL interaction lock contention, fan-out memory, owner background scheduling and many-client update pressure; split locks or add backpressure only where measured, not to preempt a hypothetical bottleneck.
- **6.3** Run Rust fmt/check/full tests, detachable CLI checks after API migration, docs structural/change-impact checks, and repository architecture policy. Use refreshed Lexicon/Arcana structure checks if a valid current snapshot exists; supplement with direct source search.
- **6.4** Verify deletion of all obsolete ambient-host paths and ensure current docs describe shipped behavior, not planned behavior.
- **Final gate:** complete authoritative architecture, no unintended duplicate owner/store/processing state, preservation of all pre-existing semantic tests, tested multi-instance synchronization and concurrency semantics.

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
