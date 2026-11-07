# Hermes / Reliquary plugin — native 2.1 implementation contract

Parent index: [Architecture](INDEX.md). Product track in the [post-G4 plan](post-g4-parallel-execution-plan-2026-11-06.md).

**Status: implementation specification; no plugin implementation or hackathon acceptance claimed.** The initial target is a functioning Hermes memory plugin and dashboard demonstration using new 2.1 owners. It does not depend on D01–D03 or historical-corpus release. Existing data must not be opened with a guessed compatibility reader.

## Purpose

Expose Reliquary's accepted memory machinery in real Hermes conversations and make its behavior inspectable through the companion UI. Reuse Reliquary's owner/runtime policies and Hermes's plugin surfaces rather than embedding a second memory engine or replacing Hermes's canonical transcript authority.

## Overview

### Source-backed starting points

The inspected local Hermes checkout provides `agent/memory_provider.py::MemoryProvider`, orchestrated by `agent/memory_manager.py`, plus standalone plugin discovery. Core methods include `initialize`, `prefetch`, `queue_prefetch`, `sync_turn`, tool schemas/dispatch and `shutdown`; optional hooks cover turn/session change and pre-compression. `initialize` receives profile-scoped `hermes_home`; turn authors may vary inside a shared session. Provider worker threads must propagate the caller's contextvars through the supplied helpers.

`hermes-agent/plugins/AGENTS.md` requires new memory providers to be standalone plugins, not new in-tree backends or Reliquary-specific changes in `run_agent.py`/CLI/Gateway core. Setup uses `hermes memory setup` and the supported provider configuration hook. Pin the actual Hermes source revision during HP01; inspected interfaces are not an unbounded promise about future upstream versions.

The rebuilt Root currently exports opaque owners, trusted host admission, workspace management, maintenance and selected queue/host types. Its `src/lib.rs` does NOT yet expose every worker/retrieval/graph method a complete plugin UI needs. HP02 must close these concrete product-facing gaps through named existing workflows; G4 acceptance is not evidence those external APIs already exist.

## Transcript ownership decision

**Initial copied-input mode follows the user's November 6 direction:** Hermes remains the canonical durable history owner; Reliquary initially receives source-ID-preserving copies for memory processing. Later reference-only hydration may replace copies. This is an explicit scoped variation from the earlier [external-source integration plan](../conversation-index-integration-plan.md), which prohibited duplicate bodies. It is not permission for two competing canonical transcripts.

Copies enter supported normalized/native import workflows as explicitly imported Hermes snapshots, never as fabricated new live Gateway turns. Retain Hermes profile/source namespace, stable session/message/branch IDs, original role/time, source revision or verifiable content digest, actor identity and import receipt. A copy cannot rename/delete/rewind Hermes history or become the source from which Hermes resumes sessions. UI provenance says `Hermes source / copied snapshot`; unresolved/deleted/stale source is visible.

When Hermes edits/deletes/rewinds, invalidate affected source bindings before they can be returned as current evidence, then apply the accepted import/reconciliation behavior. If the target cannot yet represent that change, mark the affected imported view unavailable and stop retrieval from it until resynchronization; never keep silently serving stale private content. The source copy itself is private data governed by the originating profile/grant. No unsupported multimodal payload is coerced to text or reported imported successfully.

Reference-only mode is a later explicit hard cut with identical stable source identity: inventory/catch up, compare bindings, switch the sole authoritative read path, retire copied-body use after verified source coverage. Do not build two permanent mirrored ingestion paths now. Historical 2.0 conversion is unrelated to these new Hermes imports.

## Target architecture and ownership

`Hermes MemoryProvider + dashboard backend -> authenticated local Reliquary host process -> public Root / Gateway / Application -> existing owner sessions`

A standalone plugin repository (target name `reliquary-hermes`; create only during implementation) contains the Python provider, local host client, dashboard assets/backend and a small Rust host executable. The executable depends on the public `reliquary-memory` package only for domain operations, like the detached CLI; it is outside the existing 23-package production DAG. It may use transport/configuration dependencies, not import lower semantic stores directly. Necessary public operations are added narrowly to Root/Gateway/Application with capability/conformance tests.

Use a long-lived authenticated loopback service for Python/UI clients so one process owns each mounted writer and its worker lifecycle. Bind loopback only; no public network daemon, cloud service or unauthenticated port. Service identity and a per-profile credential are stored outside the browser in profile-scoped private configuration. Bind owner grants server-side; caller-supplied owner IDs never grant access. Local process discovery validates protocol version, endpoint identity and a single-instance lease; never blindly kill a process or spawn a second writer after an ambiguous probe. Multiple client sessions share the service's owner registry. A separately started service trying to mount the same writer must receive the normal lease conflict.

The protocol is versioned request/response JSON with request ID, operation, authenticated profile/principal binding, session/turn identity, bounded arguments and typed result. Reads include owner cut/generation where needed. Mutations include expected revision and stable idempotency key. No arbitrary filesystem paths, raw store handles, SQL, generic code execution or token-bearing URL parameters. Missing/disconnected service returns unavailable; it does not silently create a different owner/backend.

## Required product contract

| Capability | Entry point / target responsibility | Required semantics |
| --- | --- | --- |
| Owner binding | Existing Root create/open, trusted admission and mounts | Explicit project REL plus authenticated user PHY; durable mapping, same-owner writer uniqueness; no path guessing from model text |
| Ingestion | New narrow host facade over existing normalization/native import + Insomnia scheduling | Durable idempotent accepted snapshots; session/profile/branch binding; retries do not duplicate Memories or clocks |
| Retrieval | Typed public facade over existing agentic query/refine/traverse and delivery preparation | Existing ranking/limits, turn-scoped searched/selected IDs, exact provenance and no read reinforcement |
| Consumption receipt | Existing Gateway delivery semantics exposed narrowly to host | Confirm only the exact prepared Memory manifest actually handed to and accepted by the external model/host; repeat once |
| Status/workers | Owner-local worker lifecycle/status and bounded immutable events | Honest pending/running/held/failed/complete state; no invented percentage; quiesce/restart obeys accepted receipts |
| Inspection and UI edits | HU contract through authenticated named product operations | Bounded cut-bound reads, user-origin changes and stale-revision rejection; no direct lower-store access |

Returning a string from `prefetch`, displaying a Memory in the dashboard, or queuing a model request is NOT confirmed consumption. HP01 must identify the actual successful model-request handoff callback carrying the exact injected content. If Hermes lacks it, add the smallest generic hook with this concrete consumer, independently tested in Hermes; until available, retrieval remains non-reinforcing and the plugin cannot claim confirmed-use completion. Failed, cancelled, replaced or omitted context gets zero credit. Do not use post-response text guessing as proof.

## Provider lifecycle and isolation

`is_available` checks configuration/dependencies only, with no network. `initialize` binds the provided profile home/session and authenticated grants. Non-primary agent contexts do not duplicate primary ingestion; a delegation result is captured only through its parent-authorized path. `sync_turn` returns without blocking Hermes transcript durability, queues a bounded durable source-reference job, and signals backpressure/status if unavailable. A restarted worker rereads accepted Hermes source and validates IDs/hash before publishing. Lost jobs can be reconstructed from the host snapshot/cursor; never acknowledge a durable checkpoint before it exists.

Author identity is resolved per turn, not from the first speaker in a group session. Unknown/mixed actors cannot be attributed to the operator's PHY. Shared/bot sessions default to REL-only extraction unless an authenticated per-author PHY mapping exists; do not introduce BHY/BOT formats or finalize the previously unresolved bot-personality design here. Session switch/resume/branch/rewind invalidates stale prefetch and in-flight source bindings; profile change clears caches and credentials. Cross-thread work uses Hermes context propagation helpers.

Expose `memory_query`, `refine_query` and `traverse_graph` through provider tools with bounded JSON schemas; do not add a second model-side query controller. Treat recalled text as source content, never tool instructions or authority. Recall-status indicators describe the last actual injected batch, not a stale previous result. Config stores only nonsecrets; credentials follow Hermes scoped secret storage and never appear in logs, exported UI JSON or bundles.

## Context and compaction boundary

Initial integration supplies bounded memory context through the MemoryProvider and preserves Hermes's own live transcript assembly. It does not pretend the full planned Context Engine/Ego synthesis is implemented. Any pre-compression checkpoint uses the declared Hermes checkpoint API only when all normalized evidence was durably accepted; failed checkpoint is explicit, not a false success. Do not advertise strict checkpoint support until restart/failure tests pass. Full ContextEngine replacement and reference-only compaction proposals remain follow-on H6 work, not hidden hackathon prerequisites.

## Tickets

| Ticket | Hard predecessors | Deliverable | Acceptance / evidence |
| --- | --- | --- | --- |
| HP01 — pin host and freeze protocol | G4 | Hermes revision/API audit, real message-ID/revision source, actor/owner mapping, protocol/DTOs and consumption hook contract | Fake-provider contract tests, exact copied-snapshot authority, profile/thread isolation; no speculative hook or fallback |
| HP02 — expose narrow product operations | HP01 | Root host facade, required bounded read/edit operations, existing worker/retrieval bridge | External consumer uses only Root; no mutable stores/new production edge; full G4/GM/P01 suite; missing hook exposed generically only with tests |
| HP03 — local service and provider lifecycle | HP02 | Standalone host binary, authenticated client, setup/config/lifecycle and one-writer admission | Startup/attach/wrong token/duplicate writer/outage/restart tests; no leaked credentials, no source-chat blocking |
| HP04 — copied-source ingest and recovery | HP03 | Idempotent Hermes snapshot feed, supported import/processing, private routing and catch-up | Real source IDs, replays/edits/deletes/rewind, actor changes, crash/cursor and unavailable cases; no stale retrieval or double publication |
| HP05 — retrieval and confirmed consumption | HP04 | Memory tools/prefetch and exact successful-handoff confirmation | Grounded result through real Hermes turn; zero credit on failed/omitted/context-only read; one receipt on successful use, retry stable |
| HP06 — plugin packaging and conformance | HP05 | Installable standalone plugin + pinned host artifact + setup/uninstall/upgrade docs | Clean-profile install, provider discovery, offline/service-down behavior, two sessions/two principals, uninstall leaves owned data and does not revert source authority |
| HP07 — real demo and submission evidence | HP06, HU04 | Reproducible native-data demo with UI, recorded consented real-provider run and documented limits | New conversation -> memory/graph/provenance -> later recall -> restart -> privacy denial; no claim that deterministic mocks measure model quality |

HU01 can begin alongside HP02 after the protocol is frozen. UI design uses contract fixtures; final UI acceptance requires the real host/provider, not mocks alone.

## Testing decisions

Run plugin Python unit/integration tests through pytest, strict schema tests for transport, current public Rust consumer tests, and Hermes's actual provider/plugin validation commands after HP01 pins their local interfaces. No made-up existing command names. Add explicit tests for unsupported provider revision, blank/missing user identity, group participant change, cancelled tool call, stale prefetch, unavailable source, malformed model result and token/owner tampering. The plugin's own test entry points and exact commands are frozen in HP01 before implementation is delegated.

Deterministic endpoints are mandatory for CI. One separately labeled real Hermes/model end-to-end smoke is required for the demonstration; it uses explicitly configured/authorized credentials, never reads secret files into reports, and reports observed extraction/retrieval behavior without asserting universal accuracy. Official hackathon eligibility/deadline/platform rules must be checked when packaging submission; this spec does not invent or certify those external conditions.

## Completion, out of scope and follow-on

The plugin is complete when HP01..HP06 and the real-service UI contract pass; the hackathon demonstration closes HP07 separately from G5. Excluded: replacing Hermes canonical session storage, live host replay from Reliquary copies, a public network daemon, hosted billing, new Observation/Ego inference, autonomous bot identity redesign, unverified historical corpus loading or a second memory engine.

The earlier H1–H8 external-source plan remains the long-term reference-only integration track. Copied-input mode has a named future retirement condition; it must not quietly become a second transcript authority. Warlock is a donor for presentation code, not a runtime dependency.

## Related docs

- [Dashboard/UI implementation](hermes-reliquary-ui-implementation-spec-2026-11-06.md)
- [Earlier reference-only host plan](../conversation-index-integration-plan.md)
- [P01 public boundary](p01-public-product-api-cli-implementation-spec-2026-11-06.md)
- [G4 completed product contract](g4-complete-product-acceptance-spec-2026-11-06.md)

## Notes

Primary inspection sources are local `hermes-agent/agent/memory_provider.py`, `plugins/AGENTS.md`, dashboard extension documentation, and rebuilt `src/lib.rs`/owner-session exports. This spec defines needed additions; it does not claim that a complete Hermes adapter, graph management facade or deployable plugin already exists.
