# Multiplexing Phase 0: audited baseline and characterization gates

Parent index: [Documentation index](INDEX.md)
Target decision: [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
Implementation sequence: [Multiplexing implementation plan](multiplexing-implementation-plan.md)

## Purpose

Record the existing runtime ownership, singleton assumptions, impacted modules and protecting tests before the independent stack-wide multiplexing implementation.

## Overview

Status: Phase 0 source audit recorded 2026-10-02. This is a snapshot of current implementation and required behavioral tests, not a claim that multiplexing is already implemented.

## Scope and method

This is an independent, whole-Reliquary project. Ego's own synthesis/selection pipeline is not being refactored here, although its eventual runtime caller must use the new instance boundary. Inspected the current Rust implementation and tests directly. Lexicon reported no semantic snapshot for this checkout, so Arcana impact evidence is unavailable; direct source inspection and text-search were used instead.

The checkout had substantial pre-existing worktree changes across Archive, runtime, documentation, and the REL activity-clock effort. Phase 0 must not overwrite, commit, or assume completion of that unrelated work.

## Existing machinery to preserve

| Existing capability | Current owner and seam | Preserve/verify |
| --- | --- | --- |
| Independent mounted RELs and dependency validation | runtime_host.rs, runtime_host_mount.rs, runtime_host_topology.rs | Each durable REL owner ID mounts once, has independent owner execution, closes only its own workers, maintains deterministic dependency-first closure and detects cycles. |
| Single physical writer per mounted REL | runtime_host_owner_execution.rs Shared.runtime = Arc<Mutex<InteractionRuntime>>; runtime_host_owner_access.rs with_runtime_for | No competing open mutable handles; bounded owner-local locks and no global cross-REL critical section during inference. |
| Interaction sessions and durable Archive | interaction_runtime.rs, interaction_session.rs, Archive; the runtime already keys SessionState by session ID | Preserve explicit open/reopen-from-leaf, no ancestry across conversation IDs, durable accepted messages, independent Archive/REL activity accounting, and interrupted checkpoint recovery. |
| Source principal attribution | interaction_runtime.rs, runtime_host_interaction.rs, Archive | Continue recording verified per-message actor/source principal. The current host reads a single attached PHY; that selection mechanism is a migration target, not a guarantee. |
| Cross-owner scope and private PHY semantics | runtime_host_semantic_access.rs, runtime_host_memory_search.rs, ADRs 0029/0034/0037 | Preserve deterministic active-REL dependency traversal and owner-qualified results. Require instance-relative authorization and prevent other PHY leakage. |
| REL-local processing and worker wake signals | runtime_host_owner_execution.rs, insomnia/dream/vectors/perception runtime_host modules | Retain REL-local execution and worker independence; migrate only the PHY-specific work accidentally coupled to active REL selection. |
| Durable assistant-stream checkpoints | interaction_runtime_tests.rs, interaction_session_tests.rs, invariant 62/63 | Never broadcast visible deltas before synchronized durable checkpoint; after runtime loss a prior Streaming checkpoint must recover as Interrupted, not falsely live. |
| Project Environment and reconciliation | runtime_host_project_environment.rs, runtime_host_reconciliation.rs, ADR 0037 | Keep repository authority/correlation in Reliquary and Lore/Git boundaries unchanged; quiesce affected owners before promote/reopen. |

## Singleton assumptions to remove

| Present implementation | Source | Target |
| --- | --- | --- |
| Host-global active REL | ReliquaryRuntimeHost.active_key; active_execution() and implicit with_runtime() callers | Active REL selection in each InstanceRuntime; owner-qualified gateway access internally. |
| One host-global PHY slot | ReliquaryRuntimeHost.phylactery: Arc<Mutex<Option<Phylactery>>>; attach/detach/profile methods | Gateway-owned PHY registry keyed by durable ID; each instance attached to one authorized PHY, possibly shared by other instances. |
| Active-REL background worker gating | active_insomnia_enabled; OwnerExecution.phylactery_enabled; activation/deactivation queue reset | Gateway/owner scheduling based on durable work and configured policy, never whichever instance selected the REL last. PHY work once per PHY. |
| One managed session per REL execution | OwnerExecution.managed_session; runtime_host_session.rs closes prior managed session on start/reopen | Instance-specific managed-session selection; shared REL conversation operation registry independent of selection. |
| Global durable conversation_active as process-liveness proxy | runtime_host_mount.rs clears all active flags at mount; managed session switch/unmount rewrites flags | Define durable conversation metadata separately from transient instance presence/subscription. Preserve file-reader compatibility; agree meaning before removal/change. |
| Implicit principal from current PHY | runtime_host_interaction.rs begin_message; principal profile sync on set_active_rel and PHY attach | Verified principal bound to instance and immutable on operation. Profile projection keyed by principal must not be replaced by the latest attaching user's profile. |
| Ambient scope applied as unrestricted explicit access | runtime_host_semantic_access.rs semantic_owner() accepts any mounted REL | Authorization intersection on every instance-facing ambient and explicit operation. Internal trusted maintenance APIs are separate. |
| No gateway REL update-feed / stream-subscription distinction | Current interaction runtime persists checkpoints but has no instance distribution bus | REL-wide post-commit event fan-out to every authorized instance accessing the REL; live stream fan-out only to active conversation subscribers. |
| Session identity conflated with conversation runtime state | InteractionRuntime.sessions uses session ID as one in-memory writable continuation key, rejects duplicate open of same ID | Keep a single authoritative conversation/branch operation coordinator per REL while allowing many instance viewers/subscribers. Avoid opening duplicate writable runtimes for one conversation. |

## Stack-wide impact inventory

1. **Gateway and instance lifecycle:** new canonical gateway, REL/PHY registries, instance attachment, owner quiescence, shutdown; delete ambient singleton host once callers migrate.
2. **Archive and conversation:** managed selection becomes instance-local, while branch writes and generation operations are REL-owned; evaluate durable active flag and branch-head compare-and-commit.
3. **Durability/events:** ordered REL update publication after successful mutations (including background changes); independent, acknowledged durable assistant checkpoints; snapshot and catch-up at activation.
4. **Memory, Entity, Graph and retrieval:** owner-local persistence unchanged; cross-owner semantic composition takes the authorized instance context instead of the last activated global PHY.
5. **Processing:** REL-specific Insomnia/Dream/vector/Perception retain one execution per REL; PHY-specific workloads and profiles move behind one per-PHY execution. Capture principal at work submission where personal context is needed.
6. **Project Environment, reconcile/reclaim:** gateway owns quiescence/leases during replacement; preserve Lore/Git revision correlation, shutdown safety, and independent owner locks.
7. **ConfiguredRuntime, model routing, API/CLI and hosts:** audit public ReliquaryRuntimeHost exports and caller adaptation (Warlock/ACP/agents are integration consumers). No transport-specific ownership or provider redesign.

## Characterization tests

These tests encode behavior to preserve semantically, though tests of the old ambient host API must be migrated to the new owner/context seams rather than left permanently asserting the old class shape:

| Existing test seam | Baseline obligation |
| --- | --- |
| tests/multiplexing_baseline_contract.rs one_rel_can_keep_distinct_open_conversations_without_crossing_ancestry | New Phase 0 contract: one existing InteractionRuntime supports two independent open conversations and exact resume; does not purport to test multi-instance behavior. |
| runtime_host_topology_tests.rs graph_host_resolves_dependency_closure_and_excludes_siblings; graph_host_rejects_mounted_cycles_before_metadata_publication | Mounted closure is deterministic; siblings are not ambient; cycles/missing owner fail closed. |
| runtime_host_topology_tests.rs active_switch_keeps_one_host_owned_phylactery; unmount_stops_only_the_selected_owner_execution | Baseline PHY switching and independent REL shutdown; supersede the *single host PHY* assertion with per-instance attachment and per-owner registry tests. |
| runtime_host_session_tests.rs managed_session_owns_active_switch_and_reopen_state; managed_message_rejects_non_active_session | Managed selection, exact resume, validation and finalization; remap expected behavior to each instance independently. |
| runtime_host_session_tests.rs managed_message_and_stream_lifecycle_is_reliquary_owned; mount_repairs_stale_durable_active_conversation_flags; unmount_closes_managed_session_and_clears_durable_active_flag | Durable operation lifecycle and legacy active-flag behavior; replace lifecycle-specific expectations when presence semantics are redesigned. |
| runtime_host_semantic_access_tests.rs visible_semantic_owners_follow_dependency_closure_plus_phylactery; visible_memory_search_uses_dependency_closure_plus_phylactery; visible_archive_search_uses_dependency_closure_and_excludes_phylactery | Preserve dependency ordering, explicit/private distinction, and REL-only Archive; add cross-instance and adversarial authorization cases. |
| interaction_session_tests.rs session_chains_messages_and_resumes_from_durable_leaf_after_reopen; incomplete_or_cancelled_stream_never_becomes_source_history | Durable ancestry, resume and cancellation correctness. |
| interaction_runtime_tests.rs checkpointed_stream_survives_reopen_as_interrupted; replayed_normalized_turn_is_idempotent | Synchronized checkpoint recovery, no duplicate source history or fabricated live stream. |
| runtime_host_tests.rs inference_does_not_hold_the_interaction_runtime_lock | Background model inference does not monopolize the live interaction critical section. |
| archive/conversation_tests.rs conversation_active_round_trips_and_title_updates_preserve_it | Existing on-disk conversation-active semantics remain readable during migration; runtime presence must not become incorrect when concurrent viewers attach. |

## Required new completed-architecture tests

- Two instances on the same REL with different authorized PHYs have independent selections, attributed mutations, private retrieval and isolated subscriptions.
- Two instances sharing one PHY can work in distinct RELs without duplicating PHY workers or changing each other's state.
- Two instances viewing one REL receive every authorized committed update, including changes to inactive conversations, regardless of current conversation selection.
- Only active subscribers receive a given conversation's streaming deltas; a late subscriber receives the synchronized current checkpoint and joins future output without duplicate text.
- A simultaneous write to one branch has deterministic commit order and the selected conflict-policy result; no silent branch-head overwrite, duplicate generation or lost accepted message.
- Revocation, disconnect, slow consumer overflow and gateway restart never leak a PHY or leave an instance silently stale. Reconnect resynchronizes by mount epoch and authoritative state.
- Last-subscriber detach does not cancel shared generation; only appropriately authorized operation control may cancel. Instance shutdown does not stop shared workers or invalidate other active sessions.
- REL and PHY owner-specific background work runs at most once per owner; owner-local locks, independent owner shutdown and source principal retention are asserted under concurrency.
- Quiescing a mounted owner safely fences subscribers/operations before reconciliation, replacement or unmount.

## Verification and migration-gate discipline

Baseline existing tests are a characterization inventory, **not proof of implemented multiplexing**. Run focused existing suites at Phase 0, then add target-interface tests alongside the implementation when those interfaces exist. Do not add tests that bind the new architecture to abandoned legacy wrapper APIs.

A staged internal hard-cut migration may have known red intermediate states. Each phase needs targeted tests and a remaining-consumer search; the final integration gate requires cargo fmt/check/test, CLI build/test where public API changes, structural/changed-impact documentation policy, and architecture policy refresh. When a current Lexicon/Arcana snapshot becomes available, use it to cross-check reverse dependencies, but direct source/tests remain the completion authority.

## Phase 0 exit

- Target architecture and baseline recorded with source/test seams.
- Implementation roadmap and unresolved decision gates explicit.
- No existing semantic or persistence code changed during Phase 0.
- Focused test and documentation verification attempted; outcomes and unrelated blockers recorded below rather than reported as passing.

## Verification status

Phase 0 planning and new test authoring are complete. The new characterization test was formatted successfully but **has not been run**: two Cargo attempts waited on another process's existing build-directory lock and were cancelled without disturbing that process. Existing focused runtime suites likewise remain an execution gate, not an assumed pass. Whole-repository cargo fmt --check also detects formatting changes in unrelated active REL-activity work; only the new test file was formatted. Both documentation checks report five pre-existing findings confined to the unrelated, untracked docs/insomnia-explicit-memory-commit-plan.md (four required sections and its index entry). The multiplexing documentation's checker findings were corrected and are no longer present.

## Phase 0 verification record (2026-10-02)

- New baseline test: `tests/multiplexing_baseline_contract.rs` added to characterize two independent `InteractionRuntime` sessions in one REL, persisted ancestry and exact resume. The isolated test file was formatted successfully with `rustfmt`.
- Focused `cargo test --test multiplexing_baseline_contract` was attempted twice, including `--offline`, but both invocations were blocked by an existing Cargo build-directory lock held by another process; both attempts were cancelled without claiming a test pass. Rerun after that concurrent build completes.
- Whole-repository `cargo fmt -- --check` failed on unrelated in-progress Archive/activity and reconciliation source edits; our baseline test was subsequently formatted directly.
- Both documentation policy invocations (structural and `--changed-from origin/main`) no longer report findings for the multiplexing documents. They report **five findings only for the independently modified** `docs/insomnia-explicit-memory-commit-plan.md` (missing Purpose, Overview, Related docs, Notes, and not indexed). That file was deliberately left untouched.
- Lexicon reports no snapshot for this checkout. Arcana evidence unavailable; direct source and existing tests supply the audit evidence.
- No existing production runtime, semantic, storage or persistence implementation was changed by this Phase 0 work. Automated verification remains an outstanding exit item until the shared build lock and unrelated documentation state permit a clean run.

## Phase 0B executed follow-up (2026-10-03)

The earlier 2026-10-02 record is historical. The separate `design/multiplexing-phase0b` worktree at `a6f580c` executed the current baseline and integrated [design contracts](multiplexing-phase0b-design.md).

- The existing multiplexing baseline passed (1 test); `cargo check --locked` passed.
- Five std-only reference fixtures compiled warning-free and passed 56 tests. These are model oracles, not runtime conformance tests.
- Full `cargo test --locked` failed in existing example builds (allocator conflict and missing reliquary_memory). The library/test run passed 767/768 library tests; Lore historical reads failed with Windows file-sharing error 32. The runtime-host filter passed 43/44; its backpressure first-wave assertion failed, although that test passed in the broader library run.
- Cargo execution of the combined reference/activity tests remained incomplete: an initial dependency rebuild timed out and the second was cancelled during Lore compilation without test results.
- Warlock's initial isolated compile failed on missing dependency artifacts. Its single-job retry was cancelled after 6m16s while still compiling dependencies, so the downstream compile baseline is inconclusive.
- `cargo fmt --check` passed after formatting the fixtures for the crate's 2024 edition. Both documentation policy checks (structural and `--changed-from origin/main`) passed.

Documentation impact: five future-architecture pages and their index, plan, readiness review, ADR, behavioral-contract reference and this baseline record. No shipped-behavior claims were added. Architecture impact: no production source ownership/API/storage changes; architecture snapshot refresh is not applicable. Known gaps are runtime implementation, journal/platform crash tests, real adapter conformance, baseline failures and downstream compile/test/frontend verification.

## Related docs

- [Multiplexing implementation plan](multiplexing-implementation-plan.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
- [Current architecture](architecture.md)
- [Architectural invariants](invariants.md)

## Notes

This inventory is an audit snapshot; the implementation plan owns future work, while current-state documents must change only when that work ships. Test execution outcomes belong in the Phase 0 completion report, not as assertions that every inventory item has passed.
