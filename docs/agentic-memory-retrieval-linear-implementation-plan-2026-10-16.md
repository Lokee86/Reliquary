# Agentic Memory retrieval — linear implementation plan

**Date:** 2026-10-16  
**Status:** executed through Step 45 on isolated feature candidates; code/integration gates passed, with frozen-corpus fidelity blocked by CVAFRS03 and deployment not claimed.
**Scope:** implement the accepted three-tool agentic Memory retrieval architecture from lower-domain candidate collection through real host handoff, Freshness reinforcement, frozen-fixture verification, and final integration.  
**Source specs:** [Agentic Memory retrieval design](agentic-memory-retrieval-design-2026-10-15.md), [Agentic Memory retrieval implementation specification](agentic-memory-retrieval-implementation-plan-2026-10-16.md), [External Memory consumption and Freshness](freshness-external-consumption-contract.md), and [Freshness retrieval boundary verification](freshness-retrieval-boundary-verification-2026-10-15.md).

## Purpose

Turn the accepted AR1–AR12 architecture specification into a linear coding plan that can be executed one numbered step at a time without reopening settled design questions.

## Overview

The plan contains **45 executable steps**. It preserves the staged hard cut: existing passive Memory search remains a separate supported path, while the new agentic path converges directly on one controller, one REL-only full candidate-pool seam, one dedicated decision route, one-hop graph traversal, and one successful-host-handoff reinforcement boundary.

The sequence deliberately builds and verifies the lower-level seams before exposing the model-visible tools. It does not add a compatibility agentic mode, a fourth browse tool, a persistent controller registry, a second Freshness write path, or a second delivery journal.

## Related docs

- [Agentic Memory retrieval design](agentic-memory-retrieval-design-2026-10-15.md)
- [Agentic Memory retrieval implementation specification](agentic-memory-retrieval-implementation-plan-2026-10-16.md)
- [External Memory consumption and Freshness](freshness-external-consumption-contract.md)
- [Freshness retrieval boundary implementation plan](freshness-internal-retrieval-boundary-implementation-plan-2026-10-15.md)
- [Architecture](architecture.md)
- [Architectural invariants](invariants.md)
- [Current limitations](current-limitations.md)
- [Roadmap](roadmap.md)

## Notes

This plan records intended implementation work, not completed acceptance. The two pre-existing uncommitted Ego documentation edits are unrelated and must remain untouched by this work unless separately requested.

The current Arcana snapshot was observed to disagree with direct source for at least `runtime_host_memory_search.rs`. Step 1 refreshes Lexicon/Arcana before structural evidence is used during execution. Direct source remains authoritative whenever generated graph state disagrees with the repository.

## Model assignment

- **Luna** — bounded implementation after the interface is frozen, codec/CLI plumbing, focused fixtures, parser tests, mechanical integration, documentation closure, and deterministic verification.
- **Sol** — ownership boundaries, state-machine semantics, candidate-pool extraction, concurrency, authorization, host handoff, Freshness receipt semantics, cross-repository host integration, and final architectural convergence.
- A Luna step that exposes a genuinely new semantic or ownership decision stops at that decision and hands it to Sol rather than inventing a local policy.
- Do not downgrade a Sol step merely because the diff is small; assignments reflect semantic risk rather than line count.

## Frozen execution decisions

These decisions remove ambiguity before Step 1.

### Controller lifetime and call direction

`AgenticMemoryController` is an **in-memory value owned by the embedding host for exactly one requesting-agent turn**. It is not stored in a REL, PHY, `GatewayRuntime`, `InteractionRuntime`, config file, journal, or semantic database.

The canonical call direction is:

```text
embedding host, one requesting turn
    |
    | owns &mut AgenticMemoryController
    v
GatewayRuntime agentic operation
    |
    +-- instance authorization / REL dependency closure
    +-- owner-local Cva reads
    +-- embedding route when memory_query needs a query vector
    +-- memory_retrieval_decision route
    v
PreparedAgenticMemoryResult
    |
    | actual successful tool-result handoff
    v
GatewayRuntime delivery completion
    +-- SelectedIDs
    +-- owner-grouped AcceptedMemoryUseReceipt
    +-- Cva::record_accepted_memory_use()
```

The host creates the controller at the beginning of the requesting turn, reuses it across all three agentic Memory tools in that turn, and drops it at turn end. Reliquary owns the state-machine implementation; the host owns the lifetime.

### Controller state

The only persistent logical controller state is:

```text
CurrentQuery: Option<String>
SearchedIDs: BTreeSet<MemoryRef>
SelectedIDs: BTreeSet<MemoryRef>
```

There is no `SentMemoryIDs`, persistent pending-selection store, selected-content cache, traversal cursor, numeric turn budget, or durable controller snapshot.

Prepared delivery is an ephemeral returned value carried by the trusted host until completion. The controller does not maintain a pending-delivery map.

### Operation atomicity

The three model-visible operations are state-atomic:

- a successful `memory_query()`, including `no_matches` or `already_provided`, commits the new `CurrentQuery` and clears the prior query's `SearchedIDs`;
- a failed `memory_query()` leaves the prior controller state unchanged;
- `refine_query()` replaces `CurrentQuery` only after a valid complete rewrite and successful reconsideration result;
- `traverse_graph()` gathers all anchor results before committing any shared state; any worker/model/validation error fails the whole traversal without partial `SearchedIDs` mutation.

This is stricter than mutating state before external model I/O and satisfies the specification requirement that malformed model output cannot mutate state or authorize delivery.

### Candidate semantics

The new lower-domain candidate seam is REL-local and internal.

- Lexical retrieval exposes the complete set of current Memories matching existing Memory lexical semantics rather than a public presentation limit.
- Semantic retrieval exact-scores the searchable Memory-vector population for the selected compatibility profile before community routing, seed limits, traversal, or ten-result truncation.
- The candidate pool is the owner-qualified union of those lexical and semantic lanes.
- Existing lexical/semantic scores are retained as evidence only. No new fusion score controls presentation.
- Freshness, not lexical/semantic score, determines presentation order after eligibility has been established.
- Bound PHY is excluded from the new agentic path. Existing passive PHY-inclusive search is unchanged.

### Freshness ordering

For each owner, read Freshness at the owner activity cut captured for the operation. Stale and Dormant Memories remain eligible. A Memory with no durable Freshness record receives score `0` for ordering only and is not enrolled or mutated by the read.

Cross-owner ordering is descending Freshness, then `owner_id`, then `MemoryId`.

A legacy Memory lacking a Freshness record can therefore be presented. If it is actually handed off, the existing `record_accepted_memory_use()` validation remains authoritative; the agentic path must not fabricate a Freshness record merely to make reinforcement succeed.

### Graph neighborhood and overlap

`traverse_graph()` uses the **complete active one-hop Memory neighborhood** of each anchor: outgoing and incoming active Memory-to-Memory relations are collected. Direction and relation kind are retained as anchor-local evidence. There is no recursive traversal or depth parameter.

If multiple anchors expose the same neighbor:

- that Memory content can appear only once in the returned prepared payload;
- every applicable anchor retains its own relationship report;
- independent anchor decisions remain independent;
- if at least one anchor approves the shared Memory, the shared content is approved globally for that traversal result;
- rejection under one anchor does not veto approval under another anchor;
- the union of every evaluated candidate is added to `SearchedIDs` only after all workers have been gathered successfully.

### Delivery and retry

The host supplies a stable delivery/tool-invocation identity before result preparation. The prepared manifest binds that identity to the exact payload, exact delivered `MemoryRef` values, and per-owner activity cuts.

Successful external handoff is the first event that can update `SelectedIDs`. Because delivery has already happened, `SelectedIDs` is not rolled back if later Freshness receipt application fails. Receipt retry reuses stable per-owner use identities; owners already durably recorded by `record_accepted_memory_use()` become no-ops and failed owners can be retried.

The current generic `ConversationExecution` handoff is not Memory-consumption proof and must not be reused as such.

## Execution status

- Steps 1–36 accepted on the feature candidates: Reliquary `638992a` and Warlock `cf41c66`; per-step implementation/test evidence is in the [execution record](agentic-memory-retrieval-baseline-2026-10-16.md). The actual receiver and restart gate establishes F3 for the isolated integration candidate, not deployment.
- Step 37 accepted: the native fixture runner and worktree-copy/hash wrapper are implemented. The current-format protocol smoke passes, exercising functional scenarios and twelve fidelity probes. Inventory reached all three frozen corpora and records an explicit format limitation: each contains a CVAFRS03 prototype record rejected by this branch's CVAFRS02 reader; current-container migration returns AlreadyCurrent. Authoritative REL/PHY/manifest hashes remain unchanged.
- Steps 38–40 evidence closed with explicit corpus-data limitation: functional and fidelity attempts reached all three RELs but CVAFRS03 prevents native open. No frozen retrieval scenarios ran and recall is unmeasured; all source hashes are unchanged. See the [verification record](agentic-memory-retrieval-verification-2026-10-16.md). This is not a fidelity pass.
- Steps 41–44 complete: Luna documentation closure and hard-cut audit integrated; coordination guard strengthened; refreshed Lexicon/Arcana/Pitlord gate passed, with dynamic call-resolution limitations recorded. Step 45 closed: core fmt/check/test (1,227 library + 141 integration, three ignored), locked CLI (21 tests), focused real host (8 Memory + 5 context), architecture and diff gates pass. Both docs checks retain only the four pre-existing unrelated Ego inventory gaps. Frozen-corpus recall remains unmeasured; the candidate is not deployed.

## Execution rule

Execute Steps 1–45 in order on the convergence branch unless a step explicitly says it can be prepared independently. Do not start host integration before the Reliquary completion seam is green. Do not mark F3 complete before the real embedding host passes Step 36.

Every source-mutating step should finish with its focused tests, `git diff --check`, a review of unrelated working-tree changes, and a commit before advancing. The final whole-repository gate remains Step 45; targeted gates are acceptable during the staged hard-cut window.

## Sequential implementation steps

| # | Model | Work | Done when |
| ---: | :---: | --- | --- |
| **1** | **Luna** | **Refresh and freeze the implementation baseline.** Confirm `main`/HEAD and the unrelated dirty Ego docs; refresh Lexicon and Arcana; inspect the exact source/test ownership around passive Memory search, `MemoryRetrievalIndex`, lexical Memory indexing, `ConfiguredDecisionEndpoint`, runtime routes, graph neighbors, `ego_memory_freshness()`, and `record_accepted_memory_use()`. Run the smallest existing focused retrieval/route/Freshness boundary tests that prove the baseline. | A dated baseline note records HEAD, refreshed graph snapshot ID, touched seams, focused commands/results, and confirms no agentic runtime implementation already exists. Unrelated Ego edits remain unchanged. |
| **2** | **Sol** | **Create the canonical agentic retrieval modules and controller state model.** Register `agentic_memory_retrieval_model.rs`, `agentic_memory_retrieval.rs`, `agentic_memory_retrieval_decision.rs`, and the planned gateway module in `lib.rs`. Define `AgenticMemoryController` with exactly `CurrentQuery`, `SearchedIDs`, and `SelectedIDs`, using owner-qualified `MemoryRef` keys. Keep the controller host-owned and turn-scoped; do not add a runtime registry. | The crate builds with the new modules, controller construction is explicit, and no additional persistent state field or registry exists. |
| **3** | **Sol** | **Implement pure controller transition/result types.** Add typed query/refinement/traversal outcomes, `already_provided` and `no_matches`, graph-anchor reports, error variants, and the preliminary prepared-delivery manifest/result shape. Implement staged state transitions so operations can commit atomically only after their external work validates. | Pure state methods encode reset/preserve rules, prepared results do not mutate `SelectedIDs`, and errors can discard a staged operation without partial controller mutation. |
| **4** | **Luna** | **Protect the turn-state contract with focused tests.** Cover owner-ID collisions, new-query reset of `SearchedIDs`, preservation of turn-wide `SelectedIDs`, refinement preservation, prepared-vs-completed state, failed-operation rollback, deterministic set ordering, and absence of a fourth state set. | Controller-state tests pass and fail if a bare `MemoryId` is used where cross-owner identity is required or if preparation marks content selected. |
| **5** | **Luna** | **Add the dedicated decision route to persisted config and switchboard ownership.** Add `models.memory_retrieval_decision` to `ModelSwitchboardConfig`, config keys/codec load-save, credential validation, switchboard accessors, and auth resolution. Missing configuration means the route is unavailable; it does not fall back to entity resolution or general. | Existing configs without the new object still open with the route unset; configured round-trip and credential validation work; no fallback accessor exists. |
| **6** | **Luna** | **Generalize configured decision-endpoint construction without coupling decision capabilities.** Extract the common `DecisionModelEndpoint + ModelRequestAuth -> ConfiguredDecisionEndpoint` construction inside `configured_decision.rs`, keep entity-resolution ownership intact, and add construction for the Memory-retrieval decision route. | Entity resolution and Memory retrieval share HTTP transport code but read distinct config/auth routes; existing entity-resolution callers remain behaviorally unchanged. |
| **7** | **Luna** | **Wire the dedicated decision route through runtime routes.** Add the route field/accessor/builder in `ReliquaryRuntimeRoutes` and construct it in `ConfiguredRuntime::runtime_routes()`. Do not put Memory retrieval behind `general()`, `dream()`, or entity routes. | Configured runtime exposes the Memory-retrieval `DecisionEndpoint` only when that exact route is configured. |
| **8** | **Luna** | **Add detachable CLI configuration support.** Extend the CLI config show/set/unset surface for `memory_retrieval_decision` using the existing decision-endpoint syntax and credential references. | CLI can inspect, set, persist, reopen, and unset the dedicated route without changing another model route. |
| **9** | **Luna** | **Close AR2 route tests.** Add config codec, switchboard/auth, configured-runtime, runtime-route, and CLI tests proving route isolation, missing-route failure, invalid credentials/URL failure, successful round-trip, and no fallback to entity/general. | Focused library and CLI route/config tests pass; AR2 is complete. |
| **10** | **Sol** | **Introduce the lower-domain Memory candidate-pool model and complete lexical lane.** Add `memory_candidate_pool.rs` types for owner-local candidate identity and lexical/semantic evidence. Deepen the Memory lexical index with a crate-private complete-match path so agentic collection is not capped by the public search result limit. | A caller can collect every lexical Memory match for one REL without graph traversal, Freshness, model calls, or public presentation truncation. |
| **11** | **Sol** | **Extract reusable exact Memory-vector scoring.** Refactor the current Memory retrieval exact scorer so the same vector validation/cosine interpretation can score the complete searchable vector population deterministically without seed/community/traversal limits. Existing `retrieve_memories*` continues to use the shared scorer with its current limits. | One canonical exact scorer serves both old retrieval and the new pool; no duplicated cosine or packed-vector decoding appears in the controller/gateway. |
| **12** | **Sol** | **Implement the REL-local lexical + semantic candidate pool.** Given query text, query vector, and compatibility profile, collect complete lexical evidence plus complete exact semantic scores, union by `MemoryId`, retain lane evidence, and expose owner-local hydration inputs/raw-match identity. Do not apply Freshness or a ten-result cap here. | The pool contains lexical-only, semantic-only, and overlapping candidates with deterministic evidence and no community/seed/traversal presentation bound. |
| **13** | **Luna** | **Verify candidate-pool completeness and old retrieval parity.** Add tests where a candidate outside raw rank ten remains in the pool, lexical and semantic lanes disagree, vector/profile data is missing or invalid, and IDs tie deterministically. Run existing `retrieve_memories()` tests to prove its behavior did not change. | New pool tests pass and old owner-local retrieval output remains unchanged; AR3 is complete. |
| **14** | **Sol** | **Implement passive Freshness ordering over owner-qualified pools.** Read `ego_memory_freshness()` at each captured REL cut, assign neutral zero only to missing records, retain Stale/Dormant candidates, and produce deterministic cross-owner ordering by score then owner ID then Memory ID. The helper must not mutate activity or Freshness. | Focused tests prove full-pool ordering, neutral missing grade, stale/dormant eligibility, deterministic ties, and zero Freshness writes/activity advancement. |
| **15** | **Sol** | **Build the GatewayRuntime agentic read envelope.** Add REL-only authorization snapshotting for the selected REL dependency closure, capture owner tokens/profiles/activity cuts, acquire the embedding route for `memory_query()`, avoid owner locks across external I/O, and revalidate attachment/authority before exposing a prepared result. Explicitly exclude bound PHY. | Gateway tests can distinguish authorized dependency RELs from unauthorized RELs and PHY; revoke/detach during model I/O invalidates the staged result rather than retargeting it. |
| **16** | **Sol** | **Implement the normal `memory_query()` path.** Validate a new query, obtain one query embedding, collect every authorized owner pool, preserve raw matches, exclude `SelectedIDs`, Freshness-order the complete remaining union, send at most ten owner-qualified candidates to the dedicated decision route, validate selected identities against that batch, hydrate approved content, and stage the new query plus evaluated `SearchedIDs`. | A candidate below raw semantic/lexical rank ten can become decision candidate through Freshness; the decision endpoint never sees more than ten; no content is selected or reinforced during preparation. |
| **17** | **Sol** | **Implement `memory_query()` fallback and prepared result semantics.** If no new candidate remains, intersect raw matches with `SelectedIDs`: return `already_provided` IDs only when non-empty, otherwise `no_matches`. If at least one new Memory is prepared, suppress the previous-provided report. Neither fallback path invokes the decision model or Freshness reinforcement. | Fallback behavior exactly matches the accepted design, successful fallback commits the new query/reset state, and actual errors leave the prior state unchanged. |
| **18** | **Luna** | **Close AR4 query and authorization tests.** Cover max-ten presentation, all-ten approval, Stale/Dormant candidates, missing Freshness grade, deterministic ties, selected exclusion, raw-match fallback, mixed old/new suppression, zero decision calls on fallback, REL dependency closure, PHY exclusion, missing routes/profiles, and revoke/detach revalidation. | Focused query/gateway tests pass and passive search behavior remains unchanged; AR4 is complete. |
| **19** | **Sol** | **Implement strict semantic query rewrite for `refine_query()`.** Require an active query; send the complete current query plus refinement instruction to the dedicated route; accept exactly one valid complete replacement query; keep the old query/state if the rewrite fails. | Refinement can correct or reinterpret rather than append mechanically, and no malformed/empty rewrite can alter controller state. |
| **20** | **Sol** | **Reconsider the entire declined set without ordinary retrieval.** Compute declined refs as `SearchedIDs - SelectedIDs`, hydrate those exact owner-qualified Memories under current authorization, evaluate every declined ref against the revised query in deterministic batches of at most ten, and prepare all newly approved content. Do not rerun lexical/semantic search or create a total selection budget. | More than ten declined Memories are all reconsidered through multiple batches; no fresh-search primitive is called; `SearchedIDs` remains the same logical evaluated set. |
| **21** | **Luna** | **Close AR5 refinement tests.** Cover correction/reinterpretation, no active query, empty declined set, >10 declined IDs, selected exclusion from declined, authorization changes, malformed rewrite/selection, no ordinary retrieval, and atomic rollback on failure. | Focused refinement tests pass; AR5 is complete. |
| **22** | **Sol** | **Implement complete one-hop anchor evidence.** For each authorized owner-qualified anchor, collect active outgoing and incoming Memory-to-Memory neighbors from the same REL, collapse duplicate neighbor IDs while retaining relation kind and direction evidence, and reject unauthorized/cross-owner anchor misuse. | The collector returns the complete one-hop neighborhood and never traverses a neighbor-of-neighbor. |
| **23** | **Sol** | **Build per-anchor traversal candidate preparation.** From one pre-batch controller snapshot, calculate `unseen_neighbors` and `declined_neighbors`, exclude both `SearchedIDs` and `SelectedIDs` from fresh presentation, Freshness-order the complete remaining neighbor pool, and retain at most ten candidates per anchor. | Counts reflect pre-call state, selected/declined Memories remain legal anchors, and an anchor with >10 eligible neighbors exposes only its current Freshness top ten. |
| **24** | **Sol** | **Execute anchor decisions concurrently and gather atomically.** Run independent decision calls for each anchor against the same pre-batch state; gather every worker result before changing `SearchedIDs`; if any worker/model/validation operation fails, fail the traversal with no partial controller-state commit. | Worker completion order cannot change the final evaluated set or prepared result, and partial successful anchors cannot leak state after another anchor fails. |
| **25** | **Sol** | **Consolidate overlapping traversal results.** Union all evaluated refs into the staged `SearchedIDs`; deduplicate approved Memory content globally; preserve every anchor-local relation/count report; when anchor decisions conflict on a shared neighbor, global delivery approval is the union of anchor approvals. Prepare newly approved content once without touching `SelectedIDs`. | Shared neighbors appear once in content, appear under every relevant anchor report, and one anchor's rejection cannot veto another anchor's independent approval. Repeated traversal can expose later neighbors because earlier evaluated refs are excluded. |
| **26** | **Luna** | **Close AR6 graph/concurrency tests.** Cover >10 neighbors, repeated traversal, incoming/outgoing relations, selected and declined anchors, shared neighbors, conflicting overlap, deterministic gather order, one-hop-only behavior, invalid anchors, worker failure rollback, and content/report dedupe. | All traversal and concurrency tests pass; AR6 is complete. |
| **27** | **Luna** | **Freeze and implement the versioned candidate-selection contract.** Define versioned request/prompt/schema constants for query, refinement-reconsideration, and graph-anchor selection using complete `CurrentQuery`, operation kind, owner-qualified candidate identities, content, and optional anchor relation evidence. Parse approved identities strictly as a subset of the supplied batch. | Unknown IDs, duplicate approvals, malformed identity, extra unsupported shape, or version mismatch fail validation and cannot authorize content. |
| **28** | **Luna** | **Freeze and implement the versioned query-refinement contract.** Define the dedicated rewrite request/prompt/schema carrying the complete old query and refinement instruction; accept exactly one complete non-empty revised query and reject malformed/multi-result responses. | The parser has one unambiguous success shape and no fallback to free-form provider text. |
| **29** | **Luna** | **Run simulator-driven contract abuse tests.** Exercise malformed JSON/value shapes, unknown/duplicate candidate IDs, stale owner IDs, oversized/empty rewrite results, provider errors, and version mismatch across all operation kinds. | Every invalid response leaves controller state unchanged and yields no prepared authorized content; AR7 is complete. |
| **30** | **Sol** | **Finalize the canonical prepared-delivery manifest.** Canonicalize the model-visible payload, exact newly included `MemoryRef` values, per-owner accepted-turn cuts, and host-supplied stable delivery identity into a deterministic manifest/digest. Define the trusted internal handoff-confirmation type separately from the existing generic ConversationExecution receipt. | Identical preparation produces the same manifest identity; changed payload/ref/cut under the same host delivery identity is detectable; no new durable delivery journal exists. |
| **31** | **Sol** | **Implement post-handoff completion and `SelectedIDs` transition.** Add the GatewayRuntime completion seam that accepts only an exact successful host confirmation for the prepared manifest, revalidates instance/owner authority, and then inserts exact delivered refs into `SelectedIDs`. Failed, cancelled, unknown, or mismatched confirmation performs no completion. | `SelectedIDs` changes only after successful external handoff evidence and never during search/model preparation. |
| **32** | **Sol** | **Apply owner-grouped accepted-use receipts with stable retry identity.** Deterministically group delivered refs by REL owner, derive one stable owner-specific use ID from the host delivery identity, use the captured owner turn cut, and call existing `record_accepted_memory_use()` under normal owner mutation coordination. On retry, successful owners no-op through existing receipt idempotence while failed owners may be retried. Do not fabricate missing Freshness enrollment. | Multi-owner success produces one existing receipt per owner; identical retry gives no duplicate reinforcement; conflicting identity reuse fails; no alternate Freshness writer is introduced. |
| **33** | **Luna** | **Close AR8 completion tests.** Cover failed/cancelled/unknown handoff, success, duplicate success, changed manifest conflict, multi-owner grouping, receipt failure/retry, missing Freshness record behavior, revoked/detached authority, and proof that internal retrieval/model calls never hit the completion seam. | Completion/Freshness boundary tests pass and only confirmed handoff reaches `record_accepted_memory_use()`; AR8 is complete. |
| **34** | **Sol** | **Locate and freeze the real embedding-host success seam.** Trace the actual registration/invocation/result-return path for Memory tools in the authoritative embedding host repository at execution time. Record the exact source files, event that proves the tool result entered the requesting agent context, available stable invocation identity, and retry/restart replay mechanism. Explicitly reject generic provider responses, HTTP status, transcript admission, and current ConversationExecution handoff as substitutes. | One concrete host success callback and replay identity are identified; if the host lacks them, the missing host primitive is specified before integration continues. |
| **35** | **Sol** | **Wire exactly three model-visible tools and one controller per requesting turn.** Register `memory_query`, `refine_query`, and `traverse_graph` in the host, instantiate/reuse one `AgenticMemoryController` for the turn, pass host delivery identity into Reliquary preparation, and return prepared payloads through the normal tool-result channel. Do not expose completion as a fourth tool. | A real host turn can call all three tools with shared state and cannot directly call the internal completion API as a model tool. |
| **36** | **Sol** | **Wire actual successful handoff to completion and prove F3 end to end.** After the host's real success event, call the AR8 completion seam with the exact prepared manifest. Preserve/replay the same identity and manifest through host retry/restart. Test failed handoff, cancellation, success, exact retry, and restart replay. | Failed delivery produces zero accepted-use reinforcement; one successful delivery plus retry/restart produces one durable accepted-use effect. Only now may F3 be marked complete; AR9 is complete. |
| **37** | **Sol** | **Build the frozen-fixture acceptance harness.** Use copies/read-only opens of the authoritative 14-day, 28-day, and Ellis REL fixtures; record input hashes; build deterministic query/traversal/refinement/handoff scenarios and a broader-context evaluation harness without mutating source fixtures or requiring new general inference. | The same scenario runner can target all three corpora and can prove source fixture hashes before/after execution. |
| **38** | **Sol** | **Run functional retrieval scenarios across all three frozen RELs.** Verify Freshness changes top-ten presentation relative to raw retrieval; an anchor with >10 neighbors; repeated traversal revealing later neighbors; refinement changing relevance without fresh retrieval; shared-neighbor concurrent overlap; `already_provided`; failed handoff; and success + identical retry. | Every mandatory scenario passes on 14-day, 28-day, and Ellis, or an explicit corpus-data limitation is recorded without weakening the runtime contract. |
| **39** | **Sol** | **Run the broader-context fidelity comparison.** Compare answer/evidence recall from the agentic retrieval loop with a broader-context baseline on retained fixture tasks. Record retrieval calls, selected evidence, misses, latency, candidate counts, and any failure mode. The broad baseline is evaluation-only and never becomes a runtime mode. | A dated result explains whether the new loop preserves useful answer/evidence recall and where additional tuning is required; no runtime compatibility path is added. |
| **40** | **Sol** | **Close frozen-fixture evidence.** Recheck authoritative fixture hashes, capture the exact commands/results for Steps 37–39, and write/update the retrieval verification record without claiming unsupported host or fidelity evidence. | Fixture bytes are unchanged, all mandatory evidence is reproducible, and AR10 is complete. |
| **41** | **Luna** | **Update architectural/status documentation to shipped reality.** Update `architecture.md`, `current-limitations.md`, `roadmap.md`, and `maintainer-map.md` for the implemented controller, REL-only agentic scope, dedicated route, host-owned turn lifetime, graph behavior, and real F3 status. | Architectural docs describe only implemented behavior and do not imply PHY agentic retrieval or an additional tool/state store. |
| **42** | **Luna** | **Update Freshness/integration records and documentation index.** Reconcile the external-consumption contract, Freshness boundary plan/verification, agentic design/spec status, verification record, and `docs/INDEX.md`. Run both docs-policy checks and repair only retrieval-related findings; do not absorb unrelated Ego edits. | Retrieval docs are internally consistent, required sections exist, and AR11 is complete. Any remaining docs-policy failure is explicitly attributable to pre-existing unrelated files. |
| **43** | **Sol** | **Perform hard-cut cleanup and canonical-path audit.** Remove migration-only helpers/test shims, narrow accidental exports, and search source for duplicate agentic selection/ranking, a fourth tool, `SentMemoryIDs`, graph depth compatibility, passive-search adapters pretending to be agentic retrieval, alternate Freshness writers, or a controller registry. | One canonical agentic controller/candidate/decision/completion path remains; passive search is still separate and valid. |
| **44** | **Sol** | **Refresh Lexicon/Arcana and verify structural ownership.** Refresh semantic/graph state after cleanup; inspect impacts/call direction around the new controller, candidate collector, decision route, host gateway module, and `record_accepted_memory_use()`. Confirm no unexpected reverse dependency or surviving old consumer contradicts the hard cut. | Structural evidence agrees with direct source, expected ownership/call direction is present, and generated graph state is current; AR12 structural gate is green. |
| **45** | **Sol** | **Run the final repository integration and production-readiness gate.** Run `cargo fmt --check`, `cargo check`, `cargo test`, the full locked CLI fmt/check/test suite, both docs-policy checks, and `python scripts/check_architecture.py --refresh`. Review git status/diff, preserve unrelated Ego edits, record final Documentation impact and Architecture impact, and commit the completed implementation/evidence. | All required gates for the changed surface are green or a pre-existing unrelated failure is explicitly isolated; F3 status matches real host evidence; no frozen fixture changed; the agentic path is ready for production disposition. |

## Mapping back to AR1–AR12

| Specification milestone | Linear steps |
| --- | --- |
| **Baseline / graph refresh** | 1 |
| **AR1 — controller/state/result model** | 2–4 |
| **AR2 — decision-model route** | 5–9 |
| **AR3 — full candidate-pool primitive** | 10–13 |
| **AR4 — Freshness ordering + memory_query** | 14–18 |
| **AR5 — refine_query** | 19–21 |
| **AR6 — concurrent one-hop traverse_graph** | 22–26 |
| **AR7 — strict model contracts** | 27–29 |
| **AR8 — two-phase handoff completion** | 30–33 |
| **AR9 — real embedding-host integration / F3** | 34–36 |
| **AR10 — frozen fixtures + fidelity evidence** | 37–40 |
| **AR11 — documentation closure** | 41–42 |
| **AR12 — hard-cut cleanup + integration gate** | 43–45 |

## Verification cadence

During implementation, run the narrowest focused test set that proves each step, then expand at phase boundaries:

- Steps 2–4: controller/state unit tests.
- Steps 5–9: model switchboard/config/runtime-route tests plus detachable CLI tests.
- Steps 10–13: candidate-pool tests plus existing Memory retrieval regressions.
- Steps 14–18: query/Freshness/gateway authorization tests.
- Steps 19–21: refinement tests.
- Steps 22–26: graph traversal/concurrency tests.
- Steps 27–29: decision-contract/parser simulator tests.
- Steps 30–33: completion/accepted-use boundary tests.
- Steps 34–36: actual host integration tests; simulator evidence is insufficient for F3.
- Steps 37–40: frozen fixture and broader-context evaluation.
- Steps 41–42: docs-policy checks.
- Steps 43–45: full Rust, CLI, docs, architecture, Lexicon/Arcana convergence gates.

Do not substitute one giant final test run for focused phase evidence; failures should remain attributable to the seam just changed.

## Hard-cut prohibitions

The implementation is incomplete if any of the following is introduced or retained as an agentic compatibility path:

- `search_visible_memories()` wrapped to impersonate the new three-tool protocol;
- model-visible graph depth or recursive browse mode;
- a fourth model-visible delivery/acknowledgment tool;
- `SentMemoryIDs` or another persistent controller bookkeeping set;
- a GatewayRuntime/REL/PHY registry that owns turn-scoped controllers;
- a second accepted-use or Freshness reinforcement implementation;
- a second durable delivery journal created only to manufacture consumption proof;
- duplicate cosine/vector decoding or duplicate Freshness ranking policy;
- a persistent evaluation-status database;
- silent fallback from `memory_retrieval_decision` to entity resolution or general inference.

## Out of scope

This plan does not implement PHY Freshness or PHY agentic retrieval, change Freshness decay/propagation/scoring policy, add semantic access reinforcement, alter Ego's Memory-Web summary machinery, add recursive graph traversal, add ANN/cross-profile fusion, persist controller semantic state, or redesign passive Memory search.

## Completion gate

The implementation is complete only when:

1. the real embedding host exposes exactly the three accepted tools and shares one turn-scoped controller across them;
2. query and graph candidate pools are complete before the ten-candidate Freshness presentation cut;
3. refinement reconsiders the full declined set without rerunning ordinary retrieval;
4. graph traversal is one-hop, concurrent per anchor, deterministic after gather, and deduplicates shared content while retaining per-anchor relationship reports;
5. internal retrieval/model work produces no Freshness access reinforcement;
6. confirmed host delivery is the first `SelectedIDs`/accepted-use boundary;
7. exact retry/restart is idempotent through the existing accepted-use receipt authority;
8. 14-day, 28-day, and Ellis frozen fixture evidence is collected without modifying source fixture bytes;
9. passive existing search remains behaviorally intact and separate;
10. hard-cut cleanup finds no compatibility agentic path or second ownership system;
11. full Rust/CLI/docs/architecture gates pass for the changed surface; and
12. Lexicon/Arcana structural evidence agrees with direct source ownership.
