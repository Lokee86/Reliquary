# Entity Resolution service crate — extraction specification

**Status:** Specified, not implemented (2026-10-28).
**Repository / branch:** `C:\!bin\workspace\Reliquary-crate-first` · `refactor/crate-first-restart`.
**Baseline:** Retrieval service cut `8fbe6f75`, after Vectors `3bacc56d`. Both are integrated. The unrelated, dirty `docs/graph-crate-extraction-evidence-2026-10-26.md` must remain unstaged and unmodified.
**Canonical target:** **`crates/reliquary-entity-resolution/`**, package **`reliquary-entity-resolution`**.
**Superseded placeholder:** `crates/reliquary-perception/`, package `reliquary-perception`. There is no meaningful implementation in that placeholder. **Rename/replace the placeholder directly; do not keep both crates.**
**Scope:** A correction-only, behavior-preserving hard cut of the **already implemented Entity identity-resolution service**. This is not the planned Observation half of Perception, and it is not another durable store extraction.

## Purpose

The active Entity-resolution implementation currently crosses around eighty root-local `src/perception/entity_*.rs` files and a small number of Root/Gateway inference files. Candidate ranking, durable-identity admission, decision-model prompts, generative fallback, identity-guard policy, resolution reasoning, and reconciliation classification share a compilation unit with `Cva`, `Phylactery`, Container, Entity/Relationship storage, Graph writes, and runtime scheduling. Changing one decision rule consequently touches unrelated owners, increases rebuild and review scope, and allows lower policy to reach upward into product authority.

Consolidate the **Entity decision service** under a real compiled library with narrow immutable prepared/evidence values. Preserve the existing REL/PHY behavior and existing product-facing entrypoints. Do not solve this by renaming a root directory, exporting the whole `Cva` API, or wrapping the original engine in a forwarding crate.

The resulting developer boundary is: identity candidate and inference logic can change in the service without changing Container, Perception Store, Graph, Memory Core, Retrieval, Insomnia or Gateway. Root remains responsible for preparing authorized evidence, choosing configured endpoints, committing accepted effects, and orchestrating the lifecycle.

## Naming decision and architecture update

**`Entity Resolution` is the name of this first-class service crate.** The name `Perception` remains available for the broader semantic subsystem and the **future** Observation/receptor/synthesis architecture described in [Perception future work](perception-subsystem-plan.md). Do not use `reliquary-perception` as an alias, re-export crate, empty compatibility package or second implementation.

During the implementation's first stage:

1. Rename the reserved `crates/reliquary-perception/` package/directory to `crates/reliquary-entity-resolution/`. Update root workspace membership, `Cargo.lock`, relevant dependency paths, `docs/crate-first-layout-spec-2026-10-25.md`, `docs/crate-first-architecture.md`, `docs/architecture/crate-map.json` primary crate, component ownership mapping and dependency allowlist, and any other actual manifests/owner maps. Avoid formatting unrelated manifests or rebuilding obsolete graph evidence.
2. The accepted logical `perception` component in `component-dag.json` currently encompasses the implemented Entity service; update its responsibility/physical mapping to the actual `entity-resolution` target without adding an Observation runtime service or changing DAG edges. If renaming a logical component ID is required for consistency, update all referring IDs atomically; do not leave a dangling `perception` target. A future Observation service gets a separate explicit decision, not an accidental claim on today's Entity engine.
3. Keep **`reliquary-perception-store`** unchanged as the already-extracted durable owner of Entity, mention-resolution and transitional Relationship data. Renaming the service does **not** rename its records, `CVAERS03` codes, codecs, journal namespaces, owner files or their public store types.
4. Update current documentation and code maps to distinguish current **Entity Resolution** service from the future **Perception/Observations** roadmap. Historical ADR references and the storage crate's established name remain intact; do not rewrite history for branding.

The existing crate-map's `reliquary-perception` maximum physical dependencies are Archive, Chronos, Graph, Memory Core, Perception Store, Retrieval, Switchboard and Vectors. Transfer **only those maximum permissions** to the new service entry, installing only dependencies used by actual code. Most pure policy needs Memory Core + Perception Store + Retrieval + Switchboard; Graph/Archive/Chronos/Vectors are optional only where a real pure service implementation requires them. **No** Root, Container, Owner Lifecycle, Runtime/Gateway, Dream, Insomnia, Ego or History edge. Lower stores must never import Entity Resolution.

## Contracts and invariants

| Authority | Preserve exactly |
| --- | --- |
| [ADR 0033](decisions/0033-perception-entities-observations-and-ambiguity.md), [current Perception](subsystems/perception.md) | Entity means a continuing referent, not every occurrence, Memory or proposition. Ambiguity is representable; no inference may force an unproved identity merge. Observation generation remains future work. |
| [Entity resolution state](entity-resolution-state.md), [compact resolution contract](entity-resolution-compact-contract-2026-10-21.md) | Mention identity is exact Memory ID + field + byte range, bound to its original Memory body. Resolved/Rejected/Pending/Dormant state and `CVAERS03` record/repair semantics stay unchanged. |
| [Entity / Perception Store extraction](perception-store-crate-extraction-spec-2026-10-26.md), [evidence](perception-store-crate-extraction-evidence-2026-10-26.md) | Store owns durable Entity/Resolution/Relationship identities, revisions, replay, terminality, history, compact codecs and indexes. Service computes proposals, never writes raw records. |
| [ADR 0034](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md), [ADR 0036](decisions/0036-typed-semantic-graph-endpoints.md) | Graph relations and owner-qualified references do not transfer private payload access; REL/PHY authorization precedes evidence hydration and association. |
| [Switchboard](subsystems/model-runtime.md) | Configured `entity_resolution_decision` → `general_decision` → adapted `general` tier selection is at the **host configuration boundary**, not inside the service. Zero-candidate generative admission and nonempty-candidate decision-first fallback remain distinct and bounded. |
| [Crate-first architecture](crate-first-architecture.md), [layout](crate-first-layout-spec-2026-10-25.md) | One canonical library and one call direction; deterministic owner checks remain above the service. No duplicate policy, generic interface scaffolding or dual migration lane. |
| [Current limitations](current-limitations.md) | The separate promoted `CVAFRS07` six-file corpus-recovery gate and agentic retrieval recall limitation remain unchanged. Do not claim either resolved by moving Entity code. |

### Frozen resolution behavior

- Insomnia authors exact mention spans; Entity Resolution consumes them, never rewrites spans or creates provisional Entities for unresolved mentions.
- Candidate lanes: exact/normalized name or alias, existing Memory–Entity association, lexical Memory evidence and bounded Graph neighbors. Normalized surface agreement **proposes comparison only**; it is not merge authority. Preserve query context, lane caps, support counts and stable final tie order.
- Admission distinguishes immediate durable concepts from recurrence-gated implementation details; obvious transient/historical occurrences are rejected. Single mention of a fine-grained identifier must not automatically create an Entity.
- With existing candidates, bounded decision route may resolve an existing candidate only at the current calibrated confidence **at least 0.95** and the **0.15** margin over the second choice, plus existing surface/kind/identity guards. Nonterminal/malformed/low-confidence outcomes and endpoint failures route through the existing generative fallback; do not reinterpret a text model's probabilities as calibrated evidence.
- With no candidates, the existing generative admission route may return create/unresolved/reject along with canonical metadata. Preserve its V4 schema and kind/alias/evidence validation and the existing resolver schema/version.
- Attempts are idempotent on unchanged candidate/context fingerprints; terminal resolved/rejected mentions do not get retried. Pending becomes Dormant after 30 days of unchanged evidence; dormant tombstones after 180 days. Relevant evidence/events—not ordinary reads, elapsed time alone or detached Graph operations—may wake reconsideration.
- Source and owner versions (Memory, Entity, Graph, resolution revision, owner identity) must still be rechecked **after inference and before commit**. Failed/stale result commits no Entity, alias, association, or status.
- Existing REL/PHY processing, principal special case and privacy, learned aliases, merge terminality, reconciliation guard and cancellation/backpressure/failure behavior remain unchanged. Live runtime invokes the post-Dream pass under its current bounded scheduling and queued retry rules.

## Target ownership — explicit source crosswalk

| Existing source family | Final responsibility |
| --- | --- |
| `entity_candidate_{model,rank,routing}.rs`, deterministic parts of `entity_candidates.rs` | **Service** owns candidate config, scores/evidence, ordering, surface/context query assembly, normalized/surface matching and constrained candidate selection. **Root** fetches exact entities, authorized Memory bodies, lexical hits and Graph neighbors via current owner APIs, then supplies immutable facts. Replace root-only `CandidateOwner` trait with direct prepared neighbors; do not make the service call `Cva` or `Phylactery`. Product-composed `EntityCandidateError` remains Root unless a pure error shape is needed. |
| `entity_identity_guard.rs`, `entity_admission.rs`, `entity_resolver_{model,schema}.rs`, pure portions of `entity_resolver.rs` | **Service** owns canonical identity guards, transient/recurrence policy, generative Admission input/output schema, response parsing, validity/abstention and materialization proposal. Store owns actual Entity identity/ID validation and persistence. Replace the store-inspecting `creation_conflicts` helper with caller-supplied current Entity snapshots; no store mutation in a policy helper. |
| `entity_resolution_{decision,engine,evaluate,prepared}.rs` and directly used pure helpers | **Service** owns model-evaluation contract, DecisionEndpoint fast-path criteria, confidence/margin guard, rule ordering, generative fallback and typed immutable `PreparedResolution`/`ResolutionEvaluation`. Move actual engine/Resolver types and tested behavior once. Canonical `GeneralEndpoint` and `DecisionEndpoint` traits come from Switchboard; configure/route them outside. Keep the current public `EntityResolutionEngine` and `EntityResolver` API directly exported by the new service and re-exported through Root, not mirrored by a product-local trait. |
| `entity_resolution_{prepare,evidence,processor,processor_support}.rs` | **Split precisely:** Root owns `impl Cva/Phylactery`, owner ID/version, mention validity, candidate/Memory fetching, admissible evidence hydration, source fingerprints supplied from authorized facts, retry-state queries and ready/complete staging. Service owns deterministic fingerprint computation, candidate policy, evaluation and prepared-result classification. Do not move `impl Cva` into the service or install broad callback interfaces. |
| `entity_resolution_{commit,owner,boundary}.rs`, `entity_owner.rs`, `entity_principal.rs` | **Remain Root** for active-source proof, revalidation, `publish_entity`/alias updates, `set_entity_association`, resolution-state records, terminal Entity checks, principal identity/profile synchronization, transaction ordering, PHY restrictions and actual Container writes. Move only genuinely independent rule/guard calculation to service. |
| `entity_resolution_batch.rs` and batch helpers | Root owns waves, concurrency/worker cap, snapshot refresh, retry order, timestamp/commit sequencing and status; extracted engine evaluates prepared jobs (potentially off-lock) with no service-owned thread pool or commit loop. Preserve `EntityResolutionBatchOutcome` public semantics and stale speculative re-evaluation. |
| `entity_reconciliation_{candidates,inference,schema,model,support}.rs`, deterministic audit rules | **Service** owns candidate-pair ranking, identity-vs-related-distinct-vs-uncertain classification, model prompt/schema parsing and pure collisions/audit computations over supplied values. Root enumerates current Entity/Memories and retains authorization. |
| `entity_reconciliation_{prepare,owner,commit,staged}.rs`, `entity_merge.rs`, `entity_reconciliation.rs`, `entity_audit.rs` owner entrypoints | **Remain Root** for owner snapshots, relation/Entity state and versions, merge/tombstone/retarget, active-resolution canonicalization, Relationship/Graph mutation, audit data access and durable transaction order. Service can return **proposed** associations/conflicts; only Root executes them. |
| `src/runtime_inference/runtime_host_{rel,phy}_perception.rs`, `runtime_host_gateway_observation.rs` | Runtime/Gateway keeps configured route choice, durable owner sessions, post-Dream scheduling, bounded wake enumeration, background model execution and actual observation event delivery. Rename implementation references to **Entity Resolution** only where they mean this exact pass; don't misname a generic Gateway observation mechanism. |
| `crates/reliquary-perception-store/**` | **Unchanged** durable owner: Entity/EntityResolution/Relationship stores and current wire identities. No new store package or codec ownership. |
| `src/perception/relationship_*.rs` | Remain at the appropriate existing Root/Perception Store owner boundaries. Current Relationship publication/maintenance and future relational-Observation synthesis are **not** extracted into the identity-decision crate merely because of the old directory name. |
| `src/perception/*tests.rs` and related runtime tests | Move only pure service unit tests; retain real REL/PHY/Container, batching, replay and Gateway scenarios at Root integration seams. No duplicated suites or placeholder assertions. |

### Error and value contract

The lower service must use canonical value types from `reliquary-perception-store` (Entity, EntityId, reason/status), `reliquary-memory-core` (Memory, MemoryId, mention keys and spans), and `reliquary-switchboard` (General/Decision endpoint trait/error). It may use `reliquary-retrieval` for tokenization/lexical utilities **only**, without gaining unauthorized data access. Import `serde_json` for existing resolver/request schema, and current hashing utilities only if used by fingerprint computation. Avoid a copy of record/ID representations.

Root's existing `EntityCandidateError`, `EntityResolverError`, `EntityReconciliationError` currently embed product-composed Memory/Graph/Entity errors. **Do not** move foreign owner error objects into a service by importing the Root crate. Keep Root-facing composed errors and map the service's narrowly typed pure/endpoint failures at the Root caller. Preserve precise failure precedence and public variant/discriminant where observable. Do not create a general error-conversion or service-locator framework.

Owner-qualified reads never become permissible because a typed `EntityRef` appears in an evaluation. Root is the sole authority deciding which Memory/Graph/Entity evidence crosses the boundary.

## Staged hard cut — move once

Declare one bounded red migration window for the actual ownership change. During this window product calls may be temporarily red; the target and final integration gate are non-negotiable. No root implementation duplicated into a new crate, compatibility `reliquary-perception` stub, giant trait wrappers or speculative SDK.

1. **E1 — Baseline, real name, frozen behavior.** Verify the committed Retrieval/Vectors baseline, inventory all `src/lib.rs` public exports and owner/runtime call sites, measure target pure functions, freeze candidate lane ordering, admission reason/ID outcomes, threshold/margin, prompt/schema JSON, retry fingerprints, stale-commit/parallel work, REL/PHY/principal privacy and immutable fixture outputs. Rename the reserved crate and update workspace lock + physical map together. No service algorithm yet.
2. **E2 — Canonical decision values and deterministic candidates.** Add only needed lower dependencies. Extract candidate value/config models, ranking, context-query/surface normalization, identity guard and direct prepared-evidence inputs; Root collects trusted snapshots, without callback access to mutable stores. Test a real consumer from another crate. Keep fine-grained files small; don't create a coordinating god-file.
3. **E3 — Inference and Admission policy.** Move `EntityResolver`, `EntityResolutionEngine`, Decision fast-path, zero-candidate Admission schema, response and materialization guards. Reuse Switchboard trait directly. Freeze exact JSON and error routing; no new provider/model calls, configured selection changes or automatic retries. Root prepares, invokes service, revalidates after inference.
4. **E4 — Reconciliation/identity audit and classification.** Move pure pair candidate selection, ambiguity-safe classifier and matching/collision rules. Root continues durable Entity/Relationship merge/retarget and Graph association; retain zero/unknown/related-distinct abstention. Verify no new inference or disk write occurs in the service.
5. **E5 — Root and runtime integration / deletion.** Rewire REL/PHY public entrypoints, candidate gather, processing, reconcilers, batch scheduling, runtime host, Insomnia post-Dream and existing consumer tests. Delete displaced root algorithms and the `reliquary-perception` placeholder, retaining only owner I/O/commit adapters, existing Relationship owner code and the separate broader Perception docs. Preserve public Rust paths through direct re-exports, not forwarding functions. Validate no unapproved upper/lower reverse import.
6. **E6 — Verification and close.** Run gates below, update `docs/subsystems/perception.md`, `docs/entity-resolution-state.md`, `docs/architecture.md`, `docs/maintainer-map.md`, `docs/api.md`, documentation coverage, component/crate maps and crate-first plan to describe **what actually landed**. Write truthfully scoped evidence with actual test outcomes. Commit the integrated extraction and affected documentation only, excluding the unrelated dirty Graph evidence file.

Split E2–E5 into bounded, independently reviewable code moves; use focused tests in each. A placeholder renamed and compiled alone is **not** done. Root's final re-export must resolve to the actual service type with no duplicate implementation.

## Testing and verification decisions

### Service contracts

- `cargo test -p reliquary-entity-resolution --locked --offline -j 1`: candidate ranking/normalization/tie order, exact mention validation, lexical/Graph support caps and identity guard, recurrent vs transient admission, principal reserved identity, decision threshold/margin, low-confidence/malformed/no-calibration fallback, generative zero-candidate admission output validation, exact schema/probe construction, pure classification and deterministic fingerprints.
- Test rejection/ambiguity across same-looking but distinct referents, punctuation/camel-case equivalent surfaces, suffix/path normalization, alias-only matches, cross-kind collisions, unused evidence, and stable canonical Entity ID/Memory key. Model mocks must not be able to bypass hard policy gates.
- Service test of non-mutating behavior: independent pure candidate/decision evaluation has no `Container`, Root or hidden store handles; no write or global clock mutation.

### Root/Runtime integration (existing tests are primary acceptance)

- Existing `entity_candidate_*`, `entity_resolution_*`, `entity_resolver_*`, `entity_admission_*`, `entity_creation_guard_*`, `entity_reconciliation_*`, `entity_audit_*`, `entity_lifecycle_*`, `entity_principal_*`, `entity_zero_bootstrap_*` regression groups. Retain full 41-query zero-Entity bootstrap expected-state and reopen checks on a disposable owner.
- No-op retry with identical candidate/context fingerprints, changed-evidence wake and post-Dream scheduling, pending-to-dormant (30 days), eventual tombstone (180 days), terminal resolved/rejected, archived/superseded source purge and retained body spans.
- Wrong/retired Entity reference fail-closed, active replacement canonicalization on reopen, alias learning, creation conflict rebinding, exactly-once Entity–Memory association, version/change detection across Memory/Graph/Entity/mention, owner identity changes, timeout/backpressure and result retry.
- Parallel batch: source order, bounded worker count, deterministic final commit timestamps/outcomes, identical sequential result, prepared snapshot change causing one re-evaluation; no inference under held mutable owner locks.
- REL/PHY privacy and principal exception, configured route inheritance (`entity_resolution_decision` → `general_decision` → adapted `general`) and distinct generative fallback, strict association authorization.
- Entity reconciliation and audit: exact/normalized/cross-kind collisions, same-identity vs related-but-distinct merge inhibition, retired canonical remap and retained Relationship replay. Verify public Root APIs and CLI remain source-compatible.
- Historical/replay safety: same durable `CVAERS03` bytes, event/record identities, persisted revisions and exact reopen; no altered Container footer/wire IDs. `CVAFRS07` is **not** a gate specific to this extraction.

### Final commands

```text
cargo test -p reliquary-entity-resolution --locked --offline -j 1
cargo test -p reliquary-perception-store --locked --offline -j 1
cargo check --workspace --all-targets --locked --offline -j 1
cargo test --lib --locked --offline -j 1
cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1
cargo fmt --all -- --check
ddocs check --indexes --links
git diff --check
```

Run focused Root Entity/candidate/resolution/reconciliation tests and configured route tests explicitly. Record existing measurement stress tests that hang independently; exclusions cannot silently become claims that all tests passed. Run architecture policy and Pitlord/Arcana only if available and current enough; Cargo enforces physical edges, while stale graphs cannot prove the extraction. The current documentation-policy checker has **28 pre-existing missing-section findings** in other extraction documents and a Windows long-path failure without the extended path prefix. Record that separation; do not block the service's implementation on regenerating a costly graph or quietly claim full policy compliance.

### Completion and audit gates

**Not complete until:** the root is linked against one real `reliquary-entity-resolution` library; the `reliquary-perception` package is gone from source/workspace/locks/maps; existing decision, candidate, admission and reconciliation **policy** has no surviving duplicate Root implementation; Store remains canonical durable owner; Root/Gateway retains sole authority for evidence, model-route selection and accepted writes; REL/PHY and public API are behavior-compatible; service crate tests plus relevant Root and CLI tests pass; current source/docs maps describe the change; the implementation and evidence are committed without Graph work.

## Non-goals

No Observation persistence/synthesis, inductive/abductive/abstractive inference, new receptors, source Web exploration, Chronos validity integration, future relational-Observation convergence, new graph semantics, broadened identity heuristics, calibration/prompt/model changes, new durable resolution state/format, new migration reader, new scheduler, new agent memory route, cross-REL/PHY permission expansion, generalized Perception crate, extra model iterations, or architecture-analysis tooling.

## Related docs

- [Current Entity Resolution state](entity-resolution-state.md), [current Perception foundations](subsystems/perception.md)
- [Entity/Relationship/Resolution Store specification](perception-store-crate-extraction-spec-2026-10-26.md), [evidence](perception-store-crate-extraction-evidence-2026-10-26.md)
- [Future Perception Observations plan](perception-subsystem-plan.md)
- [Crate-first architecture](crate-first-architecture.md), [layout](crate-first-layout-spec-2026-10-25.md), [crate map](architecture/crate-map.json)
- [ADR 0033](decisions/0033-perception-entities-observations-and-ambiguity.md), [ADR 0034](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md)
- [Behavioral contracts](behavioral-contracts.md), [API](api.md), [Maintainer map](maintainer-map.md)

## Notes

This spec commits only the decided name, boundaries and migration plan; it does not rename the reserved crate or claim that Entity Resolution has been extracted. The implementing agent must preserve the baseline receipts and not convert the whole `src/perception` directory into a library.
