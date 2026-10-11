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

## Step 1 — Warlock consumer completion

1. Wire actual execution-only provider host adapter and retain native pending authority where the host lacks durable queue semantics.
2. Reconcile remote outcomes and Unknown states across disconnect/restart; never redispatch an ambiguous operation; prove cancellation before replay.
3. Verify two authenticated independent instances, same-principal shared PHY worker and different-principal privacy, detached viewer continuity and per-context conversation/branch selections.
4. Ensure late join, slow subscribers, reconnect, mount epoch, cursor invalidation, resync and revocation use P4 durable stream/checkpoint paths; do not display provider deltas as committed output.
5. Verify exact task-root attribution and authorized project file reference propagation.
6. Run real HTTP/provider, Tauri/frontend, locked Rust, privacy, owner and no-legacy-consumer tests.

## Step 2 — Merge and pin

1. Reconcile the N3 staging line with accepted A/B/C/E/F/G commits; preserve **one** owner for each public DTO/export.
2. Re-run Reliquary formatting, locked check, all tests/examples, CLI, P2/P3/P4 guards, F/external guard and documentation checks.
3. Pin Warlock to the **same** verified Reliquary revision and matching direct Arcana revision, regenerate lockfile and confirm one Arcana package identity.
4. Re-run actual Warlock providers, admission, conversations, frontend production build and multi-context/reconnect fixtures.
5. Do not treat a green N3-local fixture as a production consumer pass.

## Step 3 — Legacy deletion

Use the prepared D0 reverse-caller inventory, not module-name globbing. Move surviving errors/reports/queries to their canonical owners and preserve persisted decoders. Remove `ReliquaryRuntimeHost`, ambient REL/PHY/session selection and duplicated workers/mount/router state. Migrate tests from legacy authority to Gateway/Instance behavior. Enforce absent imports/callers in Reliquary, CLI and Warlock. No compatibility facade.

## Step 4 — Final release verification

Use the existing P6 real-runtime harness on the **post-deletion** tree. Test multiple REL/PHY/instances, chronological pending order, branch policy, crash and receipt-loss recovery, cancellation-versus-finalization, unknown host state, stream durability, queue overflow/resync, late join, privacy/revocation, host interruption and reconnection. Measure contention, queue latency, fan-out, worker count, memory and startup against a reproducible baseline. Run full locked Reliquary/CLI/Warlock matrices plus actual UI/provider acceptance; capture logs outside active docs. Unavailable platforms or unresolved gates stay explicitly open.

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
