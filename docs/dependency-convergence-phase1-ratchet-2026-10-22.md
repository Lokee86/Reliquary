# Phase 1 dependency convergence ratchet — 2026-10-22

## Purpose

Record the source-reviewed changes to the active dependency debt ledgers while
preserving the original Phase 0 before-image.

## Overview

This is a ledger reconciliation checkpoint against the existing local `.arcana` snapshot. It is provisional: the indexed `.lexicon/repo/source` snapshot differs from the current Rust tree in 16 files, including 7 production files. The captured `tools/pitlord/convergence-graph.json` was deliberately left unchanged. Refresh and review the Arcana snapshot after source edits settle before treating this ledger as final acceptance.

## Reconciliation performed

The current Pitlord report contains 4,620 findings, compared with 4,609 in the prior baseline: 66 current findings were absent from the baseline and 55 baseline findings were absent from the current report. The baseline and debt ledgers now contain the same 4,620 fingerprints.

All 55 stale fingerprints were deleted from both ledgers. This removes resolved findings instead of preserving them as historical debt.

For the 66 current findings, 53 matched an existing stale fingerprint through the analyzer's `same_edge_stale_candidates` evidence. Their reviewed classification and phase/disposition were retained, and the record was rebound to the current endpoint paths and symbol names. The 13 remaining findings are recorded as test-only after checking their source locations/names; they are test modules and do not add production dependency direction. No new production finding was accepted as debt.

The production rebindings are relocations/renames recorded from the stale-candidate evidence and direct source review:

| Current source → target | Prior endpoint evidence | Review disposition |
| --- | --- | --- |
| `rewrite/plan.rs` closure → `facade/cva.rs::memory_revision` | Same source/target call, closure span shifted from line 235 to 229 | Retained existing history-rewrite responsibility |
| `rewrite/plan.rs` closure → `facade/cva_freshness.rs::freshness_policy` | Same source/target call, closure span shifted from line 490 to 486 | Retained existing history-rewrite responsibility |
| `rewrite/plan.rs` closure → `facade/phylactery.rs::memory_revision` | Same source/target call, closure span shifted from line 271 to 265 | Retained existing history-rewrite responsibility |
| `rewrite/file.rs` callers → facade `open_read_only` / `open_from_container` | Existing rewrite-to-facade `open` calls under the matching caller symbols | Retained existing history-rewrite composition responsibility; no new owner edge approved |
| `imports/reconciliation.rs::reconcile_import_epochs` → facade `open_read_only` | Existing call to facade `open` | Retained existing native-import composition responsibility |
| `imports/reconciliation_replay.rs::replay` → facade `open_from_container` | Existing call to facade `open` | Retained existing native-import replay responsibility |
| `project_environment/lifecycle.rs::inspect_project_adoption_source` → facade `open_read_only` | Existing call to facade `open` | Retained existing project-environment source inspection responsibility |
| `perception/entity_resolution_store_write.rs::retired_entity_reference_repairs` → memory historical-key validation and error | Existing `repair_retired_entity_references` calls to those same Memory endpoints | Retained the existing Phase 1 semantic-repair integration debt; not considered resolved by the renamed method |
| `freshness/freshness_storage_stream.rs` closure → archive `decode` | Existing closure call to the same Archive decoder | Retained the existing freshness/archive codec dependency |

The `rewrite/file.rs` row includes the matching call sites for `plan_rewrite_file`, `validate_emitted`, `emit_phy`, `emit_rel`, and their REL/PHY facade open/replay targets. Existing stale records were matched by source symbol where several candidates shared the same file and target.

## Gate status and limits

This is not a final acceptance claim. The snapshot predates the latest source edits, so its graph cannot prove that the current tree has no additional or resolved findings. Do not refresh `convergence-graph.json` as a blanket recapture; refresh the analysis only after the final source tree is stable, then review new production evidence individually and delete resolved findings explicitly.

## Related docs

[Phase 1 verification](dependency-convergence-phase1-verification-2026-10-22.md),
[Phase 0 capture](dependency-convergence-phase0-2026-10-21.md), and
[target DAG](dependency-convergence-target-dag-2026-10-21.md).

## Notes

The convergence checker also rejects a refreshed graph whose indexed Rust source
manifest differs from the authored tree; advancing the snapshot ID alone cannot
establish freshness.


## Final Phase 1 handoff audit

The initial Phase 0 routing labels are not themselves evidence of production physical-layer inversion. The active Phase 1 bucket contained 106 findings when reviewed: 80 test-only edges, 25 analyzer symbol-resolution artifacts, and one source-confirmed production semantic read. Eight of the test-only edges had been marked `source-confirmed` because their relocated test call sites were source-reviewed; they remain tests and do not count as production dependencies.

The 25 analyzer-artifact findings were moved to Phase 7 with their original source inspection preserved and an appended audit rationale. These records describe missing imports/references or incorrectly resolved symbols (including Container framing helpers that are locally owned), not actual cross-owner calls. Their Phase 7 disposition is to correct analyzer evidence or adapter/policy handling; none is accepted as production dependency debt.

The sole source-confirmed non-test Phase 1 edge, `src/freshness/freshness_storage_stream.rs` → `src/archive/archive_import.rs::decode`, was moved to Phase 2. Freshness safely uses the bounded physical scan but interprets Archive import payloads, making this a later-scope semantic read/proof ownership issue rather than a physical Container inversion. Its Phase 2 disposition records the likely neutral codec/contract seam or Freshness-owned decoding route.

After this audit, 80 Phase 1 findings remain and all are test-only. No source-reviewed production physical upward edge was found in the remaining Phase 1 entries. The handoff still depends on the fresh graph analysis currently running; this note does not certify final graph acceptance.


## Final source-acceptance evidence

The frozen source scan completed on 2026-10-22 and published Arcana snapshot `sha256:160581ebf8726e4ac6010578c809c81786f49cc430a5c61a7cd4bfbb329ddc22`. Pitlord reported no violations under the 25-rule semantic policy (99,791 source nodes and 9,264 relationships). The original `tools/pitlord/convergence-graph.json` capture remains unchanged.

The fresh convergence report contains 4,625 findings. Reconciliation deleted 9 stale fingerprints, rebound 9 equivalent moved findings, and added 5 new test-only records. The 7 test rebindings cover entity-audit fixture call-site/symbol changes and semantic-graph test closure span changes. The other 2 rebindings are `runtime_host_reconciliation.rs` calls now targeting `open_read_only`; source review confirms they are the existing runtime-to-facade composition already classified as Phase 5 debt. The 5 added findings are test edges in cfg(test)-gated interaction-stream and Memory publication-writer modules. No new production dependency was accepted, and no new physical-layer upward dependency was found. The final Phase 1 bucket is 92 findings, all test-only: the earlier 80-entry handoff count plus 7 rebound test records and 5 newly classified test records.

The convergence checker passed with 4,625 classified findings and no new or stale debt; its 13 positive/negative fixtures passed. `check_architecture.py` also passed against the fresh snapshot and confirmed no Pitlord violations. The physical-boundary guard passed all 13 fixtures, and the external-ownership production guard passed. These results supersede the provisional scan note above for this frozen source tree. The Phase 2 Freshness-to-Archive semantic payload read remains explicitly tracked; its later-scope disposition is not a claim that it is resolved.

### Final cleanup refresh

A final failure-path audit extended identity-checked staging cleanup in
`container_repair.rs` to lease-acquisition and late validation errors. The source
was refreshed again after that edit. The accepted final snapshot is
`sha256:01a79a32693b56f712748f2a62c973ebf5a4bc9e8d34f98ba1b5a1c64ec7c69b`:
25 rules, 99,800 source nodes and 9,233 relationships, with no violations.
The convergence guard again passed all 13 fixtures and all 4,625 classified
findings, with no new or stale debt. No ledger recapture was needed for this
cleanup. `.verification-tmp/phase1-source-cleanup-acceptance.log` records the
refresh and successful checks. This snapshot supersedes the preceding snapshot
for the final source manifest.
