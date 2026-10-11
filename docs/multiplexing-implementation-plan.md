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
| 1.1 | Audit current Warlock consumer against N3 interfaces and actual dependency pins | Not started |
| 1.2 | Finish execution-only provider adapter | Partial |
| 1.3 | Implement remote outcome reconciliation, reconnect and Unknown recovery | Partial |
| 1.4 | Complete cancellation and provider operation lifecycle | Partial |
| 1.5 | Complete multi-instance context and session management | Not verified |
| 1.6 | Verify shared PHY ownership and cross-principal isolation | Not verified |
| 1.7 | Integrate checkpointed durable conversation streams with Warlock UI/Tauri | Not verified |
| 1.8 | Implement public reconnect, resync and subscription recovery | Not verified |
| 1.9 | Complete exact task-root attribution and ProjectFileRef propagation | Not verified |
| 1.10 | Run full real-provider, frontend, privacy and Warlock acceptance matrix | Blocked on 1.2–1.9 |

**Exit gate:** upgraded Warlock uses canonical Gateway/Instance APIs with correct real-provider recovery, streaming, authorization and multi-context behavior; commit and log exact tests and pins.

## Step 2 — Merge and pin

**Status: partial.** E/F/G/B-E core is staged in N3, while final convergence and production dependency pins are unaccepted.

| ID | Action | Status |
|---|---|---|
| 2.1 | Audit all active integration branches, unique changes and accepted SHAs | Not verified |
| 2.2 | Establish final convergence candidate from accepted N3 source | Partial |
| 2.3 | Integrate accepted Gateway/Instance, background and queue ownership cuts | Partial |
| 2.4 | Integrate accepted live runtime and management surfaces | Partial |
| 2.5 | Integrate accepted native import and external-reference core | Partial |
| 2.6 | Resolve exports, interface ownership and dependency conflicts | Partial |
| 2.7 | Run full locked Reliquary/CLI tests, examples and architecture/ownership guards | Blocked on final candidate |
| 2.8 | Pin Warlock to exact accepted Reliquary and Arcana revisions | Blocked on Step 1 and final candidate |
| 2.9 | Run full pinned Warlock provider, context, stream and frontend matrix | Blocked on 2.8 |
| 2.10 | Commit and verify integrated source, exact pins and acceptance evidence | Blocked on 2.7–2.9 |

**Exit gate:** one integrated code revision, one intended Arcana package identity, and no untested or conflicting runtime interfaces. Do not reimplement already accepted core features.

## Step 3 — Legacy deletion

**Status: blocked on Steps 1–2.** D0 reverse-caller inventory accepted at `bfdd267`; no claim of completed deletion.

| ID | Action | Status |
|---|---|---|
| 3.1 | Revalidate accepted legacy-host inventory on converged source | Partial: historical inventory exists |
| 3.2 | Identify all remaining old callers and live consumers | Blocked on convergence |
| 3.3 | Relocate retained types, domain queries and persisted readers | Blocked |
| 3.4 | Remove `ReliquaryRuntimeHost` and public exports | Blocked |
| 3.5 | Delete old ambient REL/PHY/session selector authority | Blocked |
| 3.6 | Remove duplicate worker, mount, router and execution authority | Blocked |
| 3.7 | Migrate/delete tests that encode legacy-only behavior | Blocked |
| 3.8 | Enforce no-legacy-import/caller/build guards in Reliquary, CLI and Warlock | Blocked |
| 3.9 | Verify storage compatibility, restart and crash recovery | Blocked |
| 3.10 | Run complete post-deletion locked functional matrix and commit | Blocked |

**Exit gate:** one authoritative Gateway/Instance implementation with no legacy facade, duplicated state, callers or regressions. Use reverse dependencies, not filename-based deletion.

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