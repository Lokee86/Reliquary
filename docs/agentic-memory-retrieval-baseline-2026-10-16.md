# Agentic Memory retrieval Luna baseline — 2026-10-16

## Purpose

Freeze the Step 1 implementation baseline before agentic retrieval runtime changes.

## Overview

Step 1 is complete. Luna inspected the source and committed the initial attempt in `50ae1ff`; the primary executor repaired invocation/cache mechanics and collected the successful results below. No runtime implementation existed in the audited source.

## Snapshot

- Convergence branch: `feat/agentic-memory-retrieval` in `C:\!bin\workspace\Reliquary-agentic-retrieval`.
- Initial source HEAD: `596f5ceaac10b4a70ad7f8109dc5dc87cd92a018`. The subsequent `50ae1ff` baseline-note commit changed documentation only.
- Original `main` had exactly two unrelated dirty Ego documents; they were not copied into this isolated worktree and remain untouched.
- Refreshed Lexicon/Arcana snapshot: `sha256:0f00d82eb905356cedd4d2c27200817a415c1bf3625dc7f5f91e4c3ff5c872b8`.
- Arcana reports 74,737 nodes and 189,538 relationships, including 15,650 `calls` relationships. Source lookup correctly places `GatewayRuntime::search_visible_memories` in `runtime_host_gateway_semantic_search.rs`.
- Direct source search found no existing `AgenticMemory*` runtime/controller implementation.

## Ownership inspection

Passive gateway composition is in `src/runtime_inference/runtime_host_gateway_semantic_search.rs`; `runtime_host_memory_search.rs` is a shared-model re-export. Owner-local semantic retrieval lives in `src/retrieval/memory_retrieval.rs`, its index/build/vector modules, and `src/facade/cva_memory_retrieval.rs`. Complete lexical work will deepen `src/retrieval/lexical_index/memory.rs` and `lexical_search.rs`. Decision transport and routes are owned by `configured_decision.rs`, `configured_runtime.rs`, and `runtime_host_routes.rs`. Graph reads remain in `cva_graph.rs`. `freshness_access.rs` owns passive grades and the sole accepted-use receipt application.

## Commands and results

- Release-adapter `lexicon init --repo <convergence> --adapters <convergence>/.lexicon/release-adapters --languages rust`: passed, approximately nine minutes. A disposable packaged copy of the installed Rust release adapter avoided the slow debug invocation; only Rust was enabled. The adapter-root discovery sentinel is an empty `python` directory; no Python analysis was requested.
- `arcana sync --lexicon <convergence>/.lexicon --state <convergence>/.arcana`: passed, zero compatibility warnings.
- Arcana `stats` and `search_nodes` for `search_visible_memories`: passed and agreed with direct source placement.
- `cargo test --lib memory_retrieval -j 2`: 5 passed.
- `cargo test --lib model_switchboard -j 2`: 15 passed.
- `cargo test --test freshness_retrieval_boundary -j 2`: 3 passed.
- Documentation checks on the initial baseline record reported only the four pre-existing missing sections in `ego-status-decision-inventory-2026-10-14.md`; no retrieval finding was introduced.

## Failure and recovery

Delegated command windows first expired during analysis/building. A timed-out wrapper did not reliably terminate its child process; the primary executor inspected exact process identities before stopping the obsolete debug refresh. One test link then failed with LNK1181 because a generated `hashbrown` archive was absent despite existing fingerprints. `cargo clean -p hashbrown --target-dir <convergence>/target` removed only 21 generated cache files (3.4 MiB); the subsequent focused build and tests passed. No source workaround or dependency change was made.

## Related docs

- [Linear execution plan](agentic-memory-retrieval-linear-implementation-plan-2026-10-16.md)
- [Architecture](architecture.md)
- [Architectural invariants](invariants.md)
- [External Memory consumption](freshness-external-consumption-contract.md)

## Notes

This is baseline evidence, not acceptance of the agentic feature. The independent Luna AR2 route branch is preparation only until integrated in linear order. Generated tool state is disposable and uncommitted.

## Step 11 -- shared exact scorer

Extracted exact Memory scoring and query validation into `memory_retrieval_vectors.rs`. Passive retrieval still applies its existing routing, seed, and traversal limits after calling the shared scorer; packed decoding and cosine interpretation remain canonical.

Verification: `cargo test --lib memory_retrieval -j 2` passed all 20 matching tests; `cargo fmt --check` and `git diff --check` passed. The initial run exposed Windows access errors in two configuration tests using the system temporary directory; rerunning with `TEMP` and `TMP` set to the worktree-local `.test-tmp` passed. No product workaround was required. Steps 12-45 remain outstanding.

## Step 12 -- complete REL-local candidate pool

`Cva::collect_memory_candidate_pool` now unions complete lexical evidence and every searchable exact vector score by Memory ID in deterministic ID order. Missing vector bindings leave lexical eligibility intact. Query validation runs even for an empty vector population; known profile dimensions are enforced. The shared loader preserves existing searchable-population validation for passive retrieval. The pool performs no Freshness reads, graph traversal, inference, truncation, or reinforcement.

Verification: all 20 tests matching `memory_retrieval` passed with worktree-local temporary files; `cargo fmt --check` and `git diff --check` passed. Step 13 supplies the focused completeness/invalid-data coverage before proceeding to Freshness ordering.


## Step 13 -- candidate-pool verification

Added focused candidate-pool tests for a 16-item complete union, independent lexical and semantic evidence, shared-lane overlap, archived exclusion, deterministic ID ordering under score ties, lexical-only results for missing vectors or profiles, invalid query vectors, zero-norm and non-finite stored vectors, wrong-width bindings, and parity with legacy GlobalExact semantic scores. The baseline search still exposes its established limited seed result; the complete internal pool retains every eligible candidate.

Verification: `cargo test --lib memory_candidate_pool -j 2` passed all 6 focused tests; `cargo test --lib memory_retrieval -j 2` passed all 20 matching regression tests; `cargo check -j 2`, `cargo fmt --all -- --check`, and `git diff --check` passed. Tests and temporary paths were kept inside `target/test-tmp`. Steps 14-45 remain pending.

## Step 14 -- passive cross-owner Freshness ordering

The controller module now owns operation-local grading and ordering over complete owner-qualified candidates. It reads the existing `ego_memory_freshness()` at an explicit owner cut, supplies neutral zero only for missing records, and sorts descending score then owner ID/Memory ID. No grade is persisted in controller state, and Stale/Dormant rows remain eligible.

The focused `freshness_order_is_complete` test passed: 17 cross-owner candidates remain available, the last raw-ID candidate ranks first through Freshness, missing grades stay neutral, cold rows remain, identity collisions are distinct, and activity/version/record/file bytes are unchanged by the read. Future cuts fail. Formatting and diff checks passed. A missing generated Lore archive required a package-local cache rebuild; the initial test fixture also needed valid pinned-policy initialization and genuine activity-based decay. These repairs changed no production Freshness semantics. Steps 15-45 remain outstanding.

## Step 15 -- authorized REL read envelope

The gateway captures the requesting attachment, selected REL, independently granted dependency closure, owner tokens, active profiles, and activity cuts. Bound PHY and unrelated granted RELs are excluded. Route acquisition remains separate from owner reads; final read delivery revalidates selection, closure, grants, attachment and mount tokens. The existing attachment-delivery guard is reused with parent-module visibility rather than duplicated.

All three `agentic_read_envelope` tests passed, covering authorized dependencies, unauthorized dependency/unrelated REL/PHY exclusion, selection changes, detach, revoke/regrant, topology changes, and missing dedicated routes. Formatting and diff checks passed. Additional missing generated direct-dependency artifacts required Cargo to rebuild the test dependency graph; no dependency or runtime source workaround was introduced. Steps 16-45 remain outstanding.

## Step 16 -- normal Memory query preparation

`GatewayRuntime::memory_query` validates a staged query and stable host identity, obtains exactly one query embedding, collects every authorized REL pool, grades the complete union, excludes turn-selected refs, and presents at most ten candidates to the dedicated route. One strict versioned parser accepts only unique owner-qualified approvals from that exact batch. Content is prepared without access reinforcement; the new query/evaluated IDs commit only after final read-envelope validation succeeds.

Both focused `memory_query_` tests passed: all ten presented candidates can be approved from a 14-Memory population while SelectedIDs, activity and Freshness stay unchanged; an invented approval fails with the prior controller intact. Formatting and diff checks passed. Step 17 adds raw-match fallback semantics; Steps 18-45 remain outstanding.

## Step 17 -- raw-match fallback and result semantics

The query path retains its complete raw owner-qualified match set before selected exclusion. With zero newly approved content, its intersection with turn-selected refs yields `already_provided` IDs without content; an empty intersection yields `no_matches`. Newly prepared content suppresses old-match reporting. An all-selected batch never invokes the decision endpoint. Successful fallback still commits the new query/reset; preparation does not change SelectedIDs.

All five focused `memory_query_` tests passed, including selected-only fallback without a decision call, mixed old/new suppression, and empty-scope no-matches without a decision route. Formatting and diff checks passed. Step 18 remains the Luna AR4 coverage gate before refinement implementation.

## Step 18 -- Luna AR4 query and authorization gate

Two Luna sub-agents supplied focused Freshness/fallback and authorization/route tests. Coverage proves a Memory at raw semantic rank 14 enters the ten-candidate decision batch through Freshness, while neutral unenrolled legacy content and Stale/Dormant content remain eligible. Equal-score ordering is deterministic. Both all-selected/no-decision-route and declined-new/previously-selected fallback paths pass. Missing embedding, dedicated decision and active profile fail without state changes. Revocation and detachment inside a valid decision callback complete without held owner locks, invalidate the prepared result, preserve controller state, and leave owner activity/Freshness unchanged.

Verification: 14 query tests, three envelope tests and all 21 tests matching `memory_retrieval` passed; formatting and diff checks passed. Both documentation policy checks continue to report only the four pre-existing missing sections in the unrelated Ego inventory. Primary integration corrected a callback boolean/unit mismatch and calibrated the aging fixture to 1,700 accepted turns (Stale begins at -50). No production source change was required for this gate. Cargo temporary environment is fixed to worktree `target/test-tmp` for subsequent builds. AR4 is complete; Steps 19-45 remain outstanding, including host integration/F3 and frozen-fixture acceptance.

## Step 19 -- strict semantic rewrite preparation

The dedicated route receives the complete old query and refinement instruction and must return exactly one versioned non-empty complete replacement. Unknown fields/alternatives, lists, free-form text, version mismatch and oversized results fail closed. Gateway rewrite preparation requires an active query, captures/revalidates the read envelope, and returns a staged transition without changing CurrentQuery before reconsideration succeeds.

Both new parser tests and the gateway `refinement_rewrite` test passed (the broad rewrite filter also passed an existing Chronos regression). Correction changes baseball to basketball rather than mechanically appending. No-active-query, malformed rewrite and valid-but-uncommitted rewrite preserve controller state. Formatting and diff checks passed. Steps 20-45 remain outstanding.

## Step 20 -- full declined-set reconsideration

`GatewayRuntime::refine_query` rewrites once, computes `SearchedIDs - SelectedIDs`, hydrates exactly those refs, and reconsiders the entire set in deterministic batches of at most ten. Approved content across all batches is prepared together without a total ten-result cap. The revised query commits only after every batch and final authority validation succeed. Search and selected sets retain their existing meaning; no ordinary lexical/semantic retrieval or embedding route is used.

All three tests matching `refinement_` passed. The focused gateway case has no embedding route or active Memory profile, reconsiders 13 declined Memories in batches of ten and three, returns all 13 approvals, excludes the one already-selected Memory, retains both ID sets, and leaves activity/Freshness unchanged. Formatting and diff checks passed. Step 21 is the Luna AR5 failure/authorization coverage gate.

## Step 21 — refinement contract closure

Luna supplied six contract tests and three authorization tests. `cargo test --lib refinement -j 2` passes all 12 matching tests: complete correction, empty declined set, no active query, all declined batches, selected/unsearched exclusion, malformed rewrite/selection, provider failure, revoke/detach, and atomic rollback. No embedding route is configured in the reconsideration fixtures. `cargo fmt --check` and `git diff --check` pass.

## Step 22 — complete one-hop evidence

The authorized anchor collector reads active Memory relations in both directions within its REL, preserves kind/direction, deduplicates identical evidence, skips archived neighbors, and never expands a neighbor. Invalid owners and missing anchors fail. `cargo test --lib agentic_graph_evidence -j 2` passes; all 12 refinement tests also pass. Formatting/diff checks pass. Both documentation checks still isolate the four known Ego inventory gaps.

## Step 23 — per-anchor preparation

Unique neighbor counts use the pre-call controller snapshot. Both ID sets are excluded from new presentation; declined neighbors are counted separately. The complete remaining owner-local pool uses the canonical passive Freshness grader/order before truncation to ten. Selected and declined Memories remain legal anchors. Both `agentic_graph_` tests pass, including unchanged controller and counts around selected/declined bridges. Larger-neighborhood acceptance is delegated to Luna Step 26. Formatting/diff checks pass.

## Step 24 — concurrent gather

Independent scoped anchor workers use the same complete query and already prepared snapshot. Every worker is joined into an ordered result vector before failure propagation; provider/validation failures and worker panics return errors without controller mutation. Three focused graph tests pass, including a two-party rendezvous proving concurrent entry and both completions before a validation error returns. Formatting/diff checks pass.

## Step 25 — traversal consolidation

`GatewayRuntime::traverse_graph` validates and canonicalizes anchors, prepares against one unchanged controller, gathers concurrent decisions, unions evaluated IDs, and prepares globally unique approved content in stable ref order. Independent reports retain relations/counts/approvals; one approval wins shared-neighbor delivery. Conflicting shared body snapshots fail closed. Only successful final authority validation permits the staged searched-set commit; SelectedIDs is unchanged. All four focused graph tests pass, including duplicate-anchor normalization, shared-content/report retention and repeat exclusion. Formatting/diff checks pass. Luna Step 26 supplies the full AR6 gate next.

## Step 26 — Luna AR6 graph gate

Two Luna agents supplied four core graph tests and seven concurrency/authorization tests. All 15 tests matching `agentic_memory::graph` pass: >10 neighbors and Freshness ranking, repeat exposure, both directions and kinds, one-hop exclusion, selected/declined anchors, snapshot counts, overlapping approval union and unique content, inverted worker completion order, provider/validation/panic rollback after peer success, revoke/detach rollback, and invalid/no-query anchors. Three test-only integration corrections added the controller import and used approved MemoryRef membership directly. Controller and passive owner state remain unchanged on errors. Formatting/diff checks pass; AR6 is complete.

## Step 27 — Luna versioned selection contract

Named V1 operation/instruction/schema constants now freeze complete-query selection requests for all three operations. The existing single evaluator/parser remains authoritative: strict object shapes, exact version, supplied owner-qualified subset, and no duplicate approvals. Three new tests assert complete request/prompt/schema and invalid responses. `cargo test --lib agentic -j 2` passes all 58 tests, including these contracts and the complete query/refinement/graph regressions. Formatting/diff checks pass.

## Step 28 — Luna versioned rewrite contract

The dedicated rewrite request/instruction/schema are frozen with V1 constants. One strict response contains exactly version and complete revised query; no free-form fallback or alternative list exists. Parser validation retains the existing nonempty UTF-8 byte bound. `cargo test --lib rewrite -j 2` passed all 10 matching tests, including exact complete-old-query/refinement/prompt/schema assertions and malformed/version/alternative/commentary/oversized rejection. A subsequent assertion ties the schema length to the shared query limit; its focused rerun is pending in the current dependency rebuild and will be included before AR7 closure. Formatting/diff checks pass.

## Step 29 — Luna AR7 abuse gate

The simulator matrix rejects 12 selection response/error cases across query/refinement/traversal (36 cases), plus seven rewrite failures. Cases include malformed Value shapes, missing/extra fields, wrong identity type, valid unknown Memory ID with correct owner, stale owner, duplicate approval, version mismatch and provider errors. Rewrites reject empty/oversized/multi-result/free-form/extra/version/provider failures. Every case compares the whole controller clone and returns no prepared result. The Step 28 length-limit assertion is verified by the focused two-test rewrite rerun. The resulting test executable was then run directly with `agentic --quiet`: all 60 tests pass, including both table-driven abuse tests. This avoids an unnecessary Cargo dependency rebuild while executing the newly compiled tests. Formatting/diff checks pass; AR7 is complete.

## Step 30 — canonical ephemeral delivery manifest

Payload JSON and the host manifest are canonicalized recursively with stable object keys and observable array order. SHA-256 binds the exact model-visible payload, exact delivered refs, per-owner accepted-turn cuts, and host delivery identity. Shared owner-qualified identity encoding now lives with the manifest rather than the decision parser. A distinct trusted handoff-confirmation enum represents success/failure/cancellation/unknown; preparation cannot manufacture successful evidence. No delivery store or journal was added. Both focused manifest tests pass, proving deterministic digest and changes for each bound input plus content-free fallback. The resulting executable passes all 62 agentic tests. Formatting/diff checks pass.

## Step 31 — post-handoff SelectedIDs boundary

The trusted completion API accepts only the exact success identity/digest, verifies included-content/ref/cut consistency, and revalidates the current instance/REL closure before extending SelectedIDs. Other handoff statuses, mismatches and malformed manifests fail without state changes. CurrentQuery/SearchedIDs are preserved; successful retry deduplicates SelectedIDs. Three focused completion tests pass, covering all statuses, identity/digest/content mismatch, inconsistent refs, exact selection and retry. Formatting/diff checks pass. Receipt recording remains Step 32; this API is not registered as a model tool.

## Step 32 — existing owner-grouped accepted-use receipts

Successful completion groups exact delivered IDs by durable REL owner, derives a stable owner-specific use ID from the host invocation identity (independent of payload/cut), and applies the captured owner cut through existing `Cva::record_accepted_memory_use`. A narrowly typed trusted-consumption mutation family requires the exact ViewRel capability and reuses normal owner admission/sequencing/sync/notifications; management grant rules are unchanged. Successful receipt effects invalidate retrieval, exact replay does not append or notify, and uncertain receipt failures invalidate the distribution cut. All owners are attempted before returning the first receipt error. SelectedIDs remains committed after actual delivery even when a receipt fails; no missing Freshness enrollment is fabricated. Four focused completion tests pass. The resulting executable passes 66 agentic, 13 mutation-event and two accepted-use regressions; the new receipt test verifies durable receipt lookup and unchanged bytes/records/activity on exact retry. Formatting/diff checks pass. Step 33 is the Luna AR8 gate before host integration.

## Step 33 — Luna AR8 completion gate

Luna supplied five core and six authority/boundary tests. All 15 completion tests pass: multi-owner grouping, exact retry, original-confirmation mismatch, changed refs/cuts under reused identity, receipt failure then retry, missing Freshness without enrollment, revoke/detach/scope/unmount, unsuccessful handoffs, and proof that all three internal preparation operations remain passive. The retry test caught an over-broad distribution fence in Step 32: no-write receipt validation failures must remain retryable. The coordinator now compares the receipt backing-file extent on error and fences only changed/unknown extent (or existing source-cut changes). A new coordinator fault-injection test proves both safe no-write retry and fencing after an append followed by failure. SelectedIDs remains committed after confirmed delivery despite receipt errors. A fixture timestamp conversion and unused import were corrected during integration.

Verification: focused completion 15/15; newly built executable `agentic --quiet` 77/77, `mutation_events --quiet` 14/14, `accepted_use --quiet` 2/2. A missing cached PEM archive caused one linker failure; regenerating its cache marker/dependencies restored the build, with no product source workaround. Formatting/diff checks pass. Both docs checks still report only the four known unrelated Ego inventory gaps; main checkout retains exactly its original two Ego edits. AR8 is complete. Real host integration/F3 remains pending Steps 34–36.

## Step 34 — real host source and missing primitive frozen

The read-only Warlock trace identifies registration, invocation, generic Echo recording, actual requesting-context exchange insertion, provider serialization, immutable task authority and available turn/call identity. The current host lacks an exact receiver-owned success/manifest replay primitive; generic Echo, provider responses, HTTP status and ConversationExecution are explicitly rejected as substitutes. Requirements and exact source paths are frozen in [host handoff seam](agentic-memory-host-handoff-seam-2026-10-16.md). Integration will branch from the multiplexed Warlock candidate `c960e42`, a descendant of main `4da92b3`; dirty main graph/UI edits and existing verification artifacts remain untouched. F3 remains incomplete. The documented target cleaner explains disappearing build archives; the active Reliquary cache is now protected by an ignored worktree-local `bin/agentic-cargo-cache` destination and `target` junction, with no machine policy change. Documentation/diff review passes; Step 35 begins only after this missing primitive is recorded.
