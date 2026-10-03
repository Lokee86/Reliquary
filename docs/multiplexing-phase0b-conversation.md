# Phase 0B G1 — shared conversation execution and durable submissions

Parent plan: [Multiplexing implementation plan](multiplexing-implementation-plan.md)  
Readiness gates: [Multiplexing readiness review](multiplexing-readiness-review.md)  
Baseline: [Multiplexing Phase 0 baseline](multiplexing-phase0-baseline.md)  
Authority: [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)

## Purpose

Specify the G1 shared-conversation and durable-queue contract needed before instance-local session selection can move. This is a design artifact and executable reference oracle, not production implementation evidence.

## Overview

Status: Phase 0B G1 draft, source-backed against the audited Reliquary checkout. It resolves queue ordering, restart durability, cancel/finalize fencing, Archive duplicate prevention, shared continuation attachments, and visible branching. Runtime integration and the other Phase 0B gates remain open.

## Status and scope

This is the concrete G1 contract for the Phase 0B design gate. It describes target behavior and a test oracle; it does not claim the runtime implements or proves this contract. It covers one REL's shared conversations, durable native pending submissions, cancellation/finalization, viewer attachment, restart recovery, and the optional visible branch policy. Principal grants and authorization are preconditions enforced by the gateway on every attach, enqueue, dispatch, cancel, and commit; G2/G4 own their full contracts.

The source audit establishes why this must precede instance-local managed selection: `runtime_host_session.rs` has one `ManagedSessionState` per REL and closes it before starting/reopening another conversation; `InteractionRuntime` separately keys sessions by session ID and rejects an already-open ID. `complete_message` writes an accepted turn to Archive, and accepted-turn ingestion synchronizes; stream checkpoints are separately persisted and synchronized. `InteractionStreamRecord` currently has no principal field. Therefore the target must not open one writable session per viewer, must use the verified instance principal for a user turn, and must not make queue state an Archive turn. Existing `ConversationMetadata.active` is durable Boolean metadata, not a viewer count or liveness lease.

## Identity and ownership

The gateway owns exactly one coordinator per `(rel_owner_id, conversation_id)`. Within it, each branch has one authoritative head and at most one active generation continuation. A continuation has stable `operation_id`, `generation_id`, monotonically increasing fence, branch ID, captured parent/head revision, initiating instance/principal, and durable checkpoint cursor. A lower-level writable Archive/Interaction session is opened once by this coordinator when needed; attaching a viewer never opens a second writer.

Each instance owns only its selected conversation/branch and its attachment token. A viewer attachment is identified by `(instance_id, attachment_id)`; it is a many-to-one subscription reference to the continuation. Attach returns a snapshot/checkpoint and cursor atomically, followed only by later deltas. Detach removes that reference. The generation continues when its initiator disconnects or when the last viewer detaches; no viewer gains cancellation authority from attachment. Authorized cancellation targets the operation ID and a capability, not the viewer set.

`conversation_active` remains readable as legacy conversation metadata. During multiplexed operation it does not encode current viewers, open continuations, or process liveness and is not toggled on each attach/detach. Viewer/presence state is gateway-transient and rebuilt on attach. Conversation/branch head and Archive ancestry remain durable.

## Native queue journal

Reliquary-admitted work needs a durable provisional journal separate from Archive history. Use one append-only journal per stable REL owner UUID under the configured gateway state directory, e.g. `<state-dir>/conversation-queues/<rel-uuid>.rqj`. A journal stores submissions across gateway restarts on that host; it is not a portable REL transcript or a cross-host queue. The REL owner UUID, not a mutable filename/path, keys it. One gateway writer owns the journal while that REL is mounted. If the state directory cannot be opened, locked, or synchronized, reject admission; never acknowledge volatile-only native work.

The versioned header contains magic, schema version, REL owner UUID, and header checksum. Each frame contains bounded length, monotonically increasing journal sequence, schema/tag, payload, and checksum. The append-only event set is `Enqueued`, `CancelWon`, `FinalizeWon`, `ArchiveCommitted`, `SubmissionCommitted`, `PostCommitScheduled`, and optional queue-policy change. Append is serialized with state transition under the per-conversation coordinator; write the whole frame and call durable file sync before acknowledging that transition. The successful journal append is the linearization point for enqueue, cancellation, and finalization claim. Never hold a filesystem or REL lock during inference.

An enqueue record contains stable submission ID, payload hash and payload, verified principal ID, conversation ID, requested branch and expected leaf/revision, gateway-verified receive timestamp, random tie key, admission sequence, and idempotency key. For attachments, first write each bounded immutable payload into the gateway spool keyed by content hash, flush it, then reference it from Enqueued; an absent or hash-mismatched spool object blocks recovery and dispatch rather than silently dropping the attachment. Bound an encoded journal frame to 16 MiB; reject larger requests before acknowledgment. The timestamp is generated by the trusted gateway, never copied from client time. Persist `max(wall_clock_ns, prior_verified_timestamp_ns)` so a backwards wall clock cannot make later admissions look earlier. Draw a cryptographically random fixed-width tie key at admission and persist it before acknowledgment. Pending order is ascending `(verified_receive_ns, tie_key, admission_sequence)`; the tie key is never regenerated on recovery or retry. The sequence is a final uniqueness fallback, not the ordinary simultaneous-order policy. Client-provided timestamps are retained only as untrusted payload metadata if a caller needs them; they do not affect ordering.

An idempotent retry with the same submission ID and identical principal, scope, payload hash and request metadata returns the recorded status. Reuse of an ID for different content/scope fails closed. Journal payload and attachment staging are provisional: no Archive node, conversation activity, Episode, Insomnia input, committed-message event, or branch is created until the finalization protocol below. Admission acknowledgment is sent only after the Enqueued frame is durable.

Cap each encoded frame at 16 MiB and each per-REL journal at 1 GiB. Compact before 768 MiB by freezing only that conversation queue's transitions, writing a checksummed snapshot containing every Pending/Finalizing record plus a compact terminal tombstone for every submission ID (ID, request fingerprint, terminal status, Archive node/branch receipt, and admission sequence), flushing the new file, atomically replacing the old one, reopening and validating it, then resuming transitions. Terminal tombstones are retained for the full lifetime of the REL queue; compaction drops terminal payload bytes only after Archive and post-commit scheduling receipts are durable. If tombstones alone reach the cap or compaction fails, reject new enqueue and preserve all prior records; never expire a dedup ID. On Windows, use `FlushFileBuffers` for the new file and `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)` (or a verified `ReplaceFileW` equivalent), then reopen/checksum the installed journal. On Unix, use `fsync` on the new file, `rename`, and `fsync` on the parent directory. If the active filesystem cannot provide the platform durability/replace operations, fail initialization or compaction and reject further admission rather than weakening the promise. A truncated final frame is a torn append: recover the longest fully checksummed prefix and truncate only that tail before accepting writes. A checksum/sequence/owner mismatch in a complete interior frame is corruption: stop dispatch, expose recovery-required status, and preserve the file; never skip a possibly durable cancellation. Durability is a hard admission precondition: flush each complete frame to stable storage (Windows: the implementation must confirm `FlushFileBuffers` through the native handle; Unix: `fsync`). When creating the journal, also flush the containing directory entry on the active filesystem. If the OS/filesystem cannot confirm the file and directory durability operations, fail gateway journal initialization and reject native enqueue; do not silently downgrade the guarantee. This format is a focused runtime journal, not a generalized storage abstraction.

## State-transition contract

Submission states are durable journal states unless marked transient.

| Current | Command / condition | Next state and linearization | Visible effects |
| --- | --- | --- | --- |
| absent | valid enqueue, unique ID | `Pending` after synced `Enqueued` | provisional queue status only |
| `Pending` | cancel by authorized submitter before finalization claim | `Cancelled` after synced `CancelWon` | cancellation result/status to authorized REL subscribers; no Archive mutation |
| `Pending` | coordinator selects next pending item | `Finalizing` after synced `FinalizeWon` capturing exact branch and parent | cancellation now returns too-late; still no committed turn yet |
| `Finalizing` | idempotent Archive append/sync succeeds | Archive contains exact turn identified by stable submission ID | committed-message update only after Archive sync |
| `Finalizing` | Archive append succeeds, process crashes before journal marker | remains `Finalizing` in journal; Archive lookup finds matching stable turn ID | recovery records committed, never appends duplicate/activity |
| `Finalizing` | Archive append has not happened at crash | remains `Finalizing` | recovery performs the same idempotent commit; cancellation cannot overtake the durable claim |
| `Finalizing` | commit sync fails or owner unavailable | remains `Finalizing`, dispatch fenced | report unknown/recovery status; do not cancel or dispatch a duplicate |
| `Committed` | cancel submission | unchanged | return already committed; interrupting its generated operation is a distinct authorized command |
| `Cancelled` | duplicate cancel/retry after restart | unchanged | idempotent cancellation result; never replay |
| terminal | duplicate enqueue with matching ID | unchanged | return terminal status; no duplicate event |

The queue coordinator has one serialization point for `Pending -> Cancelled` and `Pending -> Finalizing`. Exactly one synced event wins. A cancellation response is successful only after its durable marker; it is never sent after FinalizeWon. Conversely, FinalizeWon is never recorded if a durable cancellation already won. Quiescence stops new admission and new dispatch, but preserves durable Pending entries; it may wait for a Finalizing/active operation or fence it for recovery according to the gateway shutdown contract. It does not silently cancel queued work.

### Finalize and Archive duplicate prevention

The stable submission ID is the Archive user-turn node ID (or a reversible collision-free encoding of it), unique within its conversation. FinalizeWon fixes the final branch ID, exact parent node, committed timestamp, verified actor principal, normalized payload hash, and submission ID before any Archive mutation. The Archive-side `commit_queued_turn_once` seam must be idempotent: if the ID is absent, append that exact turn and sync; if present, compare conversation, role, principal, timestamp, parent, content hash, and staged attachment identities and return the original receipt only on exact equality. Any mismatch is a hard integrity conflict and blocks replay. The operation must not use a fresh timestamp, activity identity, Episode scheduling decision, or random node ID on retry.

After Archive sync, append/sync `ArchiveCommitted` (or a single `SubmissionCommitted` terminal record carrying the Archive receipt). If the crash occurs between these owners, recovery queries Archive by stable node ID. A matching node proves Archive commitment; journal replay then records terminal status without a second append. A missing node under durable FinalizeWon is retried with the exact captured fields. A mismatching node stops recovery. The journal itself never causes an Archive turn merely by being replayed as Pending.

The committed turn advances the resolved branch head and contributes Archive activity under the stable turn identity. Post-commit Episode/Insomnia scheduling is a separate idempotent step keyed by that same identity: after Archive sync, recovery retries scheduling until the owner queue/episode planner returns its durable receipt, then records `PostCommitScheduled` in the journal. The consumer must deduplicate by stable turn/operation ID before publishing derived work; a crash between its durable receipt and the journal marker is a safe retry. Queued/cancelled records are excluded from Archive activity and Episode/Insomnia work. Because Archive, journal, and processing receipt are separate durability domains, this is idempotent reconciliation, not a claim of cross-file atomicity.

### Ordering and branch policy

The conversation policy is durable conversation metadata; a missing value on legacy conversations means `SequentialChronological`. The default processes pending requests on one branch in the persisted ordering above. Only not-yet-finalizing requests may move as newly admitted requests arrive. Once FinalizeWon captures ancestry, no later timestamp can rewrite it. If the first item has finalized before an older delayed request arrives, that older timestamp cannot reorder history; the new request is processed after already committed ancestry. The gateway publishes queue position updates separately from committed-message updates.

An optional `VisibleAutoBranch` policy applies only when an accepted submission's expected branch head is stale because another concurrent submission advanced that branch. It preserves the submitted expected parent on a distinct branch with a stable branch ID derived from the winning submission ID, and emits an explicit branch-created event with a human-readable label and selected branch in the submitting instance. It never silently switches another instance's selected branch. Normal submissions against the current head remain sequential. A request whose expected parent is no longer valid for the conversation fails closed for explicit user resolution; it is not silently attached to an unrelated leaf. Branch ID, parent, policy revision, and display label are captured in FinalizeWon so recovery reproduces the same branch. Existing branch ancestry is append-only; the default policy never forks solely because another viewer attached.

A brief interpretation boundary remains: "genuinely simultaneous" is operationalized by equal gateway-verified timestamps, with the persisted random key resolving that tie. No client clock participates. If ingress later defines a wider simultaneity window, it must be an explicit versioned policy applied before acknowledgment; existing durable order keys are not rewritten.

## Generation, viewers, and recovery

One branch continuation can have one generation at a time. A queue item accepted while a generation is running remains Pending and is finalized against the next safe continuation; it cannot change the in-flight prompt or captured context. Each generated message uses a stable operation/generation ID and increasing checkpoint sequence. Before publishing a delta, sync its checkpoint; a late viewer attaches by an atomic snapshot-and-cursor handshake and receives no duplicate earlier deltas. The conversation/REL status feed reports provisional queue state and committed turn/branch changes; only authorized viewers actively attached to that conversation receive generation text.

Initiating instance, durable actor, and generation authorization are separate fields. User-turn authorship is the verified principal who submitted that turn. A generated operation records its initiator and authorization grant generation independently; it must never infer that principal from a parent agent turn. Recheck the instance grant generation and REL mount epoch immediately before dispatch and before Archive commit. Stale fences reject writes. Revocation behavior is owned by G4; this contract requires a denied commit to stay non-committed and visible as recovery/authorization failure rather than falling back to another principal.

On gateway process start, initiate configured host connection/capability exchange immediately and concurrently with local owner and journal recovery; do not wait for a long local replay before opening the host connection. Locally, open and verify each native queue journal, recover its valid prefix, reconcile Finalizing IDs against Archive, restore Pending order and durable cancellations, and rebuild/fence active generations from synchronized checkpoints as Interrupted or status-unknown. Host-bound dispatch waits for both local recovery and host cancellation/ownership reconciliation; cancellation reconciliation precedes replay. Independent native work may resume once its local owner and authorization checks pass. Unknown external operation status is held for reconciliation, never replayed blindly. A disconnected viewer reconnects by reauthorization plus authoritative conversation/head/queue/operation snapshot and checkpoint cursor; its old cursor is only an optimization. Recovery never infers a live viewer from durable `conversation_active`.

## Executable contract fixture

`tests/multiplexing_phase0b_conversation_contract.rs` is a self-contained, standard-library-only reference model. In PowerShell, run `rustc --edition=2024 --test tests/multiplexing_phase0b_conversation_contract.rs -o "$env:TEMP\\mux-g1-contract.exe"; & "$env:TEMP\\mux-g1-contract.exe"`, or run it through Cargo as an integration test. It explores durable event order, Archive state, crash points and viewer references. It is intentionally not linked to production runtime code; passing it shows the oracle is internally consistent, not that Reliquary satisfies it. Phase 1 must add runtime tests against the actual Archive and coordinator seams.

The fixture must cover at least:

- equal verified timestamps with persisted random ties; restart retains the exact order key, and same-tick admission can reorder only still-Pending work;
- every crash boundary around enqueue sync/ack, cancel sync/ack, FinalizeWon, Archive commit sync, and terminal journal sync;
- cancel-first and finalize-first races, each producing exactly one observable winner;
- duplicate enqueue/cancel and stable-ID/payload mismatch;
- recovery with Archive absent/present/matching/mismatching for a Finalizing submission, including exactly one activity/Episode scheduling receipt;
- two viewers attaching to one generation/checkpoint, one viewer switching/detaching, and the generation continuing for the other and after last detach;
- queue events never appearing in canonical turns, activity, Episode, or Insomnia inputs before finalization;
- stale owner/generation fence at dispatch/commit, and quiesce preserving pending work.

## Phase 1 handoff gate

G1 is ready for implementation only when the reference fixture passes and Phase 0B's other applicable authorization, attribution, event, locking, consumer, and executed-baseline gates are resolved. The implementation must add adversarial integration tests at the actual Archive/journal/gateway seams, demonstrate duplicate prevention across injected crash/reopen points, and pass the repository's required Rust, CLI-if-applicable, architecture and docs-policy checks. A passing reference model is necessary and not evidence of production correctness by itself.

## Related docs

- [Multiplexing implementation plan](multiplexing-implementation-plan.md)
- [Multiplexing readiness review](multiplexing-readiness-review.md)
- [Multiplexing Phase 0 baseline](multiplexing-phase0-baseline.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
- [Architecture](architecture.md), [invariants](invariants.md), and [storage format](storage-format.md)

## Notes

This document closes only the G1 design draft. Its reference-model tests are an executable contract oracle, not production test evidence. It does not close G2-G6 or approve Phase 1. The transcript timestamp-ordering migration seam, stable Archive idempotency API, gateway journal filesystem durability implementation, and post-commit scheduler deduplication still require implementation-level verification.

## Source observations

- `src/runtime_inference/runtime_host_session.rs`: one managed session per REL; starting/reopening closes the current one; close finalizes and toggles durable active metadata.
- `src/runtime_inference/interaction_session.rs` and `src/archive/interaction_stream.rs`: sessions are keyed by session ID; one in-flight message per session; completion accepts the Archive turn before advancing the local leaf.
- `src/archive/interaction_stream_persistence.rs`: streaming checkpoint is persisted and `sync()`ed before a checkpointed delta can be safely published.
- `src/archive/interaction_stream_record.rs`: persisted stream record lacks principal attribution today; recovery/late-join attribution must be covered by G2.
- `src/archive/archive.rs` / `src/archive/archive_history.rs`: nodes are Archive semantic records with Archive/global versions; a stable-ID commit seam must preserve those clocks and current Archive activity behavior exactly once.
- The conversation transcript/page read path currently orders merged turns using `timestamp_ns`; migration must keep UI display order separate from canonical branch ancestry. Equal and decreasing timestamps cannot sort committed turns into a different conversation path. Add runtime characterization for same-timestamp turns and a delayed older-timestamp turn.
- `AGENTS.md`: one mutable domain has one owner; derived checkpoints do not become authority; documentation is implementation; the standalone CLI and doc/architecture policy checks apply when touching those surfaces.
