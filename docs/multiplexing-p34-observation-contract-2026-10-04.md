# Multiplexing P3/P4 frozen observation contract

Parent index: [Documentation index](INDEX.md)

## Purpose

Define the shared data boundary used by the parallel conversation and distribution lanes.

## Overview

The foundation extends accepted Phase 2 head `c883f3655f86087f7cfd2d7dab395a3944a6df23` with the internal `gateway::observation` module. It defines data only. It has no queue, event bus, provider implementation, subscriber registry, mutable store handle or public API re-export. P3 implements producers; P4 implements consumers and delivery.

## Contract and ownership

All source paths are under `src/runtime_inference/`.

| Contract | Frozen source type | Producer and allowed consumer |
| --- | --- | --- |
| Safe REL observation | `RelObservationSummary`, `RelConversationSummary`, `DurableWatermarks` | Gateway/P3 owner readers; P4 broad snapshot adapter |
| Conversation observation | `ConversationObservationSnapshot`, `BranchObservation`, `SubmissionObservation`, `GenerationObservation`, `HostOperationObservation` | P3 coordinator; P4 conversation snapshot adapter |
| Post-durability facts | `ConversationDurableDelta`, `ConversationTransition` | P3 after the named durability point; P4 assigns notification envelopes/cursors |
| Durable checkpoint identity | `GenerationCheckpointObservation`, `CheckpointContentIdentity` | P3 after checkpoint sync; exact authorized P4 stream only |
| Checkpoint text | `GenerationCheckpointContent` | P3 checkpoint reader; exact authorized P4 stream only |
| Policy vocabulary | `ConversationPolicy` | P3 canonical metadata/scheduler; missing legacy value means SequentialChronological |

P3.1 defines the policy enum beside canonical Archive conversation metadata; the observation module imports that enum. Its two variants and observation shapes are unchanged, so no parallel policy vocabulary or compatibility wrapper is introduced. See [P3.1 ownership](multiplexing-phase3-policy-acceptance-2026-10-04.md).

Submission identity/state types reuse the existing REL journal. Order is the exact verified receive-time/tie-key/admission-sequence tuple. Host phase/capability reuse the existing host journal/router types. Branch and operation identities are stable IDs rather than writable session references.

REL summaries exclude submission bodies, attachments, request metadata, checkpoints, text and PHY data. Conversation snapshots also exclude text. Only the separate checkpoint-content type carries text. Broad generation summaries may carry durable checkpoint sequence as a status watermark, but never its content digest or prefix/delta. Selection is a requesting-instance projection and does not appear in broad REL summaries.

Watermarks are optional source clocks. None means unavailable, not a fabricated zero or a transient delivery revision. P3 checkpoint/journal sequences remain distinct from P4 delivery cursors and semantic versions.

## Durability and synchronization

SubmissionChanged follows durable enqueue/cancel or another synchronized journal transition. FinalizeWon follows durable capture. ArchiveCommitted follows Archive sync and exact receipt recording. PostCommitRecorded follows durable scheduling receipt. BranchCreated follows stable branch/parent/label capture and successful durable branch publication; it is not permission to announce an uncommitted branch. PolicyChanged follows canonical metadata sync. GenerationChanged and HostOperationChanged follow durable operation state; they cannot invent an acknowledgement for Unknown host work.

Checkpoint observations are created only after checkpoint synchronization and bind REL/epoch/conversation/operation/generation/branch/sequence to exact UTF-8 length and SHA-256 content identity. A delta names its preceding durable checkpoint; a prefix is independently reconstructable. P3 owns reconstruction and validation; P4 owns stream recipient checks and delivery order.

Data values alone do not make a snapshot atomic. P3.7 owns coherent coordinator/journal/Archive observation and a documented lock order. P4 owns its snapshot/register publication gate. At convergence, mutations and snapshots must share the necessary linearization boundary; neither a copied struct nor a test provider proves that production handshake.

## Lane file ownership

P3 owns `runtime_host_gateway_conversation.rs`, `multiplexing_journal.rs`, conversation policy/generation modules and their tests. P4 owns `runtime_host_gateway_events.rs`, `runtime_host_gateway_subscriptions.rs`, `runtime_host_gateway_streams.rs` and their tests. The existing gateway root declares the shared observation module in this foundation.

The lanes add real modules with implementation on their own branches; this foundation adds no empty distribution modules or placeholder bus. Their root declarations and public re-exports are reconciled at convergence. Shared observation shape changes require a common foundation update rather than divergent compatibility wrappers.

Both lane branches must start from the same frozen foundation commit, not independently from Phase 2. No P3 or P4 runtime behavior is claimed by the foundation.

## Verification

The [P3.0 baseline record](multiplexing-phase3-baseline-2026-10-04.md) records source inventory, gates and the exact foundation commit after packaging.

## Related docs

- [Phase 3 implementation plan](multiplexing-phase3-conversation-implementation-plan-2026-10-04.md)
- [Phase 4 implementation plan](multiplexing-phase4-distribution-implementation-plan-2026-10-04.md)
- [Architecture](architecture.md)
- [Phase 0B events and authorization](multiplexing-phase0b-events-authorization.md)

## Notes

The module permits unused internal contract definitions during the declared P3/P4 implementation window. The allowance is scoped to this data-only module and should be removed once the lanes consume the complete contract.
