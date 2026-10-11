# Multiplexing — authoritative completion plan

Parent: [Documentation index](INDEX.md) · Decision: [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)

## Purpose

Complete multiplexing across Reliquary and its Warlock and Hermes consumers with one canonical execution plan.

## Overview

See the ordered status table below for the current implementation and remaining work.

## Scope and source of truth

This is the **single active execution and progress plan** for Reliquary multiplexing, Warlock adoption and Hermes integration. Status records are updated **in place**, not in dated acceptance/checkpoint files. Current implemented interfaces belong in [architecture](architecture.md), [API](api.md) and [behavioral contracts](behavioral-contracts.md). Verification evidence belongs in test output, CI, commit history and retained logs, not new root documentation snapshots.

**Assessment date: 2026-10-11.** This plan records the latest inspected committed evidence, **not** a fresh test run. Reliquary `main` at `2407dbe` is a planning branch, **not** the implementation frontier. Current N3 integration: `feat/multiplexing-n3-staging` at `dcf500e` (October 10). Do not merge forward without rerunning the combined gates.

## End-state

One Gateway authority owns each mounted REL/PHY; many independently authorized instances share it. REL conversations and pending work remain durable and recoverable. REL-wide updates reach authorized instances; active generation text reaches only authorized conversation subscribers, after synchronized checkpoints. Host execution is negotiated per capability; never build a duplicate queue or legacy-host facade. Hermes retains authority over its own canonical transcripts when integrated through the external reference index.

## Progress and linear execution

| # | Owner | Deliverable | Status | Completion condition |
|---|---|---|---|---|
| Foundation | Reliquary | Gateway/Instance, REL/PHY worker identity, durable conversation scheduler, P4 distribution | **Accepted on existing implementation lines** | Historical integration verification exists; must remain green during final integration |
| 1 | Warlock | Actual provider execution-only adapter, remote outcome/reconnect, multi-context privacy, P4 public delivery, exact task-root attribution | **In progress / blocking** | Provider lifecycle works against exact staged Reliquary; tests demonstrate reconnect, cancellation, privacy and checkpoint-only visible text |
| 2 | Reliquary + Warlock | Combine accepted runtime, management, import and external-reference source; pin one Reliquary and one Arcana revision in Warlock | **Partially staged / blocking** | One integrated source, one Arcana identity, locked Warlock checks/tests/frontend, downstream fixtures and exact commit-pinned acceptance |
| 3 | Reliquary | Delete `ReliquaryRuntimeHost` and old-only mutable selection, duplicate workers and forwarding | **Inventory done; removal blocked on 1–2** | No legacy host imports/exports/callers; preserved readers, relocated useful types, full targeted matrix green |
| 4 | Both | Release verification of post-deletion runtime and real Warlock | **Harness exists; final pass blocked on 3** | Crash/reopen, multi-instance identity/privacy, queue/cancel/replay, stream slow/reconnect/epoch, actual provider, locked builds and measured baseline pass |
| 5 | Hermes + Reliquary | Implement Hermes feed/hydration and plugin session/runtime adapter | **Planned; not yet accepted** | Live authorized bridge maps Hermes identities, sessions, branches and lifecycle without copying canonical transcript bodies to REL |
| 6 | Hermes + Reliquary | End-to-end Hermes multiplexing acceptance | **Not started** | Two concurrent sessions/principals, shared/isolated PHY behavior, restart/replay, revoke/delete/rewind, exact hydration and failure isolation pass |

Only completion of step 4 qualifies **Reliquary + Warlock multiplexing** as finished. Only step 6 qualifies **Hermes multiplexing** as finished. Independent external format adapters and selective F5 import/export are tracked separately and are not grounds to keep the old runtime alive.

## Existing implementation and evidence

- Reliquary N3 `dcf500e`: E5 live runtime, F-core management, native import, external reference-only core, and B/E execution-only negotiation are integrated. The latest documented combined matrix reports **1,159 unit, 137 integration and 19 CLI tests passed**.
- Warlock `be17ad5` against the staged Reliquary revision: native provider binding, stable request digest, process-local Unknown/no replay, authorization-first cooperative Stop/status passed **212 tests**, with one existing external benchmark ignored. This is **not** complete remote reconciliation or full product multi-context/P4 acceptance.
- A fresh Rust publication/Arcana refresh on the B/E cut verified all authored Rust source hashes and **22 owner/content rules**. Its graph reported zero relationships, so it **does not prove dependency/call-graph correctness**.
- P6 real-runtime test harness `81a64b6`, instrumentation work `b2b4bf4`, and D0 deletion inventory `bfdd267` exist on separate branches. Historical acceptance of these branches does not substitute for post-deletion testing.
- No new claim is made that Warlock production dependency pins, deletion or release acceptance have occurred.

## Execution rules and dependency gates

The six numbered deliverables are the **only current progress sequence**. Subtasks may run concurrently, but acceptance is ordered. A passing isolated branch does not imply a finished release. Update the status table **in place** with accepted SHA, owning repo/branch, test command and counts, skipped/blocked fixtures and next action; keep raw logs in CI/external verification storage.

- **Source of implementation truth:** Reliquary N3 `dcf500e`; `main` currently holds the documentation cleanup, not the new implementation.
- **Warlock source baseline:** `be17ad5` accepted the native execution/Stop/status slice against N3. Inspect current Warlock tip and dependency pins before implementation; these may have moved.
- **Parallel work allowed:** Warlock real provider/context/stream work, Reliquary N3 consolidation preparation, P6 fixture/measurement work, and Hermes contract inspection. Merge ownership changes only through their canonical source.
- **Hard dependencies:** Final pin acceptance waits for the working Warlock adapter. Legacy deletion waits for accepted pinned consumers. P6 release checks run on the post-deletion tree, not only against the pre-deletion harness. Hermes implementation signs off against the frozen runtime, although planning can start earlier.
- **Failure policy:** Unknown remote provider outcomes stay held for reconciliation; do not replay ambiguous work. Do not introduce a second queue, ambient global context, compatibility host, or duplicate transcript authority to make tests pass.

### Ready-to-close gate — Warlock

1. Implement execution-only provider adapter against negotiated live readiness; keep native pending queue when host pending cancellation/reconciliation is insufficient.
2. Verify stable execution ID and request digest across restart, authorized Stop, remote timeout/unknown reconciliation, and pending-vs-running cancellation semantics against real HTTP/provider fixtures.
3. Verify two independently selected instances, same-principal PHY worker deduplication, cross-principal privacy, viewer detach and independent REL/branch selection.
4. Verify Tauri-visible text is only committed after P4 checkpoint synchronization, with late join, slow readers, revocation, resume, epoch rotation and required resync.
5. Verify exact task-root and ProjectFileRef attribution across authorized retrieval and management.
6. Record exact Warlock commit, tests, frontend production build, source guards and remaining product/environment exceptions.

### Ready-to-close gate — convergence and pins

1. Merge accepted N3 A/B/E/F/G core exports and B/E execution-only negotiation into a single candidate; preserve one implementation owner for each type.
2. Run full locked Reliquary and detachable CLI checks/tests/examples, ownership/coverage guards, docs and current architecture checks.
3. Pin Warlock to exact accepted Reliquary **and** direct Arcana SHAs; regenerate lockfile, prove exactly one intended Arcana identity, rerun provider/context/stream tests plus frontend.
4. Commit merged source and pin changes; record successful real-consumer acceptance (not a local wrapper-only pass).

### Ready-to-close gate — legacy removal

1. Use the accepted D0 reverse-caller inventory `bfdd267`; recheck it against merged source and Warlock.
2. Relocate surviving report/error/query types and persisted readers; remove `ReliquaryRuntimeHost` and obsolete session/worker/mount routing authority.
3. Replace legacy-behavior tests with Gateway/Instance equivalents, reject legacy symbols/callers, and run complete locked suites.
4. Confirm no second journal owner, queue, personal worker, writable global selector or compatibility facade.

### Ready-to-close gate — post-deletion release

1. Port P6 harness `81a64b6` and H5 diagnostics `b2b4bf4` to the final integrated source.
2. Test multiple REL/PHY/instances, queue chronological order and cancellation/finalization races, branch history, crash/reopen, durable receipt loss and no duplicate work.
3. Test stream join, slow subscribers, bounded overflow/resync, revocation, epoch rotation, checkpoint durability and private PHY delivery.
4. Test actual Warlock provider, host reconciliation, frontend/Tauri and model interruption; do not equate local transport stop with remote stop.
5. Measure worker count, queue latency, lock contention, fanout memory, restart time and throughput on frozen before/after workloads; record exact hardware, profiles and variance.
6. Run all locked test matrices, code guards, documentation validation and current architecture enforcement. Mark unreached environments explicitly incomplete.

### Hermes adapter — build and acceptance

1. Inventory actual Hermes plugin/source API and canonical session/message/branch identity; capture change-feed and authorization contracts.
2. Freeze a minimal normalized feed (floor, high-water, cursor, operation ID, hashes, edit/delete/rewind semantics) and authorized transient hydration.
3. Implement plugin-side adapter for sessions, live events, capability-specific execution, cancellation and lifecycle over Gateway/Instance; keep Hermes canonical transcripts in Hermes, not duplicated in REL Archive.
4. Use existing reference-only external core and Insomnia normalizer; prove deterministic reindex/rebuild, no double publication, no cross-profile hydration and Hermes independence during Reliquary outage.
5. Exercise simultaneous sessions/contexts, same-principal shared PHY, different-principal isolation, stream reconnect and ambiguous in-flight recovery with actual Hermes fixtures/providers.
6. Pin, test and commit both sides; record real end-to-end test evidence. This is the second release milestone, independent of optional import/ACP adapter breadth.

## Step 1 — Warlock consumer completion

**Status: partial.** Native provider/Stop/status accepted on `be17ad5`; end-to-end Warlock acceptance outstanding. Tasks marked partial reflect existing slice evidence, not independently reverified completion.

| ID | Action | Status |
|---|---|---|
| 1.1 | Audit current Warlock consumer against N3 interfaces and actual dependency pins | Audited 2026-10-11: main `4da92b3`, consumer branch `90fdb61`; production pins remain pre-N3; build compatibility unverified |
| 1.2 | Finish execution-only provider adapter | Implemented on consumer branch `90fdb61` plus local readiness fix; N3 library suite 223 passed, 1 benchmark ignored; final pins remain Step 2 |
| 1.3 | Implement remote outcome reconciliation, reconnect and Unknown recovery | Partial; provider contract audit found no remote recovery API in either registered transport |
| 1.4 | Complete cancellation and provider operation lifecycle | Local lifecycle repaired and HTTP Stop fixtures pass; remote termination acceptance still depends on 1.3 |
| 1.5 | Complete multi-instance context and session management | Backend task contexts verified against N3; independent desktop window selection remains unaccepted |
| 1.6 | Verify shared PHY ownership and cross-principal isolation | Verified against N3 `dcf500e`: 9 production runtime tests and 3 Warlock consumer tests passed; final pinned/product acceptance remains open |
| 1.7 | Integrate checkpointed durable conversation streams with Warlock UI/Tauri | Implemented and targeted N3/frontend checks passed; interactive Tauri operator acceptance remains open |
| 1.8 | Implement public reconnect, resync and subscription recovery | Implemented with bounded observer reconnect; targeted N3/backend and frontend checks passed |
| 1.9 | Complete exact task-root attribution and ProjectFileRef propagation | Task-root attribution verified; exact ProjectFileRef propagation blocked on N3 submission API |
| 1.10 | Run full real-provider, frontend, privacy and Warlock acceptance matrix | Targeted matrix passed; full fresh rebuild hit Lore/rustc environment ICE; final acceptance remains blocked on 1.3, 1.5, 1.9 and pins |

**Exit gate:** upgraded Warlock uses canonical Gateway/Instance APIs with correct real-provider recovery, streaming, authorization and multi-context behavior; commit and log exact tests and pins.

### Step 1.1 audit — 2026-10-11

Warlock `main` is `4da92b3`; it does not contain consumer branch `feat/multiplexing-p5c-final-cut` at `90fdb61` (including `69c4642` and the earlier `be17ad5` slice). The consumer worktree was clean at audit time; main has existing graph/UI and roadmap edits. Reliquary N3 remains `dcf500e`. This was a source and Git audit; no fresh build or test pass is claimed.

Both Warlock manifests still pin Reliquary `83b3ac87a80a4221039fa07d9722f5f8076464ba` and direct Arcana `66b4e96ecf18cc79dfd6b1ea30232bef7c86bba3`; the consumer lockfile agrees and contains one Arcana package entry. N3 requires Arcana `9ebccd6e7d089b98a8992c5451ba57f903f295fb`. Therefore the committed production dependency graph does not establish N3 compatibility; changing only the Reliquary pin would leave distinct Arcana revisions.

The later consumer source implements `GatewayHostAdapter` with execution-only conversation capability, native pending ownership, local status/cancel/reconciliation, authorized `LiveSession` checkpoint receipt consumption, resync snapshots and bounded viewer reattachment. Local cancelled/unknown outcomes remain remotely Unknown. Remote provider reconciliation is still absent. Legacy-named `ReliquaryRuntimeHostError` imports survive and require type relocation during Step 3; their names alone do not demonstrate a duplicate host owner.

Next action: verify the current main consumer against N3 with aligned Arcana in an isolated candidate, retaining production pin acceptance for Step 2. Exercise provider recovery and current task/viewer behavior before upgrading the remaining Step 1 statuses. Existing source/tests are implementation evidence, not fresh acceptance of real HTTP providers, frontend delivery or privacy gates.

### Step 1.2 implementation — 2026-10-11

The existing execution-only adapter on `Warlock-p5c-final-cut` is installed at startup and uses the real provider lifecycle. Local changes require negotiated `ExecutionOnlyReady` before provider context preparation and again before durable generation creation. The new production-task fixture rejects unconfigured and Unknown-held execution and accepts negotiated readiness. Pending work remains native; remote reconciliation remains Step 1.3.

Fresh command: `scripts/cargo-local-reliquary.ps1 test -ReliquaryPath C:\!bin\workspace\Reliquary-n3-staging --offline --lib`, with TEMP/TMP inside the consumer worktree and GIT_CEILING_DIRECTORIES set to that temporary directory: **223 passed, 0 failed, 1 existing benchmark ignored** against N3 `dcf500e`. The first run had 222 passed and one missing-Git-repository fixture failure caused by discovering the enclosing worktree; bounding discovery resolved it. Existing compiler/linker warnings persist. Both Warlock docs modes and existing Pitlord enforcement passed (63 rules); fresh graph refresh, frontend release acceptance and locked production pins remain open. The transient wrapper restores the production lockfile and currently resolves two Arcana revisions.

### Step 1.3 provider contract audit — 2026-10-11

The registered transports are OpenAI-compatible Chat Completions and OpenAI Codex. `ProviderTransport` exposes inference and credential operations, with no remote operation lookup, resume or terminal-outcome reconciliation contract. Codex request construction sets `store: false`. The local execution record binds owner/operation/digest but retains neither a remote recovery receipt nor a provider-specific query identity. The host adapter therefore reconciles only process-local records; missing and locally cancelled executions remain Unknown. REL remains the durable generation/no-replay owner.

Completing remote recovery requires selecting a provider protocol that supplies authenticated durable operation lookup and defining how its receipt, request digest and terminal result bind to the native generation. Generic HTTP retry or a fabricated status endpoint cannot supply that evidence. Step 1.3 remains partial pending that protocol decision; no remote recovery implementation or new test acceptance is claimed by this audit.

### Step 1.4 local lifecycle — 2026-10-11

Consumer changes finalize the durable generation and execution lease before optional usage accounting, preventing telemetry failure from bypassing completion/cleanup. The public readiness query now includes negotiated execution-only readiness. The actual HTTP Stop fixture additionally verifies authorized repeated Stop and rejection of late output after interruption; its three provider acceptance tests pass against N3 `dcf500e` through the transient wrapper (`test --offline --lib provider_execution_acceptance_tests`). Production pins remain unchanged. Local Stop requests cooperative cancellation and fences durable output; remote termination cannot be accepted until Step 1.3 supplies a recovery protocol. No complete cancellation/release acceptance is claimed.

### Step 1.5 backend context verification — 2026-10-11

Existing production operating contexts attach distinct authenticated instances to one Gateway and keep provider REL/conversation/branch roots independent of desktop selection. A new fixture verifies duplicate context-key rejection, detach invalidation, key reuse with a fresh instance, and preservation of the other selected session. The five `chat_task_context_tests` pass against N3 `dcf500e` through `scripts/cargo-local-reliquary.ps1 test --offline --lib chat_task_context_tests` with the exact N3 path override. These fixtures also cover selection changes during two accepted generations, shared PHY identity, task-local retrieval and grant revocation.

The desktop RPC surface still uses a primary workspace instance and viewer opening checks the currently selected owner. Independently selected backend tasks/viewers are implemented; fully independent desktop windows have not been implemented or accepted by this verification. Production dependency pins, UI acceptance and remote recovery remain open.

### Step 1.6 PHY ownership and privacy verification — 2026-10-11

Fresh locked/offline N3 library compilation used `cargo test --locked --offline --lib --target-dir ../Warlock-p5c-final-cut/src-tauri/target`; TEMP/TMP stayed inside the N3 worktree. The initial filename-based filter selected zero tests and is not acceptance evidence. The resulting `reliquary_memory-d6a36fdd0a41b529.exe` then passed **9 production tests** using filters `gateway::phy_tests` (4), `combined_search_and_private_reads`, `private_search_rejects`, `attachment_churn_preserves`, `private_provider_io`, and `broad_snapshot_never` (1 each).

These tests count exactly one three-worker PHY set across same-principal attachment/detach, reject foreign-principal binding and retargeting, fence stale tokens across remount, join workers at shutdown, preserve ownership during attachment churn, deny foreign private semantic reads, revalidate revocation during embedding/provider I/O, and exclude PHY bodies from broad snapshots. They exercise production Gateway code; the Phase 0B reference-model contract files were not counted.

The current N3-built Warlock test executable passed **3 consumer tests**: `actual_task_contexts_share_one_phy` and both `workspace_admission_tests`. This verifies shared PHY identity and authenticated grant/identity isolation through Warlock's existing adapter. No source change was required for Step 1.6. Compiler/linker warnings remain; production pins, independent-window UI and release acceptance remain separate gates.

### Step 1.7 checkpointed Tauri/UI integration — 2026-10-11

Existing `DurableStream` stages provider text, synchronizes its native generation checkpoint, and emits only the authorized P4 receipt consumed through `ChatLiveDelivery`. The UI replaces full prefixes. Consumer changes add direct-channel operation/message/epoch validation and integer-safe monotonic receipt handling, rejecting duplicates, wrong generations and regressing deliveries.

Fresh verification: the N3-built Warlock executable passed both `chat_live_delivery_tests` and both `chat_stream::prefix_tests` (**4 tests**); `npx --no-install tsx scripts/durable-text.test.ts` passed identity, duplicate, ordering, malformed-sequence and beyond-safe-integer checks; `npm run build` passed (existing large-chunk warning persists). Backend fixtures cover checkpoint-before-text, Unicode, late join, slow-reader resync, revocation, channel failure, remount and durable no-replay. Interactive Tauri operator acceptance, final pins and full release gates remain open; this is targeted integration evidence.

### Step 1.8 public reconnect and resync — 2026-10-11

The public `generationObserver` now treats a failed viewer read as recoverable while an active generation exists. It closes the stale authorized handle, re-reads the current generation hint, opens a fresh viewer, consumes the server-authoritative prefix snapshot, and resumes polling with bounded exponential backoff. It never submits work or restarts a provider operation. The initial open still reports an error to the UI; subsequent transient reconnect attempts remain quiet until they recover or the generation disappears.

Fresh verification passed: `chat_live_delivery_tests` (2), `chat_generation_viewers_tests` (2), the durable receipt UI checks, and `npm run build`. These cover late join, slow-reader overflow/resync, Unicode prefixes, revocation, failed recipients, mount-epoch rotation, reauthorization, detach/rejoin and no duplicate provider dispatch. Interactive Tauri reconnect under an actual window lifecycle remains a release-level check; production pins and final release gates remain open.

### Step 1.9 attribution and ProjectFileRef audit — 2026-10-11

Task-root attribution is implemented and verified: `ChatTaskContext` freezes owner, conversation, branch, accepted leaf and submission identity from the native Archive receipt; provider context, tools, checkpoints, completion and viewer delivery use that frozen authenticated attachment. The existing consumer tests also verify selection changes do not retarget accepted work, sibling owners are rejected, and stable attachment retries do not duplicate the user turn.

Exact project-file propagation remains blocked at the Reliquary boundary. Warlock calls `ingest_project_attachment_for` for the original owner and project directory, but discards the returned `StoredFile`. It then sends raw bytes in `GatewayConversationAttachment`; N3 `GatewayConversationSubmission` has no `ProjectFileRef` or project-file identity field, and N3 admission stages embedded attachment bytes. Warlock cannot reconstruct canonical repository revision/path provenance safely after that point. The canonical fix belongs in Reliquary: return/bind the ingested file identity as part of the immutable submission attachment contract and preserve it through journal admission, retries, rejection side effects and finalization.

Fresh targeted evidence passed against the N3-built Warlock executable: `stable_attachment_retry_does_not_duplicate_the_user_turn` (1), `workspace_memory_provenance_tests` (2), and `actual_task_contexts_share_one_phy_and_keep_roots_and_native_writes_independent` (1). The provenance tests validate existing ProjectFileRef reads for already-bound records; they do not close the new-submission propagation gap. No Warlock-side workaround or completion claim is recorded.

### Step 1.10 Warlock acceptance matrix — 2026-10-11

The existing N3-built Warlock acceptance executable passed **224 tests, 0 failed, 1 ignored** in a serial run with repository-local TEMP/TMP and bounded Git discovery. The matrix includes the actual OpenAI-compatible HTTP transport, authorized Stop, digest/idempotency/replay guards, task-root and privacy isolation, shared PHY/context behavior, P4 checkpoint delivery, viewer reconnect fixtures, ProjectFileRef read/provenance checks, workspace lifecycle and frontend-facing command contracts. The ignored test is the existing Arcana inspection benchmark requiring an external fixture.

The frontend production build and durable checkpoint UI checks passed. Warlock docs, change-impact docs and Pitlord passed (63 rules, 16,214 source nodes, 23,857 relationships). A fresh full rebuild against the N3 override compiled application sources but failed during Lore linking with a rustc 1.97.1 compiler ICE (`Os { code: 2, kind: NotFound }`) while producing `lore.dll`; this is an environment/toolchain failure, so the prebuilt executable result is retained as the runtime matrix evidence and the fresh locked build is not claimed green. The build also resolves two Arcana revisions (`9ebccd6` and the production `66b4e96`), confirming the Step 2 pin issue.

Step 1.10 does not close the release gate: remote provider reconciliation (1.3), independent desktop window selection (1.5), exact new-submission ProjectFileRef propagation (1.9), final dependency pins and interactive operator acceptance remain open.

## Step 2 — Merge and pin

**Status: partial.** E/F/G/B-E core is staged in N3, while final convergence and production dependency pins are unaccepted.

| ID | Action | Status |
|---|---|---|
| 2.1 | Audit all active integration branches, unique changes and accepted SHAs | Audited 2026-10-11: accepted lane tips are identified; no source merge performed |
| 2.2 | Establish final convergence candidate from accepted N3 source | Candidate established 2026-10-11 at `feat/multiplexing-final-convergence` / `dcf500e`; overlay review remains |
| 2.3 | Integrate accepted Gateway/Instance, background and queue ownership cuts | Integrated in candidate 2026-10-11; candidate guards pass, architecture refresh environment-blocked |
| 2.4 | Integrate accepted live runtime and management surfaces | Integrated in candidate 2026-10-11; source guards pass, locked Rust acceptance build-blocked |
| 2.5 | Integrate accepted native import and external-reference core | Integrated in candidate 2026-10-11; ownership guards pass |
| 2.6 | Resolve exports, interface ownership and dependency conflicts | Complete for candidate `dcf500e`: locked graph and library compilation passed; exact consumer pins applied in 2.8 |
| 2.7 | Run full locked Reliquary/CLI tests, examples and architecture/ownership guards | Partial verification 2026-10-11: full Rust/CLI tests, compile checks, formatting, examples and static guards pass; refreshed architecture snapshot remains environment-blocked |
| 2.8 | Pin Warlock to exact accepted Reliquary and Arcana revisions | Pinned 2026-10-11; locked metadata verified through local candidate override; remote Reliquary fetch remains unavailable |
| 2.9 | Run full pinned Warlock provider, context, stream and frontend matrix | Complete 2026-10-11 against Reliquary candidate `dcf500e` and Arcana `66b4e96`; local candidate override used because Reliquary revision is unpublished |
| 2.10 | Commit and verify integrated source, exact pins and acceptance evidence | Local integration commits created; final release gate remains blocked on 2.7 evidence and remote publication |

**Exit gate:** one integrated code revision, one intended Arcana package identity, and no untested or conflicting runtime interfaces. Do not reimplement already accepted core features.

### Step 2.1 branch audit — 2026-10-11

The active branch audit used the exact local refs and recorded tip SHAs below. `dcf500e` is the N3 staging tip. The accepted lane tips marked **in N3** are ancestors of that staging tip; their changes are already represented in the N3 source candidate and must not be reimplemented. The later inventory/diagnostic branches are evidence overlays from the recorded convergence base and are not additional runtime implementations.

| Lane | Ref and accepted tip | Relationship to N3 | Scope / convergence note |
|---|---|---|---|
| N3 staging | `feat/multiplexing-n3-staging` — `dcf500e` | Candidate base | B/E negotiation, architecture acceptance and staging verification |
| B/E execution-only | `feat/multiplexing-be-execution-only` — `b8fba6a` | In N3 | Execution-only negotiation; preserve native pending authority |
| P5-A REL consumers | `feat/multiplexing-p5-rel-consumers` — `7350149` | In N3 | Canonical REL and semantic consumer APIs |
| P5-B background | `feat/multiplexing-p5-background` — `c45f50e` | In N3 | Background controls and executable host capability cut |
| E5 live runtime | `feat/multiplexing-p5-live-runtime` — `874851a` | In N3 | Live runtime acceptance and native receipt seams |
| F-core management | `feat/multiplexing-p5-management` — `13df1ff` | In N3 | Workspace management facade |
| P5-G external core | `feat/multiplexing-p5-external` — `7c63eb8` | In N3 | External canonical transcript R0–R4 |
| P5-C API | `feat/multiplexing-p5c-api-integration` — `4ce4911` | In N3 | Exact dispatch and acknowledged transcript recovery |
| P5-C conversation API | `feat/multiplexing-p5c-conversation-apis` — `f64676e` | In N3 | Managed branch selection and conversation correlation |
| P5-C project contract | `feat/multiplexing-p5c-project-contract` — `a999f49` | In N3 | Canonical project creation and admission guard |
| P5-C conversation contract | `feat/multiplexing-p5c-conversation-contract` — `beb122c` | Two commits beyond convergence base; not in N3 | Contract export overlay; requires explicit conflict/ownership review before inclusion |
| P5-D0 inventory | `feat/multiplexing-p5d-inventory` — `bfdd267` | One commit beyond convergence base; not in N3 | Legacy-host inventory/deletion manifest; verification evidence, not runtime authority |
| P5-G native import | `feat/multiplexing-p5g-import` — `99b9fa5` | In N3 | Native import contracts and supported profiles |
| P6-H5 diagnostics | `feat/multiplexing-p6-h5` — `b2b4bf4` | One commit beyond convergence base; not in N3 | Reproducible N0 diagnostics and measurements |
| P6-H harness | `feat/multiplexing-p6-harness` — `81a64b6` | One commit beyond its P6 verification base; not in N3 | Real-runtime crash/profiling harness |
| Warlock consumer | `Warlock/feat/multiplexing-p5c-final-cut` — `90fdb61` | Separate repository; contains Warlock `main` `4da92b3` | Consumer frontier; production pins still target Reliquary `83b3ac8` and Arcana `66b4e96` |

The audit found no clean final convergence commit. Reliquary `main` at `f03ab9d` is documentation/planning history and is not a replacement for the N3 source candidate. The working trees also contain uncommitted Step 1 changes (this plan in Reliquary and consumer/runtime changes in Warlock), so the listed SHAs are accepted branch tips, not yet release pins. Step 2.2 must create a clean convergence candidate from N3, explicitly review the four overlay branches, and retain only their accepted evidence or source deltas. Step 2.8 must then align both the Reliquary and direct Arcana revisions; the current Warlock graph still resolves the pre-N3 Arcana revision alongside N3's Arcana revision.

No source merge or dependency pin change was performed for this audit.

### Step 2.2 convergence candidate — 2026-10-11

Created the local Reliquary ref `feat/multiplexing-final-convergence` at exact commit `dcf500ea8438e9b3e782959dd0176efcb58b7b0a` (short SHA `dcf500e`). This is a clean candidate ref at the N3 staging tip, with zero candidate-only commits and zero diff relative to `feat/multiplexing-n3-staging`. The earlier `feat/multiplexing-p5-convergence` ref (`999fbf7`) is an ancestor of N3 and is retained as historical convergence-base evidence, not as the final candidate.

The candidate intentionally does not cherry-pick or merge the four overlay branches identified in Step 2.1. P5-C conversation-contract (`beb122c`) requires an ownership/export review; P5-D0 (`bfdd267`), P6-H5 (`b2b4bf4`) and P6-H harness (`81a64b6`) are inventory/diagnostic evidence and must not silently become runtime source. Their inclusion, if any, belongs in Steps 2.3–2.6 with targeted verification. The candidate also does not include Warlock's uncommitted Step 1 work or change dependency pins.

The current checkout remains on Reliquary `main` with the implementation plan modified in the working tree; creating the candidate ref did not switch or rewrite that checkout. The candidate is therefore a stable source starting point for Step 2.3, but not yet a release commit: full locked checks, overlay decisions, Arcana identity resolution and real Warlock acceptance remain outstanding.

### Step 2.3 Gateway/Instance, background and queue ownership — 2026-10-11

The N3 candidate already contains the accepted ownership cuts; no duplicate implementation or cherry-pick was required:

- `GatewayRuntime` is the canonical mounted-owner authority. It owns the shared REL/PHY execution objects, owner journals, mount epochs and owner-scoped operation paths in `src/runtime_inference/runtime_host_gateway.rs` and its gateway submodules.
- `InstanceRuntime` is a selection/authorization context, not a second owner. `src/runtime_inference/runtime_host_instances.rs` keeps active selection, grants and token generations while detaching only the instance context.
- Background controls are exact-owner gateway operations in `src/runtime_inference/runtime_host_gateway_background.rs`. They authorize before reading or mutating owner state and deliberately do not create a second queue or store authority.
- The accepted queue path remains owner-local: background status and wake operations reach the mounted owner's perception queue, while the interaction background processor remains the typed processing seam in `src/runtime_inference/interaction_background.rs`.

Candidate verification from an isolated worktree at `dcf500e` passed the P4/P4.8 ownership guard (75 fixtures, 161 public API classifications), workspace-management guard, and external-source ownership guard. A fresh architecture refresh was attempted with the sibling Lexicon/Arcana tools, but Arcana failed while syncing the generated Lexicon snapshot with `os error 2`; this is an environment/toolchain exception, not a source failure. The previously recorded N3 architecture acceptance remains the applicable source evidence until the refresh can be rerun successfully.

Step 2.3 therefore closes the source-integration portion. Remaining work is Step 2.4 management/live-runtime verification, followed by the full locked candidate checks and dependency pin acceptance.

### Step 2.4 live runtime and management surfaces — 2026-10-11

The candidate at `dcf500e` contains both accepted tips: P5-E live runtime `874851a` and P5-F-core management `13df1ff`. Their composition is present through the existing gateway seams rather than a second host:

- `GatewayRuntime::live_runtime_adapter` binds live sessions to a verified instance. `LiveSession`, resume cursors/hints, generation status, cancellation, tool receipts and generated-artifact finalization remain under the existing P3/P4 durable and delivery authorities.
- `GatewayRuntime::workspace_management` binds management calls to a verified instance. `WorkspaceManagement` is a forwarding facade; it does not retain semantic data, queue state, selection copies, recipient state or an independent cache.
- The crate-root exports in `src/lib.rs` expose the named E/F types (`LiveSession`, `LiveStreamCursor`, `LiveStreamEvent`, `LiveStreamSnapshot`, `LiveToolEvent`, `WorkspaceManagement`, management DTOs and bounds) from their single composition roots.
- Maintainer ownership is explicit: live recipient adaptation is in `interaction_live*.rs`, management composition is in `workspace_management*.rs`, and gateway management tests cover the authorized binding boundary.

Candidate-specific guards passed: P4/P4.8 coverage, workspace-management ownership, and external-source ownership. A locked focused Rust run was started against the exact candidate with the existing N3 target. It did not reach test execution: the Lore build failed with missing generated dependency artifacts (`can't find crate` / missing `.rlib` paths) while compiling the locked graph. The retry was stopped after reproducing the dependency rebuild path; this is retained as a build-environment limitation, not a source-level failure. Prior N3 E5/F-core acceptance records remain the applicable runtime evidence.

Step 2.4 closes source integration and guard verification. Full locked candidate execution remains required before Step 2.7, and Step 2.8 still requires aligned Reliquary/Arcana pins.

### Step 2.5 native import and external-reference core — 2026-10-11

The candidate at `dcf500e` contains both accepted lanes: P5-G native import `99b9fa5` and P5-G external core `7c63eb8`.

- Native import is composed through `src/imports/` and the archive/episode import seams. The normalized `NativeImport` contract, provider adapters, receipts, branch/artifact identity and CLI import boundary remain one import authority; the fixtures and `tests/native_import.rs` are present in the candidate.
- External references are composed through `src/external_source/` and the gateway external modules. `ExternalConversationSource`, immutable message/chunk references, content hashes, source evidence and owner-local publication/processing receipts remain the canonical external boundary.
- `runtime_host_gateway_external*.rs` performs instance-bound authorization before index, search, hydration, processing or provenance reads. It forwards to the existing owner-local external source state and does not create a second gateway, store or ownership registry.
- The candidate root includes the external-source module and the import module without introducing a competing runtime host export. Project/file provenance continues through the existing `ProjectFileRef` and source-evidence contracts.

Candidate guards passed: external-source ownership (11 negative/positive fixtures plus production-boundary checks) and P4/P4.8 coverage (75 fixtures, 161 public API classifications). A `cargo fmt --all -- --check` attempt in the existing N3 worktree was blocked by Windows path canonicalization access denied (`os error 5`) before formatting diagnostics; no formatting change was made. The prior N3 import/external acceptance records remain the applicable runtime evidence pending the clean locked candidate run.

Step 2.5 closes source integration and ownership verification. Full locked tests and formatting remain release evidence for Step 2.7; no new import or external implementation is needed.

### Step 2.6 exports, interfaces and dependencies — 2026-10-11

Candidate `dcf500e` defines GatewayRuntime, InstanceRuntime, LiveSession and WorkspaceManagement once each. The existing gateway composition re-exports E/F types through the runtime and crate roots; no additional export overlay is required.

Exclude conversation-contract tip `beb122c` from convergence. Its five metadata/branch/Echo methods already exist in `runtime_host_gateway_conversation_metadata.rs`; replaying the old ambient implementation would duplicate those methods. Its additional `open_conversation_for` returns the historical InteractionSession cursor and is not needed by the accepted managed conversation/live-session API. Retain the branch only as historical evidence. D0 and P6 inventory/diagnostic overlays remain separate evidence pending current-source verification.

The candidate Cargo manifest and lockfile agree on one Arcana revision, `9ebccd6e7d089b98a8992c5451ba57f903f295fb`. Warlock still pins Reliquary `83b3ac87a80a4221039fa07d9722f5f8076464ba` and Arcana `66b4e96ecf18cc79dfd6b1ea30232bef7c86bba3`. Both must change together in Step 2.8 after candidate acceptance. Step 2.6 is therefore partial across repositories: export and overlay ownership decisions are resolved, while dependency alignment and compiled verification remain outstanding. No source or pin changes were necessary for the reviewed candidate.

### Step 2.6 completion verification — 2026-10-11

The candidate dependency choice is verified: locked Windows metadata resolves exactly one Arcana package at `9ebccd6e7d089b98a8992c5451ba57f903f295fb`, and `cargo check --locked --lib -j4` passed on the exact N3 candidate outside the filesystem sandbox (15 existing warnings). Earlier Windows canonicalization/missing-artifact failures did not recur in this unrestricted check. This establishes compiled candidate exports and dependency compatibility. The preceding partial assessment is superseded for candidate integration; actual production Warlock pin application and consumer verification remain the explicit Steps 2.8–2.9. Warlock's manifest pins were retained together at their existing compatibility pair until that application step.

Documentation impact: inspected the candidate manifests, lockfile, composition roots and overlay diff; updated this plan; runtime/storage documentation unchanged; both documentation policy modes passed. Architecture impact: no ownership rules or source boundaries changed; existing ownership guards remain applicable. Full test and refreshed architecture acceptance remain Step 2.7.

### Step 2.7 verification — 2026-10-11

Verification was rerun against the aligned candidate SHA `c36ca5f` in `Reliquary-n3-staging`. The Windows path failure was isolated to long-path canonicalization by running the candidate through a temporary drive mapping (`R:`); no source workaround or repository mutation was required.

- Passed: `cargo fmt --all -- --check` and CLI formatting through the mapped candidate path.
- Passed: `cargo check --locked --tests --examples -j2`.
- Passed: `cargo check --manifest-path cli/Cargo.toml --locked --offline` after aligning the CLI lockfile to Arcana `66b4e96`; the lockfile update is committed at candidate `c36ca5f`.
- Passed: full `cargo test --locked --offline -j2`: 1,159 unit tests, 138 integration tests, and doc tests; 0 failures.
- Passed: full locked CLI tests: 19 tests; 0 failures.
- Passed: P4/P4.8 coverage (75 fixtures, 161 API classifications), external-source ownership (11 fixtures and production boundaries), workspace-management ownership, and both documentation-policy modes.
- Still blocked: refreshed architecture synchronization. The static guards pass, but Lexicon initialization still reports `File not found` for the candidate `.lexicon` state and does not complete; prior Arcana synchronization also returned `os error 2`. This is an external Windows Lexicon/Arcana environment failure, not a Reliquary test or formatting failure.

This leaves 2.7 partially verified solely because the refreshed architecture snapshot cannot be completed in the current Lexicon/Arcana environment. All locked Reliquary and CLI tests, examples/target compilation, formatting, ownership guards and documentation checks now have fresh passing evidence. Earlier N3 acceptance records remain historical evidence and are not used to fill the architecture-refresh gap.

### Step 2.8 Warlock dependency pins — 2026-10-11

Warlock `feat/multiplexing-p5c-final-cut` now pins the candidate pair in `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`:

- Reliquary: `dcf500ea8438e9b3e782959dd0176efcb58b7b0a`.
- Direct Arcana: `66b4e96ecf18cc79dfd6b1ea30232bef7c86bba3`.

The prior Reliquary `83b3ac8` and Arcana `9ebccd6` pins are absent. Locked Cargo metadata resolves the aligned Arcana revision, and the dependency graph contains one Arcana package identity. The direct GitHub fetch for Reliquary cannot complete because `dcf500e` is not published on that remote; this remains a 2.10 publication/release limitation.

### Step 2.9 Warlock consumer matrix — 2026-10-11

The Arcana alignment fix is applied in both Reliquary and Warlock: Reliquary `Cargo.toml`/`Cargo.lock` and Warlock `src-tauri/Cargo.toml`/`src-tauri/Cargo.lock` now use `66b4e96ecf18cc79dfd6b1ea30232bef7c86bba3`. The previous direct-only `9ebccd6` alignment is removed. Rust verification used the repository-local Reliquary override in an isolated `.verification-tmp` target directory so the unpublished candidate could be compiled without changing the committed lockfile.

The earlier all-target failure against Arcana `9ebccd6` is superseded: the compatible metadata-reader API is now available from the same Arcana `66b4e96` revision in both repositories. No Warlock source workaround or duplicate Arcana identity was introduced.

Accepted 2.9 evidence:

- Reliquary `cargo check --locked --lib -j2`: passed with Arcana `66b4e96`.
- Warlock all-target check through the local Reliquary candidate override: passed.
- Provider execution tests: 4 passed.
- Task-context tests: 5 passed.
- Generation-viewer tests: 2 passed.
- Live-delivery/P4 stream tests: 2 passed.
- Warlock Clippy all-targets: passed with existing warnings.
- Frontend production build: passed.
- Graph-render client checks: passed.
- Documentation checks and changed-document policy: passed.
- Pitlord architecture enforcement: passed.

The direct GitHub fetch for Reliquary still cannot complete because `dcf500e` is not published. That distribution step and final release commit remain in 2.10.

### Step 2.10 commit and release evidence — 2026-10-11

Local integration commits are now created:

- Reliquary candidate `80a5fde` (`Align candidate with Arcana metadata reader`) updates the candidate's Arcana manifest and lockfile to `66b4e96`; candidate `c36ca5f` (`Align CLI lockfile with Arcana metadata reader`) updates the detachable CLI lockfile as well.
- Warlock `dee7ba0` (`Complete multiplexed Warlock consumer integration`) records the consumer changes and exact Reliquary/Arcana pins.

The aligned pair compiles and passes the focused 2.9 consumer matrix. The final release gate remains open because Reliquary `dcf500e` is not published at the declared Git source, and Step 2.7 still lacks accepted full Reliquary/CLI test, example, formatting and refreshed-architecture evidence.

The plan update is recorded on Reliquary `main` at `28a59c1`. Step 2.10 can close after the committed candidate is published and the outstanding 2.7 evidence is accepted.

## Step 3 — Legacy deletion

**Status: blocked on Steps 1–2.** D0 reverse-caller inventory accepted at `bfdd267`; no claim of completed deletion.

| ID | Action | Status |
|---|---|---|
| 3.1 | Revalidate accepted legacy-host inventory on converged source | Partial: converged-source scan rerun; reviewed baseline refresh required |
| 3.2 | Identify all remaining old callers and live consumers | Audited: four Reliquary test callers and the root export remain; Warlock uses canonical shared support types |
| 3.3 | Relocate retained types, domain queries and persisted readers | Complete in candidate `ba1ce9e`: shared contracts moved; canonical Gateway query/read paths retained; locked Reliquary and Warlock all-target checks pass |
| 3.4 | Remove `ReliquaryRuntimeHost` and public exports | Blocked |
| 3.5 | Delete old ambient REL/PHY/session selector authority | Blocked |
| 3.6 | Remove duplicate worker, mount, router and execution authority | Blocked |
| 3.7 | Migrate/delete tests that encode legacy-only behavior | Blocked |
| 3.8 | Enforce no-legacy-import/caller/build guards in Reliquary, CLI and Warlock | Blocked |
| 3.9 | Verify storage compatibility, restart and crash recovery | Blocked |
| 3.10 | Run complete post-deletion locked functional matrix and commit | Blocked |

**Exit gate:** one authoritative Gateway/Instance implementation with no legacy facade, duplicated state, callers or regressions. Use reverse dependencies, not filename-based deletion.

### Step 3.1 inventory revalidation — 2026-10-11

The accepted D0 inventory at `bfdd267` was rerun against the converged Reliquary candidate and current Warlock
consumer. The scanner self-tests passed all 17 fixtures. The candidate reports 918 Rust files and 139 runtime-host
modules, compared with the accepted D0 counts of 873 files and 134 modules. It still reports 170 legacy methods and
10 legacy fields, with zero CLI legacy references. Warlock at `dee7ba0` reports zero concrete legacy references.

The scanner's `--check` mode correctly rejected the result as inventory drift. No baseline was rewritten
automatically: the 45-file/5-module growth requires source and caller review before a refreshed inventory is accepted.
The remaining legacy methods and fields are deletion targets for Step 3, not evidence that deletion is complete.
Therefore 3.1 remains partial pending the reviewed baseline refresh and the D1 ownership cuts.

Documentation impact: this plan records the revalidation result; no runtime, storage, or source deletion was performed.

### Step 3.2 caller and consumer audit — 2026-10-11

The converged-source audit separates concrete old-host callers from canonical support consumers:

- Concrete external old-host callers are confined to four Reliquary test surfaces: `src/project_environment_tests.rs`,
  `src/project_environment_lore_tests.rs`, `src/project_environment_git_tests.rs`, and
  `src/semantic_graph/semantic_graph_runtime_host_tests.rs`. They construct `ReliquaryRuntimeHost` and are the
  explicit D2/D3 replacement set; the semantic-graph fixture also requires the canonical graph-owner mutation path.
- `src/lib.rs` is the remaining reachability seam: it registers `runtime_host`, re-exports
  `ReliquaryRuntimeHost`, and registers the legacy test modules. D1 must split common exports before D2 removes this
  seam.
- Reliquary's canonical Gateway/REL/PHY modules still share the runtime-host-named implementation files for current
  Gateway, worker, journal, publication, stream, and maintenance code. These are live canonical consumers and must be
  retained or relocated according to the D0 disposition; they are not evidence of external old-host use.
- Warlock `dee7ba0` has no concrete `ReliquaryRuntimeHost` caller. Its live imports are the reusable
  `ReliquaryRuntimeHostError` and `ReliquaryRuntimeRoutes` support types in `chat_gateway_execution.rs`,
  `reliquary_routes.rs`, `workspace_admission.rs`, `workspace_set.rs`, and related test support. These are the
  shared-type relocation set, not a surviving host facade.

This closes the caller-identification portion of 3.2. The source changes remain deferred to 3.3–3.8, after the
reviewed inventory refresh and ownership cuts; no compatibility alias or forwarding layer is introduced.

### Step 3.3 retained contracts and readers — 2026-10-11

Candidate `ba1ce9e` moves the reusable error, route, semantic-owner, Echo-source, and memory-search contracts into
`runtime_inference/runtime_shared.rs`. The old host module re-exports these existing types while it remains in the
tree for Step 3.4; the types no longer depend on the host implementation. The canonical Gateway already owns the
live semantic search, archive search, Echo, memory-provenance, knowledge, project-file, and external-source reader
entry points. Their persisted decoders and storage authorities remain in the underlying Archive/REL/PHY owners; no
new reader or writer was introduced.

`cargo check --locked --lib -j2` passed on the candidate, and Warlock's locked offline all-target check passed against
that exact local candidate through the repository-local verification override. Formatting and diff checks passed.
The remaining concrete legacy-host callers and host removal belong to Steps 3.4–3.8.

## Step 4 — Final release verification

**Status: harness prepared, final acceptance blocked on Step 3.** Existing P6 harness `81a64b6` and H5 diagnostics `b2b4bf4` are baseline evidence, not the final release verdict.

| ID | Action | Status |
|---|---|---|
| 4.1 | Port accepted harness to post-deletion runtime | Blocked |
| 4.2 | Verify concurrent instances and shared REL/PHY execution | Blocked |
| 4.3 | Verify grant enforcement, revocation and principal privacy | Blocked |
| 4.4 | Verify durable scheduling, cancellation/finalization races and ties | Blocked |
| 4.5 | Verify conversation branch policy and Archive ancestry consistency | Blocked |
| 4.6 | Execute crash, restart, receipt-loss, replay and recovery matrix | Blocked |
| 4.7 | Verify event distribution, stream checkpoints, resync and reconnect | Blocked |
| 4.8 | Verify real provider lifecycle, Unknown hold and truthful cancellation | Blocked |
| 4.9 | Run actual Warlock UI/Tauri integration tests | Blocked |
| 4.10 | Measure lock contention, workers, queues, memory, fan-out and throughput | Partial: diagnostic tooling exists |
| 4.11 | Run all locked code, test, architecture and documentation checks | Blocked |
| 4.12 | Repair regressions and record final release acceptance with evidence | Blocked |

**Exit gate:** actual post-deletion production consumer and runtime pass, with measured performance and no unresolved unacknowledged exceptions.

## Steps 5–6 — Hermes

Hermes remains the canonical owner of its conversation history. Design its stable feed/high-water/cursor, profile/message/branch identity, authorized hydration, edit/rewind/delete, and replay/rebuild behavior around the shared external-reference index. Build a thin plugin-facing adapter for Hermes sessions, provider execution and lifecycle over Gateway/Instance APIs. Preserve owner authorization; do not build a second multiplexing scheduler inside the plugin. Validate concurrent sessions and principals, shared PHY de-duplication, no transcript-body duplication, idempotent indexing and restart/cancellation/reconnect; verify that Hermes continues working while Reliquary is unavailable. The current conversation-index design is useful but **does not yet constitute a complete live plugin contract**.

## Stable technical references

- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md) — architecture decision
- [Architecture](architecture.md), [API](api.md), [behavioral contracts](behavioral-contracts.md), [invariants](invariants.md) — implemented state
- [External canonical conversation design](conversation-index-integration-plan.md) — remaining Hermes-specific design details, not implementation status
- [Current limitations](current-limitations.md) — known incomplete behavior
- Git history and retained verification logs — historical checkpoint evidence

## Documentation lifecycle rule

One active plan. No new dated multiplexing acceptance, baseline, checkpoint, or next-wave Markdown files. Update the status table and canonical current-state documentation; store exact command logs in CI/external verification artifacts and reference commit hashes only where they matter. Historical documents retired from the active tree remain recoverable from Git. If new design materially changes an interface, update the stable design owner or record an ADR; don't create another parallel project plan.

## Related docs

See the Stable technical references section above.

## Notes

This document is an execution roadmap, not proof that any unrun release test passed.
