# Multiplexing Phase 4 — REL distribution and resynchronization implementation plan

Parent: [Multiplexing implementation plan](multiplexing-implementation-plan.md)  
Foundation: Phase 2 accepted head plus the [shared P3/P4 observation contract](multiplexing-p34-observation-contract-2026-10-04.md); exact commit recorded in the [P3.0 baseline](multiplexing-phase3-baseline-2026-10-04.md)  
Event/authorization contract: [Phase 0B G3/G4](multiplexing-phase0b-events-authorization.md)  
Conversation contract: [Phase 0B G1](multiplexing-phase0b-conversation.md)  
Lifecycle contract: [Phase 0B G5/G6](multiplexing-phase0b-lifecycle-consumers.md)  
Authority decision: [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)  
Status: **P4.0 accepted; P4.1–P4.3 implemented and targeted gates accepted on independent P4 branch; architecture refresh remains incomplete; P4.4 next**

## Purpose

Implement one authoritative, bounded, authorization-aware REL distribution layer for gateway clients: ordered REL-wide change summaries, atomic snapshot-and-subscribe, mount-epoch/cursor resynchronization, reconnect/revocation handling and conversation-generation stream delivery.

Phase 4 runs in parallel with Phase 3. Its REL-wide feed, epoch/cursor, subscriber, overflow and reconnect machinery do not depend on the final branch policy. Only the final conversation-stream adapter and conversation-specific event hooks wait for Phase 3's stable coordinator observation contract.

## Overview

The current gateway already has:
- mounted REL executions with mount epochs;
- instance attachment/grant generations;
- owner fences and revoke/detach behavior;
- explicit instance-authorized gateway mutation/read surfaces;
- Phase 1 durable conversation queue state;
- Phase 2 exact PHY privacy boundaries.

P4.1 now provides one internal `RelSequencer` per mounted REL, token-bound bounded subscriber state and lifecycle teardown. P4.2 adds the canonical internal atomic snapshot/register handshake and live delivery checks. P4.3 adds bounded resync acknowledgement and recipient disconnect. Public exports, complete production publication, reconnect policy and conversation stream hub remain later steps. See the [P4.1 implementation record](multiplexing-phase4-core-2026-10-04.md).

Phase 4 introduces those as one canonical distribution subsystem. It does not create another semantic owner, another durable event log, or another conversation coordinator.

## Parallel implementation contract with Phase 3

Both lanes start from the exact same accepted Phase 2 + P34 seam-freeze commit described in the Phase 3 plan.

P4 owns:
- REL event envelope and cursor allocation;
- `RelSequencer` lifecycle;
- subscriber registry;
- bounded queues and ResyncRequired state;
- authorized snapshot+subscribe/reconnect;
- conversation stream subscriber delivery/cursors;
- event coverage enforcement.

P3 owns:
- queue/branch/generation semantics;
- durable checkpoint/operation state;
- conversation observation snapshot/deltas.

### Work that is fully parallel

P4 can complete independently:
- REL sequencer and cursor/epoch mechanics;
- broad subscriber registry;
- bounded queue/overflow behavior;
- REL snapshot/watermark handshake;
- reconnect/remount/revocation behavior;
- generic event coverage for non-conversation mutations;
- privacy payload checks;
- stream hub mechanics against a test provider.

### Work that waits only at convergence

Do not guess P3's final branch/generation state.

Defer only:
- binding the stream hub to the production P3 conversation observation provider;
- emitting final queue/branch/generation REL summaries from P3 transition points;
- late-join checkpoint assembly from the final P3 coordinator.

No other P4 work should wait for P3.

### File ownership during parallel work

P4 should add dedicated modules, for example:
- `runtime_host_gateway_events.rs`
- `runtime_host_gateway_subscriptions.rs`
- `runtime_host_gateway_streams.rs`
- corresponding test modules

Avoid parallel edits to P3-owned conversation/journal files. Root module declarations and final shared data shapes belong in the seam-freeze commit; remaining root wiring is done on the convergence branch.

## Problem statement

Without a gateway-owned distribution layer, authorized instances cannot reliably know when shared REL state changes unless they poll or infer changes from whichever conversation they currently view. That fails for inactive conversations, semantic/index mutations, queue status, project state, lifecycle changes and reconnect after gaps.

A naive broadcaster is also unsafe:
- broad REL feeds must never leak generation text or PHY-private payloads;
- a subscriber joining during a mutation can otherwise miss the mutation between snapshot and registration;
- unbounded queues can exhaust memory;
- dropping individual events silently makes client state divergent;
- old cursors cannot cross mount epochs or revoked grant generations;
- conversation stream text has different recipients and cursor semantics from REL summaries.

The final architecture therefore needs one REL sequencer as distribution authority, while semantic stores remain the actual data authorities.

## Target architecture

### One RelSequencer per mounted REL execution

Attach one sequencer to the final `RelExecution`.

It owns:
- current mount epoch identity;
- checked monotonic REL event cursor;
- current typed durable watermarks;
- registered REL subscribers;
- bounded per-subscriber delivery queues or equivalent bounded fan-out state;
- overflow/resync latch;
- optional bounded retained event window for same-epoch replay;
- admission lock used for atomic publish vs subscribe snapshot handshakes.

The cursor orders notifications only. It is not a semantic version and never replaces owner/store clocks.

If cursor continuity is lost, remount/reopen cannot prove continuity, or cursor space exhausts, rotate epoch and require snapshot resync rather than manufacturing continuity.

### REL event envelope

Use the accepted Phase 0B shape:
- REL owner ID;
- mount epoch;
- cursor;
- event class;
- available durable watermarks;
- invalidation/stable IDs;
- visibility classification.

Broad feed payloads are summaries and invalidation keys. They do not contain transcript bodies, prompts, private PHY Memory bodies or generation text.

Canonical broad classes remain:
- ArchiveChanged;
- ConversationOperationChanged;
- SemanticOwnerChanged;
- RetrievalInvalidated;
- ProcessingStatusChanged;
- SubmissionQueueChanged;
- HostWorkChanged;
- ProjectStateChanged;
- OwnerLifecycleChanged.

Do not create a class for every call site if one stable semantic class covers the consumer contract.

### Atomic snapshot and subscribe

`subscribe_rel(instance, rel, known_cursor?)` is an authorization and sequencing operation, not a transport convenience.

Under one sequencer gate:
1. revalidate attachment, ViewRel grant and current REL epoch;
2. obtain an authoritative broad snapshot from gateway-owned readers;
3. capture current sequencer cursor/watermarks;
4. register the subscriber;
5. release;
6. return snapshot + epoch/cursor watermark.

Every event published after release has a strictly greater cursor.

The snapshot includes:
- conversation/branch heads and safe metadata;
- queue status identities/order/status;
- operation status summaries;
- semantic/project/retrieval watermarks;
- lifecycle/host safe status.

It excludes:
- provisional queue body text;
- generation text/checkpoint content;
- PHY-private state.

### Publication path

For gateway user-visible mutation paths:

1. authorization/fence validation;
2. authoritative mutation;
3. required store/journal synchronization;
4. capture safe event summary/watermarks;
5. publish through the sequencer;
6. return mutation result.

The mutation remains authoritative if subscriber delivery fails. Client socket acknowledgement is never the commit point.

Do not hold provider/network/model I/O while holding the sequencer.

Do not make every low-level standalone facade mutation emit gateway events. The gateway-authenticated mutation surface is the distribution boundary. Trusted maintenance operations that bypass incremental publication must rotate epoch/force resync if their effects cannot be covered safely.

### Bounded fan-out and resync

Each subscriber has a hard capacity.

On overflow:
- latch one `ResyncRequired { epoch, latest_cursor }`;
- discard/replace pending ordinary incremental events according to one deterministic policy;
- do not enqueue further ordinary events while latched;
- require a new authorized snapshot/watermark;
- resume incrementals only after resync acknowledgement.

Never silently drop one event and keep pretending the cursor stream is contiguous.

### Conversation stream hub

Conversation text/checkpoints use a separate stream namespace.

Key by:
- REL epoch;
- conversation ID;
- operation/generation ID;
- branch ID where required;
- stream cursor/checkpoint sequence.

Joining a stream:
- requires current ViewRel and selected/explicit authorized conversation scope under the accepted REL-wide conversation visibility policy;
- obtains the P3 authoritative conversation/checkpoint snapshot;
- registers before releasing the stream-coordinator gate;
- returns durable content/checkpoint plus stream watermark;
- delivers only checkpoints persisted after that watermark.

Joining grants observation only, not cancellation.

The broad REL feed receives safe operation/queue/branch summaries; text stays in the exact conversation stream.

P4 may implement the hub and tests against a deterministic test provider before P3 converges. Production provider wiring waits for P3.

## Implementation decisions

- No persistent general event log in Phase 4. Authoritative stores + snapshots recover state.
- Same-epoch bounded replay is optional optimization, not correctness authority.
- Mount epoch is already an owner-fencing concept; P4 consumes the final gateway epoch rather than adding another owner epoch.
- Grant generation and attachment incarnation are revalidated on subscribe, replay/resume and delivery.
- Revocation removes subscriber authority before any subsequent delivery.
- REL dependencies are independently authorized; subscribing to REL A does not subscribe to dependency B.
- Private PHY changes never enter a REL-wide payload. A REL projection change may emit a safe principal-keyed invalidation only after the REL projection itself mutates.
- P4 does not own branch policy or generation state.
- P4 transport APIs remain in-process/runtime contracts. Network protocol design is outside this phase.

## Migration strategy

Use a staged hard cut on the new gateway distribution path.

Do not maintain a permanent polling/broadcast fallback beside the final sequencer for gateway consumers.

Move once:
1. create final sequencer/subscriber modules;
2. establish atomic snapshot+subscribe;
3. instrument gateway mutation families directly;
4. implement overflow/resync/reconnect;
5. implement generic stream hub;
6. integrate P3 conversation provider;
7. remove temporary test-only mutation publishers and any duplicate delivery path.

A red migration window is acceptable on the P4 branch while event coverage is incomplete. P4's local integration gate restores its targeted gateway/event tests before convergence.

## Execution sequence

### P4.0 — Baseline and mutation inventory

**Accepted:** [P4.0 baseline and mutation inventory](multiplexing-phase4-baseline-2026-10-04.md), independently branched from shared foundation `388bd8f`. All mutation families classified; 115 gateway cases, 10 event/auth cases and 1 public API case pass, along with library check/format/docs. P4.1 is recorded below.

Start from accepted Phase 2 + seam-freeze.

Run:
- Phase 0B events/auth fixture;
- gateway authorization/lifecycle tests;
- public gateway API contract;
- affected library tests.

Inventory every instance-visible gateway mutation family and classify it against the Phase 0B event coverage matrix.

**Exit:** no unclassified gateway mutation family.

### P4.1 — RelSequencer core

**Implemented:** [P4.1 core and acceptance](multiplexing-phase4-core-2026-10-04.md). Cursor/epoch/isolation/lifecycle checks pass; architecture refresh and CLI test build remain verification gaps recorded there. P4.2 is recorded below.

Implement one sequencer per mounted REL.

Required:
- epoch binding;
- checked cursor allocation;
- typed event envelope;
- publish gate;
- subscriber registration IDs bound to attachment/grant generation;
- lifecycle teardown on unmount/shutdown;
- deterministic test seams.

Do not yet instrument all mutations.

**Tests:** cursor monotonicity, epoch mismatch, subscriber isolation, two RELs independent, cursor exhaustion fails/rotates safely.

### P4.2 — Atomic REL snapshot/subscription

**Implemented:** [P4.2 snapshot/subscription and acceptance](multiplexing-phase4-snapshot-2026-10-04.md). The internal handshake, current safe readers and race/revocation/privacy tests pass. Final P3 observations and full production publication remain P4.7/P4.4; public exports reconcile at convergence. P4.3 recovery is implemented below; P4.4 is next.

Implement the canonical broad snapshot builder and atomic registration.

Snapshot builder uses only authorized gateway reads and stable P3 seam data available from the shared foundation/current coordinator. P3-specific final branch/generation fields can be filled at convergence without changing the handshake.

**Tests:** mutation racing subscribe is snapshot-or-event exactly once; two simultaneous subscribers; revoked grant during snapshot; inactive conversation included; no queue body/private PHY payload.

### P4.3 — Bounded delivery and ResyncRequired

**Implemented:** bounded overflow, gated fresh snapshot preparation, exact-watermark acknowledgement and explicit recipient disconnect. Post-snapshot events stay bounded and hidden until acknowledgement; a second overflow invalidates that snapshot. See the [snapshot/resync record](multiplexing-phase4-snapshot-2026-10-04.md). 932 library and 93 integration tests, library check, formatting, direct source ownership and both documentation gates pass. Architecture refresh timed out during Lexicon analysis at 240 seconds; refreshed enforcement remains incomplete.

Implement capacities, overflow latch and resync acknowledgement.

Requirements:
- no unbounded Vec/channel;
- exactly one latched resync state;
- no ordinary event after latch until fresh snapshot acknowledgement;
- current latest cursor/epoch exposed safely;
- subscriber disconnect frees delivery state without changing REL work.

**Tests:** exact-capacity edge, burst overflow, repeated overflow, disconnect while latched, resubscribe after overflow.

### P4.4 — Non-conversation mutation coverage

Instrument gateway mutation families that do not depend on final P3 semantics:

- semantic REL Memory/Entity/Graph/community/Ego changes exposed by gateway;
- retrieval/index invalidation;
- Processing/Dream/Perception/Insomnia safe status where gateway owns the transition;
- project/repository correlation and project policy changes;
- owner lifecycle/reopen/degraded status;
- host capability/work summaries where relevant.

Publish only after the authoritative durability boundary.

Add a coverage guard so a new instance-visible mutator cannot silently omit event classification.

**Tests:** one event per committed mutation, no event on failed mutation, correct watermarks/invalidation IDs, unrelated REL subscribers receive nothing.

### P4.5 — Reconnect, remount and revocation

Implement:
- fresh snapshot on reconnect;
- same-epoch cursor replay only when retained and authorized;
- prior-epoch cursor rejection;
- epoch rotation/resync on continuity loss;
- subscription invalidation on detach/revoke;
- grant-generation change prevents later delivery;
- owner reopen/reconcile publishes safe lifecycle state only after authoritative reconcile.

**Tests:** reconnect with valid/expired cursor, remount, grant refresh, revoke racing publish, detach while queue has events, host unavailable while native REL work continues.

### P4.6 — Generic conversation stream hub

Build the final stream subscriber/delivery machinery against the frozen P34 conversation observation/provider contract or a deterministic test implementation.

Required:
- exact conversation+operation stream identity;
- separate stream cursor from REL cursor and Archive version;
- durable checkpoint visibility only;
- atomic snapshot+register;
- late join no duplicate prefix;
- terminal/interrupted status;
- bounded queue/resync strategy appropriate to streams;
- no cancellation authority granted by observation.

Do not read mutable InteractionRuntime session fields directly.

### P4.7 — P3 convergence adapter

This is the only Phase 4 step that waits for completed P3 semantics.

On the convergence branch:
- bind stream snapshot to P3 authoritative conversation observation;
- emit broad queue/branch/operation summaries at P3's post-durability transition points;
- publish branch-created summary only after durable branch/finalization identity is fixed;
- publish checkpoint text only through exact conversation stream after checkpoint sync;
- keep broad REL event free of generation text.

No P3 coordinator/subscriber merge: P4 retains subscriber state.

### P4.8 — Coverage/lock/privacy audit

Audit:
- every gateway instance-visible mutator classified;
- no private PHY body in REL event/snapshot;
- no subscriber handle stored in semantic stores or P3 coordinator;
- no network/backpressure wait under semantic owner/journal locks;
- no event published before corresponding durability point;
- maintenance bypass either emits correct summary or rotates epoch/forces resync.

### P4.9 — Phase 4 local/convergence acceptance

Before P3 convergence, require P4.0–P4.6 targeted gates green against test provider.

After convergence, require:
- Phase 0B events/auth fixture;
- P3/P4 integration tests;
- gateway lifecycle/auth tests;
- library/integration tests;
- fmt/check/docs;
- direct coverage/privacy source guard.

## Testing decisions

Use deterministic in-process subscribers; do not require sockets.

Critical races:
- mutation vs subscribe;
- revoke vs delivery;
- overflow vs publish;
- remount vs replay;
- checkpoint persistence vs stream delivery;
- P3 branch/generation transition vs broad summary publication.

Use bounded channels/queues with tiny capacities in tests.

Treat reference fixtures as policy oracles only.

## P3/P4 convergence test matrix

Must include:

| Scenario | Expected result |
| --- | --- |
| Inactive conversation commits | REL feed summary reaches every authorized REL subscriber |
| Active generation checkpoint | broad feed gets status only; exact stream viewers get text |
| Late stream join | durable prefix once, then later deltas once |
| Sequential queue reorder among Pending | queue summary follows durable P3 order |
| Visible auto-branch conflict | explicit branch summary; only submitter's selection changes |
| Subscribe races Archive commit | commit in snapshot watermark or later event, never missed |
| Queue overflow | ResyncRequired then fresh snapshot; no fake contiguous replay |
| REL remount | old epoch/cursors invalid |
| Revoke during queued delivery | no later payload delivered |
| Dependency REL without grant | no subscription or event leakage |
| PHY private mutation | no REL payload unless a REL-owned projection actually changes |
| P3 restart Interrupted generation | fresh snapshot/status; no duplicate text replay |

## Out of scope

- P3 branch/queue/generation policy.
- Persistent universal event log.
- Network protocol/socket implementation.
- Cross-process fan-out.
- General PHY-private feed.
- Legacy host consumer migration/deletion — P5.
- Performance tuning beyond bounded correctness — P6.

## Completion gates

Phase 4 is complete when:

- every mounted REL has exactly one sequencer/distribution authority;
- broad feed subscribe is atomic with an authoritative snapshot/watermark;
- all instance-visible gateway mutation families are classified and covered;
- bounded overflow forces explicit resync rather than silent gaps;
- reconnect/remount/revoke behavior is epoch/grant safe;
- conversation text uses separate exact stream subscriptions;
- P3 conversation semantics integrate without moving subscriber state into the coordinator;
- REL-wide payloads contain no private PHY state or generation text;
- no duplicate polling/broadcast authority survives in the new gateway path;
- P4 convergence tests and repository gates pass.

## Related docs

- [Multiplexing implementation plan](multiplexing-implementation-plan.md)
- [Phase 0B events and authorization](multiplexing-phase0b-events-authorization.md)
- [Phase 0B conversation coordination](multiplexing-phase0b-conversation.md)
- [Phase 2 implementation plan](multiplexing-phase2-implementation-plan-2026-10-04.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)

## Notes

Most of Phase 4 is deliberately independent of Phase 3. The convergence adapter is small by design: P3 supplies authoritative durable conversation observations; P4 supplies cursors, subscribers, bounded delivery and resynchronization.
