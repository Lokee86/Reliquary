# Phase 3 execution realignment — owner lifecycle, Maintenance and Import

Date: 2026-10-22
Status: **Phase 3 executable work breakdown v2 — 119 unique tasks; source continuation verified at bounded scope**
Parent specification: [Phase 3 correction-only specification](dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md)
Progress register: [Phase 3 execution ledger](dependency-convergence-phase3-execution-2026-10-22.md)
Immutable authority: [34-component target DAG](dependency-convergence-target-dag-2026-10-21.md), [allowlist](../tools/pitlord/convergence-component-dag.json) and [Phase 3 frozen baseline](dependency-convergence-phase3-baseline-2026-10-22.md)
Concrete interfaces: [Phase 3 contracts](dependency-convergence-phase3-contracts-2026-10-22.md)

## Purpose

Keep the 119-task execution breakdown, revised sequencing and source handoff aligned with independently verified outcomes. The execution ledger remains the formal gate register.

## Overview

The original planning snapshot below explains the 21 additional scoped tasks. Continuation evidence is recorded separately; historical failures and earlier receipts remain attributed to their original checkpoints.

## Decision and reason for the change

The original P3.3/M3 block treated Maintenance ownership as eight serial tasks, but its contract also required moving the complete `Cva::open_from_container` / `Phylactery::open_from_container` reconstruction and validation stack into lower `owner_lifecycle`. That is a separate, substantial prerequisite involving REL/PHY state, replay, accepted history, retained evidence and physical source authority. The in-flight hard cut also completed preparation relevant to later P3.4 pruning and P3.7 Import before M3 could be accepted. Reporting only **23 of 98 tasks accepted** hid that progress; accepting an entire M3 task merely to increase the count would instead misstate verification.

**The 21 new rows are real execution tasks and count toward Phase 3.** Preserve the original 98 stable task IDs, all twelve formal gates, accepted Phase 0–2 evidence, frozen format/corpus authority, the approved settlement-size correction and the intended later phases. The revised task denominator is **98 original + 21 separately scoped enabling/extraction/verification tasks = 119**. This is a correction-only breakdown, not 21 new product features, new runtime functionality or a restart. The original 98 rows continue to represent their remaining owner-policy, integration, hard-cut deletion and gate-level acceptance obligations; a completed enabling task cannot be counted *again* as that parent task's completion without its additional original obligations. Tasks already accepted through P3.2.08 remain accepted. The 21 rows below are unique accepted/in-progress/pending implementation outcomes, not uncounted commentary. Where work was performed before the revision, its acceptance is retrospective **only to the scope proven by the cited source and tests**.

At the documentation snapshot, committed source is `1bfbc95` on `main`, following `408d775`, `8103711`, `d8ea6a5`, `0654564`, `f28ab2f` and `e09f331`. A further **uncommitted** REL Maintenance/Import hard cut is present. No source, test, fixture, graph capture, runtime, index or configuration file is changed by this planning correction.

## Four ownership lanes, not one accidental gate

1. **L — Lower REL/PHY lifecycle convergence.** `owner_lifecycle` constructs and validates concrete owner aggregates; decodes ordered physical replay and existing metadata; guards frozen source identity, physical cursor, repair staging, copy and publication. It must remain lower-level, not absorb semantic eligibility, authoring policy, inference, History, Import or Maintenance orchestration. Existing owner stores retain their own durable facts; lifecycle must not become a new `Cva`.
2. **H — Read-only History proof completion.** History owns cut/source, retained-evidence, publication-birth, epoch-scope and settlement *inspection* over immutable owner facts. Import/Freshness own settlement authoring and accepted publication; Maintenance owns mutation. Any new History helper, particularly the in-flight `history/transformed_copy.rs` and `history/reliquary_open.rs`, must be inspected for writer/publication capability before acceptance; a read-only module may not perform transformed writes.
3. **M — Maintenance mutation authority.** Explicit repair, authorized prune/reclaim, stable source/destination policy and thin public dispatch are here. The **M3 gate** proves a single authorized Maintenance operation boundary over the new lifecycle and History seams; detailed prune/reclaim semantics, retention coverage and obsolete route deletion remain the separate **P3 gate**, even if their implementation is moved early to avoid duplicate migrations.
4. **I — Import epoch policy preparation.** Import owns scoped readiness/admission, epoch/fence state, accepted completion and historical settlement publication through narrow lower capabilities. Physical `ImportSession` performs bounded guarded writes but cannot decide epoch semantics. Generic reconciliation is Maintenance; import-specific reconciliation is Import. **No Import → Maintenance edge**, and no owner/History upward callback. Imported functionality may be prepared now; **I3 acceptance still follows the original R3 gate**.

These lanes are responsibility views over the **existing accepted components**, not four new DAG nodes. Implemented source may span more than one lane because moving a consumer once can require the lower provider at the same time. Credit preparation in the lane it satisfies without marking its eventual formal gate accepted ahead of its prerequisites.

## Added execution tasks — 21 unique bounded outcomes (part of the 119-task denominator)

States: **Verified (scoped)** means that the bounded extraction/evidence output described in this *new* row was independently closed at its cited committed source/test checkpoint; it counts as one completed task, **not** as acceptance of an overlapping original integration task or gate. It does **not** establish that the latest uncommitted worktree is green. **Active/unverified** means source or integration is underway without a passing current checkpoint for that bounded output. **Pending** means the task's evidence/closure obligation remains. The [execution ledger](dependency-convergence-phase3-execution-2026-10-22.md) combines these 21 rows with the original 98 for a **single 119-task progress count**; no gate is accepted early.

| ID | Bounded owner/result and evidence | State | Maps to |
| --- | --- | --- | --- |
| L.01 | PHY construction/profile/metadata and local replay moved below facade; lifecycle/PHY prep commit `8103711`, Maintenance preparation receipt | Verified (scoped) | P3.3 prerequisites; Phase 6 facade simplification |
| L.02 | Concrete REL aggregate, empty initialization and lower accepted physical replay; `d8ea6a5`, lower replay preparation receipt | Verified (scoped) | P3.3 lifecycle prerequisite |
| L.03 | Existing Dream/Freshness/External Index replay and acceptance ordering live below facade; `d8ea6a5`, 1,971-test bounded receipt | Verified (scoped) | P3.3 lifecycle prerequisite; future Runtime consumers |
| L.04 | Ordered REL finishing and accepted replay/tombstone refusal; `f28ab2f`, 1,974-test finalization receipt | Verified (scoped) | P3.3 lifecycle prerequisite |
| L.05 | Complete validated REL reopen routed through lower owner-lifecycle, final outward callers removed or thin; continuation receipt below | Verified (scoped) | P3.3.07–.08 |
| L.06 | Immutable REL/PHY physical reclamation source handles, freeze-before-scan/copy refusal; `d8ea6a5` receipt and two source-growth tests | Verified (scoped) | P3.3.05; P3.4.07 |
| H.01 | Post-H3, extract read-only physical Import-completion evidence into a History-owned proof rather than leaving it in Import mutation/coordinator code; `0654564` and retained-proof receipt | Verified (scoped) | P3.7.04–.06 enabling proof; not a second credit for accepted H3 |
| H.02 | REL retention, vectors/descriptions and completed/pruned evidence composed read-only; `0654564`, retained-proof receipt | Verified (scoped) | P3.3.02–.04; P3.4.02–.06 |
| H.03 | Physical Import epoch-scope and birth-cohort evidence, with existing refusal order; `e09f331`, focused proof/cohort receipts | Verified (scoped) | P3.7 prep (not I3 acceptance) |
| H.04 | Historical settlement/Archive timeline/corpus content proof is read-only and publication remains higher; `1bfbc95`, 1,974-test settlement/session receipt | Verified (scoped) | P3.7–P3.9 prep; not settlement minimization |
| H.05 | Audit and prove all later History helper additions still cannot write, mutate, repair or authorize publication; continuation receipt below | Verified (scoped) | H3 invariant maintained through M3/I3/D3 |
| M.01 | Explicit PHY prune/reclaim and maintenance policy slices, with source and authorizer preserved; `8103711`, PHY/prune focused receipts | Verified (scoped) | P3.3 + P3.4 prep |
| M.02 | Explicit PHY tail/EntityResolution repair and compaction operation dispatch; `8103711`, seven repair + 32 CLI tests | Verified (scoped) | P3.3.06 |
| M.03 | REL reclamation implementation through Maintenance, including exact source/output identity and route migration; continuation receipt below | Verified (scoped) | P3.3.05–.07; P3.4.04–.07 |
| M.04 | REL prune implementation through Maintenance, original eligibility/retention order and destination proof; continuation receipt below | Verified (scoped) | P3.3.02–.04; P3.4.01–.06 |
| M.05 | Finish single Maintenance authorizer + thin outward entrypoints; no duplicate facade policy or unsupported shortcut; continuation receipt below | Verified (scoped) | P3.3.01–.08 / M3 |
| M.06 | Produce one bounded REL/PHY Maintenance authorization + prune/reclaim verification matrix, with source guards and refusal-order evidence; formal M3 and P3 closure still require their separate original obligations; continuation receipt below | Verified (scoped) | P3.3.08 and P3.4.08 test evidence; neither gate accepted by this row alone |
| I.01 | Scoped lifecycle `ImportSession`, durable fenced completion append without facade callback; `1bfbc95` receipt | Verified (scoped) | P3.7.04–.06 prep |
| I.02 | Import readiness and semantic admission rule extraction, including existing runtime/provider fact ordering; continuation receipt below | Verified (scoped) | P3.7.01–.03 prep |
| I.03 | Import completion coordinator and epoch continuation without writable facade callbacks or upward dependency | Active/unverified | P3.7.04–.08 prep |
| I.04 | Record focused Import ready/closed, failure/retry and source-DAG parity evidence on a coherent source checkpoint; integrate it into I3 only after R3 prerequisite; continuation receipt below | Verified (scoped) | P3.7.07–.08 enabling verification; not independent I3 gate acceptance |

**Historical planning snapshot (superseded by continuation below):** **35 of 119 tasks closed at their own acceptance scope (29.4%)** = **23 original formally accepted tasks + 12 new verified (scoped) tasks**. The remaining **84 tasks** are **7 active/unverified new tasks, 2 pending new tasks and 75 pending original tasks**. The twelve formal gates are tracked independently (**B0/B1/H3 accepted; M3 and later nine pending**). New scoped closure is real task completion, but **not** a claim of full latest-tree green or an advance of any original task without its full obligations. Never calculate 35/98 or claim all 35 had full integration acceptance. The two populations are disjoint task IDs, not repeat credit for identical original completion conditions; any redundant new row discovered must be retired or rewritten as an independent deliverable before counting it. Recompute from unique IDs and cited receipts, not inference or a time estimate.

## Sequencing correction / gate attribution

- **H3 remains accepted**, with final full-suite historical identity anomaly carried into A3. A change to History after `408d775` must maintain H3 invariants, without reopening H3 by default.
- **L comprises separately counted lower-lifecycle enabling execution tasks before M3 closes**; it is not a new formal gate and adds no dependency edge. Accept each L output independently when its bounded tests and source deletion are evidenced; M3 still needs the authorizer and integration obligations in original P3.3. A source cut may be red temporarily; don't stage compatibility wrappers or claim an unverified pass.
- **M3** closes when there is one narrow Maintenance authorization boundary for the existing explicit operations, validated owner-lifecycle source/open, owner-local proof input, writer/lease and refusal behavior, and outward error/public API mapping. Complete the M3-specific duplicate removal. It **does not** require the exhaustive P3 history prune/reclaim parity gate, W3 rewrite, R3 generic reconciliation or I3 Import completion to finish first.
- **P3** then closes exact REL+PHY pruning and reclamation selection, manifest, retained/removed proof, physical publication and obsolete policy route deletion. Early-implemented slices earn preparatory evidence, not a retroactive P3 pass.
- **W3** rewrite and **R3** generic reconciliation keep their original separate acceptance. Reuse lifecycle frozen-copy/proof operations when they already satisfy the required semantics; do not re-extract or create second implementations.
- **I3** remains formally after R3 per original dependency order. Its source can be migrated opportunistically while adjacent facade callers are changing; do not add an Import → Maintenance dependency. Later **S3 → C3** handles field audit, representation selection, current-only format migration, six canonical corpus promotion and obsolete codec/converter deletion.
- **D3** performs the single stabilized source graph/Arcana refresh and target-DAG production ratchet; **A3** completes the full Rust/CLI, long-running historical oracles, six canonical-file semantic/clock and byte/mtime acceptance, and documentation handoff. Arcana's existing `wrong field count` manifest remains a documented tooling blocker for the source-index-dependent check, not permission to recapture the baseline.
- **Phases 4–6 handoff:** lower lifecycle, physical frozen-copy, read-only History and scoped Import completion already extracted in Phase 3 are consumed **as accepted final interfaces** once their own gates pass. Do not schedule equivalent lifecycle/facade extractions a second time in Project Environment, Runtime or final facade cleanup.

## Original paused handoff (historical snapshot)

Committed tip at the planning snapshot is `1bfbc95`. Dirty source is the in-progress REL Maintenance/Import cut. The latest local compile log `.verification-tmp/phase3-maintenance-coordinator-check.log` failed in `src/maintenance/reliquary_prune.rs`: an unresolved `crate::OwnerError` import in the moved test module and an untyped `output_path.as_ref()` on a `&Path` causing cascading `E0283` errors. Those are **unfixed at this documentation snapshot**, and none of the dirty files is a verified build. Earlier `phase3-completion-coordinator-check.log` and `phase3-rel-reclamation-check.log` did compile before the later prune cut. The last recorded broader passing library run was **1,974 passed, zero failed, five ignored, four explicitly filtered long-running cases** for the `1bfbc95` settlement/session checkpoint, not this dirty tree.

On resumption: finish the already active REL prune/import change first; repair the reported compiler errors locally; run a **single non-overlapping** targeted build and relevant REL prune/reclaim, Import completion, REL reopen/repair and source/DAG guards. Account for the four filtered long-running tests and the historical `Container(StaleIdentity)` full-run anomaly only at an appropriate clean integration gate. Then update this 21-task register and the original 98-task ledger, and recompute the unified 119-task progress count from their actual receipts; do not backfill success, run the six-file ~6,400-second oracle on every slice, spawn repeated inference reviews, or refresh Arcana until D3.

## Change-control rules

This document changes **planning, task sequencing and evidence attribution only**. It does not authorize new product behavior, changes to data formats, feature-roadmap work, new tests by the hundreds, alternative writer authority, rebaselining the original 34-node/163-allowed-edge DAG, or an additional generic transaction/replay framework. Existing hard-cut deletion policy and one-current-format development-corpus policy remain binding.

If later evidence shows a prerequisite is larger than expected, give it a bounded named checkpoint here; do not silently absorb unrelated gates into M3. Any original task acceptance still requires its original outcome and its concrete verification/deletion obligation. The original 98 IDs and twelve formal gates are never renumbered solely to improve progress percentages. The added L/H/M/I IDs are independently counted and must each have a verifiable output distinct from the original task's *remaining* integration/deletion obligations. Total execution tasks are **119**, not 98. The 21 added rows may be completed early when evidence warrants; their parent gates are not silently accepted.
## Verified source continuation after ceaa5b8

At the `d685e0c` continuation, progress was **59/119 closed (49.6%)**: **39 original tasks accepted + 20 added tasks verified**, with **one added task active (I.03) and 59 original tasks pending**. Formal gates **B0/B1/H3/M3/P3 are locally accepted (5/12)**; W3 and later gates remain pending. The original 35/119 snapshot above is historical.

The [REL Maintenance/Import continuation receipt](../benchmarks/dependency-convergence-phase3-rel-maintenance-import-resume-2026-10-22.json) records the bounded source checkpoint, source hashes and overlapping REL/PHY authorization, retention, prune/reclaim, explicit repair, source identity and Import verification matrix. The two paused compiler errors are fixed. History now borrows replay state and returns `ReliquaryReconstruction` without the physical session; lifecycle retains final writer-bearing aggregate assembly. The History guard rejects owned replay inputs, writer aggregates and handoff calls. `transformed_copy.rs` only compares immutable accepted facts. Maintenance owns REL prune/reclaim/repair policy and the public adapters dispatch through it. Import owns readiness report decisions and completion policy; scoped lifecycle publication retains the fence through sync.

The **1,975-pass** bounded library receipt precedes the last readiness-report extraction; the final **90-pass Import slice** verifies that adjustment. The final source also passes the locked CLI check and **32 CLI tests**. All six canonical hashes match the frozen baseline. The initial restricted library attempt failed and stalled, then was terminated; it is not a passing receipt. The permitted local rerun passed. Four long cases and five ignored tests remain A3 obligations.

The added scopes close independently from the original M3/P3 obligations: lifecycle construction and read-only capability evidence close L.05/H.05; concrete operation extractions close M.03/M.04/M.05; the explicit cross-operation verification matrix closes M.06; owner-work/readiness extraction and final failure/retry/source-DAG evidence close I.02/I.04. Original P3.3/P3.4 acceptance additionally verifies supported public integration, original error/refusal order, owner-specific eligibility/retention, exact marker/raw-copy proof, transaction clocks, source/output identity, and deletion of the facade policy implementations. I.03 remains active because Import epoch-aware reconciliation still needs its canonical lower-service cut under R3/I3; completion extraction alone does not close that row.

Next source work is **P3.5/W3 rewrite**. Preserve the source receipts and current sole CVAFRS06 codec; do not run or claim C3 conversion, the six-file semantic oracle, D3 graph refresh or A3 full-suite acceptance here. Pitlord documentation ownership remains unavailable: the installed binary lacks `docs`, and the existing local docs-capable binary fails on Arcana's preexisting `wrong field count` manifest. This is a disclosed tool blocker, not a pass or authority to refresh the graph ahead of D3.

## W3 accepted continuation

The W3 checkpoint progress was **67/119 closed (56.3%)**: **47 original tasks accepted + 20 added tasks verified**; **I.03 remains active and 51 original tasks remain pending**. **B0/B1/H3/M3/P3/W3 are accepted locally (6/12)**. The [W3 receipt](../benchmarks/dependency-convergence-phase3-rewrite-2026-10-22.json) records 1,975 bounded library passes, 18 public rewrite contract passes and 32 CLI passes, exact source hashes, planner/physical/owner guards and unchanged canonical hashes. The original eight P3.5 obligations close on their distinct canonical policy, immutable input, concrete emission, exact selection, source/lineage refusal, public integration and obsolete-route deletion evidence.

Next work is **P3.6/R3**, separating generic Maintenance reconciliation from Import epoch policy. Reuse the accepted local Archive replay and validated lifecycle/History seams. No Import-to-Maintenance callback is permitted. I3 remains after R3; S3/C3 still require measured representation alternatives and verified current-only corpus conversion; D3/A3 retain the single source refresh and full long/oracle acceptance obligations.

## Related docs

- [Phase 3 specification](dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md)
- [Execution ledger](dependency-convergence-phase3-execution-2026-10-22.md)
- [Concrete contracts](dependency-convergence-phase3-contracts-2026-10-22.md)

## Notes

A passing bounded batch does not replace A3 full integration or the C3 converted-corpus oracle. The four deferred long tests, ignored opt-in tests and earlier H3 full-run identity anomaly remain explicit acceptance obligations.


## R3 continuation verification

The [reconciliation receipt](../benchmarks/dependency-convergence-phase3-reconciliation-2026-10-22.json) records the generic Maintenance move, lower producer replay, explicit Import coordinate mapping and identity-owned cleanup. The bounded library run passed 1,975 tests before final review corrections; final source passed 86 reconciliation tests and 90 Import tests. Owner/History production guards and negative fixtures pass, as does the physical boundary guard. Full/changed documentation policy, Archivist integrity and all 32 CLI tests passed for this continuation. I3's remaining callback consumers are explicitly enumerated in the receipt.

Independent Luna preparation also produced the [six-file settlement field audit](../benchmarks/dependency-convergence-phase3-settlement-field-audit-2026-10-22.json). The 207,071-byte event-spine candidate is conditional on framed-import proof reconstruction and is not approved for initial unframed scopes; the full-authority dictionary layout models 1,140,580 bytes. These are measurements only. No S3 selection, codec change, C3 conversion, source graph refresh or final A3 oracle is claimed.


The accepted R3 continuation closes the eight original P3.6 tasks: **75/119 closed (63.0%) = 55 original accepted + 20 added verified**, with **I.03 active and 43 original pending**. Gates B0/B1/H3/M3/P3/W3/R3 are accepted locally (7/12). Continue **P3.7/I3** from the remaining consumer list in the receipt; no renewed source graph capture or settlement codec change is authorized ahead of the specified gates.
