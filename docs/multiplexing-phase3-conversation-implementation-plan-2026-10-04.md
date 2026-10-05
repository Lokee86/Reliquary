# Multiplexing Phase 3 — conversation coordination implementation plan

Parent: [Multiplexing implementation plan](multiplexing-implementation-plan.md)  
Foundation: accepted Phase 2 + frozen P34 seam `388bd8f319ae0baafcc89f6c4c5288b78248a3ef`
Conversation contract: [Phase 0B G1](multiplexing-phase0b-conversation.md)  
Authorization contract: [Phase 0B G3/G4](multiplexing-phase0b-events-authorization.md)  
Lifecycle contract: [Phase 0B G5/G6](multiplexing-phase0b-lifecycle-consumers.md)  
Authority decision: [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)  
Status: **P3.0–P3.7 accepted; P3.8–P3.9 remain planned.**
Observation: [P3.7 implementation and gates](multiplexing-phase3-observation-acceptance-2026-10-05.md)
Generation: [P3.6 implementation and acceptance](multiplexing-phase3-generation-acceptance-2026-10-05.md)
Auto-branch: [P3.5 implementation and acceptance](multiplexing-phase3-auto-branch-acceptance-2026-10-05.md)
Sequential: [P3.4 implementation and acceptance](multiplexing-phase3-sequential-acceptance-2026-10-05.md)
Scheduler: [P3.3 ownership and acceptance](multiplexing-phase3-scheduler-acceptance-2026-10-05.md)
Branches: [P3.2 ownership and acceptance](multiplexing-phase3-branch-acceptance-2026-10-05.md)
Policy: [P3.1 ownership and acceptance](multiplexing-phase3-policy-acceptance-2026-10-04.md)
Baseline: [P3.0 record](multiplexing-phase3-baseline-2026-10-04.md)
Shared contract: [P34 observation contract](multiplexing-p34-observation-contract-2026-10-04.md)

## Purpose

Complete the user-visible shared-conversation concurrency model on top of the Phase 1 coordinator and durable REL journal without creating a second coordinator, queue, writable session, or branch authority.

Phase 1 already established durable admission, one REL-scoped provisional journal, shared continuation ownership, instance-local selection, viewer attachment, cancellation/finalization fencing, restart recovery and accepted-work lifetime. Phase 3 deepens that existing machinery into the final conversation scheduler: chronological pending order by default, visible automatic branching on actual stale-head conflicts, branch-local generation ownership, durable policy, reconnectable observation state and explicit generation control.

Phase 3 may be implemented in parallel with Phase 4. It does **not** own the REL-wide sequencer, subscriber transport, cursor retention, backpressure or resynchronization machinery. Those belong to Phase 4.

## Overview

Current production ownership already has:

- one `ConversationRegistry` under `GatewayRuntime`;
- one `RelJournal` writer per mounted REL;
- coordinators keyed by `(REL, conversation)`;
- durable queue order fields: verified receive time, persisted random tie key and admission sequence;
- durable cancel/finalize/Archive/post-commit receipts;
- branch-keyed live processing leases and unfinished journal continuation reservations;
- instance-local selected conversation/branch state;
- transient viewer registrations;
- stable generation/host-operation identity and durable principal attribution.

The coordinator now owns branch-local contention, journal-derived recovery reservations and automatic durable selection across eligible branches. Callers request dispatch for a conversation without supplying a fresh submission ID. Existing durable continuations have a separate resume operation. Default sequential finalization and VisibleAutoBranch stale-head conflict handling are implemented. Native generation/checkpoint control and restart reconciliation are implemented in P3.6.

Phase 3 makes the existing coordinator the one canonical owner of those semantics.

## Parallel implementation contract with Phase 4

After Phase 2 is accepted, create one small shared **P34 seam-freeze commit** before branching Phase 3 and Phase 4.

That commit defines only final cross-lane data contracts and module ownership. It must not add a no-op event bus, forwarding wrapper, dual queue or temporary compatibility API.

Freeze these concepts:

- **REL observation summary:** owner ID, mount epoch, conversation/branch heads, queue status identities, operation status identities and durable watermarks. No transcript bodies.
- **Conversation observation snapshot:** REL ID/epoch, conversation ID, selected branch/head, current durable submission states, active operation identity/status if any, durable checkpoint sequence and branch-local generation identity.
- **Conversation durable delta:** a post-durability fact emitted by the coordinator at named transition points. It is not itself the Phase 4 cursor/event envelope.
- **Generation checkpoint observation:** operation/generation ID, branch ID, durable checkpoint sequence, terminal/interrupted status and content prefix/delta identity only after checkpoint persistence.
- **Module ownership:** P3 owns conversation coordinator/journal/policy/generation state. P4 owns REL sequencer/feed/subscriber/stream delivery.

Preferred branches after the seam commit:

- `feat/multiplexing-phase3-conversation`
- `feat/multiplexing-phase4-distribution`

Both branch from the exact same accepted Phase 2 + seam-freeze commit.

### File ownership during parallel work

Phase 3 owns:

- `runtime_host_gateway_conversation.rs`
- `multiplexing_journal.rs`
- conversation-specific production modules added beside them
- `runtime_host_gateway_conversation_tests.rs`
- G1 production/integration tests

Phase 4 owns new event/subscription/stream modules and their tests.

Avoid parallel edits to `runtime_host_gateway.rs`, `runtime_host_instances.rs`, public re-exports and current-state architecture docs. Put required module declarations/shared final data types in the seam-freeze commit. Reserve unavoidable root wiring and current-state docs for the P3/P4 convergence branch.

## Problem statement

Multiple authenticated instances may submit into one shared conversation. The existing Phase 1 machinery safely admits and durably finalizes work, but it does not yet provide the complete product concurrency behavior:

- durable chronological selection and both current policy-specific finalization decisions are implemented;
- branch leases cover finalization, host continuations and native generation/checkpoint lifecycle;
- the durable conversation policy drives finalization behavior through the production scheduler;
- generation/control uses one durable branch continuation independent of viewer lifetime;
- the P3.7 internal authoritative snapshot reads the existing runtime/coordinator/journal cut; P4 reconnect delivery remains convergence work;
- P4 cannot safely expose conversation streams until these semantics are stable.

The cost of leaving this incomplete is semantic ambiguity under concurrent writers even though the lower-level authority/durability machinery is already correct.

## Target architecture

### One coordinator per conversation, branch-local continuation ownership

Keep `ConversationRegistry` as the final owner. A `ConversationCoordinator` owns:

- durable policy reference/revision;
- branch runtime states keyed by stable branch ID;
- pending scheduler view derived from the REL journal, never a duplicate pending store;
- one active finalization/generation continuation per branch;
- operation/generation status needed to build an authoritative observation snapshot;
- transient viewer references only as delivery interest, never as writer authority.

The REL journal remains the sole durable provisional queue and transition ledger. The coordinator may cache derived scheduling state but must rebuild it from the journal and Archive after restart.

Do not create per-conversation queue files or per-viewer writable `InteractionRuntime` sessions.

### Final policies

`SequentialChronological` is the default and the legacy/missing-policy interpretation.

Its scheduler:

1. selects the oldest still-Pending submission by the journal's durable ordering tuple;
2. never rewrites already committed ancestry when a delayed older timestamp arrives;
3. waits when the target branch already has an active generation/finalizer;
4. captures final branch/parent/revision in durable `FinalizeWon`;
5. processes the next safe item only after the branch continuation reaches the required safe boundary.

`VisibleAutoBranch` uses the same durable ordering but changes stale-head conflict handling:

- if the accepted submission expected the branch head that was current at admission and another concurrent submission advanced that branch first, finalize the stale submission on a distinct visible branch preserving its submitted expected parent;
- derive the branch identity deterministically from stable submission identity or another frozen deterministic key;
- persist branch ID, parent, policy revision and display label in `FinalizeWon`;
- select the new branch only for the submitting instance when that attachment still exists and remains authorized;
- never silently move another instance's selected branch;
- malformed/unrelated expected parents fail closed rather than attaching to an arbitrary leaf.

A viewer attach alone never creates a branch.

### Branch-local generation

One branch has at most one active generation continuation. Separate branches may progress independently if owner/resource policy allows.

A queued submission cannot mutate the prompt/context of an already active generation. It remains Pending until the branch reaches the next safe dispatch point.

Generation identity is stable and independent of the initiating connection. Viewer count does not control lifetime.

The coordinator owns durable operation state and checkpoint identity. P4 later owns delivery cursors/subscribers.

### Conversation observation model

P3 must expose an internal authoritative snapshot suitable for P4 without depending on P4's implementation.

The snapshot includes only facts P3 owns:

- conversation/branch heads;
- durable queue state/order identities;
- active or interrupted generation identity/status;
- durable checkpoint sequence/watermark;
- selected policy/revision;
- branch labels/ancestry identifiers.

It does not include transport subscriber state or a P4 REL cursor.

## Implementation decisions

- Reuse `RelJournal`; do not introduce another queue.
- Reuse the existing Archive/branch primitives and one writable continuation per branch.
- Durable policy is conversation metadata. Missing legacy policy means `SequentialChronological`.
- Queue order remains the Phase 0B tuple `(verified_receive_ns, tie_key, admission_sequence)`. Client clocks never participate.
- Keep cancellation of pending submissions distinct from cancellation/interruption of an already running generation.
- Cancellation authorization continues to use current attachment/grant checks; accepted work remains coordinator-owned after submitter detach.
- `conversation_active` remains legacy durable metadata and does not become viewer count or liveness.
- P3 produces post-durability transition observations but does not allocate P4 event cursors.
- No compatibility facade is added around the old host/session singleton. Phase 5 removes old consumers later.

## Migration strategy

This is a staged hard cut inside the new gateway path.

Temporary whole-repository redness is acceptable only inside a declared P3 branch migration window. Do not add forwarding wrappers merely to keep old explicit-processing APIs and the final scheduler simultaneously authoritative.

Move once:

1. deepen `ConversationCoordinator` to the final branch-aware state shape;
2. make the scheduler derive next work from `RelJournal`;
3. move policy/branch conflict decisions into the coordinator;
4. migrate existing explicit process paths/tests onto coordinator scheduling;
5. add generation state/recovery;
6. remove gateway-internal APIs that permit bypassing the coordinator's scheduling decision when no supported external contract requires them.

If a test-only exact-submission processing seam remains useful for deterministic fault injection, keep it test-only and non-authoritative.

## Execution sequence

### P3.0 — Baseline and seam verification

Start from accepted Phase 2 + P34 seam-freeze.

Run:
- gateway conversation tests;
- Phase 0B conversation and journal contracts;
- public gateway API contract;
- library check.

Record the exact current coordinator/journal source inventory and any inherited failures.

**Exit:** known-green baseline and no unresolved P2 authority dependency.

### P3.1 — Durable policy ownership

Add/read/write the two policy values through the canonical conversation metadata owner.

Requirements:
- missing legacy value = SequentialChronological;
- policy mutation requires the explicit management capability chosen by the existing grant model;
- policy revision is captured with finalization;
- changing policy affects only still-Pending work; it cannot rewrite Finalizing/Committed ancestry;
- restart recovers the same policy.

Do not introduce a second policy file.

**Tests:** legacy default, persisted round-trip, policy change with pending/finalizing/committed items, unauthorized mutation.

### P3.2 — Branch-aware coordinator hard cut

Replace the single conversation `processing: bool` authority with branch-keyed continuation state.

The coordinator must:
- serialize finalization per target branch;
- allow unrelated branches to have independent continuation state;
- prevent two finalizers/generations on one branch;
- rebuild state from journal + Archive/checkpoints after restart;
- preserve zero-viewer operation lifetime.

**Tests:** same branch contention, independent branches, detach/switch during generation, restart with one active/interrupted branch and another idle branch.

### P3.3 — Automatic durable scheduler

**Accepted:** automatic selection and reservation, held-branch skipping, test-only fresh exact-entry seams and explicit durable-continuation resumption. See [acceptance evidence](multiplexing-phase3-scheduler-acceptance-2026-10-05.md).

Add coordinator-owned "process next eligible" scheduling from journal state.

Rules:
- select Pending by durable order, never caller order;
- skip branches currently unable to accept another finalization without changing the durable ordering of remaining work;
- do not reorder a previously captured FinalizeWon;
- preserve equal-timestamp random tie order after restart;
- same submission retry never creates a second scheduler entry.

Remove production reliance on caller-supplied "process this submission next" where it bypasses policy.

**Tests:** delayed older admission, equal timestamp ties, multiple conversations sharing REL journal, blocked branch with eligible different branch, restart exact ordering.

### P3.4 — Sequential policy finalization

**Accepted:** selected-branch identity, safe sequential ancestry, synchronized publication, immutable retries and seven restart handoffs. See [acceptance evidence](multiplexing-phase3-sequential-acceptance-2026-10-05.md).

Implement the default policy on the real Archive path.

For each selected entry:
- validate expected conversation/branch identity;
- choose the current safe head according to sequential policy;
- durably capture parent/branch/revision in FinalizeWon;
- append/sync Archive exactly once;
- preserve existing post-commit/Activity/Episode scheduling idempotency;
- leave pending queue records outside Archive until finalization.

**Tests:** cancel-vs-finalize races, every crash boundary, stale fence, remount, postcommit marker loss, no provisional activity/Episode leakage.

### P3.5 — Visible automatic branching

**Accepted:** authoritative stale-head classification, deterministic durable conflict branches, submitter-local selection and restart-stable recovery. See [acceptance evidence](multiplexing-phase3-auto-branch-acceptance-2026-10-05.md).

Implement stale-head branch conflict policy.

Requirements:
- conflict classification occurs against the authoritative branch head at finalization;
- only the accepted stale-head conflict described by the policy creates a branch;
- branch ID/label/parent are deterministic and persisted before Archive commit;
- crash/restart creates exactly the same branch;
- submitting instance selection may move to its new branch only through its own current attachment;
- other viewers remain selected where they were;
- invalid/unrelated parent fails closed.

**Tests:** two simultaneous writers, three-writer ordering, crash before/after branch creation commit, submitter detached before branch finalization, another viewer on original branch, invalid expected parent.

### P3.6 — Generation/control lifetime

Status: accepted with the documented architecture-tool limitation; verification results are recorded in [P3.6 acceptance](multiplexing-phase3-generation-acceptance-2026-10-05.md).

Make branch generation state explicit.

- one active generation per branch;
- stable operation/generation ID;
- queued submissions wait without modifying an in-flight prompt;
- initiator disconnect does not cancel;
- zero viewers does not cancel;
- authorized generation cancellation/interruption is distinct from pending-submission cancellation;
- checkpoint persistence precedes any observation;
- restart marks unreconciled local generation Interrupted/Unknown until authoritative recovery decides otherwise;
- host-bound operation ownership continues to respect the Phase 1 capability router.

**Tests:** detach initiator, all viewers detach, late viewer, cancellation authorization, restart during checkpoint, host status unknown, branch A generation while branch B advances.

### P3.7 — Authoritative conversation observation snapshot

Status: accepted with the documented architecture-tool limitation. [P3.7 evidence](multiplexing-phase3-observation-acceptance-2026-10-05.md) records the internal seam, runtime -> coordinator -> journal lock order, existing owner sync barrier, instance-local selection and durable handoff tests.

Implement the final P3-owned observation builder used at P3/P4 convergence.

It must atomically observe coordinator/journal/Archive state without opening a second writer. Do not hold the REL journal writer while acquiring semantic owner locks in the reverse direction.

Snapshot must distinguish:
- Pending/Finalizing/Committed/Cancelled submissions;
- current branch heads;
- active/interrupted/terminal operations;
- durable checkpoint sequence;
- policy revision.

P3 tests this as a plain internal snapshot. P4 later adds cursor/subscriber semantics.

### P3.8 — Recovery and hard-cut cleanup

On mount/restart:
- reconcile Finalizing journal entries against Archive;
- rebuild branch continuation state;
- recover Pending order;
- recover policy;
- recover checkpoint/operation identity;
- never infer viewers from durable metadata;
- never blindly replay unknown host work.

Audit the new gateway conversation path for:
- duplicate scheduler authority;
- per-viewer writer creation;
- alternate branch policy decisions outside coordinator;
- bypass processing APIs.

### P3.9 — Phase 3 local acceptance

Required green gates before convergence:
- fmt/diff/check;
- gateway conversation tests;
- journal tests;
- Phase 0B G1 fixtures;
- library tests affected by conversation/session behavior;
- public gateway contract if surface changed;
- direct source guard for one coordinator/one journal authority;
- docs checks.

Do not require P4 feed tests for P3 local acceptance.

## Testing decisions

Use the existing production seams first:

- `GatewayRuntime::submit_conversation`;
- coordinator-owned process-next scheduling;
- `RelJournal` durable state;
- real Archive branches/turns;
- viewer attach/detach;
- restart/remount.

Retain the Phase 0B G1 reference fixtures as policy oracles, not production proof.

Prefer deterministic barriers over sleeps for cancel/finalize and concurrent-writer races.

Add fault injection at:
- FinalizeWon durability;
- Archive append/sync;
- branch creation/receipt;
- checkpoint sync;
- postcommit marker;
- restart reconciliation.

## P3/P4 convergence requirements

P3 may merge to the integration branch without P4 conversation delivery, provided its authoritative observation model is green.

At P3/P4 convergence:
- P4 REL feed publishes queue/branch/operation **summaries** after the exact P3 durability points;
- P4 conversation stream adapter consumes P3 checkpoint/generation observation, not raw mutable sessions;
- P4 subscriber state never enters `ConversationCoordinator`;
- P3 never allocates REL or stream delivery cursors;
- revocation/detach is checked by P4 delivery and by P3 control operations independently.

## Out of scope

- REL-wide feed/cursor/backpressure implementation — P4.
- Network/socket transport.
- PHY-private subscriptions.
- Remaining runtime-host consumer migration/deletion — P5.
- Performance lock splitting absent measurement — P6.
- New branch merge semantics or collaborative editing beyond the selected two policies.

## Completion gates

Phase 3 is complete when:

- one existing `ConversationRegistry` remains the canonical coordinator owner;
- the REL journal is the only durable provisional queue;
- default scheduling follows the persisted chronological order automatically;
- VisibleAutoBranch creates stable, explicit branches only for real stale-head conflicts;
- each branch has at most one active generation and viewer lifetime is independent;
- pending cancellation, generation control and finalization have unambiguous authorities;
- restart reconstructs scheduler/branch/generation state without duplicate Archive work;
- the authoritative observation snapshot is stable enough for P4;
- no gateway-internal bypass creates a second scheduling or branch authority;
- P3 local acceptance gates pass.

## Related docs

- [Multiplexing implementation plan](multiplexing-implementation-plan.md)
- [Phase 0B conversation coordination](multiplexing-phase0b-conversation.md)
- [Phase 0B events and authorization](multiplexing-phase0b-events-authorization.md)
- [Phase 2 implementation plan](multiplexing-phase2-implementation-plan-2026-10-04.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)

## Notes

P3 is deliberately transport-agnostic. It must finish conversation semantics without waiting for P4's REL feed machinery, while exposing the final observation/checkpoint seams P4 needs at convergence.
