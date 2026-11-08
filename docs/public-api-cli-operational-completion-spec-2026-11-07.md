# Public API and CLI operational completion — 2026-11-07

Companions: [source-backed gap audit](public-api-cli-gap-audit-2026-11-07.md), [interaction/CLI contract](public-api-cli-interaction-contract-2026-11-07.md), [Switchboard configuration restoration](switchboard-configuration-restoration-spec-2026-11-08.md). Entry point from the current [public API guide](public-product-api.md).

**Status: specified, not implemented or accepted.** Implementation target: `Reliquary-rebuild`, baseline `a3c3a9643f494bced51faf92ba057b16bcdac8a7`. Existing uncommitted smoke-run changes are candidates to review, not an accepted prerequisite. Ticket IDs OC01–OC09 identify this operational closure only; they do not renumber completed capabilities or change D01–D03/G5/HP/HU statuses.

## Purpose

A supported external host and the normal CLI must be able to create native files, import real conversations, run the existing memory machinery, query the resulting Memories, inspect their sources and history, and recover after interruption using only the public `reliquary-memory` package. Internal crate access or a custom provider bridge cannot substitute for that usable product boundary.

This corrects the practical gap between the earlier narrow P01/G4 acceptance and a complete runnable import/process/retrieve workflow. Preserve the accepted architecture and semantics; finish their exposure and composition. Old-corpus byte conversion, Hermes, a network service and future cognition features are not prerequisites.

## Overview

Canonical call direction:

`CLI or external host -> public Root handles -> authenticated Gateway -> named Application workflows -> owner-session / existing semantic services`

`configured inference -> existing Switchboard provider endpoints`

The CLI is a trusted local host, not a second implementation. Root validates product inputs and wraps opaque capabilities; it does not own semantic scheduling policy, mutable stores or provider HTTP code. Application owns multi-stage composition; Gateway owns admission, mounts, scoped lifecycle, delivery and recovery. Switchboard owns endpoint construction/selection. The package DAG remains unchanged at 23 packages / 76 production edges.

The following are equally required: ingestion/processing, read-only search and inspection, agentic query/refine/traverse, and the existing conversation/context/host lifecycle. An importer alone does not complete this specification.

## Observable outcomes

A user can run the two-day ChatGPT sample with the ordinary CLI, reopen both new owner files and ask grounded queries. A Rust host can perform the same operations with no dependency on internal Reliquary crates. It can implement the existing trusted-host contracts using only public types, inspect a prepared result, confirm actual consumption and reconcile an ambiguous handoff. Read-only investigation never awards Freshness or creates duplicate Memories.

The supported CLI keeps its existing maintenance/history behavior and gains operational commands through the same public methods. Interactive inspection is a command loop over those methods, not a new agent/chat product or a daemon. A selected operation with missing configuration fails explicitly; it never falls back to the custom Codex/Python bridge.

## Target public contracts

Names below define the logical product operations. OC01 fixes concrete Rust signatures using accepted value types, and records a one-to-one symbol/test map. Equivalent responsibility-local names are acceptable; missing operations, lower-crate imports or unusable private argument types are not.

| Public capability | Contract |
| --- | --- |
| Configured runtime | Open the existing `reliquary.cfg` / companion master-key format, validate product/model routes and encrypted credentials, then build an opaque runtime/service bundle using the existing Switchboard transports and fallback rules. Publish redacted readiness diagnostics. |
| Native import | Inspect/normalize a declared source, produce a bounded import plan, start or resume one idempotent source-bound import, inspect status and errors, and process it to its actual completion criteria. |
| Owner processing | Start/poll/stop a bounded owner job for accepted Insomnia, Dream, Entity, vectors and Freshness workflows. Expose stage and pending/held/failed/completed counts and stable work IDs. A process-and-drain convenience operation invokes the same coordinator. |
| Passive search | Authorized lexical/hybrid/semantic search with automatic query embedding through the selected compatible profile, bounded result values, exact revisions and provenance. No model-side selection unless requested as agentic query. |
| Stateful retrieval | Opaque owner/principal/turn-bound query session with query, refine and traverse, plus a non-reinforcing inspection mode and the existing exact delivery/receipt recovery path. Details are normative in the companion interaction contract. |
| Inspection/management | Bounded Memory list/get/provenance/history, source conversation/transcript/branch, graph/Entity/relationship/community views and already-supported user-origin changes. Immutable results; explicit cut/version and availability state. |
| Conversation/context | Existing queue submission, Episode finalization, committed streams, cancellation/reconnect, context preparation/finalization and checkpoint semantics through Root. Reuse retained host contracts; expose all transitive types needed to implement them. |
| External sources | Existing manifest/revision/index/search/hydration and source-authorized publication operations required by retained consumers. Scope remains explicit; no Hermes-specific data provider is introduced. |
| Lifecycle/maintenance | Read mounted state through the mounted session, quiesce workers, independently verify and reopen; retain existing verified-copy maintenance APIs. |

Do not simply re-export the internal `Instance`, `RelWriter`, `PhyWriter`, stores or `Container`. Public result types may be immutable existing types or small product wrappers. An exported trait is incomplete if a downstream implementor cannot name its associated argument/return types from Root alone. Add compile tests for that entire type closure.

## Implementation decisions

### Configuration and providers

[Switchboard configuration restoration](switchboard-configuration-restoration-spec-2026-11-08.md) is normative for OC02. The persisted compatibility boundary is the existing binary `reliquary.cfg` plus `reliquary.master-key.json`; the unaccepted JSON/environment bootstrap is superseded and must not be integrated.

Use the existing Switchboard-owned config document framing, model-route codecs, encrypted credential objects, master-key handling, device login/refresh, provider transports and fallback rules. Root restores the small product `ReliquaryConfig` wrapper for product-domain settings and constructs `ConfiguredRuntime` from that validated configuration. The CLI accepts `--config` (default `reliquary.cfg`) and never requires secret material on argv.

Opening/showing/verifying/runtime construction must be byte-neutral. Explicit credential/model changes use the existing atomic replace path and preserve unknown/unrelated objects. The rebuilt CLI restores credential list/add/login/refresh/remove and route set/clear operations through Root. The current 401 is a stale stored credential to refresh, not a reason to change formats.

Dedicated/shared/general decision selection preserves accepted order. Memory Retrieval does not require a dedicated object when shared/general fallback is available. A selected provider failing is not permission to silently choose a different model. Preserve retry/backpressure/cancellation distinctions, strict structured output and response-size limits. Freeze route/model/profile identity in each processing run without logging secrets.

### Source import and processing composition

The ChatGPT adapter decodes only source data into the accepted normalized import model: stable source conversation/message IDs, visible roles/text, branch ancestry/canonical branch, original timestamps and explicit source revision/digest. It does not extract Memories, author Graph edges, set Freshness or manufacture receipts. Keep provider-specific parsing below the product API in a focused Application/source-normalization unit; CLI only supplies the file and selection options.

Support raw `conversations*.json` exports and the known normalized visible-turn input through explicit format selection. ZIP/path traversal, duplicate IDs, unsupported content, malformed parentage, timestamp omissions and source changes must produce bounded diagnostics before publication. Stream bounded selections; do not silently load an entire export's media into memory. The two-day window is half-open and recorded exactly. Shared branch prefixes are stored/processed once under the accepted Episode model. Preserve original parent references when visible-turn filtering changes the retained ancestry, with an explicit mapping.

A text-only import can record unsupported asset references but must label that omission before execution and in its result; it cannot claim all export content was imported. Source archives and existing corpora stay unchanged. Never reclassify imported turns as live turns merely to bypass an import fence.

Expose import-scoped prepared Insomnia publication through the normal Application/Gateway path. Claims, lease validation, source binding, REL/PHY routing, partial publication and retries use owner-authenticated evidence. Same-source replay is a no-op; a changed source under the same identity is a conflict. Validate before cross-owner publication; partial failures resume idempotently rather than duplicating a user Memory.

One Application coordinator owns stage dependencies. It must drive imported extraction, required first-cycle Dream evidence, Entity/relationship processing, configured vector/profile coverage and historical Freshness settlement through their supported scoped operations. Any missing import-scoped variant is a concrete integration gap at Application/owner-session, not a reason to clear the fence or call raw store writes. `ready` means the declared stages are complete and independently reopenable; `idle`, zero queued jobs or successful process exit alone do not establish readiness.

Each stage reports executed, not configured, unsupported, held, failed or complete separately. Optional stages are declared in the run plan before execution; omitting a required stage fails acceptance. Two-day corpus acceptance requires the existing memory pipeline and retrieval, not just Archive import. New Observation/Ego synthesis and the future full Context Engine remain excluded; existing stored Identity/Personality/Anchor and context behavior are not to be silently lost.

Cancellation stops admitting new work, settles or durably holds active claims, closes owned handles and returns an incomplete resumable run. Unknown external outcomes remain Held. Do not kill unrelated processes or manufacture completed receipts. Status/reopen reads must work while a writer is mounted without trying to open a competing owner handle.

### Interaction and inspection

The [interaction contract](public-api-cli-interaction-contract-2026-11-07.md) owns search modes, turn state, query/refinement/traversal behavior, CLI session lifetime, exact source inspection and consumption semantics. It is a mandatory part of this spec, not optional follow-up.

Passive ranking, semantic community routing and agentic selection retain their distinct accepted behavior. Expose them honestly rather than relabeling one implementation as all three. Read-only graph inspection and exact source lookup must also be available without invoking a model. A CLI query returns Memories and evidence, not an invented synthesized answer.

### Overlapping work and hard cut

Review the uncommitted two-day runner and generic HP02 hunks for reusable implementation; do not merge the Hermes worktree wholesale. Move legitimate orchestration once into Application/Gateway and expose it through Root. Keep source normalization/tests where appropriate. The live operational route must then use only the supported CLI/Root path.

Remove the custom inference bridge and direct-internal-crate runner from the operational path after replacement. Preserve their diagnostic artifacts and failed-run evidence privately. A retained diagnostic utility must be clearly noncanonical and cannot be cited as final public-API evidence. No permanent compatibility alias, alternate memory engine or second independently evolving scheduler is introduced.

## CLI scope

`cli/` remains a separate package depending on Root only among Reliquary packages. It may use ordinary argument/configuration/I/O libraries. It must not import Application, Gateway, stores or Switchboard directly, implement candidate ranking, create semantic records, select owners from model text or convert ordinary stdout into accepted-use receipts.

Implement the command matrix and interactive command loop in the companion contract. Existing maintenance grammar stays supported. The earlier P01 “host-owned, therefore absent from CLI” disposition for import/vectors/Insomnia is explicitly superseded. The CLI now hosts these workflows locally through the product API. No public network listener or Hermes dependency is required.

## Execution tickets

All tickets are **specified / not accepted**. Earlier code/tests may be reused but do not automatically satisfy these new external-consumer gates. Dependencies below are hard acceptance predecessors; synthetic fixtures and CLI/parser work can proceed before final live-data execution.

| ID | Hard predecessors | Deliverable / owner | Acceptance evidence |
| --- | --- | --- | --- |
| OC01 | None | Freeze this inventory into concrete Root methods/types/grants/errors and external compile fixtures; review transitive host trait types and overlap. Root plus targeted Gateway wrappers. | Consumer depends only on Root; required signatures/traits nameable; missing methods remain explicit failing cases, not removed requirements; no mutable store export or DAG change. |
| OC02 | OC01 | Restore the existing `reliquary.cfg` + master-key configuration/security boundary and CLI config management, then build the public configured runtime from it. See the [OC02 correction spec](switchboard-configuration-restoration-spec-2026-11-08.md). | Existing config opens without conversion or mutation; encrypted credentials/device refresh work; unknown objects survive writes; route fallbacks match prior behavior; JSON/env-only bootstrap is absent; bounded provider probes pass after credential refresh. |
| OC03 | OC01 | Public normalization/import plan/start/status/resume and import-scoped publication. | External test imports a branching REL/PHY cohort; exact source/timestamp/ancestry checks, invalid-source no-publication, replay no-op, conflict and interruption tests. |
| OC04 | OC02, OC03 | Complete import/live processing lifecycle, scoped stage coordination, readiness and recovery. | Root-only create/import/process/reopen; real Memory/PHY routing, first-cycle Dream, configured vector coverage, settlement; held/error/resume and no-duplicate publication tests. |
| OC05 | OC01, OC02 | Passive search and complete query/refine/traverse/inspection/delivery/recovery surface. | Interaction I01–I10 cases through Root; exact ranks/bindings/state, no read credit, actual confirmed receipt once, changed/revoked/unknown outcomes fail closed. |
| OC06 | OC01 | Bounded source/Memory/graph/semantic inspection and existing user-origin management; paired owner setup. | View/Operate/Manage separation, exact provenance, stale revisions, pagination/cut expiry, private PHY denial and mounted read behavior. |
| OC07 | OC01, OC02 | Close retained conversation/context/external-source public call chains and transitive types. | Root-only host fixture executes accepted queue/stream/recovery/context/source cases; no lower-crate type escape, simulated acknowledgement or new agent runtime. |
| OC08 | OC02, OC03, OC04, OC05, OC06, OC07 | Operational CLI plus interactive inspection loop using only Root; docs/examples. | Separate-process command/API parity, JSON output/errors, scripting and stdin session state, cancellation, no-replace, privacy and no confirmation on print. |
| OC09 | OC04, OC05, OC06, OC07, OC08 | Integrate, retire obsolete operational paths, run full regressions and the real two-day corpus through the normal CLI. | All required public and CLI cases plus native configured inference, source-grounded retrieval and fresh-process reopen pass; evidence/limits/commit recorded; no whole-product claim from mocks alone. |

Parallel frontier after OC01: OC02 configuration, OC03 import, OC06 inspection. After OC02: OC05 retrieval and OC07 retained host interactions. OC04 follows import/configuration. CLI implementation can begin against frozen public contracts before predecessor acceptance, but OC08 cannot close before them. One integrator owns shared Root/Gateway files; do not run competing mutations there. No dependent ticket waits for D01, the old six-file conversion or Hermes.

## Testing decisions and final gates

Add a standalone external operational consumer depending only on Root, or extend the existing one with an explicitly invoked operational test target. Its scenario must create the files and drive the workflow itself; consuming prepopulated internal G4 fixtures alone is insufficient. Private helper tests remain useful but are not evidence that public wiring works.

Deterministic provider fixtures test orchestration and negative cases. The real two-day smoke additionally uses the existing configured provider endpoints, frozen source selection and recorded model/profile identity. Diagnose prior provenance failures under that path before altering semantics. Zero Memories across known durable source statements, all-rejected extraction, or a never-exercised retrieval path cannot count as success; do not manufacture Memories or loosen validators to satisfy the gate.

Preserve the existing eight-conversation / 778-visible-message selection and its hash `d6c261b544a1c62d20ede3a77fb21bcb0a9fcfbc682c041ee2bf5eb8c65e8f37`. Use a new create-only run directory or a supported exact resume; keep previous failed runs. Predeclare representative retrieval questions and provenance expectations from raw user statements, not from whichever Memories a failing run happens to produce. Include at least one meaningful refinement and active-edge traversal case when the actual corpus provides an eligible relation; deterministic cases still prove traversal negatives.

The final report must separate Archive acceptance, extracted/accepted/rejected Memory counts, first-cycle processing, REL/PHY routing, vector coverage, retrieval behavior, source correctness, confirmed-use tests, restart/recovery, text-only asset omissions and live-model quality observations. Reopen in another process using the ordinary CLI. Existing six old corpora and exports remain untouched.

Required integration commands include architecture guard, workspace formatting/check/tests, separately built CLI tests, explicitly run external operational consumer tests, documentation validation and whitespace checks. Use locked/offline Cargo where applicable. At final acceptance refresh a valid architecture snapshot when available; never substitute it for behavior tests.

Before any build, inspect actual Cargo/rustc/link process command lines and target directories. The MCP job list does not enumerate every other agent. Reserve one target directory per compatible build profile, serialize writers to it, and coordinate with migration work. Never kill the migration agent, delete its cache or repeatedly launch overlapping compilers. Record debug/profile choices so avoidable recompilation is not mistaken for a semantic blocker.

## Out of scope

Old-format data conversion or activation; merging Hermes or UI work; adding an HTTP daemon, Python service protocol or bindings; a new retrieval algorithm; arbitrary SQL/raw Graph queries; new Observation/Ego inference; a general chatbot; unapproved cross-owner retrieval expansion; weakened provenance/authentication; performance thresholds unrelated to operational correctness.

## Related docs

- [Interaction/CLI contract and acceptance cases](public-api-cli-interaction-contract-2026-11-07.md)
- [Switchboard configuration restoration](switchboard-configuration-restoration-spec-2026-11-08.md)
- [Gap audit](public-api-cli-gap-audit-2026-11-07.md)
- [P01 historical consumer map](p01-consumer-contract-map.md)
- [Existing public API](public-product-api.md)
- [Retrieval acceptance](retrieval-confirmed-use-acceptance-2026-11-05.md)
- [Two-day diagnostic](native-two-day-live-smoke-2026-11-07.md)

## Notes

This specification is a targeted operational-completion plan, not a restart of the architecture rebuild. The earlier gates remain scoped historical records; operational replacement is not complete until OC09 passes. Specification review/validation does not accept the existing uncommitted runner, alter canonical tracker statuses, start inference, or authorize a merge/cutover.

Specification validation checked all local links, OC01–OC09 dependency completeness/acyclicity, I01–I10 acceptance-case coverage and whitespace. Source-file hashes were unchanged from the pre-task snapshot. No Rust suites, live provider calls or corpus mutations were performed for this documentation task. Local report: `.test-tmp/public-api-cli-spec-2026-11-07/validation.json`. The shared documentation policy cannot run in this checkout because `docs-standard.json` is absent (same failure before and after); Archivist also reports broader orphan-document/index issues. These are recorded limitations, not a claimed clean repository-wide documentation pass.
