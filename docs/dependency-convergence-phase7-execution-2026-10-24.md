# Phase 7 execution evidence

Parent index: [Documentation index](INDEX.md)

## Purpose

Record execution of the [Phase 7 specification](dependency-convergence-phase7-final-convergence-spec-2026-10-23.md), preserving its source DAG and behavior acceptance requirements.

## Overview

The immutable before-image is `benchmarks/phase7-baseline.json`, captured from integrated A6 source at `0495689` and reconciled file mapping at `89de02d`. The amendment at `772cac2` explicitly authorizes source cuts from integrated Phase 5/6 evidence and carries the inherited A5 full graph proof into A7. It is not an execution blocker and is not waived.

Implementation used local Luna subagents without connectors. Earlier Luna sessions reached their account limit after saving their cuts; root continued integration locally, and a later Luna session performed the semantic-mutation extraction. No wire migration, new feature or target-DAG edge was introduced.

## Source cuts

- REL execution now stores `RelExecutionOwner`, combining storage-only `ReliquaryOwner` with derived Lexical/Dream/Freshness runtime projections. `RelExecution` owns sessions and canonical interaction operations. Supported `Cva` and `InteractionRuntime` paths preserve concrete type identity through public bindings.
- PHY workers store `PhyExecutionOwner`, combining the original `PhylacteryOwner` with runtime caches. Physical creation/replay stays below runtime; History and reference validation run in their original upper order. The public `Phylactery` path binds this state.
- Gateway Project Environment and offline reconciliation compose in facade adapters above path claims/admission. Workers and background processing invoke canonical operations rather than private public-product convenience methods.
- Dream candidates receive Memory and vector readers. Vectors owns payload hydration/scalar decoding, shared with Retrieval. Insomnia evidence receives `ArchiveReadView`, and Retrieval index building receives Memory/vector views.
- Dream publication prepares passive Graph changes. Runtime invokes the native owner writer, then completes duplicate-cache handling in the original order. PHY Dream lifecycle policy uses a concrete Memory writer; Memory owns draft application. The obsolete PHY canonical helper was deleted after its last consumer migrated.
- External indexing uses a Vectors-owned embedding service. Provider/Insomnia/temporal processing orchestration moved into Gateway, preserving source read, authorization, provider I/O and publication revalidation order.

Registered runtime children are recorded in `phase7-runtime-residuals.json`, `phase7-adapter-cuts.json`, `phase7-interaction-owner-core.json` and `phase7-gateway-phy-owner-handle*.json`. Three reader children, one Dream-publication child and one External-provider child extend the four runtime children, alongside the offline-background adapter, semantic-mutation and Dream-owner-facts children. The separately registered [Import/Insomnia replay child](../benchmarks/phase7-import-insomnia-replay-child-2026-10-24.json) brings the total to **31 parents + 13 named children = 44 tracked obligations**, with four gates unchanged. Addenda refine these child scopes rather than adding another child.

Import reconciliation now delegates persisted Insomnia Work/Attempt decoding, physical append and owner projection application to the existing owner-lifecycle replay helper. Publication still precedes projection updates, and the original error variants are preserved. The last-consumer migration removes the obsolete decoding forwarder; the retained journal regression covers Work and Attempt history through sync and reopened reconciliation.

## Evidence and acceptance

The initial B7 import inventory has 3,580 rows, 1,284 candidates across 40 pairs and 97 uncertain imports. The disposition register identifies public root reexports separately from executable dependencies, pins exact value contracts and retains unapproved candidates. A classification is not an eliminated violation.

The original five aggregate imports have been replaced with concrete execution state and operations. Later source measurement also exposed private worker child-module ownership drift: outcome/progress are runtime algorithms, not public API bindings. Their responsibility mapping was corrected; that correction is not counted as source-edge elimination.

The [final import receipt](../benchmarks/phase7-final-import-evidence-2026-10-24.json) records 3,772 rows, 1,225 candidate rows across 34 pairs, 96 lexical uncertainties and no strict import SCC. It covers 871 reachable production modules in 33 instantiated components. The target declares 34 nodes; the existing map explicitly locates no current `ego` service source, while Ego store is present. Thirteen source seams have permanent guards. The exact `GraphRelation` value contract is separately source-reviewed; it carries IDs, enum tags, flags and versions rather than a Graph owner capability. These are lexical findings, not accepted implementation edges: compiler call acceptance remains required. Seven External Source gateway tests passed at the earlier checkpoint using a short repository-local temporary directory; the longer temporary path caused Windows publication-path failures in both old and current executables.

The compiler call checker joins MIR call operands to private rustdoc implementation spans. Its negatives reject a lower type alias that still calls a facade implementation, preserve indirect/unresolved calls and local trait implementations, and reject unapproved calls. It normalizes relative source paths, retains destructor analysis separately, and validates supplied source/artifact provenance against current bytes. Source guards separately reject Container/product capabilities in lower policy, providers in External Sources, runtime caches in durable owners, and direct Import application of Insomnia journal records. Neither target-model validation nor a clean import graph substitutes for compiled implementation evidence.

**A7 and the overall refactor remain unaccepted until all material call evidence and required integrated checks pass.** Exact command results belong in the verification receipt. Failed/cancelled compiler-emission experiments are not accepted graphs. The source-map and focused source guards can pass while final graph proof is still open.

The [final compiler receipt](../benchmarks/phase7-final-compiler-evidence-2026-10-24.json) validates before/after source provenance and artifact hashes. Its exact matching resolves 936 cross-component call instances; 191 raw target-pair candidates (182 unique endpoints) remain unpermitted, including direct facade/Container calls. Pure immutable method calls are retained separately as disposition candidates, not automatically implementation violations. There are 112,636 unresolved instances: 80,401 unresolved local operands, 32,229 destructor terminators and six indirect/nonconstant terminators. These large numbers reflect incomplete compiler-name/implementation matching as well as dispatch uncertainty; they are not counts of distinct architectural violations. No suffix-based ownership inference is accepted. Exact endpoints, tool/profile hashes and full raw evidence hashes are preserved. This is reproducible current evidence of an open gate, not an accepted graph or authorization to waive residuals.

## Stabilization checkpoint (2026-10-24)

The paused post-`7c74b51` source-cut batch was integrated and verified without another extraction. All 28 new cfg-reachable production modules are registered under explicit component owners. Exact `cargo check --locked --lib` passed after repairing 48 compile errors in the paused batch; a stale mount-identity test helper and missing test import were corrected. A behavioral regression that wrongly exposed archive compatibility profiles during an unfinished import was repaired by reusing the canonical `runtime_current_vector_generation` admission path.

The post-repair locked library suite passed **2,006 tests, zero failed, five ignored**; integration/examples passed **165, zero failed, one ignored**; CLI passed **32, zero failed**. The 45 Python checker tests, nine dependency checker tests, 26 source-cut guards, 898-module ownership map, bounded façade boundary check, Rust formatting and Git whitespace check passed. The exact command/profile and limits are in [the stabilization receipt](../benchmarks/phase7-stabilization-checkpoint-2026-10-24.json). This constitutes a behavioral/source-cut checkpoint **only**: B7/H7/R7/A7 remain open under the original acceptance standard. The old 182 compiler-call candidates are not a post-cut residual count.

## Verification

Use the following local commands:

```text
python scripts/check_phase7_component_map.py
python scripts/check_phase7_source_cuts.py
python -m unittest scripts.test_phase7_component_map scripts.test_phase7_source_cuts scripts.test_phase7_compiler_calls
python -m pytest --basetemp .pytest_tmp scripts/tests/test_phase7_dependencies.py
python scripts/check_phase7_dependencies.py --check
```

`measure_phase7_baseline.py` now writes a separate current-source receipt and refuses to overwrite the frozen B7 baseline. `--check` on the disposition register checks freshness only; it is not final acceptance. Compiler/profile hashes and uncertainty stay explicit.

The rebuilt local Pitlord implements its documentation command. Explicitly selecting the compatible release Arcana avoids an older binary's manifest-protocol error. A single bounded Rust scan consumed its 15-minute analysis budget without publishing a snapshot; Arcana sync did not start. The [refresh receipt](../benchmarks/phase7-architecture-refresh-2026-10-24.json) preserves tool hashes and the unchanged snapshot identity. The old inventory still reports 22 absent source paths. No full uncached analyzer export was repeated to hide that gap. Documentation policy and Archivist pass separately against current files.

The locked library and CLI checks pass. The normal-permission whole library run has 2,004 passing cases, five ignored cases and one subsequently corrected test-fixture failure. Both the corrected exact test and the new standalone Work/Attempt replay test pass. The [stability receipt](../benchmarks/phase7-test-correction-stability-2026-10-24.json) proves subsequent Rust edits only changed test sources and exact cfg(test) wiring, preserving production bodies. This evidence is reported as a whole run plus focused correction, not a monolithic final zero-failure run. Integration/example verification passes 165 tests with one ignored; CLI passes 32 tests; doctests and a prepared two-node Archive round trip pass. Restricted-runner failures were diagnosed with an exact local HTTP test that passes with normal Windows permissions; accepted runtime checks use those permissions and repository-local temporary paths.

## Documentation impact

- Inspected: architecture, invariants, storage, API, maintainer/coverage maps, behavior, limitations, roadmap, Phase 5/6 receipts, Phase 7 plan and shared standards.
- Updated: canonical runtime/owner/read/provider boundaries, public API bindings, behavior/format preservation, enforcement ownership and this record.
- Not affected: retained corpus, durable V2 schemas, independent grants, supported method signatures and product features.
- Compliance check: exact results are recorded with final verification; failing graph/document ownership evidence is not called passed.
- Known documentation gaps: inherited stale Arcana inventory includes obsolete paths and prevents a current complete Pitlord ownership pass.

## Architecture impact

- Standards added or changed: none; 34 components and 163 justified edges remain unchanged.
- Ownership or boundary impact: concrete runtime state separated from durable owners, lower readers/writers and semantic plans separated from physical publication.
- Pitlord enforcement impact: exact source-map guard, source-cut negatives and compiler implementation-call evidence; no blanket pair allowlist.
- Other verification impact: integrated Rust/CLI/regression gates, exact source witness/hash registers and preserving session error regression.
- Known architectural gaps: any remaining unapproved compiled calls, material indirect/cfg reachability and inherited A5 graph proof must be resolved before A7.

## Related docs

- [Specification](dependency-convergence-phase7-final-convergence-spec-2026-10-23.md)
- [Architecture](architecture.md)
- [Current limitations](current-limitations.md)
- [Phase 6 final verification](../benchmarks/phase6-final-verification.json)
- [Phase 7 verification](../benchmarks/phase7-verification.json)

## Notes

Existing Phase 5 worktrees are preserved. Both were clean and had no branch commits absent from main in the initial audit. The final integration audit and committed source checkpoint are separate from graph acceptance. No Phase 8 or reduced completion standard is introduced.
