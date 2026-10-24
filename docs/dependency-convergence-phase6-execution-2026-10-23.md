# Phase 6 public facade execution and verification

Date: 2026-10-23. Integrated checkout: `17514f007e48e81d5ea1ec7f41088564de9e7411` (`main` documentation receipt).

## Purpose

Record the integrated Phase 5 source and public-contract baseline for the Phase 6 facade-boundaries specification, complete the P6.1.16 residual named-domain census, and review P6.2 construction/runtime/consumer seams. The opening census sections describe the frozen before-image; the implementation and verification sections below record the later working changes. Acceptance remains separate from source implementation.

## Overview

Phase 5 implementation remains pinned at `9a3678f` (Rust source `fe9948e`); its final evidence commit is `17514f0`. The execution receipt explicitly limits A5: library/CLI/corpus/docs checks passed, but the required full architecture refresh did not publish a snapshot. Carry that graph-proof limitation to Phase 7 A7; it does not block this B6 census.

The full source-location record, root API export witnesses, Perception denominator, named candidates, P6.2 seam findings, and evidence limits are in [phase6-baseline.json](../benchmarks/phase6-baseline.json). The Perception machine register remains 82 exact bounded deferred rows. This census reconciles zero of those rows and claims zero eliminated; P6.1.17 still owns their row-by-row source witness disposition.

## Public and source baseline

- Supported `Cva` and `Phylactery` entrypoints remain exported from `src/lib.rs`; `docs/api.md` documents open, read-only open, explicit repair, reconciliation, rewrite, and import contracts.
- `ConfiguredRuntime` offline methods and DreamProcessor/DreamCommunityNamer offline methods remain supported per the Phase 5 execution receipt.
- Gateway/Instance and CLI are supported consumers. The CLI is a detachable package consuming the library via its public API (`cli/Cargo.toml`).
- The baseline JSON records every raw non-test-filename `impl Cva`/`impl Phylactery` source match, plus root export matches. This is a reproducible source-location inventory, not a count of public API methods: inline test blocks may be present and the syntax census is not a symbol graph.
- The known Perception `entity_principal.rs` extension is already covered by P6.1.03 and is not duplicated into residual work. Phase 4's `src/facade/cva_project_environment.rs` remains upper composition; do not duplicate or undo it.

## P6.1.16 residual census and pre-execution child tickets

The source review finds additional independent public extension/composition placements outside the named Perception/Archive/Retrieval families. The following bounded child tickets are recorded before execution and added to the Phase 6 denominator; none is started by this census:

| Child ticket | Owned source family | Bounded cut |
| --- | --- | --- |
| P6.1.16-DREAM | `src/runtime_inference/dream_{candidates,cooldown,lifecycle,freshness,publisher,temporal}.rs` | Move public `Cva` extensions and façade orchestration upward; keep Dream policy in `dream_owner_candidates` / `dream_canonical` and owner modules. Audit each call direction; preserve existing Dream behavior. |
| P6.1.16-EXTERNAL-SOURCE | `src/external_source/{provenance,processing,mod}.rs` | Place public evidence accessors and owner composition at the facade; retain External Sources validation/preparation policy below. Do not introduce another publisher/store. |
| P6.1.16-REWRITE-LINEAGE | `src/rewrite/lineage.rs` | Review `Cva`/`Phylactery` extensions against public rewrite exports; move public adapter glue to facade while keeping lineage policy in Maintenance. |
| P6.1.16-RETRIEVAL-PRIVATE-HELPER | `src/retrieval/memory_candidate_pool.rs` | Review the `pub(crate)` `Cva` collector that combines lexical/vector/store evidence; move composition upward or replace with a narrow accepted owner read contract, with no public API expansion. |

Evidence: `archive_search.rs` still declares `impl Cva` and `search_archive`; Retrieval's six named source files still contain public extensions; the residual source families and examples are recorded in the baseline. These are separate owner/source families and should not be collapsed into an unbounded “residual domains” ticket. P6.1.16's additional work is four children beyond its original census-only scope. P6.1.11–15 remain the owning tickets for Archive/Retrieval and P6.1.03 remains the Perception-principal owner.

## P6.2 construction, runtime, and consumers

- **Construction/open:** `Cva`/`Phylactery` exports and documented creation/open behavior are public contracts. The reviewed Gateway mount call at `runtime_host_gateway_mount.rs:187` invokes `Cva::open` from upper composition. No lower owner calling a public Cva constructor was witnessed. Keep construction upper and verify lifecycle delegation against owner-lifecycle contracts in P6.2.1/2.
- **Mounted Runtime:** `runtime_host_gateway.rs:1445` returns the public Cva at unmount, and `runtime_host_rel_execution.rs:276` reconstructs the public owner at the host boundary. These are upper composition edges. Phase 5 mounted execution hard cuts remain intact; do not add a facade middleman between Gateway and owner execution.
- **Offline consumers:** Preserve the Phase 5 documented offline methods. In P6.2.4, inspect their concrete calls to ensure they enter canonical owners directly and no lower service calls back through the public product facade.
- **DTOs versus callbacks:** Owner-qualified IDs, profiles, errors, and replay/result DTOs referenced by History/Import/runtime are value contracts; a path such as `crate::CvaError` or `PhylacteryProfile` alone is not evidence of a callback. Preserve owner-qualified DTO ownership; do not relocate types to generic `shared` modules.
- **Concrete callback candidates:** `runtime_inference/dream_freshness.rs` contains a private `commit(self, &mut Cva)` path and runtime-host helpers pass owner values into sessions/workers. These require callsite-direction review. A method implemented as an extension on `Cva` is a placement issue; a lower service invoking a public Cva operation is a callback violation. The available syntax census cannot prove the latter absent.
- **Public exports/API:** `src/lib.rs` keeps `Cva`, `Phylactery`, ConfiguredRuntime, Gateway/Runtime and owner/domain types exported. Remove only internal aliases proved unused; preserve signatures and CLI behavior. No export was deleted or edited in B6.

## Gate disposition and limitations

B6 source census complete for the inspected candidate families. P6.1.16 has four named child tickets added before their execution. P6.1.17 remains open: reconcile all 82 Perception rows with actual witnesses and classify absent versus genuinely owner-qualified values. P6.2 remains an audit plan, not an accepted no-callback proof. No code changes or verification runs were performed. This record does not claim Phase 6 edge elimination, full graph closure, or A5 acceptance.

## Related docs

- [Phase 6 facade boundaries specification](dependency-convergence-phase6-facade-boundaries-spec-2026-10-23.md)
- [Phase 5 execution receipt](dependency-convergence-phase5-execution-2026-10-23.md)
- [Phase 5 baseline](../benchmarks/dependency-convergence-phase5-baseline-2026-10-23.json)
- [API reference](api.md)
- [Architecture](architecture.md)

## Notes

This is dated execution evidence, not a replacement for current API/architecture documentation. Source classification is not edge elimination. The absent Phase 5 architecture snapshot remains an explicit Phase 7 graph-proof obligation.

## Integration work log

The fresh production import baseline records 3,376 mapped cross-component imports and 96 uncertain imports in [the before-image](../benchmarks/phase6-before-imports.json). The inherited current-source checker failed because the deferred-edge register did not pin the integrated map; the same cfg-aware resolver captured the before-image directly. No analyzer snapshot is claimed.

P6.1.16-AGENTIC-GRADING is an additional bounded child: `agentic_memory_retrieval::grade_agentic_memories` passed a public Cva into lower Retrieval. Its existing grading composition is moved to `facade/agentic_memory_grading.rs`, leaving ranking/order policy below and updating exact host/test callers. This child was recorded immediately after the edit rather than before it; that granularity-procedure deviation is disclosed. It does not add or change public API.

### Additional bounded supporting cuts

Before implementation, P6.1.03a covers deterministic principal policy over owner stores and Archive source facts; P6.1.06b covers concrete resolution read/write contexts and public adapters. P6.1.10a covers candidate graph routing and P6.1.06a covers concrete evidence hydration. These do not expand public API. Runtime offline extension families found by the placement guard are separately inventoried in the residual receipt.

P6.1.16-DREAM-FRONTIER is a bounded placement child for supported DreamProcessor offline concurrency composition currently in `runtime_inference/dream_processor_frontier.rs`. Public methods gather owner capabilities above the lower evaluation policy. This path is moved to facade under its existing module identifier; inference behavior is preserved.

## Implemented source boundary

The integrated source moves supported public extensions under `src/facade/` and removes their obsolete lower registrations. Perception audit, candidate routing, evidence hydration, resolution preparation/commit, reconciliation preparation/commit/round policy, principal identity policy, merge and resolution-state admission use operation-specific concrete store contexts below composition. Retrieval keeps lexical/semantic selection, conversation branch search and ranking below its adapters. External Sources reads a concrete vector store rather than receiving a facade callback. The supported offline Dream/Insomnia/vector/Freshness adapters remain upper; their owner algorithms and value contracts remain lower.

The [API preservation manifest](../benchmarks/phase6-api-preservation.json) matches all 53 explicit public signatures in the changed named method families. The detachable CLI build/tests remain a separate required consumer gate. Public `EntityResolutionEngine` and `EntityResolver` method-parameter contracts are preserved; they cannot truthfully be called eliminated or immutable values.

The [independent review](../benchmarks/phase6-review.json) found and corrected two callback inversions (External Sources vector reads and Perception audit) and a terminal-resolution error-order regression. Source reviews of principal privacy, resolution admission and merge/retarget publication retain their original checks and ordering.

## Gate state at implementation checkpoint

| Gate | State | Protecting evidence |
| --- | --- | --- |
| B6 | Source census complete | Frozen Phase 5 HEAD and 3,376 original cross-component import rows; 96 uncertain imports |
| F6 | Source cuts implemented; exact public-contract exceptions pending | Per-family source receipts and the original 82-row disposition register |
| C6 | Source and public-signature review complete; consumer tests pending | Concrete owner contexts, original project environment boundary, 53-signature manifest |
| D6 | Source guards pass; behavior regression run in progress | 839 reachable production files, zero facade boundary violations; 14 negative/positive fixtures; owner/runtime/history/physical guards pass |
| A6 | Not accepted at this checkpoint | Final tests, corpus, documentation and integration receipts remain required |

The full architecture entrypoint passed its local source guards and the retained-snapshot Pitlord policy check, then failed the convergence freshness check because Rust sources differ from the old indexed graph. No refreshed snapshot or full source-graph acceptance is claimed. The specification expressly forbids repeating the stalled Phase 5 refresh; the inherited full graph-proof obligation remains named for Phase 7 A7.

## Exact contract conflict

P6.1.17 permits original rows only to be absent or retained as genuinely nonviolating owner-qualified immutable values. The API preservation invariants simultaneously require public signatures accepting `EntityResolutionEngine`/`EntityResolver`, which are domain-service contracts, not immutable DTOs. Exact signature/call witnesses are retained in the [82-row disposition audit](../benchmarks/phase6-perception-dispositions.json). A bounded exception decision has been requested; no timeout or source relocation is treated as acceptance. The target DAG and inherited deferred-edge register are not silently broadened.

## Documentation impact

- Inspected: controlling specification, integrated Phase 5 receipt, architecture, invariants, storage format, maintainer map, current limitations, roadmap and API/consumer contracts.
- Updated: architecture/API/source ownership, maintainer and coverage maps, Perception/Retrieval/agentic subsystem boundaries, behavioral preservation, current limitations, roadmap, active specification headings/index and this receipt.
- Not affected: durable V2 schema, public serialized IDs, supported signatures, CLI behavior and Project Environment validation authority.
- Compliance check: tracked-document acceptance remains to be finalized; scoped Archivist indexes/links pass when excluding the unchanged unrelated untracked Phase 7 draft.
- Known documentation gaps: installed Pitlord does not support `docs`; no documentation-ownership pass is claimed. The preserved untracked Phase 7 draft causes the unscoped document checker to report its pre-existing missing headings/index.

## Architecture impact

- Standards added or changed: none; the target DAG remains unchanged.
- Ownership or boundary impact: concrete lower owner/domain policy with upper public adapters; no generic service, extra runtime or duplicate store.
- Pitlord enforcement impact: cfg-aware facade source ratchet is integrated into the existing architecture entrypoint and checks production imports, direct/macro extension placement and exact producer-value exceptions.
- Other verification impact: same-map production import evidence, explicit source uncertainty and independent callback/API review; serial locked Rust/CLI/corpus verification is recorded separately.
- Known architectural gaps: stale full analyzer snapshot and unresolved public service-signature exception decision; neither is counted as a passed gate or reclassified as Phase 7 implementation work.

## Expanded task denominator and final source audit

The [task ledger](../benchmarks/phase6-task-ledger.json) retains the original 35 named tickets and explicitly records 16 supporting/discovered children: 51 tracked parent-plus-child obligations, not 51 independent leaf migrations or tests. The named residual families are Dream, External Sources, Rewrite lineage, Retrieval candidate collection/runtime callers, Entity/Relationship adapters, Freshness access, Insomnia offline adapters, vector preparation, agentic grading and Dream frontier composition. Four further operation-specific children cover principal identity, evidence hydration, resolution owner admission and graph candidate routing. The agentic grading child was recorded just after its edit; that procedural deviation remains explicit.

The final [82-row audit](../benchmarks/phase6-perception-dispositions.json) distinguishes 46 implementation imports absent from facade sources, 33 immutable-value rows backed by 15 exact producer-value declarations, and three preserved public service signatures (two `EntityResolutionEngine` signatures and one `EntityResolver`). There are no unclassified rows. The three service contracts are accepted as exact existing public façade → Perception domain-service parameters, not lower-to-public callbacks and not immutable DTOs. Their current signatures and original source rows are pinned in `benchmarks/phase6-perception-dispositions.json`; no target DAG allowlist expansion is required. The 82-row register therefore contains 46 absent implementation imports, 33 owner-qualified immutable values, and three justified downward public service contracts. This source classification resolves the specification conflict only; it does not claim a complete call graph or final A6 test acceptance.

Final root review also corrected rejection-reconsideration recovery: all principal/preparation/evaluation/commit failures reach the original rejected-state restoration branch, and the entity snapshot is captured once before iteration. One focused failure-and-reopen regression test protects that behavior. No broad new feature suite was introduced.

P6.2.4-OFFLINE-CONFIGURED records the final physical placement of ConfiguredRuntime historical, Insomnia and vector offline methods in facade children. The method-body hashes match their original modules; owner-qualified immutable options/reports remain in lower model modules. A test-only Retrieval forwarding shim and an unused resolution read-context getter were deleted after callers were hard-cut.

## Exact public service-signature decision

Accepted three bounded **public-to-Perception** API method parameter contracts (two `EntityResolutionEngine` and one `EntityResolver`) with the pre-existing source witness inventory. These are not lower-to-facade callbacks and do not change the target DAG or relax the forbidden upward dependency guard. P6.1.17 classification issue resolved; all 82 original rows have explicit dispositions. The separate D6/A6 build, corpus, documentation and integrated source acceptance gates are not implied passed by this disposition.

## Final conformance correction: Container-neutral owner contracts

Before additional source work, the final DAG audit records P6.3.2-OWNER-READ-CONTRACTS (Memory/Archive/Vector read views plus Retrieval consumers), P6.3.2-RESOLUTION-PLANS (Container-free candidate/evidence/preparation and pure resolution/principal/admission policy with upper native publication), and P6.3.2-RECONCILIATION-PLANS (pure merge/reconciliation intents with upper owner publication). The source before-image has ten newly exposed Perception-to-Container imports, a component edge absent at B6 and denied by the frozen DAG, plus three newly located Retrieval-to-Container imports on an already-existing component edge. These are actual unresolved conformance findings, not accepted exceptions or Phase 7 implementation work.

The correction binds physical reads within their canonical store owners and leaves publication orchestration above pure Perception policy. It must preserve the precise validation/error sequence, alias/retry behavior and ordered appends, with no Container alias, generic facade service, callback into Cva, or ownership relabeling. Public Cva/Phylactery signatures and durable bytes stay unchanged. New work runs in isolated local worktrees pinned to the verified implementation checkpoint; root owns integration, registration/map changes and final tests.
