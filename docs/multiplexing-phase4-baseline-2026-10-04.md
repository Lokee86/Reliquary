# Multiplexing Phase 4 — P4.0 baseline and mutation inventory

Parent: [Phase 4 implementation plan](multiplexing-phase4-distribution-implementation-plan-2026-10-04.md)
Contract: [G3/G4 event coverage matrix](multiplexing-phase0b-events-authorization.md)
Status: **P4.0 accepted on shared foundation 388bd8f. P4.1 is next.**

## Purpose

Record P4.0 verification and classify current gateway mutation families before distribution implementation.

## Overview

The actual shared P34 foundation passes the focused reference, API, gateway library, check, formatting and documentation gates. This inventory describes required publication points, not implemented event delivery.

## Baseline

P4 worktree: `C:\\!bin\\workspace\\reliquary-multiplexing-phase4`, branch `feat/multiplexing-phase4-distribution`.
Shared seam foundation: `388bd8f319ae0baafcc89f6c4c5288b78248a3ef` (`feat/multiplexing-p34-foundation`).
The required seam now exists at `388bd8f`: frozen observation types and lane module ownership are defined in `runtime_host_gateway_observation.rs` and the shared observation contract. P4 branches directly from this commit, independently of P3 at `31af314`, and carries over only this unique inventory file from Phase 2.

## Source-backed mutation classification

All paths below are under `src/runtime_inference/`. Ambient convenience methods
delegate to the explicit-owner methods and must not publish a duplicate event.
Read-only search/transcript/provenance methods are not additional mutators.

| Source and mutation family | Required classification | Publication boundary and privacy |
| --- | --- | --- |
| gateway_conversation: submit_conversation | SubmissionQueueChanged | REL journal durable enqueue; stable submission/order/status only, no queued body |
| gateway_conversation: cancel_conversation_submission | SubmissionQueueChanged; HostWorkChanged for delegated cancellation | Durable native cancel or authoritative host acknowledgement; cancellation request is not terminal completion |
| gateway_conversation: process_next_conversation_submission / process_accepted_conversation_submission | SubmissionQueueChanged, ArchiveChanged, ProcessingStatusChanged | FinalizeWon, Archive sync and durable postcommit/receipt transitions remain separate facts; coordinator owns semantics |
| gateway_conversation: recovery/reconcile | SubmissionQueueChanged, ConversationOperationChanged, OwnerLifecycleChanged or epoch reset | Reconcile journal/Archive first; never resurrect cancelled work |
| gateway_ambient: set_rel_metadata / set_rel_metadata_for | OwnerLifecycleChanged; ProjectStateChanged when project policy/correlation is affected; RetrievalInvalidated for dependency changes | REL metadata sync; safe changed-field and owner identifiers only |
| gateway_ambient: create_knowledge_memory / create_knowledge_memory_for; gateway_ambient_extended: replace_knowledge_memory_for | SemanticOwnerChanged + RetrievalInvalidated | Memory publication and Cva sync; memory ID/revision, never body |
| gateway_ambient: rename_knowledge_community_for | SemanticOwnerChanged + RetrievalInvalidated | Community-name mutation and sync; stable community ID/watermark |
| gateway_ambient: mutate_knowledge_relation_for | SemanticOwnerChanged + RetrievalInvalidated | Graph batch commit and sync; stable endpoint/relation identifiers |
| gateway_semantic: refresh_principal_projection_for / commit_principal_projection | SemanticOwnerChanged + RetrievalInvalidated only when REL projection changes | Both original capabilities and PHY fence retained through REL sync; principal-keyed invalidation, no private profile payload |
| gateway_semantic: set_phylactery_profile | No REL event | Exact PHY sync; a later explicit REL projection has its own event |
| gateway_ambient_extended: set_rel_processing_enabled_for | ProcessingStatusChanged | Ephemeral owner-local switch, explicitly labelled ephemeral and recovered from safe status snapshot |
| gateway_ambient_project: validate_project_environment_for | ProjectStateChanged only if correlation changes | This apparent validation/read API can write correlation; publish after Cva sync |
| gateway_ambient_project: checkpoint_manual_project_for | ProjectStateChanged | External Git/filesystem work outside distribution/owner locks; publish REL correlation only after sync |
| gateway_ambient_project: ingest_project_attachment_for | ProjectStateChanged + ArchiveChanged | Correlation/file registration and Cva sync; file/revision IDs only, no file content or paths |
| gateway / gateway_ambient_extended: quiesce_rel[_for], reopen_rel[_for], reconcile_quiesced_rel_for, mount_rel/path, unmount_rel, shutdown | OwnerLifecycleChanged or epoch invalidation/teardown | Drain leases, stop/reconcile/restart owners at existing lifecycle boundaries; old epoch subscriptions invalid |
| gateway/admission: attach_authenticated_instance, refresh_authenticated_instance, replace_instance_grants, detach_instance | Authorization/subscriber lifecycle, no shared semantic mutation | Admission close/drain, grant-generation advance and outbound removal before later delivery |
| gateway: select_rel; gateway_conversation: select_managed_conversation, attach/detach_conversation_viewer, admit_conversation_writer | Instance-local selection/observation/admission; no broad semantic event by itself | Viewer interest is neither cancellation authority nor branch creation |
| gateway_phy_lifecycle and PHY mount/unmount | Private owner lifecycle, no private REL payload | Exact PHY epoch/fence; REL status only if a REL-owned routed operation changes |
| gateway: reconnect_host; gateway_host router remember/hold/release/handoff/retry/status/cancel | HostWorkChanged / OwnerLifecycleChanged | Authoritative host acknowledgement/reconciliation; Unknown remains held, no blind redispatch |
| REL-owned publication and workers: Insomnia Episode/receipt, Dream/Perception semantic commits, vectors/index/profile updates | ProcessingStatusChanged; SemanticOwnerChanged and RetrievalInvalidated for resulting REL changes | Publish after each owner's actual commit/sync; ephemeral work status explicitly labelled; no inference prompts |
| Trusted standalone Cva/Phylactery facades and quiesced maintenance | Maintenance bypass: correct summary or epoch rotation + resync | These are not gateway-authenticated instance APIs; never instrument every low-level facade |
| Generation checkpoint/completion/interruption and automatic branch policy | ConversationOperationChanged / ArchiveChanged; exact stream for text | Future P3 post-durability observation seam; checkpoint sync precedes visibility; no current production stream publisher |

The inventory assigns every existing gateway mutation family a class or explicit
non-feed treatment. Classification does **not** mean publication is implemented:
the sequencer/subscriber/stream modules remain Phase 4 work.

## Foundation acceptance verification

- Actual source: shared `388bd8f` foundation in the dedicated P4 worktree.
- `cargo test --locked --offline --lib gateway:: -j 2 -- --test-threads=1`: **115/115 passed**.
- G3/G4 reference fixture and public gateway contract: **10/10 + 1/1 passed**.
- `cargo check --locked --offline --lib -j 2`, formatting, structural docs and change-impact docs: **passed**.
- The initial aggregate runner hit its 900-second limit during the additional library check after all test gates had passed. The standalone check/format/docs continuation passed; the timed-out check is not counted as evidence.

## Historical Phase 2 verification

- `cargo test --locked --test multiplexing_phase0b_events_auth_contract --test multiplexing_gateway_api_contract -j 2`: passed, 10 reference cases and 1 public API case.
- `cargo test --locked --lib -j 2`: passed, 908 cases, including gateway authorization/lifecycle, conversation, privacy and publication tests.
- Initial `cargo test --locked --lib gateway:: -j 2` reached its 300-second dependency-compilation timeout before tests. The full-library pass supersedes that incomplete run; no baseline test failed.
- Documentation structural and change-impact checks initially found missing Purpose/Overview sections and an index link in this new record. Those omissions were corrected; both final checks passed with zero new findings. `git diff --check` also passed.
- A separate `feat/multiplexing-p34-foundation` worktree appeared during verification, still based at the same accepted P2 head. Its work is not modified by P4.0.

## Exit decision

P4.0 is accepted. All instance-visible mutation families are classified and the focused gates passed against the actual shared `388bd8f` foundation. P4.1 continues on this independent branch; no Phase 2 dirty state or P3 commit was used.

## Related docs

- [Phase 2 acceptance](multiplexing-phase2-implementation-plan-2026-10-04.md)
- [Phase 3 seam contract](multiplexing-phase3-conversation-implementation-plan-2026-10-04.md)
- [Documentation index](INDEX.md)

## Notes

This is a preparation/verification record, not a claim that production feeds exist.
