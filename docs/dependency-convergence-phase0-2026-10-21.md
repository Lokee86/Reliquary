# Dependency Convergence Phase 0 Baseline

Date: 2026-10-21
Parent index: [Documentation index](INDEX.md)
Campaign: [Correction-only architecture refactor](dependency-convergence-architecture-refactor-2026-10-21.md)

## Purpose

Freeze the current dependency problem and give each boundary a migration owner and deletion gate. This is architecture evidence and enforcement, not feature implementation. No application source, public behavior, durable format, or owner authority is changed by Phase 0.

## Overview

The source baseline is local `main`, commit `c43d33c136f560fdca2249619866e1acf9b7fdf3`. The graph capture is complete. The optimized whole Arcana snapshot contained 109,207 nodes and 297,303 relationships. Pitlord's configured `src/` scope contained 98,633 nodes and 268,391 relationships, with 28,545 selected for configured source analysis. All 1,102 of 1,102 authored Rust files were assigned exactly once, with no overlap or unowned files. The capture spans 19 areas and 146 directed area dependencies; the combined relation graph has one raw SCC spanning 16 areas. It contains 9,537 concrete cross-area module edges across all captured relations, 298 public crossings, and 4,609 classified exact Pitlord findings. These are capture measurements, not evidence that every raw call binding is correct or that the reported SCC consists only of production dependencies.

[convergence-graph.json](../tools/pitlord/convergence-graph.json) is the retained Phase 0 before-image. [convergence-baseline.json](../tools/pitlord/convergence-baseline.json) and [convergence-debt.json](../tools/pitlord/convergence-debt.json) are active ledgers that shrink together as reported findings resolve. The target allowed-area matrix is [convergence-target.json](../tools/pitlord/convergence-target.json), checked against the forbidden complements in [convergence.json](../tools/pitlord/convergence.json). The original graph is not regenerated to clear debt. The reviewed ledger classifies 790 findings as source-confirmed, 3,683 as test-only, and 136 as analyzer artifacts or unsupported bindings quarantined from production conclusions. [convergence-boundaries.json](../tools/pitlord/convergence-boundaries.json) records dispositions for all 146 captured directed area dependencies.

The target layer order is physical mechanics → owner-local stores → domain composition → runtime/orchestration → public facade. Dependency arrows in this record point from consumer to dependency, so they run in reverse: facade → runtime/orchestration → domain composition → owner-local stores → physical mechanics. Stable contracts remain with their actual owner. This does not authorize moving all reused types into a generic shared module.

## Baseline and scope

The configured architecture graph covers authored Rust under `src/`, including colocated test files. Integration tests, examples and the detachable CLI remain functional consumers, not durable domain owners; their behavior is protected by the existing functional suites. They are outside the domain SCC graph. Cargo dependencies remain unchanged.

Pitlord evaluates resolved `imports`, `depends-on`, `calls`, `references` and `implements` from fresh Lexicon/Arcana evidence. Its analysis owns domain SCCs and representative edges. The capture also retains concrete cross-file module edges. File ownership uses the same areas as the existing policy. Those areas are intentionally coarse: `runtime-inference` includes Dream/Insomnia durable state and execution, `retrieval` includes vector stores and search, and `facade` includes historical/maintenance composition and lifecycle vocabulary. These are current locations, not approved final responsibility assignments.

Static evidence is bounded by the Rust adapter. Crate-root reexports, inherent `impl Cva` outside the defining file, conditional tests, trait/dynamic dispatch, macros, external adapters and runtime callbacks require source review. The inventory below records boundaries that a source-level cycle alone cannot explain. No claim is made that the graph proves runtime ownership, write exclusion, repair safety, or path authorization.

The derived [import/dependency audit](../tools/pitlord/convergence-import-audit.json) filters the retained graph to `imports` and `depends-on`: 2,227 cross-area edges across 84 directed area pairs form a 15-area SCC. Source review confirmed actual cycles across config-security/runtime-inference, archive/memory, and facade/runtime-inference. This filtered view includes test and mixed source paths and is not a whole-program production graph. The all-relation capture also contains proven wrong same-name call bindings, including a local `read_u64` and an error variant bound to the wrong owner. Those are analyzer artifacts, not architecture debts. Do not infer that every raw call edge or SCC edge is a real production dependency.

## Adapter boundary probes

A disposable local Rust fixture verified grouped and aliased imports, root reexports, fully qualified and imported calls, a trait implementation, an inherent implementation in a different module, and a `cfg(test)` consumer. Imports and calls resolved to owner definitions; trait implementation emitted `implements`. Inherent implementation emitted `defines` ownership rather than `implements`, so nominal type-to-method ownership is reviewed separately. Test source was included without a cfg/test-scope attribute: test-only classifications require source inspection, not graph metadata. The fixture has no external dependencies and is excluded from authored-source analysis.

## Target dependency rules

[convergence.json](../tools/pitlord/convergence.json) expresses the target allowed domain direction as forbidden complements. Same-area dependencies remain allowed pending finer owner cuts. The full allowed sets are:

| Consumer | Allowed dependencies in addition to itself |
| --- | --- |
| Compact codec, Staleness | None; each is independent |
| Container | Pure compact codec and Staleness vocabulary |
| Archive, Memory, Graph, Perception, Ego persistence, Freshness, vector/retrieval, external-source owner areas | Container and pure vocabulary; cross-owner dependencies are classified migration debt |
| Chronos, configuration/security | Pure vocabulary |
| Context Engine | Archive, Chronos and pure vocabulary; inference execution must eventually be a narrow execution contract, not a Gateway callback |
| Project Environment | Archive correlation/reference contracts and pure vocabulary; no facade/runtime construction |
| Native import, history rewrite | Concrete owners, Container, Chronos, Project Environment and pure vocabulary; no facade or runtime implementation |
| Runtime/inference | Concrete lower areas and domain composition; no public facade |
| Public facade | Downward composition/export of all areas |

These rules deliberately identify current composition hidden inside lower-area files as debt. They do not require a crate split, interfaces for every module, or a final DAG in Phase 0. Each prohibited dependency reported by the configured Pitlord analysis must have an exact baseline entry and a reviewed debt classification. The classifier requires a category, phase, concrete disposition, `reviewed: true`, an `evidence_status` (`source-confirmed`, `test-only`, or `analyzer-artifact`), and a nonempty `review_note`. New reported prohibited findings fail even within an already cyclic pair of areas. This is a gate over the configured analysis output, not proof that every source-level dependency is detected; see the static-evidence limitations above.

## Campaign ownership and boundary dispositions

Each row assigns a responsibility, rather than treating every use of a shared identifier as a mutation. The machine ledger retains the concrete source/target relation for migration review. Shared-type edges must be relocated to their real semantic owner; calling through a neutral type is not permission to preserve an upward callback.

| Current boundary and source evidence | Responsibility | Disposition and acceptance gate |
| --- | --- | --- |
| `container/container_scan.rs`: `open`, `open_scanned`, caller visitor, `recover_truncated_tail`; `facade/cva_lifecycle.rs` replay routing | Construction/init, physical mutation, orchestration callback | Phase 1: Container retains framing/global allocation; owner replay composition stays above it. Establish non-mutating inspection versus explicit repair. Preserve incomplete-tail and invalid-publication tests before changing behavior. |
| `container/container.rs` admission fence; `imports/epoch_store.rs`, `imports/commit.rs` private write scope | Mutation dependency, durable ownership | Phase 1: Import owns admission policy; Container exposes only physical mechanics. Determine necessary writer exclusion/lease semantics without a generalized owner manager. |
| `facade/cva_lifecycle.rs`, `facade/phylactery_lifecycle.rs` open all stores; `facade/cva.rs` aggregates stores | Construction/init, durable ownership | Phases 1–2: stores open/replay through physical contracts independently; top-level lifecycle composes them. Phase 6 removes accidental facade contract homes. |
| `archive/archive_search.rs`, `archive/episode_policy.rs` inherent `Cva` operations | Read dependency, mutation dependency, shared type | Phase 2 separates Archive-local search/finalization; Phase 3 moves cross-owner sequencing above stores; Phase 6 deletes obsolete forwarding after callers migrate. |
| Memory, vector/profile/generation stores and Perception/Graph contracts | Durable ownership, shared type, read dependency | Phase 2: Memory owns revisions, Graph topology, Perception Entity/current transitional Relationship state, and vector owners their bindings/profiles. Stable IDs and narrow owner read/proof seams replace implementation reach-through. No Observation or vector optimization work. |
| `perception/entity_principal.rs` reads Memory/PHY state and mutates Entity/resolution through `Cva` | Read dependency, mutation dependency, orchestration | Phases 2–3: Perception owns local validation; explicit composition obtains other-owner evidence. Relationship stays transitional behind Perception. |
| Freshness import settlement/replay; `facade/cva_historical_import.rs`; `imports/completion.rs`, `imports/readiness.rs` | Durable ownership, read/proof, mutation, shared type | Phase 2 establishes owner proof contracts. Phase 3 audits every settlement field against its authority, minimizes authenticated settlement and hard-cuts corrected bytes only after corpus verification; deletes temporary conversion. |
| `facade` historical oracles, prune/reclaim/reconciliation; `rewrite.rs`, `imports/reconciliation.rs` | Cross-owner read/proof, mutation, construction | Phase 3: explicit historical/maintenance composition consumes concrete owner evidence. Semantic reachability stays above Container; physical reclamation remains separate. |
| `archive/project_history_store.rs`, `facade/cva.rs` ProjectHistoryStore/ProjectFileStore | Durable ownership, read dependency | Phase 4 removes duplicate project-VCS authority. Preserve exact file/revision refs and ProjectRevisionCorrelation; Lore/Git remains history authority. Inspect each history operation before deletion. |
| `project_environment/repository.rs`, `git.rs`, `git_file.rs`, `lore_file.rs` | Configuration, read dependency, authorization boundary | Phase 4: one validated repository/project-path authority must bind every repository-backed operation. Repository identity equality alone does not establish allowed project scope. No new repository-management feature. |
| `runtime_inference/runtime_host_gateway.rs`, `runtime_host_instances.rs`, `runtime_host.rs`, `runtime_shared.rs` | Orchestration, configuration, shared type, obsolete compatibility | Phase 5: Gateway/Instance owns mounted coordination and scheduling, never owner truth. Preserve queue/worker/stream/recovery behavior and short owner locks; retire old RuntimeHost ownership/forwarding. Phase 6 relocates shared result/error types and removes obsolete exports. |
| `context_engine` conversation/Echo/inference dependencies | Read dependency, execution contract | Phase 5 leaves deterministic Context Engine policy below host execution. Separate compaction refinement remains deferred. |
| `ego` persisted state versus upper synthesis/context consumers | Durable ownership, read dependency | Phase 2 isolates existing Ego persistence; Phase 5/6 keeps synthesis/context consumption above lower owners. No synthesis implementation. |
| `config_security`, Chronos, external-source and provider routes | Configuration, shared type, execution | Phases 2/5 move narrowly owned contracts out of mixed runtime/facade homes. Preserve existing provider behavior and external-source state; no adapters, Chronos expansion or provider work. |
| `lib.rs` reexports; lower-file `Cva` references and error/result imports | Shared type, obsolete compatibility | Phase 6: classify each public item by actual owner, migrate lower consumers and remove forwarding. A reexport is not a new authority. |
| Colocated architecture/owner tests, including historical oracle helpers | Test-only | Move protecting assertions with the corrected owner seam in the responsible phase; keep test-only edges visible and separate from production conclusions. Do not delete behavior coverage to improve graph metrics. |

## Public and runtime crossing entrypoints

The current supported surface is frozen, not redesigned:

- `Cva::{create, open, create_project, open_project, create_organization, open_organization, create_connection, open_connection}` and corresponding PHY lifecycle assemble and replay multiple owners.
- `Cva::{import_native, native_import_readiness, complete_native_import}` cross Archive, queue/Memory, Freshness, Dream and Perception evidence. Completion remains exclusive and coordinator-owned.
- `Cva::prune_tombstoned_history`, `Phylactery::prune_tombstoned_history`, reclaim/reconciliation and `rewrite::{plan_rewrite_file, rewrite_file, rewrite_from_plan}` compose owner proof and transformed-copy state.
- `GatewayRuntime::{new, new_with_host, authorize, select_rel, quiesce_rel, reopen_rel, unmount_rel, unmount_phy, shutdown}` and `InstanceRuntime` coordinate mounted lifecycle. `unmount_rel` currently hands back a `Cva`; that handoff is not evidence that Gateway owns durable semantics.
- `ReliquaryRuntimeHostError`, `ReliquaryRuntimeRoutes`, broad `runtime_host` reexports and `lib.rs` exports remain migration surfaces, not an independent RuntimeHost owner.
- Project Environment repository validation/file readers bridge machine-local roots to exact repository references. Their current identity-only validation does not settle project-path authorization.

## Ratchet, deletion and acceptance procedure

The existing `tools/pitlord/policy.json` remains unbaselined. The wrapper runs it first, then the separate convergence policy with its exact baseline and classified ledger. It never suppresses unrelated ownership/bypass rules.

```text
python scripts/check_architecture.py --refresh
python scripts/check_dependency_convergence.py
```

The explicit `--capture` maintainer operation is for the initial capture only. It refuses to overwrite existing baseline, debt, or graph files; it is not run by CI. It writes provisional debt classifications inferred from coarse path/area and relation heuristics, each marked `reviewed: false`. The gate requires source review and correction as needed before each record can be marked reviewed. Ordinary implementation phases remove resolved findings from both shrinking ledgers and tighten target policy; they do not regenerate a blanket baseline. Keep the original Phase 0 graph as the before-image. After Phase 7, remove all convergence suppressions and temporary campaign machinery once final ownership rules fully replace it.

Checker fixtures protect acceptance of classified existing debt, rejection of new edges inside existing SCCs, missing classifications, duplicate records, invalid phases and stale suppressions. Functional baselines remain the existing owner, integration, current-format and runtime tests. Phase 0 changes no format, so it does not rerun conversion or modify retained corpora.

## Functional baseline

The Rust source, Cargo manifests/lockfiles, public APIs and retained corpus are unchanged. Formatting and `cargo check --locked` pass. The complete `cargo test --locked --no-run -j 2` build passes, including integration tests and examples. This rerun supersedes an initial broad build attempt that reported missing library artifacts; it is build evidence, not a full test execution pass.

The focused executable baseline has 64 distinct passing tests: 21 Container/framing/format tests, four current-format integration tests, nine Memory tests, five Archive tests, two import-settlement tests, seven Instance authorization tests, seven completion tests, one owner-isolation test, five public native-import profile tests and three public Gateway API tests. The existing promoted-corpus test remains ignored by its ordinary gate. Current-format tests passed again in the integration batch and are counted once here.

A full library test attempt was stopped after the long historical recovery cases completed and provider seam tests remained running; it also reported repository/runtime failures in the restricted execution environment. No full-library execution success is claimed. Phase 0 keeps those tests and refusal boundaries intact and does not repair application behavior or reclassify failing tests as passing.

## Acceptance results

The final `python scripts/check_architecture.py --refresh` run passed: evidence refreshed, the original 23 unbaselined architecture rules remained clean, and the separate 18-rule convergence gate accepted exactly 4,609 reviewed findings with no new or stale debt. Its 13 positive/negative fixtures passed. The existing ownership/coverage guards also passed. The raw giant SCC remains the frozen before-image; Phase 0 does not claim graph closure.

`ddocs check --indexes --links` and `git diff --check` passed. Both full and changed-from-origin shared documentation policy checks reported the same seven missing sections in the separate Context Engine refinement and Phase 1 storage specifications. Those plans were not repaired by this phase. The required `pitlord docs` ownership check could not run: both installed Pitlord binaries reject the `docs` subcommand. This tool capability gap remains explicit; no documentation-ownership success is claimed.

Phase 0 was implemented and verified locally. No Rust application source, public API, durable format or corpus changed.

## Related docs

- [Architecture](architecture.md) and [Invariants](invariants.md) own current dependency constraints.
- [Maintainer map](maintainer-map.md) routes campaign changes.
- [Architecture enforcement](../tools/pitlord/README.md) owns commands, policy and debt-ledger procedure.
- [Refactor specification](dependency-convergence-architecture-refactor-2026-10-21.md) owns phase scope and exclusions.

## Notes

Graph measurements and verification results below are evidence for this source baseline only. They do not establish completion of later phases. All feature exclusions in the specification remain in force.
