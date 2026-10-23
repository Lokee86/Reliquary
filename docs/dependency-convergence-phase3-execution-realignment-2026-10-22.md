# Phase 3 execution realignment — owner lifecycle, Maintenance and Import

Date: 2026-10-22
Status: **Execution addendum / documentation-only realignment; no new implementation acceptance**
Parent specification: [Phase 3 correction-only specification](dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md)
Progress register: [Phase 3 execution ledger](dependency-convergence-phase3-execution-2026-10-22.md)
Immutable authority: [34-component target DAG](dependency-convergence-target-dag-2026-10-21.md), [allowlist](../tools/pitlord/convergence-component-dag.json) and [Phase 3 frozen baseline](dependency-convergence-phase3-baseline-2026-10-22.md)
Concrete interfaces: [Phase 3 contracts](dependency-convergence-phase3-contracts-2026-10-22.md)

## Decision and reason for the change

The original P3.3/M3 block treated Maintenance ownership as eight serial tasks, but its contract also required moving the complete `Cva::open_from_container` / `Phylactery::open_from_container` reconstruction and validation stack into lower `owner_lifecycle`. That is a separate, substantial prerequisite involving REL/PHY state, replay, accepted history, retained evidence and physical source authority. The in-flight hard cut also completed preparation relevant to later P3.4 pruning and P3.7 Import before M3 could be accepted. Reporting only **23 of 98 tasks accepted** hid that progress; accepting an entire M3 task merely to increase the count would instead misstate verification.

**Correct the execution structure, not the architectural target or source.** Preserve the original 98 stable task IDs, all twelve formal gates, all accepted Phase 0–2 evidence, all frozen format/corpus authority, the approved settlement-size correction and the intended later phases. This is a **child-checkpoint overlay** for current execution and a clarification of which work each gate actually accepts; it is not 21 additional mandatory feature tasks or a restart. Tasks already accepted through P3.2.08 stay accepted. The separate checkpoints below are tracked as source/evidence preparations until their parent task or gate actually meets its criteria.

At the documentation snapshot, committed source is `1bfbc95` on `main`, following `408d775`, `8103711`, `d8ea6a5`, `0654564`, `f28ab2f` and `e09f331`. A further **uncommitted** REL Maintenance/Import hard cut is present. No source, test, fixture, graph capture, runtime, index or configuration file is changed by this planning correction.

## Four ownership lanes, not one accidental gate

1. **L — Lower REL/PHY lifecycle convergence.** `owner_lifecycle` constructs and validates concrete owner aggregates; decodes ordered physical replay and existing metadata; guards frozen source identity, physical cursor, repair staging, copy and publication. It must remain lower-level, not absorb semantic eligibility, authoring policy, inference, History, Import or Maintenance orchestration. Existing owner stores retain their own durable facts; lifecycle must not become a new `Cva`.
2. **H — Read-only History proof completion.** History owns cut/source, retained-evidence, publication-birth, epoch-scope and settlement *inspection* over immutable owner facts. Import/Freshness own settlement authoring and accepted publication; Maintenance owns mutation. Any new History helper, particularly the in-flight `history/transformed_copy.rs` and `history/reliquary_open.rs`, must be inspected for writer/publication capability before acceptance; a read-only module may not perform transformed writes.
3. **M — Maintenance mutation authority.** Explicit repair, authorized prune/reclaim, stable source/destination policy and thin public dispatch are here. The **M3 gate** proves a single authorized Maintenance operation boundary over the new lifecycle and History seams; detailed prune/reclaim semantics, retention coverage and obsolete route deletion remain the separate **P3 gate**, even if their implementation is moved early to avoid duplicate migrations.
4. **I — Import epoch policy preparation.** Import owns scoped readiness/admission, epoch/fence state, accepted completion and historical settlement publication through narrow lower capabilities. Physical `ImportSession` performs bounded guarded writes but cannot decide epoch semantics. Generic reconciliation is Maintenance; import-specific reconciliation is Import. **No Import → Maintenance edge**, and no owner/History upward callback. Imported functionality may be prepared now; **I3 acceptance still follows the original R3 gate**.

These lanes are responsibility views over the **existing accepted components**, not four new DAG nodes. Implemented source may span more than one lane because moving a consumer once can require the lower provider at the same time. Credit preparation in the lane it satisfies without marking its eventual formal gate accepted ahead of its prerequisites.

## Checkpoint register — narrower than the original 98 tasks

States: **Verified preparation** means the cited recorded checkpoint passed its scoped tests/guards at that source state; it does **not** mean the latest uncommitted worktree is green or the associated original task/gate is accepted. **Active/unverified** means code exists or is being extracted, but an up-to-date acceptance receipt is absent or failing. **Pending** means the closure obligation is not shown complete. The original ledger remains the authoritative formal acceptance count.

| ID | Bounded owner/result and evidence | State | Maps to |
| --- | --- | --- | --- |
| L.01 | PHY construction/profile/metadata and local replay moved below facade; lifecycle/PHY prep commit `8103711`, Maintenance preparation receipt | Verified preparation | P3.3 prerequisites; Phase 6 facade simplification |
| L.02 | Concrete REL aggregate, empty initialization and lower accepted physical replay; `d8ea6a5`, lower replay preparation receipt | Verified preparation | P3.3 lifecycle prerequisite |
| L.03 | Existing Dream/Freshness/External Index replay and acceptance ordering live below facade; `d8ea6a5`, 1,971-test bounded receipt | Verified preparation | P3.3 lifecycle prerequisite; future Runtime consumers |
| L.04 | Ordered REL finishing and accepted replay/tombstone refusal; `f28ab2f`, 1,974-test finalization receipt | Verified preparation | P3.3 lifecycle prerequisite |
| L.05 | Complete validated REL reopen routed through lower owner-lifecycle, final outward callers removed or thin | Active/unverified | P3.3.07–.08 |
| L.06 | Immutable REL/PHY physical reclamation source handles, freeze-before-scan/copy refusal; `d8ea6a5` receipt and two source-growth tests | Verified preparation | P3.3.05; P3.4.07 |
| H.01 | Nonmutating source digest, cuts/frontiers and owner proofs; `408d775`, accepted H3 gate | Verified preparation | P3.2 / H3 |
| H.02 | REL retention, vectors/descriptions and completed/pruned evidence composed read-only; `0654564`, retained-proof receipt | Verified preparation | P3.3.02–.04; P3.4.02–.06 |
| H.03 | Physical Import epoch-scope and birth-cohort evidence, with existing refusal order; `e09f331`, focused proof/cohort receipts | Verified preparation | P3.7 prep (not I3 acceptance) |
| H.04 | Historical settlement/Archive timeline/corpus content proof is read-only and publication remains higher; `1bfbc95`, 1,974-test settlement/session receipt | Verified preparation | P3.7–P3.9 prep; not settlement minimization |
| H.05 | Audit and prove all later History helper additions still cannot write, mutate, repair or authorize publication | Active/unverified | H3 invariant maintained through M3/I3/D3 |
| M.01 | Explicit PHY prune/reclaim and maintenance policy slices, with source and authorizer preserved; `8103711`, PHY/prune focused receipts | Verified preparation | P3.3 + P3.4 prep |
| M.02 | Explicit PHY tail/EntityResolution repair and compaction operation dispatch; `8103711`, seven repair + 32 CLI tests | Verified preparation | P3.3.06 |
| M.03 | REL reclamation implementation through Maintenance, including exact source/output identity and route migration | Active/unverified | P3.3.05–.07; P3.4.04–.07 |
| M.04 | REL prune implementation through Maintenance, original eligibility/retention order and destination proof | Active/unverified | P3.3.02–.04; P3.4.01–.06 |
| M.05 | Finish single Maintenance authorizer + thin outward entrypoints; no duplicate facade policy or unsupported shortcut | Active/unverified | P3.3.01–.08 / M3 |
| M.06 | Demonstrate M3 source/test/refusal acceptance, then independently finish all P3 pruning/reclaim parity and deletion obligations | Pending | M3 then P3; not a combined gate |
| I.01 | Scoped lifecycle `ImportSession`, durable fenced completion append without facade callback; `1bfbc95` receipt | Verified preparation | P3.7.04–.06 prep |
| I.02 | Import readiness and semantic admission rule extraction, including existing runtime/provider fact ordering | Active/unverified | P3.7.01–.03 prep |
| I.03 | Import completion coordinator and epoch continuation without writable facade callbacks or upward dependency | Active/unverified | P3.7.04–.08 prep |
| I.04 | Accept Import gate only when R3 prerequisite, epoch-ready/closed/failure/retry parity and service DAG checks pass | Pending | I3 formal gate |

**Snapshot tally:** 12 verified preparations, 7 active/unverified, 2 pending across these **21 current-cut checkpoints**. This is not `12/21 Phase 3 complete`: it is a diagnostic for the oversized M3/lifecycle prerequisite, separate from **23/98 accepted formal tasks and 3/12 accepted formal gates**. Recompute and cite receipts whenever this table changes. Do not infer completion from commit subjects or a previous test run after new code edits.

## Sequencing correction / gate attribution

- **H3 remains accepted**, with final full-suite historical identity anomaly carried into A3. A change to History after `408d775` must maintain H3 invariants, without reopening H3 by default.
- **L is the named lower-lifecycle enabling lane before M3 can close**; it is not a new formal gate and has no separate new dependency edge. Accept L checks independently when evidenced. A source cut may be red temporarily; don't stage compatibility wrappers or claim an unverified pass.
- **M3** closes when there is one narrow Maintenance authorization boundary for the existing explicit operations, validated owner-lifecycle source/open, owner-local proof input, writer/lease and refusal behavior, and outward error/public API mapping. Complete the M3-specific duplicate removal. It **does not** require the exhaustive P3 history prune/reclaim parity gate, W3 rewrite, R3 generic reconciliation or I3 Import completion to finish first.
- **P3** then closes exact REL+PHY pruning and reclamation selection, manifest, retained/removed proof, physical publication and obsolete policy route deletion. Early-implemented slices earn preparatory evidence, not a retroactive P3 pass.
- **W3** rewrite and **R3** generic reconciliation keep their original separate acceptance. Reuse lifecycle frozen-copy/proof operations when they already satisfy the required semantics; do not re-extract or create second implementations.
- **I3** remains formally after R3 per original dependency order. Its source can be migrated opportunistically while adjacent facade callers are changing; do not add an Import → Maintenance dependency. Later **S3 → C3** handles field audit, representation selection, current-only format migration, six canonical corpus promotion and obsolete codec/converter deletion.
- **D3** performs the single stabilized source graph/Arcana refresh and target-DAG production ratchet; **A3** completes the full Rust/CLI, long-running historical oracles, six canonical-file semantic/clock and byte/mtime acceptance, and documentation handoff. Arcana's existing `wrong field count` manifest remains a documented tooling blocker for the source-index-dependent check, not permission to recapture the baseline.
- **Phases 4–6 handoff:** lower lifecycle, physical frozen-copy, read-only History and scoped Import completion already extracted in Phase 3 are consumed **as accepted final interfaces** once their own gates pass. Do not schedule equivalent lifecycle/facade extractions a second time in Project Environment, Runtime or final facade cleanup.

## Exact paused handoff; no source changes authorized by this document

Committed tip at the planning snapshot is `1bfbc95`. Dirty source is the in-progress REL Maintenance/Import cut. The latest local compile log `.verification-tmp/phase3-maintenance-coordinator-check.log` failed in `src/maintenance/reliquary_prune.rs`: an unresolved `crate::OwnerError` import in the moved test module and an untyped `output_path.as_ref()` on a `&Path` causing cascading `E0283` errors. Those are **unfixed at this documentation snapshot**, and none of the dirty files is a verified build. Earlier `phase3-completion-coordinator-check.log` and `phase3-rel-reclamation-check.log` did compile before the later prune cut. The last recorded broader passing library run was **1,974 passed, zero failed, five ignored, four explicitly filtered long-running cases** for the `1bfbc95` settlement/session checkpoint, not this dirty tree.

On resumption: finish the already active REL prune/import change first; repair the reported compiler errors locally; run a **single non-overlapping** targeted build and relevant REL prune/reclaim, Import completion, REL reopen/repair and source/DAG guards. Account for the four filtered long-running tests and the historical `Container(StaleIdentity)` full-run anomaly only at an appropriate clean integration gate. Then update this checkpoint matrix and the original task ledger with the actual receipts; do not backfill success, run the six-file ~6,400-second oracle on every slice, spawn repeated inference reviews, or refresh Arcana until D3.

## Change-control rules

This document changes **planning, task sequencing and evidence attribution only**. It does not authorize new product behavior, changes to data formats, feature-roadmap work, new tests by the hundreds, alternative writer authority, rebaselining the original 34-node/163-allowed-edge DAG, or an additional generic transaction/replay framework. Existing hard-cut deletion policy and one-current-format development-corpus policy remain binding.

If later evidence shows a prerequisite is larger than expected, give it a bounded named checkpoint here; do not silently absorb unrelated gates into M3. Any original task acceptance still requires its original outcome and its concrete verification/deletion obligation. The original 98 IDs are never renumbered solely to improve progress percentages.
