# Reliquary Architecture Enforcement

Reliquary composes the shared Laughing Skull architecture-core Pitlord policy with repository-specific source ownership and static regression rules.

## Overview

`semantic.json` assigns every authored Rust file under `src/` to exactly one architectural owner. The owners describe current responsibility seams inside the still-single Rust crate; they do not claim that those seams are already separate crates:

- `freshness`: REL-local scores, admission, immutable receipts, canonical replay and bounded propagation;
- `container`: physical REL/PHY substrate and global ordering;
- `archive`: source history, conversation state, Episodes, attachments, Echo, and repository correlations;
- `memory`: authoritative Memory state;
- `external-source`: host canonical reference contracts, rebuildable indexes and external publication composition;
- `retrieval`: lexical/semantic retrieval plus vector/profile/generation state;
- `semantic-graph`: Graph, Communities, and typed semantic-node traversal;
- `perception`: durable Entity state plus candidate/admission/resolution workflows;
- `chronos`: shared temporal semantics;
- `ego`: Identity/Personality/Anchor/synthesis persistence;
- `runtime-inference`: Insomnia, Dream, providers, and RuntimeHost orchestration;
- `config-security`: machine-local configuration and credential/key state;
- `context-engine`: provider-neutral context/evidence and compaction policy;
- `project-environment`: Lore/Git bootstrap, identity and attachment adapters;
- `staleness`: pure typed clock and review-threshold arithmetic;
- `facade`: public Reliquary/Cva/Phylactery composition, migration, and reconciliation.

The source tree is physically grouped by the same owners under `src/`. Pitlord owns those directories as coarse regions, with `src/lib.rs` explicitly facade-owned at the crate root. This remains one Rust crate: the directory layout exposes responsibility and makes ownership/dependency analysis cheap without implying a crate split.

## Permanent logical component DAG

The architecture entrypoint runs `scripts/check_phase7_component_map.py` and its four focused fixtures before other source guards. This rejects missing cfg-reachable production files, duplicate map entries, unknown components and nonproduction classifications. It validates the recorded profile's file ownership only. `scripts/measure_phase7_baseline.py` produces unsuppressed current import/SCC evidence without refreshing Arcana; its successful execution is not architecture acceptance. See the [Phase 7 execution record](../../docs/dependency-convergence-phase7-execution-2026-10-24.md) for the open prerequisite and proof limits.

The accepted [target DAG design](../../docs/dependency-convergence-target-dag-2026-10-21.md) and [34-component, 163-edge allowlist](convergence-component-dag.json) are **separate from** the current 19-area source-path policy. Every edge is directional (consumer → dependency), ranked strictly downward, explicitly justified and typed read/write/construct/invoke. Backreaching imports are forbidden even without cycles; sibling edges are denied unless explicitly included. The target includes Context Engine → Ego/Retrieval, Runtime → Retrieval, Retrieval → Memory/Archive, Maintenance → read-only History, and independently owned Vector generation and Freshness.

Fast model validation: `python scripts/check_component_dag.py --self-test`, or via `python scripts/check_dependency_convergence.py --check-target`. The normal convergence gate validates this logical DAG **and separately** checks source-derived Pitlord findings. The logical graph must be mapped to concrete source modules as they migrate; existing `convergence-target.json` and the reviewed 4,609-finding Phase 0 baseline cannot simply be replaced or reset. Before Phase 7 closure, the source-level rules must enforce the finer component allowlist and then retire temporary baseline suppression while keeping permanent DAG constraints.

## Dependency convergence campaign

The root `policy.json` remains the unbaselined architecture policy. The dependency-convergence campaign uses a separate `convergence.json` target policy and separate evidence ledgers; it does not suppress or rebaseline findings from the root policy. The Phase 0 artifact roles are:

- `convergence.json`: target allowed dependency direction, represented as forbidden complements;
- `convergence-target.json`: explicit allowed consumer-to-dependency area matrix, checked against the forbidden complements;
- `convergence-baseline.json`: active Pitlord baseline ledger for the 4,609 classified exact findings from the completed capture;
- `convergence-debt.json`: active finding classifications, categories, removal phases, dispositions, and source-review evidence;
- `convergence-graph.json`: captured graph and source evidence, retained as the historical capture record.
- `convergence-import-audit.json`: derived source-review view of cross-area `imports` and `depends-on` edges; it is not fresh analyzer evidence or a complete production graph;
- `convergence-boundaries.json`: reviewed dispositions for all 146 captured directed domain edges, including permitted dependencies.

The baseline and debt ledgers are active enforcement state and shrink as findings are resolved. When a finding disappears, remove its entry from both active ledgers. Keep the graph capture as the evidence of the original Phase 0 graph; do not regenerate it to clear debt. `--capture` is the explicit initial-capture operation: it refuses to overwrite any existing baseline, debt ledger, or graph capture. It is a reviewed maintainer operation and is never run in CI. There is no automatic refresh, blanket suppression, or recapture to clear resolved debt. Capture-generated category, phase, and disposition values are provisional heuristics based on relation type, source path/name, and coarse area pair. Source review must confirm or correct them. The convergence checker requires `reviewed: true`, a category, phase, concrete disposition, `evidence_status` (`source-confirmed`, `test-only`, or `analyzer-artifact`), and nonempty `review_note`. Proven wrong bindings are recorded as analyzer artifacts, not fictional architecture debts. This separate ledger classifies findings from configured Pitlord analysis; it does not prove every source dependency was detected.

The ordinary `scripts/check_architecture.py` path checks the original unbaselined root policy and then invokes `scripts/check_dependency_convergence.py`, which runs its fixtures followed by the full convergence check. Use `python scripts/check_dependency_convergence.py --self-test` for an isolated fixture-only run. The full convergence check can also be run directly with `python scripts/check_dependency_convergence.py`.

The completed capture is bounded by the configured Rust source scope (`src/`) and Lexicon/Arcana adapter. The whole Arcana snapshot had 109,207 nodes and 297,303 relationships; the configured source scope had 98,633 nodes and 268,391 relationships, with 28,545 selected. All 1,102 source files were assigned once. The capture spans 19 areas/146 area dependencies, has one raw combined SCC spanning 16 areas, 9,537 cross-area module edges, and 298 public crossings. These measurements describe static capture only. The adapter includes colocated test files and does not distinguish `cfg(test)` reachability in its module-edge records. Crate-root reexports, inherent `impl Cva` blocks outside the defining module, trait/dynamic dispatch, macros, external adapters, and runtime callbacks need source review and explicit probes. Proven same-name call-target errors also mean raw call edges and SCCs must not be treated as confirmed production dependencies. Area assignment follows current paths and can differ from semantic responsibility. See the [Phase 0 inventory](../../docs/dependency-convergence-phase0-2026-10-21.md) for import-audit findings and capture limitations; missing static edges do not prove that no dependency exists.

## Existing source-ownership guard

The root policy also prevents the deleted generic `WorkspaceMetadata` / `workspace_type` authority from returning outside the explicit legacy migration compatibility boundary. Policy changes that alter architectural ownership must update `docs/architecture.md` and `docs/maintainer-map.md`; consequential ownership changes also require the appropriate ADR. Do not change ownership merely to silence a Pitlord finding.

Run locally with current evidence:

```text
python scripts/check_architecture.py
```

Refresh Lexicon/Arcana evidence and enforce the root policy in one command:

```text
python scripts/check_architecture.py --refresh
```

The checker resolves Pitlord, Lexicon, Arcana, and adapters from environment overrides, PATH, or the normal sibling development repositories. Reliquary follows the same sibling `engineering-standards` source used by its documentation checker, so the shared architecture-core include must be present there. The push/PR architecture workflow builds the tools, prebuilds the Rust Lexicon adapter in release mode, refreshes evidence, and runs the root policy check. It does not capture or refresh the convergence ledgers.

The refresh explicitly selects Lexicon Rust analysis because the ownership policy covers authored Rust files under `src/`. Python ownership/coverage guards run directly before graph refresh. This prevents unrelated Python-adapter indexing from blocking Rust ownership evidence, while retaining a fresh Arcana sync and complete root-policy check. This gate does not claim a Python dependency graph.
