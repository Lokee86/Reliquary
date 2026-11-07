# Hermes Reliquary dashboard — Warlock UI port specification

Parent index: [Architecture](INDEX.md). Backend contract: [Hermes plugin](hermes-reliquary-plugin-implementation-spec-2026-11-06.md).

**Status: implementation-ready specification; UI not implemented or accepted.** Target the Hermes **web dashboard** plugin tab first, not its unrelated native desktop SDK. The required result is a working Memory graph/list, provenance/lifecycle inspection, explicit REL/PHY scope controls and a live ingestion/recall demonstration. Warlock presentation code is reusable; its Tauri transport and workspace authority are not.

## Purpose

Make Reliquary's actual memory state and behavior visible in Hermes without inventing a second data model or reporting fixture animations as live activity. The UI must expose who owns a Memory, why it exists, what source supports it, whether it is active/terminal/unavailable, and what the host actually retrieved or consumed.

## Overview

### Verified donors

The local `warlock-v2/src/features/workspace` tree contains `KnowledgePanel.tsx`, `KnowledgeInspector.tsx`, `KnowledgeRelationsEditor.tsx`, `KnowledgeGraph.tsx`, `GraphSelectionInspector.tsx`, `ScopeControls.tsx`, `GraphSourceControls.tsx`, `knowledgeApi.ts` and `knowledgeUi.ts`. `KnowledgeGraph` uses `GraphRenderClient`, `GraphRenderScene` and `tauriGraphRenderTransport` from `../graph/render`. `knowledgeApi.ts` directly invokes Tauri commands for snapshots, Memory creation/editing, relation mutation, community naming and provenance.

Port reusable React/rendering pieces after tracing their actual imports. Replace the Tauri transport with the frozen plugin backend contract in one step. Do not copy Warlock's workspace coordinator, runtime, transcript authority, Arcana integration or full-snapshot loading pattern into the plugin. Preserve donor provenance/license notices and record reused/adapted files at a pinned commit.

Hermes's inspected `website/docs/user-guide/features/extending-the-dashboard.md` describes a web plugin directory containing `manifest.json`, a prebuilt JavaScript bundle and optional `plugin_api.py` routes under `/api/plugins/<name>/`, using `window.__HERMES_PLUGIN_SDK__`. Native desktop `@hermes/plugin-sdk` is a different API and is excluded from the initial port. HP01 pins the exact loader/auth/SDK contract before UI integration.

## Target architecture and layout

`Hermes dashboard tab -> profile-authenticated plugin backend -> local service -> named Root/Gateway operations`

The browser never receives service credentials, owner file paths, direct Python/Rust store access or private source payloads outside the current grant. The backend resolves profile/principal from authenticated Hermes request context and forwards only server-bound permissions. Existing dashboard authentication must not be assumed to authorize every mounted owner; apply Reliquary grants again. Reject cross-profile route parameters, cross-origin mutation requests and stale generation tokens.

The tab has three coordinated regions: owner/scope and filters at left, paged Memory list or bounded graph in the center, and a selected-item inspector at right. A compact status area shows ingestion/worker/recall state for the selected owner/session. Scope defaults to the active session's REL; PHY is explicit and permission-checked. Show the active owner name/kind on every detail and mutation confirmation. Scope changes cancel in-flight requests and clear private selection/cache before rendering the new owner.

Graph is the default visual mode, with an accessible list alternative and keyboard navigation. No full application theme replacement is required. Reuse Hermes typography/tokens and make errors/loading/empty/unavailable states distinct. Do not add generated images or a new design system as a dependency of functional acceptance.

## UI/backend data contract

Freeze DTO schemas in HP01 before parallel UI/backend work. All responses bind owner ID/kind, history generation/cut and capability flags; integer timestamps/versions travel as decimal strings when precision requires it. Read cursors bind filters and cut; after mutation or owner remount return stale-cursor/resync rather than stitching inconsistent pages.

| View/operation | Required fields and bounds | Ownership / behavior |
| --- | --- | --- |
| Owner/session selection | Authorized owner ID/kind/display label, active session mapping, view/manage capabilities | No client-selected grant widening, no cross-owner ambient merge |
| Memory list | Stable ID, revision/body ID, title/excerpt, category/type, lifecycle/canonical status, Freshness when supported, community ID | Paginate under accepted management item/byte limits; filters applied server-side, not by fetching all bodies |
| Detail/history | Current payload availability, exact retained revisions, supersession/tombstone state, typed provenance | Explicit missing/unauthorized/pruned/corrupt handling; never substitute current content for an unavailable historical revision |
| Provenance | Hermes source namespace/session/message/branch/revision/hash; copied-snapshot label; exact authorized source range | Revalidate source authorization/revision on drill-down; deleted or stale source remains visibly unavailable |
| Graph | Stable semantic node/edge IDs, kinds/origins, source/target, revision/cut, bounded neighborhood/community summary | Initial target max 500 nodes/1,000 edges per page; lazy expansion and explicit truncation; cut-bound source resolution |
| Community | Stable generation/member count/name/name-origin, cursor | Missing routing/community metadata is not an empty graph; display actual unavailable state |
| Operational status | Accepted source cursor, last completed receipt, pending/held/error counts, latest recall selection and confirmed-use status | Read-only activity view; no fabricated progress percent or provider-success claim |
| User edits | Expected owner generation/revision, stable request ID, supported change and resulting receipt | Named product workflow, stale-update conflict, server validation and refreshed committed result |

The graph transport must not require the old Warlock tiled/binary renderer protocol if it does not fit the accepted owner read seam. Keep a small renderer adapter with bounded immutable graph pages; reuse geometry/rendering where it earns its place. Rendering IDs are not semantic Memory IDs, and a selection from an old generation cannot resolve against the new one. Arcana/repository graph switching is out of scope.

## Required interactions and mutation policy

1. Switch among authorized RELs and the current user's PHY; search/filter, graph/list toggle, select node/Memory and inspect history/provenance.
2. Show live state after accepted ingestion and later recall. A dashboard read or graph expansion never reinforces Freshness. Distinguish candidate, selected, injected and confirmed-consumed states using actual receipts.
3. Create a manual Memory, edit its body/title with expected revision, and retire it only through supported user-origin owner workflows. Scope and provenance must remain explicit; no forged source-cited Memory.
4. Port the relation editor for user-origin links and community rename through named existing semantics or a minimal HP02 owner-level addition. Preserve user-origin edges against Dream inference. Canonical/duplicate/supersession edits require their actual closure rules; do not expose arbitrary raw edge mutation that bypasses them.

Capability flags expose only implemented and tested operations. A missing API is an HP02 gap, not permission to show a successful no-op. Required v1 actions above must work to close HU04; any proposed scope reduction requires an explicit spec change, not a hidden disabled control. Unsupported advanced relation kinds can remain absent only when documented in the capability map and not promised as supported.

Confirm destructive/retirement actions with owner, Memory identity and expected effect. On conflict, preserve the user's draft but reload the committed revision; never overwrite newer state automatically. Show typed denial without leaking private titles, source existence or counts. HTML/Markdown from Memory/source is untrusted: safe rendering, no executable scripts, unsafe URLs, raw HTML injection or tool execution.

## Reuse map

| Donor | Keep/adapt | Replace or exclude |
| --- | --- | --- |
| KnowledgePanel | Layout, mode/selection/filter behavior | Whole-snapshot fetching; Warlock WorkspaceSummary authority |
| KnowledgeInspector / GraphSelectionInspector | Detail/provenance presentation, revision/lifecycle UI | Tauri calls; current-content fallback for missing source |
| KnowledgeRelationsEditor | User interaction and relation labels | Unconditional edge updates; add cut/revision checks and real capability map |
| KnowledgeGraph / GraphRenderScene | Rendering/selection/navigation after import review | Tauri transport, stale generation resolution, Arcana provider |
| ScopeControls | Explicit owner selection patterns | Warlock mount/global-workspace orchestration; browser file paths |
| knowledgeApi | Named interface concepts and field review | All `invoke` calls and broad mutable snapshot response assumptions |

## Tickets

| Ticket | Hard predecessors | Deliverable | Acceptance and evidence |
| --- | --- | --- | --- |
| HU01 — pin donor/DTO and build tab shell | HP01 | Donor dependency/license map, dashboard manifest/build entry, typed mock contract and scoped layout | Real dashboard loads bundle; no Tauri/runtime import, no embedded secrets, owner changes clear stale/private state |
| HU02 — bounded graph/list/inspector port | HU01 | Paged views, graph transport, exact provenance/history and accessible fallback | Contract-driven component tests for pagination/cut expiry, untrusted content, unavailable payloads, 500/1,000 page bounds and keyboard navigation |
| HU03 — edits, scopes and live operations | HU02, HP02 | Real mutation wiring, capability map and operation/recall indicators | Stale revision/revocation/foreign PHY denied; user-origin edit/link/name persists; inspector reads never reinforce; honest held/offline states |
| HU04 — real-service acceptance | HU03, HP05 | Actual Hermes tab + service/provider end-to-end evidence and packaged assets | Two profiles/two RELs/private PHYs, ingestion -> graph -> source -> recall -> restart; all required actions and error states tested without mock-only substitution |

UI work can begin on DTO fixtures after HP01 while backend APIs are implemented. The final gate waits for actual HP05 semantics. HU04 does not depend on HP07; HP07 consumes HU04 for the demo, avoiding a circular dependency.

## Testing decisions

Use the plugin's pinned JS build/typecheck/component/browser-test scripts, frozen by HU01 after inspecting donor/SDK versions. The bundle must use the host's supported React/SDK packaging and must not accidentally load a second incompatible React runtime. No assumed build command counts as evidence until present in the plugin package manifest.

Mandatory browser cases: no owner, unauthorized owner, empty graph, loading/error, profile switch with late response, stale graph generation, unavailable/deleted source, Unicode/multiline provenance, large paginated data, cancelled network request, service restart, viewer mutation denial, user edit conflict, explicit retirement confirmation, renderer cleanup and clipboard/export privacy. Test browser backend request authorization separately from component visibility; hiding a control is not enforcement.

Run a real end-to-end fixture in Hermes dashboard against the native 2.1 service, not just an iframe mock. Record reproducible steps, source/runtime commits and observable receipt IDs without private payloads or secrets. Measure graph first-page and interaction responsiveness on the bounded fixture; enforce cancellation and page bounds rather than claiming full-database rendering performance. Archive optional screenshots only as supporting evidence, not acceptance truth.

## Completion, exclusions and user outcome

HU04 closes when graph/list, provenance/lifecycle, REL/PHY controls, required user edits and live ingestion/recall status work through authenticated current APIs, with exact source and recovery evidence. The user can inspect and correct actual memory rather than watching a canned graph.

Excluded: Warlock runtime port, Arcana integration, native desktop SDK, global dashboard skinning, fake observations/Ego summaries, model-quality claims, raw owner-file upload in browser, arbitrary cross-owner merging and automatic history conversion. Full Context Engine UI controls remain absent until their backend behavior exists.

## Related docs

- [Hermes provider/service contract](hermes-reliquary-plugin-implementation-spec-2026-11-06.md)
- [Post-G4 parallel plan](post-g4-parallel-execution-plan-2026-11-06.md)
- [Earlier host integration plan](../conversation-index-integration-plan.md)

## Notes

The initial UI is the web dashboard extension, not a new standalone frontend or transplanted Tauri application. Backend protocol DTOs, not donor UI assumptions, define security, cut stability and current product capabilities. Warlock files are inspected donors; their names are not evidence that all 2.1 backend operations are already exported.
