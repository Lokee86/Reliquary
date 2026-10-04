# Multiplexing Phase 1 — finish plan

Parent: [Multiplexing implementation plan](multiplexing-implementation-plan.md)
Prior completion plan: [Phase 1 remaining work](multiplexing-phase1-remaining-work-2026-10-03.md)
Authoritative design: [Phase 0B integration](multiplexing-phase0b-design.md) and [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
Status: **final Phase 1 execution plan; narrow remaining work only**
Snapshot: 2026-10-04, branch `feat/multiplexing-phase1`

## Purpose

Close Phase 1 from the current late-stage implementation without reopening work already proven green or pulling Phase 2–5 responsibilities forward. This document supersedes the older Phase 1 completion plan for **remaining execution only**. The older plan remains the historical design/audit record.

Phase 1 is complete when the new in-process `GatewayRuntime` path has one owner authority per mounted REL/PHY, trustworthy instance admission, independent instance selection, one durable shared conversation coordinator, safe host-stage authority transfer for the capability actually integrated in Phase 1, and bounded fencing/quiescence. Final migration/removal of the legacy host and the remaining host worker capabilities are later phases.

## Overview

The remaining work is intentionally narrow: finish the already-implemented `ConversationExecution` ownership edge, prove durable accepted work survives instance loss, close the gateway routing/lifecycle/authorization audits, then execute one final serial verification matrix and package the branch. Previously green admission, journal, instance, library, integration, CLI and documentation work should be reused rather than reimplemented.

**Current progress:** F1–F5 ownership/recovery, accepted-work lifetime, routing, lifecycle/fencing and expanded authorization are complete locally. F6 final verification and packaging remains open.

## Verified starting point

Do not rerun or redesign these unless a subsequent change invalidates them:

- `cargo fmt --all -- --check`, `git diff --check`, and `cargo check --locked -j 2` have passed on the late Phase 1 source.
- Final focused runtime verification passed:
  - gateway: **43/43**
  - REL journal: **15/15**
  - instances: **7/7**
  - legacy/gateway exclusion: **3/3**
  - public gateway API contract: **1/1**
- Full library tests passed **836/836**.
- Integration tests passed, including all Phase 0B executable contracts and the new public admission/durable-receipt contract.
- CLI verification passed after lock refresh: check green and **19/19** CLI tests.
- Both documentation checks passed.
- The unrestricted `cargo test --locked` command still fails while building the pre-existing `archive_open_profile` example because its global allocator conflicts with `lore_base`. This is an inherited example/build issue, not a Phase 1 regression. Do not expand Phase 1 to fix it.
- Refreshed architecture indexing timed out after five minutes while Lexicon was still analyzing Rust/Python. Treat that as an **unresolved structural-evidence check**, not as proof of an architecture failure.
- All Phase 1 changes remain uncommitted from branch base `2b7dc5c`.

## Scope correction for P1.2

`RuntimeCapability` currently enumerates `ConversationExecution`, `Insomnia`, `Dream`, `Embedding`, and `ProjectEnvironment`. Phase 1 does **not** require production host handoff integration for all five.

The Phase 1 requirement is:

1. `ConversationExecution` is the one production host-stage transfer implemented in Phase 1 and must satisfy durable handoff/cancel/status/recovery semantics.
2. Any other capability not yet connected to a production gateway stage must remain explicitly `Native` or `Held`; it may not become host-owned merely because an adapter advertises it.
3. The capability router must preserve independent per-capability authority, startup reconciliation, downgrade fencing and ownership debt so Phase 5 can migrate the remaining worker capabilities without redesigning the seam.
4. Real Warlock queue/worker conformance is Phase 5.

This closes the ambiguity that previously made P1.2 look much larger than Phase 1 actually requires.

## Remaining critical path

### F1 — Close ConversationExecution host ownership

The existing conversation host path already persists stable handoff identity, payload digest, last known remote phase and sticky cancellation in the REL journal. Existing tests prove lost acknowledgements, unknown status hold, digest conflict, restart retry, cancellation racing a blocked handoff, and idempotent completion.

Finish only the remaining authority edges:

- Ensure startup restores every unresolved durable host operation into capability ownership debt **before** conversation workers may dispatch.
- Ensure reconnect cannot clear capability debt while any owned operation remains unresolved.
- Ensure a capability contract downgrade transitions new work to `Held`/native-safe behavior without redispatching an unresolved host-owned operation.
- Ensure cancellation received after durable handoff intent but before or during adapter acknowledgement is persisted first, reaches the host, and cannot later be resurrected by native replay.
- Ensure `Completed` finalizes the original admitted conversation turn exactly once; `CancelledBeforeStart` permits terminal native cancellation; `Running`/unknown outcomes remain honest nonterminal holds.
- Do not add host assistant-result semantics. Phase 1's ConversationExecution contract is about execution ownership of the admitted turn only.

**Required tests:** retain the existing host-operation suite and add only any missing direct regression for startup ordering or downgrade/reconnect debt. No new generalized host scheduler.

**F1 exit:** every ConversationExecution operation has exactly one stage authority across native pending, durable handoff intent, host ownership, cancellation and terminal Archive publication.

**F1 status: complete on 2026-10-04, locally on `feat/multiplexing-phase1`.** Six focused regressions were added and the existing restart retry regression was strengthened. Remaining F2–F6 work is unchanged; all implementation changes remain uncommitted for later F6 packaging.

The router now preserves each locally unresolved operation ID despite an adapter's empty-queue report, contract withdrawal or removed capability configuration. IDs are released only after a durable terminal result; capability debt still requires subsequent empty reconciliation. Fresh contract negotiation is retained even when operation ownership changes during reconnect, so a withdrawn dispatch contract cannot reappear. The gateway registers handoff debt before leaving the durable intent boundary. Existing startup recovery was audited and remains before owner worker start. Durable Completed/Cancelled proofs are consumed locally before adapter access, including completion after sticky cancellation intent.

### F1 verification record — 2026-10-04

All Cargo work was serial, with two build jobs and one shared target. No new inference, host scheduler, assistant-result protocol or journal format was introduced.

| Command / gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `git diff --check` | Passed |
| `cargo check --locked -j 2` | Passed |
| `cargo test --locked --lib gateway:: -j 2 -- --test-threads=2` | **49/49 passed** |
| `cargo test --locked --lib -j 2 -- --test-threads=2` | **842/842 passed**, including existing journal/instance/legacy-isolation cases |
| `cargo test --locked --test multiplexing_baseline_contract --test multiplexing_gateway_api_contract -j 2 -- --test-threads=2` | Both contracts passed, **1/1 each** |
| `cargo fmt --manifest-path cli/Cargo.toml -- --check` | Passed |
| `cargo check --manifest-path cli/Cargo.toml --locked -j 2` | Passed |
| `cargo test --manifest-path cli/Cargo.toml --locked -j 2 -- --test-threads=2` | **19/19 passed** |
| `cargo test --locked -j 2 -- --test-threads=2` | Only the inherited `archive_open_profile` / `lore_base` global-allocator build conflict; no new failure reported |
| Both documentation checks, including `--changed-from origin/main` | Passed; repeated after this final record was added |
| `python scripts/check_architecture.py --refresh` | Timed out after **180 seconds** in Lexicon Rust/Python indexing; the process tree was stopped |
| Direct gateway source audit | No legacy host construction or zero-argument global selection/execution dependency. Seven `active_rel_id(instance)` matches were inspected and resolve the gateway's attached initiating instance via `instance.selected_rel()` |

The indexing timeout remains an unresolved structural-evidence check, not a passing refreshed Pitlord result. The finish plan permits this bounded timeout with direct source evidence; F1 tests and current-state documentation are green. Full Phase 1 acceptance remains open.

Documentation impact:
- Inspected: architecture, invariants, storage format, maintainer map, API, behavioral contracts, current limitations, roadmap and both execution plans.
- Updated: F1 verification/plan routing; current host recovery semantics; contract-to-test mapping; API, limits and maintainer evidence.
- Not affected: REL/PHY and journal formats, CLI interface, semantic subsystem policies.
- Compliance check: structural and change-impact documentation checks passed.
- Known documentation gaps: no F1 gap identified; remaining Phase 1 work is explicitly open.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: journal terminal proof remains authoritative; capability readiness cannot erase unresolved operation ownership.
- Pitlord enforcement impact: repository policy unchanged; refreshed indexing timed out, so no refreshed policy pass is claimed.
- Other verification impact: deterministic reconnect/channel and recovery fixtures, full library/CLI regression, public contracts and direct source audit passed.
- Known architectural gaps: fresh Lexicon/Arcana structural evidence remains unavailable; real host conformance and later capabilities remain later-phase work.

### F2 — Prove accepted work outlives the submitting instance

The remaining conversation/lifecycle ambiguity is accepted native work after the initiating instance disappears.

- Once a submission is durably admitted, processing authority belongs to the REL conversation coordinator/journal, not to the transient instance object.
- Capture immutable principal/authorization facts required for that accepted operation at admission.
- Detaching the submitting instance must remove its viewer/delivery state and future authority, but must not orphan or silently cancel already accepted durable work.
- Cancellation after detach must require a currently attached instance with the explicit applicable cancel grant; old attachment tokens and stale grant generations must fail.
- Finalization must still revalidate the owner mount/fence and durable captured operation identity before Archive publication.
- Zero viewers must not destroy the shared continuation or accepted queue.

**Required tests:** submit → detach before processing → another authorized instance or internal coordinator completes; submit → detach → unauthorized cancel denied; submit → detach → authorized shared cancel succeeds if still pending; accepted item survives fresh gateway restart with no initiating instance attached.

**F2 exit:** durable admission lifetime is owner/coordinator-scoped, while user control remains attachment-authorized.


**F2 status: complete on 2026-10-04, locally on feat/multiplexing-phase1.** Six lifecycle/compatibility regressions were added. Accepted execution belongs to the REL journal/coordinator; attachment grants authorize new requests and receipt delivery. F3–F6 remain open. Changes remain uncommitted for F6 packaging.

Admission captures the verified principal, canonical owner/conversation/branch scope, SubmitConversation grant and policy revision in existing journal metadata. A different authorized instance or the internal owner coordinator with no attachments can finish the original turn. Detach and grant removal preserve its queue and shared continuation. Native/host journal transitions and Archive publication use mounted-owner epoch/fence checks; finalization validates durable key, principal, branch and policy identity. Cancellation requires a currently attached applicable own/shared grant. Stale attachment and revoked grant tokens fail. Public receipt delivery revalidates the caller after processing.

Earlier admissions without policy metadata retain original authority and exact request fingerprints. Exact retries match without rewriting the admission; changed content fails. Startup reconciles existing publication and preserves unfinished entries for explicit coordinator processing. No background conversation scheduler or new journal frame/event/container version is introduced.

| F2 contract | Runtime regression |
| --- | --- |
| Submit, detach with zero viewers, then finish through a different principal; preserve principal and continuation | accepted_turn_outlives_detach_and_keeps_its_admitted_principal; updated fresh-gateway Finalizing recovery test |
| Detached, foreign own-only and stale-grant cancel attempts fail; current shared cancel succeeds while pending | cancellation_after_submitter_detach_requires_current_applicable_grant |
| Restart after grant removal without attachments; retained payload/attachment completes | accepted_queue_survives_restart_and_runs_without_any_attachment |
| Changed finalize identity and stale owner epoch/fence cannot publish | accepted_finalization_rejects_changed_identity_and_stale_owner_fence |
| Detach during blocked host handoff preserves acknowledgment/completion, denies stale caller delivery | detach_during_handoff_preserves_receipt_and_owner_continuation |
| Earlier admission exact retries preserve authority/fingerprint; changed content fails | earlier_admission_retries_preserve_original_authority_and_fingerprint |

All Cargo work was serial with two build jobs, test concurrency 2 and one shared target. Logs and exits are under ignored target/f2-verification/.

| Command | F2 result |
| --- | --- |
| cargo fmt --check; git diff --check | Passed |
| cargo test --locked --lib gateway:: -j 2 -- --test-threads=2 | **55/55 passed** |
| cargo check --locked -j 2 | Passed |
| cargo test --locked --lib -j 2 -- --test-threads=2 | **848/848 passed** |
| Public multiplexing_baseline_contract and multiplexing_gateway_api_contract tests, locked, -j 2 | Both **1/1 passed** |
| CLI fmt/check/test through --manifest-path cli/Cargo.toml, locked check/test, -j 2 | Passed; **19/19 CLI tests** |
| Unrestricted cargo test --locked -j 2 -- --test-threads=2 | Inherited archive_open_profile example allocator conflict with lore_base; unchanged from F1 and outside this plan's blocking gates |
| python ../engineering-standards/tools/docs_policy/check.py --repo . and --changed-from origin/main | Passed, including final documentation record |
| python scripts/check_architecture.py --refresh | Timed out after 180 seconds during Rust/Python index refresh; process tree terminated. No fresh Pitlord pass claimed |
| Direct gateway dispatch/ownership review | No legacy/global owner-selection dispatch; 7 initiating-instance selectors reviewed |

Documentation impact:
- Inspected: required architecture, invariants, storage, maintainer-map, limitations and roadmap docs; API, behavioral contracts, implementation/finish plans, AGENTS and engineering standards.
- Updated: architecture, invariants, storage format, API, behavioral contracts, maintainer-map, current limitations and multiplexing implementation/finish plans.
- Not affected: REL/PHY container schemas, CLI commands, semantic subsystem policy and roadmap sequencing.
- Compliance check: both docs-policy modes and final whitespace checks passed.
- Known documentation gaps: F3–F6 and later Phase 0B/consumer contracts remain explicitly open.

Architecture impact:
- Standards added or changed: no new standard; existing owner/coordinator lifetime and immutable-admission invariants clarified.
- Ownership or boundary impact: durable accepted execution is REL-owned; client admission, cancellation and response delivery remain attachment-authorized.
- Pitlord enforcement impact: Timed out after 180 seconds during Rust/Python index refresh; process tree terminated. No fresh Pitlord pass claimed.
- Other verification impact: gateway, full library, public contracts and CLI passed; graph corpus paths were not changed. The inherited example allocator failure remains.
- Known architectural gaps: Fresh Pitlord indexing remains unverified, alongside F3–F6 acceptance gates; full Phase 1 and production host conformance are not claimed.

### F3 — Close the instance-routing audit

The new gateway path already has independent per-instance REL and managed-conversation selection and a broad instance-authorized ambient/project API. Finish the audit rather than migrating the entire old host.

- Enumerate every **public/new GatewayRuntime user-facing method** and verify it obtains the target from the initiating `InstanceRuntime` or an explicitly authorized owner ID.
- No gateway method may consult legacy `ReliquaryRuntimeHost::active_rel_id()`, `active_execution()`, global managed-session state, or another process-global selection fallback.
- Explicit maintenance APIs may use owner IDs directly but must not be exposed as untrusted request admission.
- Preserve the shared process claim that prevents a legacy host and gateway from concurrently owning the same REL/PHY.
- Do not delete the remaining legacy host APIs here. Final consumer migration and deletion stay in Phase 5.

**Required evidence:** direct source audit plus the existing public API and legacy-isolation tests. Add a compile/source guard test only if it can enforce the forbidden dependency cheaply and locally.

**F3 exit:** new gateway call direction is instance/explicit-owner only; legacy authority is isolated rather than synchronized.


**F3 source audit — 2026-10-04.** The inventory below enumerates all 88 externally public GatewayRuntime methods in the current production impl blocks, including trusted service controls. Token accessors, capability-router methods and pub(super) coordinator helpers are separate types/internal seams, not additional public GatewayRuntime request entry points. No routing defect was found; no legacy API migration or selection synchronization was introduced.

Ambient reads resolve active_rel_id(instance) through that attached instance's selected_rel and then authorize ViewRel. Ambient writes resolve the same initiating instance before authorizing ManageRel on the explicit owner. Paging and text/vector/dependency search wrappers preserve that route. Explicit reads and historical Echo/provenance authorize each exact source owner; dependency traversal intersects the same attachment's grants. Project mutations authorize ManageProject; resolved file reads authorize ViewRel. Instance management authorizes ManageRel on the exact owner. Conversation commands authorize SubmitConversation, ViewRel or the applicable cancellation grant on the requested owner; managed selection is stored on that instance. Viewer removal accepts only its private minted owner/mount/nonce/instance token. Durable accepted-work continuation remains the internal owner-scoped F2 seam.

Credential attachment and refresh invoke the configured trusted admission adapter; caller-supplied grants and raw mutable owner handles remain internal. Instance-free mount/unmount, raw owner enumeration/diagnostics, quiesce/reopen, host controls and shutdown are embedding-service controls, not untrusted request endpoints. The public type documentation now states this boundary. Hosts exposing client listing/maintenance must use viewable_rel_owners and instance-taking management methods.

The only process-global gateway registry is the existing live mount claim. Its purpose is exclusion by owner UUID and physical/canonical identity across gateway and legacy REL/PHY execution, not user selection. The shared claim remains intact, including legacy typed-prefix normalization and dynamic PHY attachment. Remaining legacy consumer migration/deletion is Phase 5.

The new multiplexing_gateway_routing_guard integration test discovers all production runtime_host_gateway*.rs modules, rejects the exact legacy host type (including aliased imports), active_execution and zero-argument active_rel_id calls, and verifies that active_rel_id(instance) plus the shared error type remain permitted. This conservative textual guard enforces those forbidden dependencies; it does not prove arbitrary future indirect/global fallbacks or replace this complete inventory and runtime isolation evidence. F4/F5 concurrency/authorization completeness remains open.

| Module | Public method | Target / admission boundary |
| --- | --- | --- |
| `runtime_host_gateway.rs` | `new` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway.rs` | `new_with_host` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway.rs` | `state_dir` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway.rs` | `capability_state` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway.rs` | `capability_authority` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway.rs` | `reconnect_host` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway.rs` | `select_rel` | Attached initiating instance + explicit owner |
| `runtime_host_gateway.rs` | `authorize` | Attached initiating instance + explicit owner |
| `runtime_host_gateway.rs` | `detach_instance` | Attached initiating instance selection/state |
| `runtime_host_gateway.rs` | `active_rel_id` | Attached initiating instance selection/state |
| `runtime_host_gateway.rs` | `instance_managed_state` | Attached initiating instance + explicit owner |
| `runtime_host_gateway.rs` | `mounted_rel_ids` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway.rs` | `mounted_phy_ids` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway.rs` | `owner_admission` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway.rs` | `quiesce_rel` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway.rs` | `reopen_rel` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway.rs` | `shutdown` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway.rs` | `unmount_phy` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway.rs` | `unmount_rel` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway_admission.rs` | `with_admission_adapter` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway_admission.rs` | `attach_authenticated_instance` | Verified credential admission |
| `runtime_host_gateway_admission.rs` | `refresh_authenticated_instance` | Verified credential admission |
| `runtime_host_gateway_ambient.rs` | `conversation_summaries` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `conversation_summaries_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `conversation_transcript` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `search_conversation_branch` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `echo_events` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `conversation_transcript_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `search_conversation_branch_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `echo_events_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `read_echo_source_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `files_for_source` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `files_for_source_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `project_file_ref` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `project_file_ref_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `file_bytes` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `file_bytes_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `search_archive` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `search_archive_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `read_reliquary_knowledge` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `read_reliquary_knowledge_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `knowledge_memory` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `knowledge_memory_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `read_memory_provenance` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `read_memory_provenance_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `resolve_external_memory_provenance_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `rel_metadata` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `rel_metadata_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `dependency_closure` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `dependency_closure_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `set_rel_metadata` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `set_rel_metadata_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `create_knowledge_memory` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient.rs` | `create_knowledge_memory_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `rename_knowledge_community_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `mutate_knowledge_relation_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `retrieve_reliquary_memories_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient.rs` | `background_status` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient_extended.rs` | `viewable_rel_owners` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient_extended.rs` | `conversation_transcript_page` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient_extended.rs` | `conversation_transcript_page_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient_extended.rs` | `search_dependency_archive_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient_extended.rs` | `search_dependency_archive` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient_extended.rs` | `conversation_summaries_page` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient_extended.rs` | `search_archive_page` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient_extended.rs` | `replace_knowledge_memory_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient_extended.rs` | `search_memories_by_text_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient_extended.rs` | `search_memories_by_text` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient_extended.rs` | `retrieve_reliquary_memories` | Attached initiating instance selection/state |
| `runtime_host_gateway_ambient_extended.rs` | `set_rel_processing_enabled_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient_extended.rs` | `quiesce_rel_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient_extended.rs` | `reopen_rel_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient_extended.rs` | `reconcile_quiesced_rel_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient_project.rs` | `project_repository_root_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient_project.rs` | `validate_project_environment_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient_project.rs` | `checkpoint_manual_project_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient_project.rs` | `ingest_project_attachment_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_ambient_project.rs` | `resolved_file_bytes_for` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_conversation.rs` | `submit_conversation` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_conversation.rs` | `admit_conversation_writer` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_conversation.rs` | `select_managed_conversation` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_conversation.rs` | `attach_conversation_viewer` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_conversation.rs` | `detach_conversation_viewer` | Opaque viewer token: owner/mount/nonce/instance |
| `runtime_host_gateway_conversation.rs` | `cancel_conversation_submission` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_conversation.rs` | `process_next_conversation_submission` | Attached initiating instance + explicit owner |
| `runtime_host_gateway_mount.rs` | `mount_rel_path` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway_mount.rs` | `mount_phy_path` | Trusted embedding-service control/diagnostic |
| `runtime_host_gateway_mount.rs` | `owner_mount_is_path_scoped` | Trusted embedding-service control/diagnostic |

**F3 status: complete on 2026-10-04, locally on feat/multiplexing-phase1.** All 88 public methods were inventoried; direct routing helpers, managed selection, dependency/provenance targeting and shared mount claims were reviewed. A brittle embedding-revocation assertion now also accepts the valid fenced-admission denial. The operation remained denied; no authorization behavior was weakened. F4–F6 remain open and changes remain uncommitted for F6 packaging.

| F3 gate | Result |
| --- | --- |
| cargo fmt --all -- --check; git diff --check; cargo check --locked -j 2 | Passed |
| cargo test --locked --lib gateway:: -j 2 -- --test-threads=2 | 55/55 passed |
| cargo test --locked --lib legacy_isolation -j 2 -- --test-threads=2 | 3/3 passed |
| Both public multiplexing contracts | 1/1 each passed |
| multiplexing_gateway_routing_guard | 2/2 passed |
| cargo test --locked --lib -j 2 -- --test-threads=2 | 848/848 passed |
| Both documentation checks | Passed; final record checked again |
| Unrestricted cargo test --locked -j 2 -- --test-threads=2 | Only the inherited archive_open_profile/lore_base allocator build conflict |
| python scripts/check_architecture.py --refresh | 180-second Rust/Python indexing timeout; process tree stopped; no fresh Pitlord pass claimed |

Logs are under ignored target/f3-verification/. Public signatures and CLI behavior were not changed in F3; F2's 19/19 CLI evidence remains valid and F6 repeats the CLI matrix.

Documentation impact:
- Inspected: required architecture, invariants, storage, maintainer-map, limitations and roadmap docs; API, behavioral contracts, implementation/finish plans, AGENTS and shared engineering standards.
- Updated: public control/admission boundary in API and architecture; complete F3 inventory and verification; behavior/test mapping, maintainer routing, implementation status and limitations.
- Not affected: persistent formats, semantic subsystem policy, CLI interface and later-phase sequencing.
- Compliance check: both documentation policy modes passed, including the final record.
- Known documentation gaps: F4–F6 and later consumer/privacy/event-feed contracts remain explicitly open.

Architecture impact:
- Standards added or changed: none; a local source guard enforces the existing direct call-direction boundary.
- Ownership or boundary impact: no routing/ownership migration; instance selection, explicit-owner grants and trusted maintenance controls are documented separately.
- Pitlord enforcement impact: refreshed indexing timed out after 180 seconds; no refreshed policy pass claimed.
- Other verification impact: source guard, instance-routing gateway regressions, shared legacy claim tests, public contracts and full library passed.
- Known architectural gaps: fresh structural indexing remains unavailable; F4/F5 and later-phase contracts remain open.

### F4 — Finish lifecycle/fencing adversarial cases

Existing tests already cover quiesce/reopen, stale-token fencing, grant-generation replacement, detach isolation, shutdown and separate gateway/legacy claims. Add only the remaining adversarial cases needed to prove the actual exit invariant:

- accepted pending work is frozen, not discarded, during quiesce;
- no new dispatch/commit lease begins after admission closes;
- in-flight short commit/delivery leases drain before grant revocation, detach, unmount or maintenance completes;
- a stale owner token cannot publish after reopen/remount epoch advance;
- failure during reconcile/reopen leaves that owner closed/held rather than partially reopened;
- quiescing one REL does not stop an unrelated REL;
- no provider/network/Git/filesystem work runs while holding a REL/PHY semantic owner lock;
- journal writer and semantic owner locks are never nested in reverse order.

Prefer deterministic barriers/channels over sleeps.

**F4 exit:** after a lifecycle transition returns, no stale attachment/owner capability can commit or deliver, no accepted work is lost, and unrelated owners continue.


**F4 status: complete on 2026-10-04, locally on feat/multiplexing-phase1.** Six deterministic regressions were added and delivery draining now covers both revoke and detach without scheduling sleeps. Reopen validates fence capacity before reconciliation or worker restart; an exhausted fence leaves admission closed and workers stopped, and a later reopen failure stops restarted workers.

| F4 contract | Evidence |
| --- | --- |
| Short commits drain before revoke, detach, quiesce, unmount and maintenance; closed admission rejects new leases | short_commit_leases_drain_before_lifecycle_transitions |
| Delivery drains before revoke and detach; no new delivery lease after closure | instance_delivery_leases_drain_before_revoke_and_detach |
| Quiesce preserves pending work and allows an unrelated REL to complete | accepted_queue_freezes_during_quiesce_without_blocking_another_rel |
| Reconcile conflict and fence exhaustion leave the owner closed with workers stopped | reconcile_failure_keeps_owner_closed_and_workers_stopped; failed_fence_reopen_does_not_restart_owner_workers |
| Remount fences old accepted-operation publication authority | accepted_owner_authority_cannot_publish_after_remount |
| Blocked host I/O holds no semantic/journal locks; late acknowledgment is fenced and durable ownership debt survives | quiesce_during_handoff_fences_late_ack_and_preserves_owner_debt |

The late-ack regression restores the durable Accepted receipt first, then consumes terminal host status on the next coordinator pass. Host acknowledgment is not terminal completion. Existing accepted-operation helpers separately acquire journal and semantic locks; no reverse nested lock acquisition was introduced.

F4 verification: cargo fmt/check and diff checks passed; gateway **61/61**; public API **1/1** and routing guard **2/2**; both documentation policy modes passed. Logs are under ignored target/f4-verification/. F6 repeats the full library/integration/CLI matrix.

Documentation impact:
- Inspected: required architecture, invariants, storage, maintainer map, limitations and roadmap; API, behavioral contracts and execution plans.
- Updated: lifecycle behavior, contract-to-test mapping and F4 completion evidence.
- Not affected: persistent formats, CLI commands and later-phase scope.
- Compliance check: both documentation policy modes passed; final records are checked again during F6.
- Known documentation gaps: F5–F6 acceptance remains open.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: failed reopen leaves the owner stopped and closed; late external acknowledgments cannot cross an old fence.
- Pitlord enforcement impact: unchanged; F6 owns the bounded refreshed check.
- Other verification impact: deterministic channel/gate regressions replace timing assumptions; focused tests passed.
- Known architectural gaps: refreshed structural evidence and F5–F6 remain open; later host capabilities remain out of scope.

### F5 — Close expanded ambient/project authorization

The existing tests cover dependency grants, exact Echo/provenance source owner, project attachment authorization, revoke during embedding, delivery-lease drain, and synced project attachment persistence.

Finish only the uncovered surface:

- add table-driven coverage for the newly exposed paging/search/management methods that do not yet have an authorization test;
- confirm all external work (embedding, filesystem/Git/project environment) happens outside owner locks and re-authorizes before resulting commit/delivery;
- verify invalid paging limits/offset semantics do not create an owner-enumeration or grant-bypass path;
- keep PHY-private semantic work explicitly unsupported in this gateway surface until Phase 2 rather than inventing partial behavior.

**F5 exit:** every Phase 1 user-facing gateway operation family has at least one allow case, wrong-owner/wrong-grant deny case, and revoke/detach boundary where external work can race authorization.


**F5 status: complete on 2026-10-04, locally on feat/multiplexing-phase1.** Eight regressions cover thirteen read/search families, eight mutation families, instance maintenance, all five project methods, paging boundaries and unsupported PHY semantic work. Exact owner/grant allow and deny cases also exercise revoked and detached attachments. Project methods have ten channel-controlled revoke/detach races after real filesystem/Git I/O; resulting REL state stays unchanged and stale delivery fails. Embedding detach adds the counterpart to the existing revocation race.

No production authorization defect was found. Instance-local test-only hooks observe real project I/O boundaries; a test-only journal accessor permits lock checks without widening production visibility. The Memory revision allow fixture retains its immutable body, preserving semantic policy. Authorized external Git/filesystem effects can already exist before grant loss; no cross-system rollback protocol is claimed. REL publication and result delivery retain the original token and revalidate.

F5 verification: formatting and diff checks passed, gateway **69/69**, and both documentation policy modes passed. Logs are under ignored target/f5-verification/. F6 owns final library/integration/CLI and public-contract verification.

Documentation impact:
- Inspected: required architecture, invariants, storage, maintainer map, limitations and roadmap; current API/source and behavioral contracts.
- Updated: expanded operation-family test mapping, external-effect boundary and F5 completion record.
- Not affected: journal/container formats, production API signatures, CLI and Phase 2–5 scope.
- Compliance check: both documentation policy modes passed; final records are checked again during F6.
- Known documentation gaps: F6 final acceptance/packaging remains open.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: exact attachment/owner grants remain authoritative through external work; PHY-private semantic work remains unsupported.
- Pitlord enforcement impact: unchanged; test instrumentation is absent from production.
- Other verification impact: table-driven authorization plus ten deterministic project races and embedding detach passed.
- Known architectural gaps: fresh structural evidence and F6 remain open; external side effects are not transactional with REL authorization.

### F6 — Final verification and packaging

Run one final serial verification matrix on the finished source. Reuse one warmed Cargo target; do not launch parallel heavyweight builds.

Required green gates:

1. `cargo fmt --all -- --check`
2. `git diff --check`
3. `cargo check --locked -j 2`
4. gateway tests
5. journal tests
6. instance tests
7. legacy-isolation tests
8. `multiplexing_baseline_contract`
9. `multiplexing_gateway_api_contract`
10. `cargo test --locked --lib -j 2 -- --test-threads=2`
11. `cargo test --locked --tests -j 2 -- --test-threads=2`
12. CLI fmt/check/tests
13. both documentation checks

Run the unrestricted `cargo test --locked` once for regression evidence. If the only failure remains the inherited `archive_open_profile` global-allocator conflict, record it as inherited and **do not block Phase 1**. Any new Phase 1-related failure blocks completion.

For architecture evidence:
- retry the refreshed architecture check once with a bounded budget;
- if Lexicon/Arcana indexing again times out before producing a usable current snapshot, record the timeout and complete a direct source guard audit for forbidden gateway→legacy ambient dependencies;
- architecture indexing timeout alone does **not** block Phase 1 if all enforceable source/test gates are green and no contrary structural evidence exists.

Then:
- update the Phase 1 implementation record from "in progress" to the exact final result;
- keep later-phase limitations explicit;
- review the 54-file worktree for accidental unrelated changes;
- package the Phase 1 implementation into coherent commits on `feat/multiplexing-phase1`;
- do **not** merge to `main` unless separately requested.

## Explicit non-goals before Phase 1 closes

Do not perform these while finishing Phase 1:

- multi-principal PHY publication/worker ownership — Phase 2;
- full sequential-versus-visible-branch conflict policy — Phase 3;
- REL-wide event feeds or live generation subscription protocol — Phase 4;
- production host transfer for Insomnia, Dream, Embedding or ProjectEnvironment — Phase 5;
- Warlock migration/conformance, old `ReliquaryRuntimeHost` deletion or full consumer hard cut — Phase 5;
- performance optimization absent a measured Phase 1 bottleneck — Phase 6;
- repair of unrelated example/global-allocator failures.

## Phase 1 exit gate

Phase 1 is complete when all of the following are simultaneously true:

- one process-local gateway authority owns each mounted REL/PHY and aliases cannot create a second mutable owner;
- instance identity/grants come through the trusted admission seam and stale attachment generations fail closed;
- instance REL/conversation selection is independent and is the sole selection source on the new gateway path;
- one REL-wide durable journal and one shared writable conversation continuation own accepted conversation work;
- accepted work survives detach/restart without granting detached clients continued authority;
- ConversationExecution has one durable native/host stage authority with tested cancel/status/recovery and exact-once Archive publication;
- unsupported host capabilities remain safely Native/Held until their later migration;
- quiesce/revoke/detach/unmount drain and fence stale operations without losing accepted work or blocking unrelated owners;
- all Phase 1 user-facing gateway operation families enforce exact owner/grant authorization through external-I/O races;
- the final focused/library/integration/CLI/docs verification matrix is green, with only explicitly inherited unrelated full-suite failures allowed;
- the work is documented and committed on the Phase 1 branch.

At that point Phase 2 may begin without reopening the Phase 1 authority model.

## Related docs

- [Multiplexing implementation plan](multiplexing-implementation-plan.md)
- [Phase 1 remaining-work history](multiplexing-phase1-remaining-work-2026-10-03.md)
- [Phase 0B integrated design](multiplexing-phase0b-design.md)
- [Phase 0B conversation/journal contract](multiplexing-phase0b-conversation.md)
- [Phase 0B lifecycle/consumer contract](multiplexing-phase0b-lifecycle-consumers.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)

## Notes

This plan is the authoritative late-stage Phase 1 handoff. It narrows remaining execution only; it does not rewrite the accepted Phase 0B contracts or move Phase 2–5 scope into Phase 1. Verification counts in this document describe the current uncommitted worktree and must be refreshed if later source changes invalidate them.
