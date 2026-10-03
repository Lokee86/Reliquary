# Phase 0B — owner fencing, quiescence, and consumer migration

## Purpose

Resolve the G5 owner lifecycle/fencing design and G6 downstream consumer impact for the multiplexing readiness review. This page is a concrete design draft and source audit, not evidence that runtime behavior or repository baselines have passed.

## Overview

The first deployment is scoped to one in-process GatewayRuntime per mounted owner. Owner identity, gateway fencing, lifecycle leases, and recovery are specified below; OS-backed cross-process exclusion remains a separate implementation gate. Warlock is an inspected downstream consumer with an active inference cancellation mechanism but no durable chat queue. A standalone fixture tests the local fencing/recovery contract.

Parent index: [Documentation index](INDEX.md)  
Readiness gates: [Multiplexing readiness review](multiplexing-readiness-review.md)  
Target architecture: [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)  
Current-source baseline: [Multiplexing Phase 0 baseline](multiplexing-phase0-baseline.md)

## Status and scope

This is a design draft for G5 and G6. It records the current Warlock contract and a concrete in-process fencing/recovery protocol. The accompanying std-only Rust fixture is an executable contract model; it does not prove the production host implements this behavior.

The supported first deployment is one GatewayRuntime process owning a given REL/PHY at a time. GatewayRuntime can enforce uniqueness among its own mounts and issue owner/mount-scoped operation leases. Reliquary currently has no proven cross-process file lock. Public `Cva::open`/write APIs can bypass an in-process gateway, so neither a path registry nor an owner UUID will be described as cross-process exclusion. External writers are unsupported while a gateway owns an owner. Offline maintenance requires stopping the owning application, closing/unmounting every handle, performing the operation, then reopening and reconciling before dispatch resumes. A copied owner on another device has only a local file key and local gateway: matching owner UUIDs do not create distributed locking or global at-most-once execution. Before syncing or resolving conflicts between device copies, quiesce both local gateways and reconcile/promote each local file offline; do not run concurrent external effects from both replicas. If concurrent external processes must be supported later, an OS-backed lock and tests for the actual supported filesystems are a separate required design/implementation gate.

## G5 — owner identity, mount fencing, and lock/recovery contract

### Mount identity

Durable owner UUID is semantic identity; it does not prove that two paths refer to the same physical file. The Gateway registry must maintain both:

| Identity | Source | Purpose |
| --- | --- | --- |
| `owner_id` (`rel-<uuid>` / `phy-<uuid>`) | Validated typed file header | Enforce one authoritative execution per semantic owner. |
| `file_key` | Canonical path plus OS file identity when available (Windows volume serial/file index; Unix device/inode) | Detect aliases to one physical file, including hard links where the platform exposes identity. |
| `mount_epoch` | Gateway-generated monotonically increasing value per owner installation | Fence delayed worker results, subscriptions, and callers after quiesce, replacement, or lost continuity. |

Mount rules:

1. Opening an alias with the same `owner_id` and same `file_key` reuses the existing execution; it never constructs another mutable `Cva`/PHY handle.
2. The same `owner_id` with a different `file_key` is a conflicting replica/copy and fails closed. The caller must reconcile it offline, then mount the chosen canonical file. A different path alone is not evidence of a different owner.
3. The same `file_key` reporting a different owner ID/file kind fails closed as inconsistent identity or a stale path handle.
4. If a filesystem cannot supply a stable file identity, the gateway can only deduplicate by canonical path. Such a mount is marked as path-scoped and does not claim hard-link or cross-process protection.
5. Owner UUIDs survive copy/rename/repack by design. Therefore UUID equality alone cannot distinguish a cloud-conflicted replica from an alias; the file-key comparison is required.

This is a gateway boundary, not a promise that arbitrary direct library clients cannot open the same file. Direct lower-level file APIs remain for explicit owner operations, tooling, tests, and offline maintenance; concurrent direct writes against a gateway-mounted file are unsupported.

### Admission, commit leases, and revocation ordering

A generation token by itself is not authorization to mutate. The design uses short counted commit leases:

- A captured work token binds `principal_id`, `attachment_incarnation`, `grant_generation`, `owner_id`, `mount_epoch`, and stable `operation_id`.
- Dispatch, durable commit, and outbound delivery each acquire a short lease atomically under the Gateway authority gate after validating grant, attachment incarnation, owner mount epoch, and admission state. The gate increments the active lease count and is then released before owner locks, transport work, or delivery callbacks.
- A commit lease admitted first may finish its bounded owner-local mutation. Revocation closes new lease admission under the same gate, drains already-issued short dispatch/commit/delivery leases, advances the grant generation, then clears that attachment's outbound queues. A later lease request fails. Revocation cannot report completion while an admitted write or delivery remains active.
- Quiesce similarly closes admission, freezes pending work, and drains short leases before stores close or the mount epoch advances. The authority mutex is never held while a store lock, provider call, or callback is held.
- Long inference has no REL/PHY store lock and no lease spanning model/provider calls. It carries a work token; stale/revoked work can finish computation but cannot acquire a later commit lease.
- Each cross-owner stage acquires and releases one owner lease independently. REL and PHY store locks are never nested. Revocation between stages leaves a recoverable partial saga and blocks the next owner mutation until reauthorized/reconciled.

This gives revocation a linearization point and avoids a check-then-write race: a revocation cannot report completion while a previously admitted durable write or delivery is still running. Work from a detached/re-attached instance also fails the attachment-incarnation check even if the same principal and owner are involved.

### Concrete GatewayRuntime contract

These are target methods and return contracts, not existing APIs. Exact Rust names may change while the ownership and linearization rules remain binding.

| Contract method | Required behavior |
| --- | --- |
| `mount_rel(path)` / `mount_phy(path)` | Open and validate kind/owner UUID inside the gateway boundary; reuse an exact mounted physical owner; reject a different file identity for an already-mounted UUID; return owner ID plus mount epoch. |
| `attach_instance(verified_principal, grants)` | Create an attachment incarnation bound to one verified principal/PHY and explicit owner grants; callers cannot select another PHY by request parameter. |
| `acquire_dispatch_lease(attachment, owner, operation)` | Atomically validate grant, attachment incarnation, owner epoch, and running admission before an operation crosses a dispatch boundary. It is short-lived and never spans inference. |
| `acquire_commit_lease(work_token)` | Atomically validate the same fences and count a bounded owner-local mutation before it starts. Revocation/quiesce drains leases already granted. |
| `acquire_delivery_lease(attachment, owner, cursor)` | Validate instance/owner scope before queueing or delivering an outbound event; revocation drains leases and then clears that attachment's queued deliveries. |
| `revoke_attachment(attachment)` | Close new leases, drain existing short leases, advance the grant generation, clear only that attachment's outbound queues and subscriptions, then report completion. |
| `quiesce_owner(owner, reason)` | Close owner admission, freeze accepted pending work, drain/fence active work, close owner handles, perform offline maintenance, reopen/reconcile, advance mount epoch, then resume. |
| `reconnect_host(host)` | Initiate on startup/reconnection before host-bound dispatch; negotiate real capabilities and reconcile shared IDs/status/cancellation before resuming that stage. |

### Lock order and bounded critical sections

Use a gateway admission/lifecycle gate only to validate and count operation/commit leases. Release it before touching stores. Each mutable owner has one lock and one authoritative open handle. A mutation follows:

`authority lease → one owner lock → validate owner-local preconditions → durable write/sync → release owner lock → release lease`

There is no `REL lock → PHY lock` or `PHY lock → REL lock` path. Model inference, embedding, provider HTTP, UI callbacks, stream delivery, and subscriber backpressure run without owner locks. A commit lease spans only the short durable publication and its sync; if sync cannot complete, return a failure and leave operation reconciliation state explicit.

### Quiesce and maintenance state machine

`Running → AdmissionClosed → PendingFrozen → InFlightDraining → StoresClosed → Maintenance → ReopenAndReconcile → EpochAdvanced → Running`

- **AdmissionClosed:** reject new submissions, worker dispatch, subscriptions that require mutation, and new owner operations.
- **PendingFrozen:** preserve every already accepted durable pending item and its stable identity/order. Do not silently cancel or discard accepted queued work. No frozen item may dispatch during maintenance.
- **InFlightDraining:** wait for active short commit leases. Active provider work follows explicit caller policy: normal shutdown/replacement requests cooperative interruption and records terminal interruption; a non-cancellable operation must finish before owner replacement. Work that has only computed output but not acquired its commit lease is fenced from publishing.
- **StoresClosed:** stop owner workers and verify all owner handles/commit leases are released.
- **Maintenance:** reconcile/promote/reclaim only the offline canonical file. A failure leaves the owner unavailable and admission closed; do not resume against uncertain state.
- **ReopenAndReconcile:** reopen through normal validation; reconcile stable operation IDs, Archive finalizations, native pending submissions, external host status/cancellation, and durable worker receipts before dispatch.
- **EpochAdvanced:** install a fresh mount epoch and invalidate all prior tokens/cursors. Revalidate instance grants before restoring feeds/subscriptions.
- **Running:** resume only after reconciliation has a determinate outcome. Unknown external status remains held for reconciliation, never blindly replayed.

Unrelated RELs keep running during an owner-local quiesce. Gateway shutdown quiesces all owners; owner replacement/quiescence is per-owner.

### Cross-owner publication and crash recovery

Insomnia or another REL-to-PHY operation is a staged saga, not a cross-file transaction:

1. Claim/compute under a stable operation ID; compute outside owner locks.
2. Persist or retain an explicit stage identity before the first externally visible target write.
3. For a personal result, resolve the destination PHY from immutable principal attribution. Acquire a PHY commit lease and publish idempotently under the operation ID; release the PHY lock.
4. Acquire the REL commit lease and write the REL completion receipt for that same operation ID; release the REL lock.
5. On recovery, inspect the durable target and source receipt. A repeated stage with the same ID returns the recorded result and does not publish twice. If one owner cannot prove whether a stage committed, hold the item in reconciliation-required state; never blindly replay a potentially visible publication.
6. If the destination grant was revoked between stages, stop before the next write and preserve enough recovery state for an authorized retry or terminal non-publication.

Current limitations explicitly say routed PHY publication is a separate write before the REL receipt and no cross-file atomic transaction exists. The production implementation must therefore add/prove owner-local idempotency for each stage (stable semantic IDs or durable operation receipts) before advertising exactly-once cross-owner effects. Archive finalization similarly needs a stable submission ID and recovery check between Archive commit and journal finalization. Exactly-once external provider execution is not promised without host cooperation.

The executable fixture covers admission/epoch and staged retry logic as a contract model. It cannot substitute for storage reopen, crash injection, or a real filesystem lock test.

## G6 — verified downstream consumer and migration impact

### Inspected consumer: Warlock v2

The sibling checkout inspected was `C:\!bin\workspace\Warlock-v2`. Its source confirms the following current capabilities; this matrix describes code found there, not hypothetical framework behavior.

| Responsibility/stage | Current Warlock behavior and evidence | Consequence for multiplexing |
| --- | --- | --- |
| Runtime/process owner | In-process Tauri `WorkspaceService` owns one `ReliquaryRuntimeHost`, mounted REL paths, one PHY path, and one active REL (`src-tauri/src/workspace.rs`; `docs/RUNTIME_OPERATIONS.md`). No daemon/service boundary. | Keep Warlock as one application host; instantiate/retain one GatewayRuntime and create an InstanceRuntime for each actual operating context/device/agent. |
| Startup/reconnect | Restores local `workspace.session` paths and remembered active REL through `restore_workspace_session`; it has no remote Reliquary host connection (`src-tauri/src/workspace_session.rs`). | This is local mount restoration, not a host queue reconnect handshake. A future external host adapter must connect immediately and reconcile before boundary dispatch. |
| Chat admission/queue | `send_message` reserves an inference lease, then calls `send_user_message_with_attachments` before context preparation/provider execution (`src-tauri/src/chat_commands.rs`, `conversation.rs`). There is no durable pending-chat queue or independently cancellable provisional submission in the inspected Warlock source. | Warlock cannot presently own the selected durable provisional queue. Reliquary must own it for native submissions unless a later host API supplies pending cancellation, status/reconciliation, and serialized dequeue/cancel. |
| In-flight cancellation | `InferenceLifecycle` tracks active `ProviderCancellation` values; `quiesce` disables admission, requests cancellation on active calls, and waits for leases to drop (`src-tauri/src/inference_lifecycle.rs`). Cancellation is an in-flight cooperative provider interrupt, not evidence of pending queue cancellation or durable status/replay. | Preserve as provider-call cancellation. It can be invoked by Reliquary quiescence policy; it cannot replace Reliquary queue ownership. |
| Generation/checkpoint | Warlock streams provider events via a Tauri `Channel`; durable assistant checkpoints and interrupted-stream recovery are Reliquary responsibilities, while Warlock displays events (`chat_commands.rs`, `chat_stream.rs`, Reliquary baseline). | Keep checkpoint-before-visible-delta and stream authorization in Reliquary; map authorized instance subscription to the UI channel. |
| Conversation/session | Warlock calls `start_conversation_session`, `reopen_conversation_session`, `require_active_session`, and reads the host's active session through `conversation.rs` / `conversation_stream.rs`. It expects one managed active session in the shared host and has no multi-view attach API. | This is a real caller migration: replace host-global selection assumptions with instance-local selected conversation plus gateway-owned shared execution/attachment. |
| Provider capabilities | Warlock supplies `GeneralEndpoint` / `EmbeddingEndpoint` adapters from its provider registry through `ReliquaryRuntimeRoutes` (`reliquary_routes.rs`, `reliquary_general.rs`, `reliquary_embedding.rs`). It owns provider routes, credentials, transport, and cooperative interactive cancellation. | Keep provider execution in Warlock; Gateway owns durable submission/operation identity and requests cancellation/status through a defined adapter contract. |
| Owner maintenance | Workspace transitions quiesce interactive inference, then Reliquary reconciliation/open/mount operations manage owner state (`workspace_session.rs`, `workspace_open.rs`, ADR-0010). | Coordinate the same transition with Gateway owner quiescence so no internal worker or stale commit races file replacement. |

Warlock's current dependencies are pinned in `src-tauri/Cargo.toml` and lockfile: Reliquary rev `83b3ac87a80a4221039fa07d9722f5f8076464ba`; direct Arcana rev `66b4e96ecf18cc79dfd6b1ea30232bef7c86bba3`. The inspected Reliquary worktree manifest pins Arcana `9ebccd6e7d089b98a8992c5451ba57f903f295fb`. That is a real integration version boundary to reconcile before claiming the direct Arcana dependency unifies with the new Reliquary API; update dependency pins/lockfile together and verify a single intended Arcana instance at the selected revisions.

### Public API impact and rollout boundary

Warlock directly stores/exports a `ReliquaryRuntimeHost` inside `WorkspaceService`, constructs `InteractionRuntime::new(cva)` during mount, and passes the host into helpers. A public hard cut therefore affects at least:

- runtime composition/mount: `workspace.rs`, `workspace_open.rs`, `workspace_session.rs`, plus workspace lifecycle commands;
- conversation, streaming, and tools: `conversation.rs`, `conversation_stream.rs`, `chat_commands.rs`, `chat_stream.rs`, `chat_tool_runtime.rs`;
- semantic/context consumers: `workspace_echo.rs`, `chat_echo.rs`, `workspace_memory_provenance_view.rs`, retrieval/context/compaction modules;
- project environment, reconciliation, adoption, and checkpoints: `workspace_project_*.rs`, `workspace_reconciliation*.rs`, `workspace_compaction.rs`;
- provider route integration: `reliquary_routes.rs`, `reliquary_general.rs`, `reliquary_embedding.rs`;
- corresponding Rust tests and UI command payload compatibility.

Warlock's accepted ADR-0010 says it coordinates/presents while Reliquary owns semantic/runtime behavior; provider adapters and application lifecycle remain Warlock-owned. Keep those boundaries. Existing Tauri/frontend shapes should remain stable where practical, but helpers that currently receive a raw host must migrate to explicit gateway/instance operations; preserving the host-shaped facade solely to avoid caller changes would retain the ambient singleton source of truth.

A bounded source search of the available `hermes-agent` checkout found no `reliquary_memory`, `reliquary-memory`, `ReliquaryRuntimeHost`, or `InteractionRuntime` references in Rust source or Cargo manifests. No ACP source checkout was identified in the available mounted repository set. The Reliquary roadmap names future ACP and Hermes adapters, which remain future integration work; the search result is limited to the inspected roots and extensions and is not a claim about every external framework or private checkout.

### G6 characterization and integration test sequence

1. Preserve the current Warlock package's compile/test baseline at its pinned Reliquary/Arcana revisions.
2. Add a failing contract test proving two instances can select the same shared REL/conversation independently, attach to the same generation without restarting it, and cannot finalize/cancel each other's operation.
3. Migrate one consumer boundary at a time to final GatewayRuntime/InstanceRuntime methods; remove direct ambient host calls as each group moves.
4. Run Reliquary library tests, Warlock Rust tests and frontend build against the same explicit dependency revisions; verify only one expected Arcana crate instance and no API compatibility wrapper hiding stale singleton use.
5. Test reconnect/failure behavior with Warlock's actual provider adapter: no host pending queue exists today, so queue handoff tests first require a real capability contract implementation or a fake adapter implementing cancellation/dequeue/status races.

This source audit resolves the known Warlock impact and its actual stage capabilities. The first isolated `cargo check --locked` attempt against Warlock's current checkout failed during dependency compilation with missing crate/rmeta artifacts under the dedicated target directory; no source/API diagnostic was reached. A serialized `cargo check --locked -j 1` retry was cancelled after about six minutes while it was still compiling dependencies, to release the worktree lock for the integration build; it produced no source/API diagnostic and therefore leaves the Warlock compile baseline inconclusive. This compile check is not the full Warlock test/frontend gate. Reliquary's recorded baseline remains red: root reported 767/768 library tests with a Lore file-sharing failure, the full test gate blocked by the existing example allocator conflict, and one runtime filter test group at 43/44 due to transient backpressure. These are baseline failures, not attributed to this draft.

## Related docs

- [Multiplexing implementation plan](multiplexing-implementation-plan.md)
- [Multiplexing readiness review](multiplexing-readiness-review.md)
- [Multiplexing Phase 0 baseline](multiplexing-phase0-baseline.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
- [Warlock ADR-0010 — environment coordination boundary](../../Warlock-v2/docs/decisions/0010-reliquary-environment-coordination-boundary.md)

## Notes

- The Rust fixture is deliberately std-only and models owner identity, short lease admission, quiescence, epoch advancement, unrelated-owner progress, and staged retry deduplication. Its gates are model code; it does not exercise Reliquary storage, filesystem locks, or provider cancellation.
- Current owner UUID and file-kind facts come from the typed 40-byte REL/PHY header contract; copy/repack preserves UUID. See [storage format](storage-format.md).
- The supported deployment claim remains single-process/cooperative. Cross-process exclusion, stale external writers, and filesystem lock semantics require a separate proven mechanism before that claim can expand.
