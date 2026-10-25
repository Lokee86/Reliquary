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

## Postcheckpoint compiler evidence (2026-10-24)

The verified stabilization commit `d1c0d4b` now has a new [postcheckpoint source-and-compiler inventory](../benchmarks/phase7-postcheckpoint-compiler-evidence-2026-10-24.json). It covers **898 cfg-reachable production files with no unmapped owners** and records 3,878 source imports, **1,150 lexically unapproved candidates across 36 component pairs**, 87 lexical uncertainties, and **zero strict source-import SCCs**. These are not counts of actual implementation defects.

The matching Rustdoc/MIR emission succeeded at that commit. Its static-call checker counted **129,185 constant call instances**, **884 compiler-resolved cross-component calls**, and **50 unpermitted static call instances across 12 component pairs**. The 50 include direct owner/container capabilities as well as likely pure value constructors and validation helpers; every endpoint is recorded for exact source review. The run retains **67,418 unresolved call instances** (33,730 unmatched local, 32,250 destructor, 1,432 closure and six indirect/nonconstant), and was not augmented with an exact HIR owner-span join. The prior 191-instance compiler evidence belongs to the previous source checkpoint and is not a current residual count; checker improvements also limit direct comparison.

**Acceptance is still OPEN.** Next work should first classify the 50 exact endpoints into true prohibited capabilities versus proven immutable/pure contracts, then execute only confirmed corrections as bounded component-scoped changes. HIR/destructor/indirect proof and the inherited Arcana/Pitlord graph gate remain separate evidence obligations. No wider DAG permission or blanket exception is authorized.

## Postcheckpoint static-call disposition (2026-10-24)

The exact 50-call inventory at source commit `d1c0d4b` has been reviewed against caller/callee implementations and reconciled in [the finite call-disposition register](../benchmarks/phase7-postcheckpoint-call-dispositions-2026-10-24.json). No implementation edits or DAG policy changes were made during this review. The 50 distinct MIR call instances consist of **26 confirmed prohibited owner-capability calls across five component pairs**, **20 pure immutable-value/validation contract candidates across five pairs**, and **four feature-gated Freshness tracing calls across two pairs**. The 20 value/validation candidates are *not* automatically accepted: they require the exact contract proof under the original DAG. Tracing is inert in the measured default cfg, but its enabled feature has side effects and needs independent cfg/ownership disposition.

The confirmed capability cuts are bounded by existing tickets, not new arbitrary child counts: P7.1.14 has 13 public-consumer→Container, eight public-consumer→MemoryStore, two public-consumer→Perception live-read-view, and one public-consumer→Vectors publication-preparation calls. P7.1.12 has two Gateway→Container backing-metadata calls. Each exact source line, target definition, MIR line digest and affected boundary is recorded in the JSON register; preserving the already passing targeted regression families is the exit condition for each cut. No further extraction begins as part of inventory generation.

All 13 existing child-ticket receipts were reconciled to the integrated stabilization checkpoint without rewriting their historical registration statuses. They are source-integrated and behaviorally verified, **not** formally architecture-accepted. The denominator remains 31 parents plus 13 named children. The five closure slices above are *not* additional tickets. Compiler gaps (67,418 unresolved instances including destructor/cfg/indirect analysis) and stale Lexicon/Arcana A5/A7 graph proof remain distinct obligations. **B7/H7/A7 OPEN; R7 PARTIAL.**

## Current closure work and denominator correction (2026-10-24)

The postcheckpoint closure implements the confirmed capability cuts and corrects the twenty value/validation calls through their proper owners rather than granting new DAG edges. Feature-enabled tracing uses a finite Freshness vocabulary and scoped timers instead of cross-component execution callbacks. Semantic-target enumeration retains only IDs/body coordinates and hydrates one Memory at a time in each original pass. Historical embedding uses a borrowing Gateway write-scope guard, whose destructor resets admission on ordinary return and panic unwind.

A subsequent source/callback audit found remaining REL Dream source-time callbacks, Import-to-Dream replay coordinate callbacks and Gateway-to-Insomnia progress callbacks. Their exact registrations and the current child inventory are in [the indirect-correction register](../benchmarks/phase7-indirect-call-corrections-2026-10-24.json). The historical thirteen-receipt reconciliation above omitted fourteen independently registered children already present in the repository. Deduplicating their exact identifiers yields **27 previously registered children**; the two callback seams and five further independently witnessed owner seams are registered before implementation, giving **31 parents + 34 named children = 65 tracked obligations**, with four gates unchanged. Existing receipts retain their historical denominators. Completion of source cuts does not imply acceptance of any child.

The local analyzer compatibility patch is `tools/pitlord/lexicon-toolstate-exclusion.patch`. It excludes only ignored verification directories and caches, including historical scratch-source copies. Its fixture retains authored `src` traversal. The sibling analyzer source is unchanged; the patched optimized adapter is built in repository-local generated storage. A scan started before subsequent source corrections is not accepted as current evidence.

`scripts/emit_phase7_compiler_evidence.py` captures locked rustdoc, MIR, HIR and typed HIR artifacts from stable source bytes with hashes and provenance. Exact typed-HIR call identities can join MIR spans to HIR definition spans; shortened names and material ambiguous/indirect/destructor cases remain explicit. Integrated compiler, graph and behavioral results are still required before A7 can close.

## Frozen post-bot stabilization and compiler inventory (2026-10-25)

After the bot stopped, the complete dirty source batch was frozen without starting another architectural extraction. The current worktree passes `cargo check --locked --lib`, core formatting and Git whitespace checks. All **924 cfg-reachable production modules** have explicit owners. The **106 source-cut seams**, owner and History boundaries, runtime (692 files), facade (924 files), physical boundary (13 fixtures), and External Source ownership checks pass. The child registry resolves **31 parents + 37 children = 68 obligations**, with the same four acceptance gates.

The frozen default-feature Rustdoc/MIR/HIR/THIR capture is recorded under `.verification-tmp/phase7/phase7-frozen-default-20261025/` with source fingerprint `c0c7a0c5e6aa3a04de753b3bec8bd020717e280f7f604655f0cb82e438b00243`. Compiler emission completed, but enumerating a generated built-MIR dump failed on a Windows long path. The four existing compiler artifacts were recovered without recompilation and validated against identical frozen-source hashes; **built-MIR destructor-layout proof remains incomplete**. The [exact frozen-call inventory](../benchmarks/phase7-postbot-frozen-inventory-2026-10-25.json) records **78 unpermitted compiled-call candidates across 28 component pairs**, with **58,080 unresolved call instances** separately accounted for. These raw candidates are **not** 78 confirmed architectural defects or accepted value exceptions. The earlier 320-call capture used a different compiler checker hash, so its reduction to 78 is not a controlled measurement of corrected implementation edges.

The current frozen-source `cargo test --locked --jobs 2` run passed **2,015 library and 163 integration tests (zero failures, six ignored)**, plus doctests. The separate locked CLI run passed **32/32**. The seven Python checker files passed **154 tests**, including current compiler gate and dependency checks. These are implementation/behavior checkpoints, not graph acceptance.

This checkpoint **does not close B7, H7 or A7**; R7 remains PARTIAL. The latest full Lexicon/Arcana graph evidence predates these source cuts. A fresh full graph, enabled feature/cfg coverage, explicit owner-capability versus immutable-contract disposition of the remaining calls, and bounded destructor/indirect uncertainty still require proof. No 34-component target-DAG edge, blanket exception, wire format or product feature is added.

## Verification

The current [closure verification receipt](../benchmarks/phase7-closure-verification-2026-10-24.json) records the integrated run separately from historical checkpoints. The source audit covers 901 reachable production modules with no ownership holes and 64 guarded seams. All owner, runtime, facade, History, physical-storage, External Source and public API boundary ratchets pass. The exact Freshness `ObjectRef` type-only exception is recorded explicitly; it does not widen the 163-edge target DAG. Source-cut and build results do not replace the pending same-source compiler profiles or current full graph proof.

The [postcheckpoint source-closure receipt](../benchmarks/phase7-postcheckpoint-source-closure-2026-10-24.json) preserves all 50 historical witnesses and maps them to the current replacement routes, symbols, components and source hashes. It also records the registered callback protocols and admission/hydration corrections. These source dispositions require the compiler and behavioral receipts before formal acceptance.

The exact typed-HIR join expands beyond that old 50-call inventory. The [expanded disposition checkpoint](../benchmarks/phase7-expanded-call-dispositions-2026-10-24.json) preserves 788 pre-correction non-drop instances grouped by 467 exact implementation spans. It distinguishes 47 confirmed owner-capability groups, 80 forbidden upward binding spans, two value-only decoder candidates, and unaccepted immutable/semantic candidates. It is a historical checkpoint, not the current residual count. The [Gateway forwarder cut](../benchmarks/phase7-runtime-forwarder-cut-2026-10-24.json) registers the subsequent migration of canonical invocations and status reads under existing children. None of these findings justifies widening the target DAG.

The current closure sequence is:

1. Capture stable default and tracing compiler profiles and keep the unresolved call/destructor categories explicit. Preserve generated passive-value drop layouts separately from calls to an explicit local `Drop` implementation.
2. Reconcile each expanded endpoint with its current implementation. Use exact existing child scopes for further cuts; register an independently witnessed seam before editing it. The proposed child identifiers in the expanded inventory are recommendations and do not change the denominator until registered.
3. Migrate confirmed capability calls to the established owner/runtime operation and remove private forwarding code after its last consumer moves. Pure contracts require exact value/effect evidence; classification alone does not pass a ticket.
4. Run the bounded production-scoped analyzer with the crate assembly root included, confirm actual callable/symbol coverage, synchronize Arcana, and run Pitlord policy plus documentation ownership. An empty or stale graph remains rejected even if its policy command returns zero.
5. At stable source bytes, pass the integrated Rust/CLI, feature, Archive smoke and documentation gates, refresh receipts, audit the preserved worktrees and commit the integration. B7/H7/A7 and the overall refactor stay open for any remaining material implementation or proof gap.

Use the following local commands:

```text
python scripts/check_phase7_child_registry.py
python scripts/check_phase7_component_map.py
python scripts/check_phase7_source_cuts.py
python -m unittest scripts.test_phase7_child_registry scripts.test_phase7_component_map scripts.test_phase7_source_cuts scripts.test_phase7_compiler_calls
python -m unittest discover -s scripts/tests -p test_phase7_dependencies.py
python scripts/check_phase7_dependencies.py --check
```

`measure_phase7_baseline.py` now writes a separate current-source receipt and refuses to overwrite the frozen B7 baseline. `--check` on the disposition register checks freshness only; it is not final acceptance. Compiler/profile hashes and uncertainty stay explicit.

The rebuilt local Pitlord implements its documentation command. Explicitly selecting the compatible release Arcana avoids an older binary's manifest-protocol error. A single bounded Rust scan consumed its 15-minute analysis budget without publishing a snapshot; Arcana sync did not start. The [refresh receipt](../benchmarks/phase7-architecture-refresh-2026-10-24.json) preserves tool hashes and the unchanged snapshot identity. The old inventory still reports 22 absent source paths. No full uncached analyzer export was repeated to hide that gap. Documentation policy and Archivist pass separately against current files.

At the earlier checkpoint, the locked library and CLI checks passed. Its normal-permission whole library run had 2,004 passing cases, five ignored cases and one subsequently corrected test-fixture failure. Both the corrected exact test and the new standalone Work/Attempt replay test pass. The [stability receipt](../benchmarks/phase7-test-correction-stability-2026-10-24.json) proves subsequent Rust edits only changed test sources and exact cfg(test) wiring, preserving production bodies. This evidence is reported as a whole run plus focused correction, not a monolithic final zero-failure run. Integration/example verification passes 165 tests with one ignored; CLI passes 32 tests; doctests and a prepared two-node Archive round trip pass. Restricted-runner failures were diagnosed with an exact local HTTP test that passes with normal Windows permissions; accepted runtime checks use those permissions and repository-local temporary paths.

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

## Owner-operation integration checkpoint (2026-10-25)

The expanded compiler checkpoint has 647 unpermitted call instances and 32,986 unresolved call/drop instances before the subsequent owner cuts. It is historical evidence; neither number is a current residual count. The current integration adds concrete Ego, Relationship, Archive, Dream journal, Insomnia, Perception, semantic Freshness, Vector and Graph operations under registered children. Public bindings keep their supported signatures. Live store queries and borrowed read-view constructors remain owner capabilities, including when their outputs are immutable.

The [exact call review](../benchmarks/phase7-call-review-exact-contracts-2026-10-25.json) distinguishes 13 detached-value candidates from 24 owner/service capabilities among the 37 decoder/value candidates. Its 64 live-read/endpoint groups cover 140 historical call instances and explicitly retain missing or ambiguous child-scope matches. Classification is not acceptance. At the time of this review, the registry contained 65 parent/child obligations; later registrations bring the current denominator to 31 parents plus 37 named children, or 68 obligations, and four gates.

Five registered CORE-OWNER-API scope addenda on 2026-10-25 move the conversation, activity, file-memory, historical-file, episode timestamp and Freshness due reads behind existing Gateway/runtime and mounted Archive-owner operations. Their exact methods and before/after source hashes are recorded in the [conversation-read](../benchmarks/phase7-core-owner-api-conversation-read-addendum-2026-10-25.json), [Archive-reads](../benchmarks/phase7-core-owner-api-archive-reads-addendum-2026-10-25.json), [historical-files](../benchmarks/phase7-core-owner-api-historical-files-addendum-2026-10-25.json), [episode-timestamp](../benchmarks/phase7-core-owner-api-episode-timestamp-addendum-2026-10-25.json) and [Freshness-due](../benchmarks/phase7-core-owner-api-freshness-due-addendum-2026-10-25.json) receipts. These refine existing child scopes and do not change the target DAG. Subsequent registrations bring the current denominator to 68 obligations; the registry command below verifies it. Focused formatting and façade-boundary checks passed; integrated build and same-source compiler evidence remain pending. The 647-call report above predates these cuts and is not their current residual count.

The full architecture command now requires the [compiler gate](../scripts/check_phase7_compiler_gate.py), which verifies artifact, tool, policy and source hashes and derives permissions from the unchanged DAG. A source-only run is explicitly partial. The coarse Pitlord area rules cannot establish 34-component implementation conformance on their own. The analyzer production manifest must include the crate assembly root as well as semantic modules; zero-callable snapshots are rejected.

The earlier full forwarder checkpoint passes 2,012 library and 163 integration cases with six ignored and no failures; CLI passes 32. Later source cuts require a new integrated run. The current source checks and focused compilations are recorded separately. Final compiler profiles, analyzer freshness, behavior and documentation ownership remain open until refreshed at stable source bytes.

## Same-source integration and continued correction (2026-10-25)

The [integration checkpoint](../benchmarks/phase7-integration-checkpoint-2026-10-25.json) records 2,015 passing library tests, 163 passing integration tests, six ignored cases and zero failures, plus 32 passing CLI tests. The successful run used normal Windows permissions and repository-local temporary files; the restricted attempt was stopped after local HTTP fixtures waited for requests that were blocked. These results precede the subsequent correction batch and are not its final acceptance.

The compiler emitter now drains HIR/typed-HIR output in bounded chunks. Both profiles completed with validated source provenance: the default profile reported 320 unpermitted call instances and 63,946 unresolved call/drop instances; tracing reported 323 and 64,052 respectively. These are checkpoint counts, not the residuals after further cuts. Exact generic implementation, destructor, macro-context and finite indirect-target proof remains required.

The architecture wrapper now requests the existing bounded Rust rebuild instead of an incremental scan that can skip the adapter. The rebuild acknowledged the exact 917-file source manifest (916 semantic modules plus the assembly root), extracted 12,064 callable symbols and synchronized snapshot `80a57efb7eec1e6ac5fc81721922a552502dff28020ac2868e499af42a6bbfe5` to Arcana. Root policy and documentation ownership passed. The separate [convergence reconciliation](../benchmarks/phase7-current-convergence-reconciliation-2026-10-25.json) retains 820 current fingerprints: 39 retained and 781 newly reported; 4,587 historical entries are absent. Neither a passing coarse policy nor analyzer same-name artifacts clear the exact component gate.

Further source cuts use exact pre-edit witnesses under the existing retrieval, Freshness, vector, community and Import children. The independently witnessed [configured-provider preparation](../benchmarks/phase7-configured-provider-preparation-child-2026-10-25.json) [public Insomnia work lifecycle](../benchmarks/phase7-public-insomnia-work-lifecycle-addendum-2026-10-25.json), and [live External index read](../benchmarks/phase7-gateway-external-index-read-child-2026-10-25.json) seams were registered before implementation. The current denominator is **31 parents + 37 named children = 68 obligations**, with four gates unchanged. Earlier receipts retain their historical denominators. Source review and focused checks do not accept these children without refreshed compiler and integrated evidence. **B7/H7/A7 remain OPEN; R7 remains PARTIAL.**

## Remaining closure checklist (2026-10-25)

The following work is still required after the current source edits stop. The latest architecture projection (`8bd1ff72…`) is bounded source-graph evidence, not acceptance: it maps all 917 manifest paths with no unmapped source files, but still reports 2,508 edges across DAG-disallowed pairs. The owner-source proof at `cc9d2f4` reports 1,151 unapproved import rows across 38 pairs and 87 uncertainties. The convergence reconciliation has 781 new and 4,587 absent historical fingerprints; these static findings remain subject to exact source review. The latest exact-call review uses historical compiler bytes, and the last integrated test receipt predates subsequent corrections. Keep every gate open until its required evidence is refreshed and passes.

1. **Reconcile registrations and source ratchets.** Run the child registry, component map, source cuts, dependency policy and their focused tests. The registry currently verifies 31 parents plus 37 children and four gates.

   ```text
   python scripts/check_phase7_child_registry.py
   python scripts/check_phase7_component_map.py
   python scripts/check_phase7_source_cuts.py
   python -m unittest scripts.test_phase7_child_registry scripts.test_phase7_component_map scripts.test_phase7_source_cuts scripts.test_phase7_compiler_calls
   python -m unittest discover -s scripts/tests -p test_phase7_dependencies.py
   python scripts/check_phase7_dependencies.py --check
   ```

2. **Capture and check same-source compiler profiles.** After the final edits, use fresh, unique capture names for the default and tracing profiles. Each capture emits `calls.json`, provenance, rustdoc, MIR, HIR and typed HIR under `.verification-tmp/phase7/<name>/`; retain prohibited and unresolved calls as blockers. Run the compiler gate against each report and record both outcomes and source hashes.

   ```text
   python scripts/emit_phase7_compiler_evidence.py --name phase7-closeout-default
   python scripts/emit_phase7_compiler_evidence.py --features freshness-performance-trace --name phase7-closeout-tracing
   python scripts/check_phase7_compiler_gate.py --report .verification-tmp/phase7/phase7-closeout-default/calls.json
   python scripts/check_phase7_compiler_gate.py --report .verification-tmp/phase7/phase7-closeout-tracing/calls.json
   ```

3. **Refresh graph evidence at those stable source bytes.** Run the bounded architecture refresh with the validated compiler report(s), then run the full architecture policy gate with compiler evidence. Preserve raw output and current snapshot/manifest hashes; a source-only pass, zero exit from coarse policy, or stale Arcana snapshot is not A7 proof.

   ```text
   python scripts/check_architecture.py --refresh --compiler-evidence .verification-tmp/phase7/phase7-closeout-default/calls.json
   python scripts/check_architecture.py --compiler-evidence .verification-tmp/phase7/phase7-closeout-tracing/calls.json
   pitlord docs --repo . --ddocs-root docs --changed-from origin/main --timeout 2m
   ```

   The refreshed evidence must establish the exact 34-component target, production coverage, permitted directed calls, absence of prohibited SCCs, and disposition of material unresolved, cfg, macro and indirect calls. Reconcile current findings against the preserved fingerprint ledgers only with source review; do not accept the present 781-new/4,587-stale convergence reconciliation as compiler proof.

4. **Run the integrated behavior and documentation gates after the last source edits.** Record the exact pass, fail and ignored counts, prepared Archive fixture and all output paths. The previous integration receipt is historical for this correction batch.

   ```text
   cargo fmt --check
   cargo check --locked
   cargo test --locked
   cargo fmt --manifest-path cli/Cargo.toml -- --check
   cargo check --manifest-path cli/Cargo.toml --locked
   cargo test --manifest-path cli/Cargo.toml --locked
   cargo run --release --example archive_roundtrip -- <prepared-graph.jsonl> <output.cva>
   python ../engineering-standards/tools/docs_policy/check.py --repo .
   python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
   ddocs check --indexes --links
   ```

5. **Reconcile integration and closeout.** The read-only audit found both Phase 5 worktrees clean at `17514f0` (`Reliquary-phase5` and its `target/phase5-acceptance-checkout`). Recheck them with all source/receipt changes at closeout; reconcile generated verification material; update current architecture, limits, maintainer/coverage documentation and the final receipt from the results above; commit the stable integration. If any material call or graph proof remains unresolved, report its exact scope and keep B7/H7/A7 open and R7 partial. The Phase 7 spec owns the formal ticket and gate exit conditions.
