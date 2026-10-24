# Phase 7 execution evidence

Parent index: [Documentation index](INDEX.md)

## Purpose

Record local execution of the [31-ticket Phase 7 plan](dependency-convergence-phase7-final-convergence-spec-2026-10-23.md) without substituting bounded import evidence for architecture acceptance.

## Overview

Execution started from `0495689c34aa9a1c10699fd31f7b504db03b9492` on 2026-10-24, using local Luna subagents and no connectors. The supplied specification was the only initial untracked file. No Rust implementation or persistent format changed.

**Phase 7 is not accepted.** Phase 5's source/behavior checkpoint is integrated and Phase 6 A6 is accepted. The controlling specification now explicitly permits B7/H7/R7 execution from these checkpoints without an accepted Phase 5 A5 full architecture refresh. Phase 5 P5.7.1 and its full graph proof remain incomplete; Phase 7 A7 must resolve that evidence gap against the original final architecture contract. No A5 completion or reduced proof standard is implied.

The [baseline receipt](../benchmarks/phase7-baseline.json) measures current cfg-aware source imports against the existing 34-component map. It retains every unapproved edge and uncertainty rather than consulting historical debt as permission. The [runtime residual register](../benchmarks/phase7-runtime-residuals.json) anchors the remaining runtime/public aggregate witnesses and candidate cuts. Neither receipt is a compiler-resolved call graph.

The corrected-map measurement contains 3,580 cross-component import rows, including 1,284 rows across 40 pairs without a target-DAG edge or exact registered immutable-value contract, plus 97 uncertain imports. These are review candidates, not 1,284 proven executable violations. Their capability/value dispositions and ownership cuts remain B7 work. The strict graph has one two-component SCC with 138 internal import rows and five unapproved upward rows. The supplementary graph containing only unapproved edges has zero SCCs; that does **not** make the actual graph acyclic, because cycles can combine permitted and forbidden edges. The two rows added relative to A6's 3,578 come from restoring the omitted file mapping, not from new source dependencies.

## Gate disposition

| Gate | Disposition | Evidence still needed |
|---|---|---|
| B7 | Open; bounded baseline and source witness preparation | Comparable historical coverage and exact classification of candidate imports; material call/cfg uncertainty resolved or bounded by concrete source evidence. Phase 5 A5 is an A7 proof obligation, not a B7 prerequisite. |
| H7 | Not started | Accepted B7 and actual implementation cuts or proved absence for every conditional seam |
| R7 | Not accepted; standalone source-policy checks prepared | Integration after B7/H7, representative callback/placement negatives and material uncertainty disposition |
| A7 | Open | Source-proven allowed DAG, complete required verification, documentation ownership and integrated final implementation |

P7.0.1–P7.0.4 have evidence preparation, not gate acceptance. None of the sixteen P7.1 conditional categories is closed merely because no import witness was found. P7.2 tooling is independent preparation and does not accept R7. P7.3 integration and formal closeout remain unfulfilled. The controlling denominator remains 31 tickets and four gates. Proposed runtime children are scope proposals, not completed tickets or an approved denominator change.

The new reachable-file ownership check found one missing explicit row for `src/runtime_inference/insomnia/native_processor.rs`. The existing owner policy already assigns `insomnia::native_processor` to Insomnia; direct inspection confirms claim validation and Archive reader evidence preparation. Adding that row reconciles the inventory without changing production responsibility. The check went from one error to zero across 859 reachable files. Four positive/negative fixtures protect missing files, duplicate entries, invalid classifications and unknown components; the architecture entrypoint now runs them and the live map check before its existing guards.

## Residual source boundary

The inherited strict import SCC is `public-consumers` / `runtime-gateway`. Five known upward imports bind runtime composition directly to `Cva` or `Phylactery`: interaction runtime, Gateway, project lifecycle, PHY execution and offline reconciliation. Gateway and PHY workers actually store/use the public aggregate. Moving imports or public methods without separating concrete execution state would leave that capability dependency intact.

The residual register proposes separate scopes for Gateway/PHY storage, interaction binding, project administration and offline reconciliation. Preserve the exact public construction/access/unmount contracts, one mounted owner writer, independent instance grants, publication order, queues, cancellation and error/reopen behavior. Do not introduce a generalized owner manager or broaden the target DAG to clear this SCC. Each scope requires its callsite/consumer proof and preserving tests before implementation.

## Reproduction and verification

Run `python scripts/measure_phase7_baseline.py` to regenerate bounded source evidence. Its report includes its proof limits and input hashes. Source-policy fixture and live-tree commands, results and remaining tool limits are recorded in the [verification receipt](../benchmarks/phase7-verification.json). Full Lexicon/Arcana refresh was not repeated: the plan forbids relaunching the inherited stalled full export as a substitute for bounded evidence.

Rust/CLI/corpus acceptance in the Phase 6 receipt remains historical evidence for unchanged Rust; no fresh full behavior pass is claimed. No retained corpus or wire-format migration was performed. Both existing Phase 5 worktrees were clean, and `main..phase5-runtime-convergence` contained no commits; they are preserved. This read-only audit does not accept final A7 integration.

## Documentation impact

- Inspected: architecture, invariants, storage format, maintainer map, limitations, roadmap, Phase 5/6 receipts and Phase 7 plan.
- Updated: current graph-proof limitation, evidence/tool ownership, navigation and this execution record.
- Not affected: supported public APIs, V2 schemas, domain policy, runtime behavior and subsystem contracts.
- Compliance check: see local verification receipt; unavailable or failing tools remain explicit.
- Known documentation gaps: full Pitlord documentation ownership is not accepted without a working command.

## Architecture impact

- Standards added or changed: none; 34 components and 163 allowed edges remain authoritative.
- Ownership or boundary impact: one omitted file row restored to its existing Insomnia owner; no Rust cut.
- Pitlord enforcement impact: reachable-file mapping and its fixtures run before the existing architecture gate; no debt suppression or gate replacement.
- Other verification impact: reproducible current source inventory, exact import/SCC witnesses and negative policy fixtures.
- Known architectural gaps: inherited A5 full graph-proof obligation, runtime/public SCC, material call/cfg proof, historical comparability and final A7 integration remain open. Phase 5 A5 is not a Phase 7 execution prerequisite.

## Related docs

- [Phase 7 specification](dependency-convergence-phase7-final-convergence-spec-2026-10-23.md)
- [Phase 5 execution](dependency-convergence-phase5-execution-2026-10-23.md)
- [Phase 6 final acceptance receipt](../benchmarks/phase6-final-verification.json)
- [Architecture](architecture.md)
- [Current limitations](current-limitations.md)

## Notes

Resume B7 evidence reconciliation immediately from the pinned integrated Phase 6 baseline and residual register; proceed to witnessed H7 source cuts only after B7 acceptance. The incomplete Phase 5 A5 graph proof remains required for A7, not B7. No Phase 8 or reduced closure standard is introduced.
