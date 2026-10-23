# Dependency convergence — Phase 1–3 production-source closure audit

Date: 2026-10-23  
Status: **diagnostic complete; corrective implementation not authorized**  
Source checkpoint: `448d77a` (Phase 3 accepted locally). Historical source: `c43d33c` (Phase 0).  
Parent: [Dependency-convergence campaign](dependency-convergence-architecture-refactor-2026-10-21.md)  
Machine evidence: [Phase 1–3 audit receipt](../benchmarks/dependency-convergence-phase1-3-closure-audit-2026-10-23.json)

## Purpose

Determine why Phases 1–3 produced little apparent reduction in the raw Arcana strongly connected component (SCC), test their actual exit criteria separately, and identify bounded architectural corrections versus explicitly deferred Phase 4–7 work. **This is an audit, not a new refactor phase or implementation authorization.** Do not recapture the Phase 0 immutable before-image, alter existing Phase 1–3 acceptance task denominators, or reopen the accepted development format.

## Overview

**Conclusion:** The raw 22-area graph is the wrong *sole* acceptance measure for Phase 1–3. It combines tests, same-name analyzer bindings, immutable identity/value contracts, owner stores, owner lifecycle, public extension implementations, services, and orchestration in broad pathname areas. A bidirectional area relationship is not necessarily an implementation dependency cycle between the target 34 components. Nevertheless, real service↔facade and orchestration↔facade coupling survives and the prescribed **source-conformance proof for the complete 34-component target DAG is missing**. The recorded Phase 2 promise of a *materially reduced central SCC* was never demonstrated with a comparable production-component metric.

### Reproducible comparison: one policy, two snapshots

| Evidence | Phase 0 snapshot | Current Phase 3 snapshot |
| --- | ---: | ---: |
| Arcana full cross-area evidence, five relations | 9,537 | 10,468 |
| Unfiltered directed area pairs, five relations | 146 | 201 |
| Unfiltered largest area SCC, five relations | 16 | 19 |
| Unfiltered directed area pairs, imports/depends-on | 84 | 118 |
| Unfiltered largest area SCC, imports/depends-on | 15 | 19 |
| Candidate non-test edges after explicit cfg/test-source exclusion, five relations | 2,991 | 3,738 |
| Candidate non-test directed area pairs, five relations | 106 | 144 |
| Candidate non-test largest area SCC, five relations | 16 | 19 |
| Candidate non-test directed area pairs, imports/depends-on | 52 | 81 |
| Candidate non-test largest area SCC, imports/depends-on | 10 | 16 |

The full cross-area source evidence was retrieved from the existing immutable Arcana snapshots using the *same current* 22-area policy and relationship selection. The candidate non-test filter excluded file-named test modules, explicitly `#[cfg(test)]`-gated external and inline blocks and annotated nodes, using the corresponding historical and current source. The filter rejects **6,546 of 9,537** historical relations and **6,730 of 10,468** current relations.

**Interpretation limits:** The filter is conservative source inspection, **not an exact Rust compilation/cfg profile**; call bindings can still be analyzer artifacts; source identity and ownership changed; the current 22-area classification is intentionally broader than the 34 logical components. The candidate graph is therefore an instrument for investigation, not proof of a true compiled production SCC. Crucially, the imports-only candidate area SCC remains large, so test filtering alone does **not** explain the unresolved knot. Do not advertise either set of SCC counts as the number of real runtime dependency cycles.

### Contract assessment by phase

| Contract | Evidence and verdict |
| --- | --- |
| **Phase 1: physical storage must not depend upward on semantic/facade/runtime implementation** | **Supported within the checked boundary.** The eight previously source-confirmed Container→upper-area findings are all physically located in `src/container/*_tests.rs` test modules. The physical negative/positive source guards pass, as do ordinary-open and owner replay contract tests. Raw Arcana calls to Archive from Container do not establish a live physical callback. No Phase 1 code repair identified by this audit. |
| **Phase 2: durable stores operate through owner-local contracts, not facade/runtime implementation** | **Supported for the registered store import subset, not exhaustively for every compiled edge.** The current reviewed owner-boundary map registers 405 source files with component ownership, including 247 files belonging to the twelve `kind=store` components. Across both ends of that store-file subset, 56 cross-store `imports/depends-on` edges were found; **all 56 match explicit `producer_value_contracts`** for immutable IDs, errors or proof values. No other direct store-to-store imports were observed in the mapped subset. `scripts/check_owner_boundaries.py` and its negative fixtures pass. Calls, unregistered modules, value-contract misuse and non-store orchestration remain distinct unproven surfaces. |
| **Phase 2: central SCC materially reduced** | **Unproven at acceptance and not demonstrated by the coarse graph.** A proposed 34-component DAG was validated as acyclic, but the validator does not derive that DAG from the source. A comparable production-only **component** SCC series was not recorded when Phase 2 was accepted. This is a specific verification failure, not proof the owner-store extraction was wasted. |
| **Phase 3: History read-only, Maintenance/Import composition downward, no forbidden callback** | **Supported by local source guards and corpus/integration acceptance, with mapping ambiguity retained.** History consumes `imports/epoch_model.rs`, `epoch_store.rs`, and `epoch_codec.rs` for evidence reconstruction; Import consumes History proof/read types. These are different component roles despite an apparent History↔native-import *area* cycle. Verify the distinction in the future full component source graph rather than treating all path-prefix edges as a newly introduced service callback. No direct contrary source example established in this audit. |
| **Later phases: remaining facade/runtime knot** | **Definitely not complete and explicitly deferred.** Real code dependencies remain between broad Runtime and `Cva` and between public facade methods and Runtime EmbeddingEndpoint types. Phase 5 owns runtime/orchestration convergence, Phase 6 facade/export convergence, Phase 7 final graph elimination and policy closure. These cannot be charged to earlier phases solely because the giant raw area SCC persists. |

### Concrete source evidence explaining the discrepancy

1. **Legitimate immutable-owner value references look like owner-to-owner cycles.** `src/archive/file_memory_link_model.rs:1` imports `MemoryId` while `src/memory/memory_model.rs:1` imports `EpisodeId`; both are explicitly accepted as immutable producer values by `tools/pitlord/owner-boundaries.json`. Similarly, Graph carries Perception `EntityId`, and Memory contains the accepted immutable `MemoryEntityMentionKey`. Counting these type references as forbidden store implementation cycles invalidates the intended owner-store measurement.
2. **Facade extensions reside in lower-looking pathname areas.** `src/archive/archive_search.rs:1` imports `Cva` and declares `impl Cva`; `src/retrieval/conversation_search.rs` likewise implements `Cva`; `src/perception/entity_merge.rs` implements owner-level public merge methods. These are genuine source relationships but are public composition code located under broad Archive/Retrieval/Perception directories, **not evidence that the underlying owner stores invoke the facade**. Their physical relocation/contract extraction belongs with Phase 6 if needed to enforce the final component graph.
3. **Concrete current facade↔runtime imports remain.** `src/runtime_inference/configured_runtime_historical.rs:4` uses `Cva`, while `src/facade/cva_memory_vectors.rs:3` consumes `EmbeddingEndpoint`. After candidate test filtering the area graph still contains **35 runtime→facade** and **27 facade→runtime** import edges. The Phase 5/6 hard cuts are therefore real outstanding work; rewriting an owner store is not the solution.
4. **Persisted value/owner modules still sit under Runtime.** `src/retrieval/compatibility_profile_model.rs:1` imports `EmbeddingMode` and `VectorNormalization` from `src/runtime_inference/embedding_endpoint.rs`; these exact values are currently permitted exceptions in the owner contract. `src/owner_lifecycle/memory_ordered_replay.rs:2–3` consumes the **Insomnia store and completion codec** currently located in `src/runtime_inference/insomnia/`. Classify these as store/value components, not wholesale runtime callbacks; confirm implementation versus type-only dependencies at the Phase 5 extraction seam.
5. **Phase 3's History/Import cycle is a component granularity issue unless further evidence shows otherwise.** `src/history/completion_proof.rs:5–7` imports the Import epoch *store/model/codec* to read historical evidence, and `src/imports/completion_coordinator.rs` imports History proof types. Owner store→History and Import service→History are not the same edge. A component-correct map must distinguish `import-store` from `import`. The History read-only source guard currently passes.
6. **The checker proves the target drawing, not complete source conformance.** `scripts/check_component_dag.py` checks the acyclicity/rank of the hand-authored 34-node, 163-edge `convergence-component-dag.json`. The Pitlord area rules and reviewed fingerprint ledger are valuable *incremental ratchets* but do not alone verify the complete 34-component source graph. Full source mapping of non-test modules and proof-level distinction between immutable values and capabilities are the missing connection.

## Root cause and fault classification

**Primary planning/instrumentation problem (confirmed):** Progress was credited against locally passing extraction/guard/corpus tasks and a mechanically valid *target* DAG. The campaign lacked a versioned, production-only *actual* 34-component DAG at every phase gate. Coarse-area SCC counts were reported later without accounting for type contracts, facades implemented in owner-named trees, or cfg(test). That makes the apparent lack of graph movement partly a measuring problem.

**Expected incomplete work (confirmed):** Phases 4–7 intentionally defer Project Environment, Runtime↔facade, and final graph cuts. Those connections were not removed by Phases 1–3.

**Earlier-phase implementation defect (not yet proved):** Direct source inspection and approved owner contracts did **not** uncover a specific unapproved store-to-store production import in the 247 registered store files, or a live Container→semantic callback. It remains possible that an unregistered module or runtime call violates those contracts. Treat this as a testable open question, **not as permission to reopen or rewrite Phases 1–3**.

## Bounded correction / closure register

These are audit findings and follow-up verification cuts; **not additional Phase 1–3 implementation tasks** and not a request to rerun the 1,988 library tests.

| ID | Responsibility | Required evidence / action | Exit condition |
| --- | --- | --- | --- |
| **C-01 (blocking, instrumentation)** | Source-map acceptance | Produce a complete Rust-cfg-aware mapping from production source modules/symbols to the existing 34 target components. Explicitly separate store, service, facade-extension and runtime boundaries; report unmapped code rather than infer approval. Use existing Lexicon/Arcana snapshots, no fresh corpus/inference loop. | Source coverage measured; every production cross-component edge has stable source proof or explicit analyzer uncertainty. |
| **C-02 (blocking, graph)** | Comparable SCC metric | Compute before/current *component-level* SCCs under the same component map and relation selections; show strict imports, call edges, allowed immutable-value references, verified capability calls and questionable analyzer edges separately. Keep original frozen Phase 0 captures untouched. | Reproducible cycle witnesses, counts, and direct accepted/deferred disposition for every remaining cycle-forming production edge. |
| **C-03 (blocking, Phase 1–2 closure)** | Owner/physical guards | Apply current negative physical and owner guards to the comprehensive source component inventory; verify extra/unmapped files and every approved immutable value exception against the actual use. Do **not** infer defect from file path alone. | Either concrete missing Phase 1/2 cut with exact source path and correction, or documented evidence that earlier boundaries hold; no generic expansion. |
| **C-04 (blocking, Phase 3 closure)** | History/Import classification | Split Import store/model/codec from Import service and owner lifecycle; verify History has no upward service callback and Maintenance/Import invokes only permitted lower contracts. | Explicit proof for History↔Import and related composite cycles, with narrow repairs only if a true forbidden edge is found. |
| **C-05 (phase deferred)** | Runtime/service physical layout | Carry owner Dream/Insomnia value types, EmbeddingEndpoint contract placement and runtime orchestration callbacks into the **existing Phase 5** plan where required; avoid migrating harmless value types solely to improve an area-level count. | Phase 5 exit enforces runtime downward direction on actual source. |
| **C-06 (phase deferred)** | Public facade composition | Track `impl Cva` and `impl Phylactery` code inside Archive/Retrieval/Perception/Runtime module trees as **Phase 6** public-composition dependencies. Keep supported API behavior until that phase. | Phase 6 exit removes unapproved owner/service→facade imports and calls. |
| **C-07 (blocking handoff gate)** | Sequencing/ratchet | Add C-01–C-04 to the prerequisite evidence checkpoint before **Phase 4 implementation**, not as a new phase. Retain all accepted source/corpus receipts and original task totals. Escalate only concrete verified missing cuts. | Phase 4 starts with a verifiable completed-layer graph and an explicitly classified deferred-edge register. |

## Standards / contract review

**Architecture standard:** PASS for the sampled physical-store and registered value-only store-import boundaries; **INCOMPLETE** for total production graph conformance. No source change to architectural policy was performed as part of this audit.

**Original Phase 1–3 specifications:** Functional/local acceptance is supported by their existing tests, source guards, and six-file oracles. Phase 2's separate **SCC-shrink exit assertion is NOT independently verified**; the acceptance methodology omitted the production-component comparison. Phase 3 D3's classified debt report does not substitute for that assertion. The full corrected source mapping must determine whether the shortfall is only proof, or proof **and** implementation.

## Execution continuation — prerequisite closure

The user subsequently authorized resolving C-01–C-04 before executing Phase 4. The original audit observations and frozen captures above remain historical evidence. This continuation records the new verification and bounded corrections; it does not change the accepted Phase 1–3 task denominators or accept any Phase 4 implementation gate.

The [production-component inventory](../tools/pitlord/production-components.json) now records current and baseline responsibilities under the existing 34-component policy, with test and uncertainty classifications kept explicit. A current default-feature Windows compiler inventory independently corroborates the cfg-aware module inventory: [compiler evidence](../benchmarks/dependency-convergence-phase1-3-compiler-inventory-2026-10-23.json). The reproducible [component-graph verifier](../scripts/check_production_component_graph.py) reads the existing immutable snapshots without a scan; its [component comparison receipt](../benchmarks/dependency-convergence-phase1-3-component-closure-2026-10-23.json) distinguishes metric export from closure acceptance. Its Arcana path now queries only the bounded induced `src/` file-node graph and caches the snapshot/binary-pinned result; it does not export `range(node_count)` or claim symbol-level completeness. C-01 is resolved as measured source-map coverage with 48 current and 42 baseline non-test paths explicitly left uncertain. C-02 remains open because 82 current cycle-forming source-import edges have no accepted/deferred disposition. C-07 therefore remains blocked and no Phase 4 implementation gate is opened.

The [bounded deferred-source register](../tools/pitlord/convergence-deferred-source-edges.json) records exact existing source imports, component roles, source hashes and Phase 4/5/6 ownership. It is migration evidence, not a target-DAG allowlist. Macro-generated Cva/Phylactery methods are classified from their actual local macro definitions and invocations as public composition. The native completed-layer import review applies the direct target DAG and exact producer-value contracts before considering later-phase dispositions; no native store/physical/History/Import/Maintenance callback can be hidden in the deferred register. Five exact Freshness-to-pure-timeline value contracts document existing immutable input/result/error reexports without adding a general service dependency.

The expanded source review identified a responsibility-classification error: the grouped Insomnia completion codec combines multiple owners' evidence and belongs to owner-lifecycle composition, whereas InsomniaStore consumes only accepted attempt facts. Both completion codec files are now explicitly covered by the lifecycle import guard. The shared candidate-count value moved to the Insomnia model without changing its value or any stored bytes. Import's replay adapter moved from the Insomnia service module to `owner_lifecycle/insomnia_import_replay.rs`, with its three callers migrated and its unused encoding wrapper removed. Existing service orchestration dependencies remain visible for Phase 5; they are not approved by adding new target edges. See [Architecture](architecture.md#grouped-insomnia-completion-boundary) for the current boundary.

The physical, owner and read-only History production guards and negative fixtures pass after these corrections. C-04's core History/Import/Maintenance source evidence is accepted; facade historical embedding adapters are public composition, not native read-only History. C-03's whole-inventory and immutable-value receipt is accepted for the bounded owner/physical boundary covered by the current source map; unresolved graph/source-map uncertainty remains explicitly outside that gate and is not promoted to C-01/C-02 acceptance. No Project Environment hard-cut implementation has begun.

## Related docs

- [Phase 0 frozen graph and evidence](dependency-convergence-phase0-2026-10-21.md)
- [Phase 1 specification](dependency-convergence-phase1-storage-foundation-spec-2026-10-21.md)
- [Phase 1 accepted verification](dependency-convergence-phase1-verification-2026-10-22.md)
- [Phase 2 specification](dependency-convergence-phase2-owner-local-boundaries-spec-2026-10-22.md)
- [Phase 2 accepted execution](dependency-convergence-phase2-execution-2026-10-22.md)
- [Phase 3 accepted execution](dependency-convergence-phase3-execution-2026-10-22.md)
- [Phase 3 to Phase 4 handoff](dependency-convergence-phase3-phase4-handoff-2026-10-23.md)
- [Target component DAG](dependency-convergence-target-dag-2026-10-21.md)

## Notes

The original audit was diagnosis and correction planning only. Its source-based production-candidate filtering is intentionally not presented as a conclusive compiled source dependency measurement. The execution continuation above records subsequent authorized prerequisite work separately. Pin both original and current snapshot IDs and the source checkout for repeatable acceptance.
