# Hermes / Reliquary plugin — native 2.1 implementation contract

Parent index: [Architecture](INDEX.md). Product track in the [post-G4 plan](post-g4-parallel-execution-plan-2026-11-06.md).

**Status: implementation specification; no plugin implementation or hackathon acceptance claimed.** Revised November 7 to freeze copied-source tombstoning, REL/PHY identity topology and the temporary retrieval-confirmation HACK. The initial target is a functioning Hermes memory plugin and dashboard demonstration using new 2.1 owners. It does not depend on D01–D03 or historical-corpus release. Existing data must not be opened with a guessed compatibility reader.

## Purpose

Expose Reliquary's accepted memory machinery in real Hermes conversations and make its behavior inspectable through the companion UI. Reuse Reliquary's owner/runtime policies and Hermes's plugin surfaces rather than embedding a second memory engine or replacing Hermes's canonical transcript authority.

## Overview

### Source-backed starting points

The inspected local Hermes checkout provides `agent/memory_provider.py::MemoryProvider`, orchestrated by `agent/memory_manager.py`, plus standalone plugin discovery. Core methods include `initialize`, `prefetch`, `queue_prefetch`, `sync_turn`, tool schemas/dispatch and `shutdown`; optional hooks cover turn/session change and pre-compression. `initialize` receives profile-scoped `hermes_home`; turn authors may vary inside a shared session. Provider worker threads must propagate the caller's contextvars through the supplied helpers.

`hermes-agent/plugins/AGENTS.md` requires new memory providers to be standalone plugins, not new in-tree backends or Reliquary-specific changes in `run_agent.py`/CLI/Gateway core. Setup uses `hermes memory setup` and the supported provider configuration hook. Pin the actual Hermes source revision during HP01; inspected interfaces are not an unbounded promise about future upstream versions.

The inspected persistence path stamps each durable logical message with `message_uid` and its persisted timestamp before the completed `messages` list reaches `sync_turn`, so normal live copying has a real source for stable message identity and time. HP01/HP04 still prove that behavior with the actual provider and separately cover restart catch-up; the callback itself is not treated as a complete-history export. The currently audited Hermes checkout does not expose a provider `on_session_delete` hook, so HP01 must repin to a supported revision that exposes archive/delete lifecycle notification, or record the smallest generic lifecycle addition if no supported revision provides it. Do not scrape Hermes SQLite or invent deletion polling as a substitute.

The rebuilt Root currently exports opaque owners, trusted host admission, workspace management, maintenance and selected queue/host types. Its `src/lib.rs` does NOT yet expose every worker/retrieval/graph method a complete plugin UI needs. HP02 must close these concrete product-facing gaps through named existing workflows; G4 acceptance is not evidence those external APIs already exist.

## Transcript ownership decision

**Initial copied-input mode follows the user's November 6 direction:** Hermes remains the canonical durable history owner; Reliquary initially receives source-ID-preserving copies for memory processing. Later reference-only hydration may replace copies. This is an explicit scoped variation from the earlier [external-source integration plan](../conversation-index-integration-plan.md), which prohibited duplicate bodies. It is not permission for two competing canonical transcripts.

Copies enter supported normalized/native import workflows as explicitly imported Hermes snapshots, never as fabricated new live Gateway turns. Retain Hermes profile/source namespace, stable session/message/branch IDs, original role/time, source revision or verifiable content digest, actor identity and import receipt. A copy cannot rename/delete/rewind Hermes history or become the source from which Hermes resumes sessions. UI provenance says `Hermes source / copied snapshot`; unresolved/deleted/stale source is visible.

Hermes edits/rewinds reconcile or invalidate the affected source binding before stale evidence can be returned. **Hermes archive and delete intentionally have the same v1 Reliquary effect:** tombstone the corresponding copied conversation/source artifact. The plugin does not physically delete Reliquary data and does not prune tombstones; pruning remains a Reliquary maintenance operation. A tombstoned source cannot be resurrected by an older queued sync/retry. Any effect on Memories already derived from that source follows Reliquary's own lifecycle/provenance rules rather than a plugin-side cascade. The source copy itself is private data governed by the originating grant. No unsupported multimodal payload is coerced to text or reported imported successfully.

Reference-only mode is a later explicit hard cut with identical stable source identity: inventory/catch up, compare bindings, switch the sole authoritative read path, retire copied-body use after verified source coverage. Do not build two permanent mirrored ingestion paths now. Historical 2.0 conversion is unrelated to these new Hermes imports.

## Target architecture and ownership

`Hermes MemoryProvider + dashboard backend -> authenticated local Reliquary host process -> public Root / Gateway / Application -> existing owner sessions`

A standalone plugin repository (target name `reliquary-hermes`; create only during implementation) contains the Python provider, local host client, dashboard assets/backend and a small Rust host executable. The executable depends on the public `reliquary-memory` package only for domain operations, like the detached CLI; it is outside the existing 23-package production DAG. It may use transport/configuration dependencies, not import lower semantic stores directly. Necessary public operations are added narrowly to Root/Gateway/Application with capability/conformance tests.

Use a long-lived authenticated loopback service for Python/UI clients so one process owns each mounted writer and its worker lifecycle. Bind loopback only; no public network daemon, cloud service or unauthenticated port. Service identity and a per-profile credential are stored outside the browser in profile-scoped private configuration. Bind owner grants server-side; caller-supplied owner IDs never grant access. Local process discovery validates protocol version, endpoint identity and a single-instance lease; never blindly kill a process or spawn a second writer after an ambiguous probe. Multiple client sessions share the service's owner registry. A separately started service trying to mount the same writer must receive the normal lease conflict.

The protocol is versioned request/response JSON with request ID, operation, authenticated transport/profile scope, session/turn identity, bounded arguments and typed result. Reads include owner cut/generation where needed. Mutations include expected revision and stable idempotency key. No arbitrary filesystem paths, raw store handles, SQL, generic code execution or token-bearing URL parameters. Missing/disconnected service returns unavailable; it does not silently create a different owner/backend.

### Settled v1 owner topology

Reliquary owns one **main/general REL** for ordinary Hermes conversations and one **project REL per project**. Every project REL is a child of the main REL. A conversation maps durably to either the main REL or its project's REL; sessions do not create new RELs. Project selection and the stored session-to-REL binding are service/configuration concerns, never inferred from model text.

The active PHY is independent of REL selection. While Reliquary is active, the integration's `user_id` is derived from the active PHY's stable identifier. Do not derive human identity from Hermes `agent_identity`, bot/profile name or `HERMES_HOME`, and do not create a new PHY when the bot profile changes. Hermes profile storage may still scope plugin configuration/credentials, but bot-profile-to-PHY mapping, bot-specific PHYs/BHYs and future Context Engine identity/personality behavior are explicitly deferred.

## Required product contract

| Capability | Entry point / target responsibility | Required semantics |
| --- | --- | --- |
| Owner binding | Existing Root create/open, trusted admission and mounts | Main/general REL; one child REL per project; durable session-to-REL mapping; active PHY stable ID supplies the Reliquary `user_id`; no bot-profile-to-PHY inference |
| Ingestion | New narrow host facade over existing normalization/native import + Insomnia scheduling | Durable idempotent accepted snapshots; session/REL/source binding; retries do not duplicate Memories or clocks; archive/delete tombstones source artifacts |
| Retrieval | Typed public facade over existing agentic query/refine/traverse and delivery preparation | Existing ranking/limits, turn-scoped searched/selected IDs and exact provenance; internal candidate inspection/dashboard reads remain non-reinforcing |
| **HACK — retrieval confirmation shim** | Isolated Rust host adapter around the current Gateway delivery gate | When Reliquary query agents return selected Memories to the requester, synthesize the exact confirmation the current rebuilt API requires so its existing retrieval/Freshness path runs exactly once; no Hermes callback or Python consumption protocol |
| Status/workers | Owner-local worker lifecycle/status and bounded immutable events | Honest pending/running/held/failed/complete state; no invented percentage; quiesce/restart obeys accepted receipts |
| Inspection and UI edits | HU contract through authenticated named product operations | Bounded cut-bound reads, user-origin changes and stale-revision rejection; no direct lower-store access |

The plugin does **not** own Freshness or downstream model-consumption accounting. Reliquary's intended boundary is internal: candidate examination is non-reinforcing; when the Reliquary query agents return the selected Memory set to the requester, Reliquary records those Memories as retrieved and its Freshness machinery runs. The Python provider forwards queries/results and sends no consumption acknowledgment.

The current rebuilt Gateway incorrectly requires an external confirmation receipt before that existing retrieval/Freshness path executes. For the hackathon/plugin milestone, do not redesign that core path. Implement a narrowly isolated Rust-host compatibility shim that fabricates the exact current receipt at the Reliquary result-return boundary. Mark it conspicuously `HACK`, preserve delivery ID/manifest digest/idempotency checks, and test exact-once behavior. Its removal condition is explicit: once core Reliquary records retrieval when its query agents return selected Memories, delete the shim and its compatibility tests. **Do not add `model_request_handoff_v1`, `consumption_confirm`, provider-success tracking or a Hermes core callback.** Existing HP01 plugin-contract fixtures that introduced those concepts are superseded by this revision and must be corrected before implementation proceeds.

## Provider lifecycle and isolation

`is_available` checks configuration/dependencies only, with no network. `initialize` binds the provided Hermes configuration scope/session plus the selected REL and active PHY binding. `HERMES_HOME`/`agent_identity` scope plugin configuration and bot runtime state; they are not the human-memory identity. The active PHY's stable identifier supplies the Reliquary integration `user_id`. Non-primary agent contexts do not duplicate primary ingestion; a delegation result is captured only through its parent-authorized path. `sync_turn` returns without blocking Hermes transcript durability, queues a bounded durable source-copy job, and signals backpressure/status if unavailable. Restart/catch-up must use a supported Hermes history/source interface, validate stable IDs/timestamps/digest and deduplicate against accepted source IDs; never acknowledge a durable checkpoint before it exists.

The v1 personal-memory owner is the explicitly active PHY. Do not infer or auto-create additional PHYs from Hermes bot profiles, `turn_author`, display names or group participants. Content that cannot safely be attributed to the active user under the existing trusted source rules remains REL-only. Bot-specific personality/identity mapping is future Context Engine work. Session switch/resume/branch/rewind invalidates stale prefetch and in-flight source bindings; archive/delete tombstones the copied source artifact; profile/configuration change clears scoped caches and credentials without changing PHY identity by implication. Cross-thread work uses Hermes context propagation helpers.

Expose `memory_query`, `refine_query` and `traverse_graph` through provider tools with bounded JSON schemas; do not add a second model-side query controller. Treat recalled text as source content, never tool instructions or authority. Recall-status indicators describe the last actual injected batch, not a stale previous result. Config stores only nonsecrets; credentials follow Hermes scoped secret storage and never appear in logs, exported UI JSON or bundles.

## Context and compaction boundary

Initial integration supplies bounded memory context through the MemoryProvider and preserves Hermes's own live transcript assembly. It does not pretend the full planned Context Engine/Ego synthesis is implemented. Any pre-compression checkpoint uses the declared Hermes checkpoint API only when all normalized evidence was durably accepted; failed checkpoint is explicit, not a false success. Do not advertise strict checkpoint support until restart/failure tests pass. Full ContextEngine replacement and reference-only compaction proposals remain follow-on H6 work, not hidden hackathon prerequisites.

## Tickets

| Ticket | Hard predecessors | Deliverable | Acceptance / evidence |
| --- | --- | --- | --- |
| HP01 — pin host and correct/freeze protocol | G4 | Hermes revision/API audit; prove `message_uid`/timestamp feed; verify archive/delete lifecycle hook; freeze REL topology, active-PHY `user_id`, protocol/DTOs; remove the superseded handoff/consumption-confirm contract | Fake-provider contract tests, exact copied-snapshot authority, configuration/thread isolation; no bot-profile-to-PHY inference and no speculative consumption hook |
| HP02 — expose narrow product operations | HP01 | Root host facade, required bounded read/edit operations, existing worker/retrieval bridge, plus the isolated **HACK** confirmation shim around the current Gateway gate | External consumer uses only Root; no mutable stores/new production edge; full G4/GM/P01 suite; shim is clearly marked, exact-manifest/idempotent and has an explicit removal condition |
| HP03 — local service and provider lifecycle | HP02 | Standalone host binary, authenticated client, setup/config/lifecycle, general/project REL selection and active-PHY identity binding | Startup/attach/wrong token/duplicate writer/outage/restart tests; main REL + project-child mapping persists; active PHY supplies stable `user_id`; no leaked credentials or source-chat blocking |
| HP04 — copied-source ingest, tombstone and recovery | HP03 | Idempotent Hermes snapshot feed, supported import/processing, restart catch-up and archive/delete tombstone feed | Real source IDs/timestamps, replay/edit/rewind, archive/delete same tombstone effect, stale queued replay cannot resurrect, crash/cursor/unavailable cases; no double publication or plugin-side prune |
| HP05 — retrieval and recall integration | HP04 | Memory tools/prefetch through real Hermes and the existing Reliquary query-agent path | Grounded result through real Hermes turn; candidate/dashboard reads do not reinforce; returned selected Memories record retrieval exactly once through the temporary shim; no external consumption acknowledgment |
| HP06 — plugin packaging and conformance | HP05 | Installable standalone plugin + pinned host artifact + setup/uninstall/upgrade docs | Clean-profile install, provider discovery, offline/service-down behavior, two sessions/two principals, uninstall leaves owned data and does not revert source authority |
| HP07 — real demo and submission evidence | HP06, HU04 | Reproducible native-data demo with UI, recorded consented real-provider run and documented limits | New conversation -> memory/graph/provenance -> later recall -> restart -> privacy denial; no claim that deterministic mocks measure model quality |

HU01 can begin alongside HP02 after the protocol is frozen. UI design uses contract fixtures; final UI acceptance requires the real host/provider, not mocks alone.

## Testing decisions

Run plugin Python unit/integration tests through pytest, strict schema tests for transport, current public Rust consumer tests, and Hermes's actual provider/plugin validation commands after HP01 pins their local interfaces. No made-up existing command names. Add explicit tests for unsupported provider revision, missing/invalid active PHY binding, stable active-PHY `user_id` across bot-profile changes, main-vs-project REL routing, restart catch-up, archive/delete tombstoning, stale queued replay, stale prefetch, unavailable source, malformed model result and token/owner tampering. Retrieval tests must distinguish non-reinforcing candidate/dashboard reads from returned selected Memories and prove the HACK shim produces exactly one current Gateway receipt without any Python/Hermes consumption protocol. The plugin's own test entry points and exact commands are frozen in HP01 before implementation is delegated.

Deterministic endpoints are mandatory for CI. One separately labeled real Hermes/model end-to-end smoke is required for the demonstration; it uses explicitly configured/authorized credentials, never reads secret files into reports, and reports observed extraction/retrieval behavior without asserting universal accuracy. Official hackathon eligibility/deadline/platform rules must be checked when packaging submission; this spec does not invent or certify those external conditions.

## Completion, out of scope and follow-on

The plugin is complete when HP01..HP06 and the real-service UI contract pass; the hackathon demonstration closes HP07 separately from G5. Completion additionally requires the Python/provider contract to contain no external consumption-confirmation or model-handoff requirement, the temporary Rust confirmation shim to be isolated and visibly labelled `HACK`, archive/delete to tombstone rather than prune, and the general/project REL plus active-PHY identity bindings to survive restart.

Excluded: replacing Hermes canonical session storage; live host replay from Reliquary copies; a public network daemon; hosted billing; plugin-driven pruning; new Observation/Ego inference; automatic Hermes bot/profile-to-PHY mapping; bot-specific PHY/BHY/personality design; full Context Engine identity policy; unverified historical corpus loading; or a second memory engine.

The earlier H1–H8 external-source plan remains the long-term reference-only integration track. Copied-input mode has a named future retirement condition; it must not quietly become a second transcript authority. Warlock is a donor for presentation code, not a runtime dependency.

## Related docs

- [Dashboard/UI implementation](hermes-reliquary-ui-implementation-spec-2026-11-06.md)
- [Earlier reference-only host plan](../conversation-index-integration-plan.md)
- [P01 public boundary](p01-public-product-api-cli-implementation-spec-2026-11-06.md)
- [G4 completed product contract](g4-complete-product-acceptance-spec-2026-11-06.md)

## Notes

Primary inspection sources are local `hermes-agent/agent/memory_provider.py`, `plugins/AGENTS.md`, dashboard extension documentation, and rebuilt `src/lib.rs`/owner-session exports. This spec defines needed additions; it does not claim that a complete Hermes adapter, graph management facade or deployable plugin already exists.
