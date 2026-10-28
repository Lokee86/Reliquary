# Insomnia service crate extraction — implementation specification

**Status:** Evaluator extraction and Root evidence coordinator integrated; remaining verification is recorded in the companion evidence (2026-10-28).
**Repository / branch:** `C:\!bin\workspace\Reliquary-crate-first` / `refactor/crate-first-restart`.
**Baseline:** Entity Resolution service integration `3cda7eb5`; verify the actual HEAD and status before starting implementation. The pre-existing modification to `docs/graph-crate-extraction-evidence-2026-10-26.md` is unrelated: do not change or stage it.
**Canonical target:** `crates/reliquary-insomnia/`, package `reliquary-insomnia`.
**Scope:** Extract the **existing** Episode-to-Memory semantic processing service. `reliquary-insomnia-store` already owns its durable work/attempt state; this is not a second store extraction.

## Purpose

Move Insomnia's authority ledger, grouping, metadata, ownership classification, wording, Entity-mention enrichment, bounded inference and deterministic candidate validation out of the Root library into one compiled service. Preserve all current REL/PHY and external-source entrypoints through direct re-exports or thin owner adapters. Keep owner reads, durable publication, replay, model route configuration and runtime scheduling in their existing authorities. No changed Memory semantics, new model workflow or file format.

**Outcome:** The service can evaluate an immutable, owner-authorized Episode/turn input and return an extraction or a bounded evidence request without reading `Cva`, `Phylactery`, Container, Archive stores, LexicalIndex, or any durable queue. Root supplies authorized evidence and publishes validated results.

## Overview

The current semantic implementation lives in `src/runtime_inference/insomnia/**`; `src/runtime_inference/insomnia.rs` exports the public types. The `reliquary-insomnia` crate is empty. The separately integrated Insomnia Store already owns Episode-keyed work/attempt records, leases, retry/ready scheduling and exact lower-store reconstruction. Its product adapter explicitly converts the store's fixed-byte Episode/Memory IDs and authenticates Archive inputs; do not undo this boundary.

Insomnia processing has a **two-part interface** rather than a callback into Root: (a) evaluate the supplied immutable Episode/turns, possibly producing an evidence request; (b) accept a bounded, already-authorized evidence response and finish the single permitted round. A pure service cannot enumerate or authorize the Archive. The existing external-input path never requests an Archive fallback.

## Dependency and authority contract

| Component | Authority after extraction |
| --- | --- |
| **`reliquary-insomnia`** | Immutable extraction input and output; ledger schema, prompts, validation and repair; fixed synthesis groups; metadata/ownership/wording model stages; deterministic candidate keys, text/receipt/authority guards, provenance-shape checks, enrichment span validation; evidence-request parsing/limits and one-round state; pure retry/backpressure classification if it needs only endpoint errors. |
| **Product Root (`Cva`, `Phylactery`)** | Episode/turn authentication; Archive and lexical evidence search, authorization and hydration; exact source/body/quote revalidation; MemoryDraft and grouped publication preparation; accepted owner selection; REL/PHY commits, Freshness birth proofs, completion receipts, import/replay and error translation. |
| **`reliquary-insomnia-store`** | Durable work queue, leases, attempt and completion projections, priority/retry ordering and restore. No extraction models, Archive/Memory access or service dependency. |
| **Archive / Memory Core** | Canonical `Episode`, `EpisodeId`, `ResolvedTurn`, Memory, body/source IDs and routing mention value types. Neither is a service-owned cache or a source of authority by mere type possession. |
| **Chronos** | Temporal assessment/disambiguation and verification over prepared text and exact source times; Root binds conclusions and source/body chronology before persistence. |
| **Switchboard** | `GeneralEndpoint`/`DecisionEndpoint` traits and configured endpoints. Host selects ledger/metadata/enrichment/ownership/temporal routes and credentials; Insomnia receives endpoints only. |
| **Runtime/Gateway** | Wake/scheduling and backlog drain, worker threads, leases/heartbeats, progress, retries, backpressure, active REL/PHY locks, vectorization and cross-owner execution; no service-owned background runner. |
| **Entity Resolution / Retrieval / Freshness / Vectors / Dream / Ego** | Their existing independent semantics and authority stay unchanged. Entity mention extraction does not invoke Entity identity resolution; retrieval/evidence hydration and vectors remain above the Insomnia service. |

The permitted maximum direct dependencies for `reliquary-insomnia` in `docs/architecture/crate-map.json` currently include Archive, Chronos, Freshness, Insomnia Store, Memory Core, Entity Resolution and Switchboard. **Use only actually needed dependencies**, ordinarily Archive for canonical immutable input values, Memory Core for mention/provenance values, Switchboard for endpoint interfaces, and Chronos only if pure temporal policy is extracted. A listed maximum is not authorization to move durable Freshness, store access or Entity identity decisions into this service. No dependency on Root, Container, Runtime/Gateway, Owner Lifecycle, History, Dream, Ego or higher coordination. Any unapproved physical edge is an architecture failure, not a reason to introduce a facade crate.

### Canonical public values and errors

- Preserve current public Root paths/semantics for `InsomniaExtractor<E>`, `InsomniaExtraction`, `InsomniaCandidate`, `InsomniaRejection`, `InsomniaOwnership`, `InsomniaRoutingMetadata`, `InsomniaEvidenceResult`, `InsomniaEvidenceTurn`, `InsomniaExtractionError`, contract constants, `insomnia_schema` and the semantic progress stages. Export actual service types and re-export them from Root; **never** maintain two active implementations.
- Retain `InsomniaWork`, `InsomniaAttempt`, `InsomniaLeaseToken`, queue error and store-specific ID conversion at the Root/Store seam. The previously approved **owned** `Cva::insomnia_work` return remains owned; do not restore the borrowed incompatible type.
- Retain the externally visible `Cva::process_claimed_insomnia_episode`, routed variant, drain APIs and configured-host behavior. Their `InsomniaProcessResult` and `InsomniaProcessError` still compose Root Memory, Archive, queue, PHY and publication errors; do not pull these foreign errors down into the service. Preserve error precedence and map the service's endpoint/schema failures at the exact existing caller.
- Reuse canonical lower-owner Memory/Archive types and Switchboard endpoint traits; avoid cloned ID families, a `Cva` trait abstraction, project-wide service locator or broad generic callback interfaces. Where a service method currently accepts `&mut Cva`, split the **evidence hydration** into a small Root adapter and keep the actual extractor type in the service.

## Source ownership crosswalk

| Existing source | Extraction disposition |
| --- | --- |
| `insomnia/{contract,input,ledger,synthesis,metadata,ownership}.rs` | **Service:** exact `v3-4` selection contract, dynamic per-Episode ledger schema/repair, fixed groups, metadata, conservative REL/PHY ownership, wording and immutable input representation. |
| `insomnia/{candidate,candidate_policy,candidate_receipt_policy,candidate_shape,candidate_source,candidate_text}.rs` | **Service:** bounded parsing, accepted/rejected candidates, deterministic semantic key/provenance validation, receipt/question/grounding guards, text normalization. No owner writes. |
| `insomnia/{extraction,enrichment}.rs` | **Service:** `InsomniaExtractor`, optional stage endpoints, structured outcomes and Entity-span enrichment; split out `extract_with_evidence(&mut Cva,...)` Archive fetching into Root. Keep external-derived extraction source-only. |
| `insomnia/evidence.rs` | **Split:** Service parses/validates requests and enforces count/turn/byte bounds on immutable supplied evidence; Root owns `LexicalIndex`, Archive search, request authorization, `Node`/`Fragment` selection, `Container` hydration and truncation of returned evidence. Only one read-only round is allowed. |
| `insomnia/temporal.rs` | **Split:** If needed, move pure Chronos text assessment or deterministic selection on supplied drafts; Root retains source timestamp/MemoryBodyId binding and applies verified Chronos inference to prepared owner publications. Do not import Root's `TemporalDisambiguator` wrapper or alter Chronos routing. |
| `insomnia/processor/{source_validation,prepared,application}.rs` | **Root:** authenticated node/quote provenance, source timestamp lookup, `MemoryDraft` + routing preparation, cross-owner source refs, duplicate checks, PHY publication, grouped Memory/Freshness completion, exact `CVAINSC7`/`CVAINSC8` authoring and import replay. Move only deterministic checks that operate entirely on prevalidated immutable values. |
| `insomnia/{cva,completion,rebuild,product_adapter,error,model}.rs` | **Remain Root/Store adapter:** queue APIs, validated work/completion envelopes, open/replay, public error mapping and existing `reliquary-insomnia-store` canonical work transitions. |
| `insomnia/{runtime_step,worker,worker/runtime,backpressure,progress}.rs` and configured/host Insomnia entrypoints | **Remain Runtime/Root:** claim, renew, recover, release, fail, terminal, bounded retry, progress, concurrent worker drain, provider routing, REL/PHY lock and vectorization. Move only a demonstrably pure rule if necessary; do not move the runner. |
| `insomnia/*tests.rs`, `worker/**` tests and Root/Gateway integration tests | Transfer pure evaluator tests to the service; leave owner, replay, worker, PHY, loopback and crash tests on the product boundary. No duplicate regression implementation. |

## Frozen behavioral invariants

1. **Authority ledger first.** Every authoritative user turn is accounted for; assistant statements are not independent authority. Distinguish direct/correction, explicit adoption/retention, grounding and omitted clauses. Preserve conservative rejection of interrogatives, execution receipts, transient troubleshooting and hallucinated detail; retain modality and latest applicable user-authoritative state.
2. **Fixed membership and wording.** `v3-4` ledger drives fixed group identities. Metadata, ownership and wording stages cannot add, drop, split, merge or silently reinterpret accepted propositions. Repair of missing ledger fields/turns stays bounded and schema-validated. Candidate limit is **64**.
3. **Evidence trust.** Read-only evidence only for legitimate callbacks/grounding; at most **4 requests**, **64 evidence turns** and **128 KiB**; one request/response round maximum. Evidence does not become user authority. The external-derived path may use only its supplied authorized window; an Archive request is invalid, not an implicit Root lookup.
4. **Identity and provenance.** Candidate/source roles, exact source quote and byte ranges, source order, deterministic keys/dedupe, full text shape and semantic authority guards do not change. Entity mention spans must be exact UTF-8 ranges in final title/content and respect current per-Memory count and routing text caps.
5. **Ownership.** User-global PHY routing remains conservative and project is the ambiguous/non-user default; ownership classification itself grants no access. A personal candidate without a valid PHY destination cannot be silently written to REL. REL/PHY output and authenticated external owner refs never leak across boundaries.
6. **Chronos.** Preserve current source timestamp, temporal status and deterministic assessment; inference is requested only where current assessment needs it and committed only when exact source/body evidence matches. No semantic fact is created from temporal inference alone.
7. **Durability and retries.** Zero-Memory results are valid. Processing work identity, attempt counts, lease tokens, ready/retry/terminal state, backpressure, completion order, and exactly-once Memory/grouped completion publication remain unchanged. A stale claim, source drift, failed PHY publication or malformed inference must never mark a work item successfully complete. Existing `CVAINSC7`/`CVAINSC8` bytes, Freshness birth proofs and replay semantics stay untouched.
8. **No model drift.** Preserve exact prompts/schema, stage calls, repair behavior, optional endpoint fallbacks, accepted errors and configured route choices. Do not change worker caps/defaults (**48** default, **64** maximum; 15-minute lease; 5-second retry; 50 ms poll; 5 attempts) or turn-count scheduling in a mechanical extraction.
9. **Boundary and scope.** No new explicit-retention feature (see [plan](insomnia-explicit-memory-commit-plan.md)), no Observation implementation, no provider replacement, no new storage schema, no corpus rewriting and no redesign of Dream/Ego/Entity Resolution. Previous `CVAFRS07` promoted six-file recovery and agentic recall limitations remain separate product gates.

## Execution — small staged hard cuts

Complete the following in order, with **one canonical implementation at every completed step**. A temporarily red compile during a coordinated call-site move is acceptable; a second production policy implementation is not.

1. **I1 — Baseline inventory.** Confirm Entity Resolution HEAD and preserve unrelated Graph evidence. Record Root public exports, all native/external caller sites, `reliquary-insomnia-store` ID conversions and byte fixtures. Freeze `v3-4` prompt/schema, default worker limits and focused test expectations before moving code.
2. **I2 — Compile the target shell.** Add only justified direct dependencies to `crates/reliquary-insomnia/Cargo.toml`; wire Root dependency and the current crate map without adding unapproved edges. Prove a real external consumer can import one existing service value, not just a placeholder crate.
3. **I3 — Input/contract cut.** Move canonical source input, `InsomniaOwnership`, fixed contract constants, generic/dynamic ledger schema and validation. Re-export from Root. Run deterministic ledger/provenance tests before proceeding.
4. **I4 — Semantic groups and classification.** Move ledger repair and fixed group building, metadata, ownership and wording stages with their existing endpoints and exact JSON. Validate all turn keys, required groups, assistant adoption and ambiguous project-default routing.
5. **I5 — Candidate validation.** Move parsing, authority/receipt policy, shape/source field rules, stable keys and rejected-candidate ordering. Keep Archive-backed timestamp and cross-source validation in Root. Run adversarial malformed-output and provenance tests.
6. **I6 — Entity mention enrichment.** Move the enrichment stage and span validators using lower Memory Core mention types, with exact body range checks and unchanged upper limits. Do not call Entity Resolution or create Entities.
7. **I7 — Evidence split.** Move request schema/one-round state to the service; Root handles bounded search, authorization, source lookup, read-only hydration and truncation. Rewire `start`/`finish_evidence` to pass immutable results. Prove no `Cva` or `Container` reference survives in the service crate.
8. **I8 — Extractor identity and paths.** Move the actual `InsomniaExtractor<E>`, endpoints and native/external results into the service; Root retains one narrow `extract_with_evidence` coordinator calling the service twice at most. Remove displaced Root algorithms. Check external no-Archive behavior and direct Root API re-exports.
9. **I9 — Prepared/Chronos integration.** Keep publication drafts and source/body binding above the service. Extract only genuinely independent temporal rules, then rewire both direct and routed `process_claimed_insomnia_episode` paths; verify owner/post-inference revalidation.
10. **I10 — Runtime and recovery integration.** Rewire worker runtime, configured host, Gateway and external-source callers to the service-owned types. Preserve lease/heartbeat/backpressure, concurrency, deterministic completion, crash reopening and vector result reporting. No new service-owned scheduler.
11. **I11 — Delete and audit.** Remove obsolete Root policy modules and unused private aliases; retain only deliberate adapters and Root integration tests. Verify Cargo graph/allowlist has no dependency inversion or duplicate active implementation; update current canonical Insomnia/API/code-map docs to describe what actually landed.
12. **I12 — Full acceptance and commit.** Run the tests and tools below, write a concise extraction evidence record with actual results and remaining gaps, and commit only the integrated cut and its docs. Never stage the pre-existing Graph evidence modification.

Each step is bounded by the named source family and its immediate call sites. If a change requires a broad Owner/Runtime rewrite, split it into a separate reviewable seam; do not expand one step into an unmeasured multi-system migration. Prefer small source files and coordinators; avoid a new god-file or generic framework.

## Verification and acceptance

**Focused service:** `cargo test -p reliquary-insomnia --locked --offline -j 1` plus one external-consumer compile/test. Freeze and compare prompt/schema JSON, multi-clause ledger repair, fixed groups, ownership, candidate key/order, exact Entity spans, zero-Memory outcomes, endpoint failure and the one evidence round. The service crate must contain production logic and must have no Root/Container/Runtime dependency.

**Durable owner:** `cargo test -p reliquary-insomnia-store --locked --offline -j 1`; validate queue/lease/attempt and completion projection behavior. Verify no change to historical/current work/attempt/completion format or Store's Container-only dependency.

**Product and runtime:** Run the existing `insomnia::*`, extraction/gold/grounding/evidence/ownership/temporal/queue/recovery/worker/backpressure/vector tests, REL and PHY direct/routed publishing, external-derived input, configured model routes and Gateway background processing. Include duplicate claim, lease expiration, fail/terminal, failed PHY write, atomic grouped completion and reopen; verify source/endpoints are not called while holding mutable owner locks.

**Workspace:** `cargo check --workspace --all-targets --locked --offline -j 1`; `cargo test --lib --locked --offline -j 1`; `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`; `cargo fmt --all -- --check`; `git diff --check`. If loopback-dependent tests require network access, rerun with an environment that permits local test servers; record unavailable tests as unverified, not passed.

**Architecture/docs:** `ddocs check --indexes --links`, shared documentation-policy checks, architecture guard/refresh and Pitlord docs ownership where tool support exists. Update `docs/subsystems/insomnia.md`, `docs/architecture.md`, `docs/api.md`, `docs/maintainer-map.md`, `docs/documentation-coverage.md`, `docs/crate-first-layout-spec-2026-10-25.md`, `docs/crate-first-architecture.md`, and crate/component maps for actual moved ownership. Record tooling limitations instead of concealing them.

**Frozen-corpus fidelity:** On existing authorized fixtures compare accepted Memory IDs and bodies, source refs and timestamps, accepted/rejected counts, model prompts, routing, Entity mention metadata, REL/PHY state, queue completion and reopen. The promoted `CVAFRS07` six-file semantic recovery gate remains explicitly outstanding unless independently run and passed. An unavailable fixture blocks that particular acceptance claim, not all lower-level verification.

**Completion is defined by:** compiled real service ownership; unchanged product APIs and behavior; no duplicate Root policy implementation; correctly layered dependencies; all executable focused/product checks passing; scoped evidence documenting any remaining environment/corpus blocker; and a committed integrated cut. A compiling placeholder, pure-file copy or passing unit suite alone is insufficient.

## Related docs

- [Current Insomnia subsystem](subsystems/insomnia.md)
- [Insomnia Store extraction](insomnia-store-crate-extraction-spec-2026-10-26.md) and [evidence](insomnia-store-crate-extraction-evidence-2026-10-26.md)
- [Crate-first target architecture](crate-first-architecture.md) and [crate layout](crate-first-layout-spec-2026-10-25.md)
- [ADR 0012: Episodes and Insomnia authority](decisions/0012-deterministic-episodes-and-insomnia-memory-authority.md)
- [Insomnia semantic validation](insomnia-semantic-validation-2026-08-24.md)
- [Explicit Memory commit plan](insomnia-explicit-memory-commit-plan.md)
- [Behavioral contracts](behavioral-contracts.md) and [Current limitations](current-limitations.md)

## Notes

Specification only: no source, Cargo manifest, fixture, queue, runtime behavior or persistent format is modified by writing this document. Implementation acceptance is recorded separately when the extraction is executed.
