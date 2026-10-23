# Phase 3 local execution ledger — 2026-10-22

Parent index: [Documentation index](INDEX.md)

## Purpose

Track the **119 current Phase 3 execution tasks** (98 original P3.x IDs plus 21 additional L/H/M/I tasks) and twelve unchanged formal gates of the [Phase 3 specification](dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md) against actual local source, tests and corpus evidence.

## Overview

Execution starts at `b113bcf` on `main`; accepted Phase 2 source is `1380d0b`. Preserve the unrelated `.gitignore` change. No push is authorized. Full phase acceptance requires all gates, including the independently invoked six-file semantic oracle after format conversion.

The [baseline](dependency-convergence-phase3-baseline-2026-10-22.md) and [current baseline receipt](../benchmarks/dependency-convergence-phase3-baseline-2026-10-22.json) pin 1,134 source hashes with zero mismatches and six canonical hashes independently verified against Phase 2. Immutable local before-images are held under `.verification-tmp/phase3-before/`. Earlier Phase 2 test/oracle passes remain prior receipts, not new Phase 3 passes.

## Execution accounting realignment (historical planning snapshot, 2026-10-22)

The [execution realignment](dependency-convergence-phase3-execution-realignment-2026-10-22.md) promotes previously untracked REL/PHY lifecycle, read-only History, Maintenance and Import preparation into **21 additional, independently scoped execution tasks**, rather than leaving them as invisible prerequisites inside P3.3. **Planning-snapshot task progress was 35/119 (29.4%)**: 23 accepted original tasks plus 12 verified new extraction tasks, each closed only at its documented source/test scope. **That snapshot accepted B0/B1/H3 only (3 of 12); the continuation below supersedes it.** Do not award original integration/deletion task credit a second time when a new scoped extraction task is closed.

The revised task breakdown comprises **98 original tasks** (23 accepted, 75 pending) and **21 added tasks** (12 verified/closed at scoped acceptance, 7 active/unverified, 2 pending): **35 complete or verified, 7 active, 77 pending = 119 total**. The [21-task register](dependency-convergence-phase3-execution-realignment-2026-10-22.md#added-execution-tasks--21-unique-bounded-outcomes-part-of-the-119-task-denominator) is part of the actual denominator, not an optional metrics overlay. "Verified (scoped)" is a completed new bounded task at an earlier committed source checkpoint; it does not establish greenness of the current dirty tree or acceptance of the original P3.3/P3.4/P3.7 integration and deletion tasks. Maintain unique IDs and acceptance proofs for both populations; never silently increase the denominator without increasing the completion register.

At the snapshot, the last committed checkpoint was `1bfbc95`; an uncommitted REL pruning/reclamation, Import readiness/completion and owner-lifecycle hard cut follows it. The latest `.verification-tmp/phase3-maintenance-coordinator-check.log` compilation **failed** in `src/maintenance/reliquary_prune.rs` on an unresolved `crate::OwnerError` and ambiguous `output_path.as_ref()`; these have no green current-tree receipt. The last successful broader batch belongs to the earlier `1bfbc95` settlement/session checkpoint (**1,974 passed, zero failed, five ignored and four filtered**). No source/fixture/test/graph changes or migrations were made to perform the realignment.

M3 now closes only the Maintenance mutation-authorization/service boundary and its lower validated lifecycle prerequisites; **P3 still separately accepts full REL/PHY prune/reclaim policy, exact retained history and obsolete prune route deletion**. Opportunistically completed Import preparation does not allow I3 to bypass R3. The original frozen 34-component DAG, six canonical source hashes, current-only format rule and one-time D3 source graph refresh remain authoritative.

## Gates

| Gate | State | Evidence / remaining obligation |
| --- | --- | --- |
| B0 | Accepted locally | Source/hash verification, bounded operation inventory and starting guards in baseline. |
| B1 | Accepted locally (design) | [Concrete contracts](dependency-convergence-phase3-contracts-2026-10-22.md), reviewed against exact DAG; implementation proofs remain pending. |
| H3 | Accepted locally (source cut) | One read-only proof path; final targeted 23 History + source-order regression passed. Full-run identity anomaly is retained below for A3. |
| M3 | Accepted locally | Canonical explicit Maintenance operation boundary, validated lifecycle/History reconstruction, thin public dispatch and obsolete facade policy deletion; continuation matrix below. |
| P3 | Accepted locally | REL/PHY selection, owner retention/marker proofs, exact retained raw history/clocks, source/output guards and prune/reclaim tests; continuation matrix below. |
| W3 | Accepted locally | Immutable planning, concrete lifecycle execution, owner-local replay, selected semantic/vector emission and source-preserving publication; W3 receipt below. |
| R3 | Accepted locally | Generic Maintenance policy; lower producer replay; 86 final reconciliation and 90 Import tests; [receipt](../benchmarks/dependency-convergence-phase3-reconciliation-2026-10-22.json). |
| I3 | Pending | Import coordination. |
| S3 | Pending | Measured settlement alternatives and selected design. |
| C3 | Pending | Format/corpus conversion, parity, promotion and deletion. |
| D3 | Pending | Stabilized graph refresh and source enforcement. |
| A3 | Pending | Full integration, corpus and documentation acceptance. |

## Task register — 119 total

The original 98 P3.x task rows follow. The [21 additional counted L/H/M/I task rows](dependency-convergence-phase3-execution-realignment-2026-10-22.md) carry separate, bounded extraction or verification outcomes. **Current totals: 55/98 original accepted + 20/21 additional verified = 75/119 closed; one active and 43 pending.** The original P3.x rows remain pending where the whole original authorization, cross-owner integration, parity or deletion obligation remains; their accepted child-extraction evidence must not be reimplemented or double-counted as original task completion. Evidence for the baseline tasks is in the baseline document and its receipt. Later entries are updated only when their own source/test/deletion obligations have evidence.

| Task | State | Owner / evidence |
| --- | --- | --- |
| P3.0.01 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.0.02 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.0.03 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.0.04 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.0.05 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.0.06 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.0.07 | Accepted | Baseline / Phase 2 immutable receipt |
| P3.1.01 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.02 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.03 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.04 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.05 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.06 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.07 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.1.08 | Accepted (design) | Contracts reviewed against DAG; later implementation gates pending |
| P3.2.01 | Accepted | Production cuts/frontier separated from test fingerprints |
| P3.2.02 | Accepted | History owner-cut composition over immutable store facts |
| P3.2.03 | Accepted | Lifecycle bounded physical cursor; existing Container parser |
| P3.2.04 | Accepted | Source, Graph, vector, Community and manifest proof modules |
| P3.2.05 | Accepted | Normalized owner-history digest; relocation/membership tests |
| P3.2.06 | Accepted | Ephemeral historical base replay; publication remains outside History |
| P3.2.07 | Accepted | Byte/mtime/clock snapshots and live Freshness record/receipt assertions |
| P3.2.08 | Accepted | Old digest/frontier/encoding files deleted; integrated REL/PHY checks pending |
| P3.3.01 | Accepted locally | Existing owner maintenance store composed by Maintenance; no second manifest authority; continuation receipt |
| P3.3.02 | Accepted locally | Explicit operation authorization uses owner retention proofs and preserves invalid-reference refusals; continuation receipt |
| P3.3.03 | Accepted locally | History composes immutable accepted source/manifest/import authority; no service backreach; continuation receipt |
| P3.3.04 | Accepted locally | REL Archive support remains separate from PHY owner-domain selection; continuation receipt |
| P3.3.05 | Accepted locally | Held lifecycle sources and prune freeze guards enforce identity/head/extent before copy and success; continuation receipt |
| P3.3.06 | Accepted locally | Explicit REL/PHY tail, EntityResolution and compaction operations retain writer/repair guards; continuation receipt |
| P3.3.07 | Accepted locally | Public prune/reclaim/repair adapters gather owner facts and dispatch; CLI 32 passed; continuation receipt |
| P3.3.08 | Accepted locally | Facade REL prune/reclaim/tail policy bodies deleted; canonical Maintenance operations and source guards pass; continuation receipt |
| P3.4.01 | Accepted locally | Canonical REL/PHY Maintenance prune selection matches original supported domains; continuation receipt |
| P3.4.02 | Accepted locally | Domain retention proofs feed Maintenance; Container owns only mechanics; continuation receipt |
| P3.4.03 | Accepted locally | Original marker identity/revision/digest/order and typed payload-unavailable evidence retained; continuation receipt |
| P3.4.04 | Accepted locally | Conservative liveness reclaim remains separate from pruning and tombstones; continuation receipt |
| P3.4.05 | Accepted locally | History retained-evidence comparison and raw-copy checks preserve accepted semantic state/clocks; continuation receipt |
| P3.4.06 | Accepted locally | Retired profile/vector, compact body and description tests pass in continuation matrix; continuation receipt |
| P3.4.07 | Accepted locally | Frozen source, writer lease, destination collision and identity-guarded cleanup preserved; continuation receipt |
| P3.4.08 | Accepted locally | Old facade policy routes removed; REL/PHY prune/reclaim and production source/DAG guards pass; continuation receipt |
| P3.5.01 | Accepted locally | Canonical model, plan, semantic emission and file coordinator under Maintenance; public reexports and lineage read adapters retained; W3 receipt |
| P3.5.02 | Accepted locally | Immutable owner planning borrows and effect guard; referential closure separated from concrete destination emission; W3 receipt |
| P3.5.03 | Accepted locally | Exact retained owner facts and local Archive/Memory/Graph/Ego/vector publishers; no facade/provider callback; W3 receipt |
| P3.5.04 | Accepted locally | New/preserved owner identity, lineage and generation refusal semantics preserved; W3 receipt |
| P3.5.05 | Accepted locally | Exact selected Memory/Archive vector rows and profile state; no embedding inference; W3 receipt |
| P3.5.06 | Accepted locally | Held source digest/identity and no-overwrite atomic publication retained; stale source and collision tests pass; W3 receipt |
| P3.5.07 | Accepted locally | Controlled selection, retained revisions, profiles, imports, Ego and Graph verified by library and public rewrite matrix; W3 receipt |
| P3.5.08 | Accepted locally | Old production rewrite planner/file/model and semantic policy routes removed; compatibility module is reexports/read adapters only; W3 receipt |
| P3.6.01 | Accepted locally | Generic comparison and divergent policy in Maintenance; Import epoch policy stays separate; R3 receipt |
| P3.6.02 | Accepted locally | Read-only common-prefix comparison and History copy authority checks; R3 receipt |
| P3.6.03 | Accepted locally | UUID/generation/clocks/refusal parity through lower source reads and ordered replay; R3 receipt |
| P3.6.04 | Accepted locally | Native Archive/Memory/Insomnia/Graph/Perception/Interaction contracts; Ego/external refusals preserved; R3 receipt |
| P3.6.05 | Accepted locally | Import direct lower replay/copy; no Maintenance service callback in either direction; R3 receipt |
| P3.6.06 | Accepted locally | 86 final reconciliation tests and 90 Import tests include compact/terminal/retry/source-tail cases; R3 receipt |
| P3.6.07 | Accepted locally | Exact-created-output cleanup guard; retained leases and existing atomic promotion/restore contract; R3 receipt |
| P3.6.08 | Accepted locally | Production facade producer bridges retired to cfg(test); remaining I3 callbacks enumerated; R3 receipt |
| P3.7.01 | Pending | Awaiting prerequisite gate |
| P3.7.02 | Pending | Awaiting prerequisite gate |
| P3.7.03 | Pending | Awaiting prerequisite gate |
| P3.7.04 | Pending | Awaiting prerequisite gate |
| P3.7.05 | Pending | Awaiting prerequisite gate |
| P3.7.06 | Pending | Awaiting prerequisite gate |
| P3.7.07 | Pending | Awaiting prerequisite gate |
| P3.7.08 | Pending | Awaiting prerequisite gate |
| P3.8.01 | Pending | Awaiting prerequisite gate |
| P3.8.02 | Pending | Awaiting prerequisite gate |
| P3.8.03 | Pending | Awaiting prerequisite gate |
| P3.8.04 | Pending | Awaiting prerequisite gate |
| P3.8.05 | Pending | Awaiting prerequisite gate |
| P3.8.06 | Pending | Awaiting prerequisite gate |
| P3.8.07 | Pending | Awaiting prerequisite gate |
| P3.8.08 | Pending | Awaiting prerequisite gate |
| P3.9.01 | Pending | Awaiting prerequisite gate |
| P3.9.02 | Pending | Awaiting prerequisite gate |
| P3.9.03 | Pending | Awaiting prerequisite gate |
| P3.9.04 | Pending | Awaiting prerequisite gate |
| P3.9.05 | Pending | Awaiting prerequisite gate |
| P3.9.06 | Pending | Awaiting prerequisite gate |
| P3.9.07 | Pending | Awaiting prerequisite gate |
| P3.9.08 | Pending | Awaiting prerequisite gate |
| P3.9.09 | Pending | Awaiting prerequisite gate |
| P3.9.10 | Pending | Awaiting prerequisite gate |
| P3.9.11 | Pending | Awaiting prerequisite gate |
| P3.10.01 | Pending | Awaiting prerequisite gate |
| P3.10.02 | Pending | Awaiting prerequisite gate |
| P3.10.03 | Pending | Awaiting prerequisite gate |
| P3.10.04 | Pending | Awaiting prerequisite gate |
| P3.10.05 | Pending | Awaiting prerequisite gate |
| P3.10.06 | Pending | Awaiting prerequisite gate |
| P3.10.07 | Pending | Awaiting prerequisite gate |
| P3.10.08 | Pending | Awaiting prerequisite gate |
| P3.11.01 | Pending | Awaiting prerequisite gate |
| P3.11.02 | Pending | Awaiting prerequisite gate |
| P3.11.03 | Pending | Awaiting prerequisite gate |
| P3.11.04 | Pending | Awaiting prerequisite gate |
| P3.11.05 | Pending | Awaiting prerequisite gate |
| P3.11.06 | Pending | Awaiting prerequisite gate |
| P3.11.07 | Pending | Awaiting prerequisite gate |
| P3.11.08 | Pending | Awaiting prerequisite gate |

## New execution tasks (21 additional IDs; counted, not merely notes)

The complete [L.01–L.06, H.01–H.05, M.01–M.06 and I.01–I.04 task register](dependency-convergence-phase3-execution-realignment-2026-10-22.md) supplies the 21 independently credited scopes and evidence. These are separate execution tasks with **20 verified (scoped)**, **one active/unverified (I.03)** and **none pending** at the continuation checkpoint. Each is individually credited once. Original integration/gate tasks are accepted only when their broader distinct obligations pass; a new verified extraction does not prematurely complete M3, P3 or I3.

## History source cut evidence

The source cut moves reusable read-only behavior into `src/history/` and routes physical reads through `src/owner_lifecycle/history_source.rs`. Container remains the framing, extent, clock, path identity and lease authority. Domain stores supply their own local-version-at-global-cut queries and normalized marker encoding. The old production `facade/owner_history_proof.rs`, test frontier implementation and duplicate canonical manifest encoder are removed. Archive/Memory prune-evidence comparison now lives under History; Archive owner proof/publication reads accept immutable Container access through lifecycle. Test-only global-cut adapters and fingerprint/corpus oracles remain test-only.

History contains no Cva, Phylactery, Import coordinator, Container writer, provider invocation, sync or settlement publication. The existing upper adapters collect author/service facts (including External Sources and Dream authority) and map errors at the public boundary. Import owns historical event preparation because it invokes Freshness planners; this preserves the explicit no History-to-Freshness-service and no Import-to-Maintenance rules.

The integrated `history::` filter passed 21 tests before the final retention/error-order corrections (`.verification-tmp/phase3-history-current-tests3.log`), including normalized Graph prune/reopen/reclamation and source snapshots covering vector relocation, malformed ObjectRef rejection, typed mid-frame refusal and unchanged bytes/clocks/mtime. Locked check passed after the immutable Archive retention bridge (`phase3-check8.log`). The actual read-only History/import boundary guard passed after both retention moves (`phase3-history-boundary9.log`); self-tests alone are not treated as source enforcement. Review corrected double-wrapped policy errors and restored per-source lookup/content-validation refusal order. A focused manifest test pins deferred policy errors and descriptor-before-digest precedence.

The broad `historical_` filter was cancelled before completion: byte-by-byte recovery tests were still running, and unrelated sandbox filesystem/process/provider fixtures failed. A configured-runtime historical embedding fixture failed while saving configuration under sandbox permissions and passed when rerun directly with local permissions. No broad historical-suite pass is claimed. The interrupted test executable temporarily prevented Windows relinking; it was stopped before rebuilding the updated source. The first full-library attempt was also interrupted after its historical manifest/event/matrix/base cases passed: it inherited system temporary paths, and configuration/process fixtures failed under sandbox restrictions. The replacement full-library run uses repository-local TEMP/TMP and local subprocess permissions (`phase3-h3-library-local-temp.log`); its result is pending. The latest source includes additional retention moves and error-order fixes. Interrupted runs are not passes. Exhaustive recovery and six-file oracle remain integration-gate work.

The repository-local full library run completed with 1,970 passed, one failed and five ignored. The single failure was `Container(StaleIdentity)` on the second reopen of a checkpoint-prefix fixture (`historical_recovery_tests.rs:88`), not a proof comparison. The guard remains unchanged. The retained failed artifact also opened and reconstructed successfully on a disposable copy (`phase3-checkpoint-probe.log`). The concurrency/OS cause is not established; the failed full invocation is not reported as a pass. A clean final full integration run remains an A3 obligation. The exact isolated checkpoint rerun passed all 1,046 non-vector and 1,484 vector-backed prefixes (`phase3-checkpoint-failure.log`). This does not erase the full-run failure. All 25 public integration suites passed: 163 tests, zero failures, one existing ignored case (`phase3-h3-integration2.log`). The latest source-order regression passed at runtime (`phase3-scoped-order-test.log`); all 23 final History-filter tests passed (`phase3-history-final-tests.log`). All six canonical source hashes still match the baseline (`phase3-h3-canonical-hashes.json`).

## Verification

Current baseline: `cargo check --locked` and `cargo check --locked --tests` passed; physical boundary, owner boundary self-tests and component DAG self-tests passed. Focused frontier baseline passed: 2 tests, zero failures. The CLI suite passed all 32 tests (`phase3-cli-tests.log`); doctests completed with zero examples; full-library and isolated results are recorded above; no full Phase 3 pass or source graph refresh is claimed. Full and changed documentation policy checks and `ddocs check --indexes --links` passed before the latest documentation updates.

## Related docs

- [Specification](dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md)

- [Baseline](dependency-convergence-phase3-baseline-2026-10-22.md)

- [Contracts](dependency-convergence-phase3-contracts-2026-10-22.md)

## Notes

Current format remains CVAFRS06. No semantic feature or format migration is implied by an extracted source owner. Source refresh is reserved for stabilized D3.


## M3 implementation in progress

H3 is committed locally as `408d775`. Current work extracts lower REL physical replay, complete PHY store reconstruction, file metadata and concrete local validation; Memory owns the CVAXMEM2 codec and External Index owns its disposable snapshot codec/store. Provider authority remains above these owners. Project stores return concrete lower errors, mapped outward without changing public error messages.

PHY prune and reclamation now have concrete Maintenance operations, with read-only retained-evidence reconstruction in History. Shared physical reclamation selection/copy and relocation also live under Maintenance. REL's higher ordered finalization checks and other Maintenance entrypoints remain outstanding; no P3.3 task or M3 gate is accepted by this partial extraction. Formats and all six canonical fixtures are unchanged.

The intermediate lifecycle regression batch passed **1,969 tests**, with five ignored and four explicitly filtered long-running cases. The four cases are `historical_native_checkpoint_every_container_byte_prefix_reopens_and_retries`, `historical_native_due_reopen_and_every_live_use_prefix_preserve_all_recipients`, `canonical_history_scaling_measurement` and `long_owner_reopen_keeps_finite_receipts_and_exact_evicted_retry`. This is not a clean full-suite receipt and does not resolve the H3 full-run identity anomaly. The initial PHY prune extraction additionally passed **47 pruning tests**; subsequent reclamation extraction validation is recorded when complete.

Explicit EntityResolution inspection/repair and PHY staged-prefix tail repair are now under Maintenance. The remaining M3 contract work includes complete lower REL reconstruction, REL tail repair and narrower lifecycle reclamation source handles replacing the intermediate direct physical argument. Current source checks are evidence of allowed imports, not proof that all intended operation contracts are finished.

`maintenance/compactions.rs` provides the explicit conversation-compaction dispatch to the existing owner-local recovery operation; normal open still performs no recovery.


The [Maintenance preparation receipt](../benchmarks/dependency-convergence-phase3-maintenance-preparation-2026-10-22.json) records the later combined batch: **1,969 passed, zero failed, five ignored and the same four long cases filtered**, followed by **seven final explicit-repair tests** and **32 CLI tests**, all passing. Current library/test compilation, CLI compilation and format checks passed; production owner/History checks and physical/DAG negative fixtures passed. All six canonical hashes still match the pinned baseline. The first focused repair build hit Windows executable locking while the broader test binary was running; the isolated rerun after it exited passed. The changed-document checker also raced a disappearing test directory; its quiescent rerun is tracked separately. No failed or interrupted command is counted as a pass.

The quiescent changed-document policy check passed. Luna read-only review found no concrete behavior defects in the PHY prune/reclaim/tail and EntityResolution extraction. M3 remains in progress with the contract gaps listed above.


## M3 continuation after local checkpoint 8103711

`8103711` records the reviewed lifecycle/PHY Maintenance preparation. Its full and quiescent changed-document policy checks and link/index checks passed. Further work now replaces direct physical reclamation arguments with concrete private REL/PHY lifecycle source handles: Maintenance builds the disposable liveness selection; lifecycle freezes source extent and owns physical copy/relocation. REL still hands its candidate to the upper adapter until complete lower finalization converges. Two new tests check source growth before policy scanning and before copy, requiring refusal without destination creation. Validation of this continuation is in progress.

Immutable birth cohort validation and physical-prefix Dream population replay now live in `owner_lifecycle/publication_births.rs`; upper scoped adapters use this single canonical implementation. The journal replay callback path has not yet converged. No format change, settlement migration or M3 acceptance is claimed.

The continuation passed **20 reclaim**, **27 birth-evidence**, **34 population** and **two frozen-source growth** tests. Source compilation and production History/owner boundaries passed before the next replay extraction. Luna review found no concrete defects in the frozen-source/copy move.

The concrete REL owner aggregate and empty-owner initialization, Dream journal replay without a generic visitor, accepted Freshness rebuild, derived External Index backing validation and epoch/Archive receipt comparison now live under lifecycle. Luna review found the same durable initialization order and Freshness/Dream check ordering; a new combined bounded regression batch is running. Final REL reopen/retention/completion extraction is still outstanding, so M3 remains open.

The [lower replay preparation receipt](../benchmarks/dependency-convergence-phase3-replay-preparation-2026-10-22.json) records **1,971 library tests passed**, zero failed, five ignored and the same four long cases filtered; **32 CLI tests passed**. The two new source-growth refusal tests pass both independently and in the batch. Compilation, production owner/History guards and library/CLI format checks pass. All six canonical hashes remain unchanged. Retention and completion proof drafts are still isolated and are not included in this receipt; M3 and later gates remain open.

The installed Pitlord binary lacks `docs`; a local build of the available Pitlord source provides it, but that check currently stops because Arcana rejects the existing repository manifest (`wrong field count`). This is recorded as an unavailable architecture-document ownership check, not a pass. Existing documentation-policy checks pass independently. The approved one-time source graph refresh remains reserved for stabilized D3; no source snapshot was regenerated to bypass this issue.


## M3 retained-evidence continuation after d8ea6a5

REL description/vector retention, physical Import completion evidence and accepted semantic Import birth-scope proofs now have one read-only implementation under History. Public/facade callers delegate using immutable lifecycle sources and concrete owner-store borrows. Retention source failures stay deferred until the original physical-read point; initial-history semantic scope validation returns without constructing a physical source. Two new regression cases pin eligibility-before-source-error and empty-vector-validation behavior. Production owner and History guards and their negative fixtures pass. Test compilation passes after correcting moved test imports; focused runtime verification is in progress. No M3 acceptance, settlement format change or canonical fixture migration is claimed.

The [retained-proof preparation receipt](../benchmarks/dependency-convergence-phase3-retained-proof-preparation-2026-10-22.json) records **five retention**, **90 Import** and **44 prune** tests passed with zero failures. Test compilation, formatting and the unchanged 34-component/163-edge DAG checks pass; all six canonical hashes match the baseline. Full and changed-document policy checks and the link/index check pass. This focused receipt does not replace the pending final full integration gate.


After local checkpoint `0654564`, physical Import epoch-scope reconstruction and settlement-scope comparison are being consolidated under History. Metadata and receipt preflight remain at their original caller checkpoints, with immutable lifecycle-mediated publication reads. Contiguous Entity/Relationship validation and vector owner-store finishing now delegate to concrete lifecycle functions; History, FreshnessDue and Import checks stay at their existing ordered checkpoints. This continuation is under compilation/review and has no runtime acceptance receipt yet.

The first combined regression build stopped before runtime because Windows held three generated object files (`os error 32`); it is not counted as a test run. The subsequent accepted-replay and legacy-birth slices preserve their original checkpoints. Review caught and corrected a direct Episode-map lookup that bypassed conversation tombstone filtering; a focused regression checks successful birth proof before tombstoning and the original Episode-unavailable refusal afterward. Compilation and runtime verification of this final continuation remain pending. The latest documentation link/index check passed.

The [REL finalization preparation receipt](../benchmarks/dependency-convergence-phase3-finalization-preparation-2026-10-22.json) records **1,974 passed, zero failed, five ignored and four explicitly filtered long cases** in 100.94 seconds. The focused tombstone regression independently passed, as did final test compilation, production owner/History guards and documentation policy checks. All six canonical hashes remain unchanged. All **32 CLI tests** passed. M3 remains open: these concrete finishing steps do not yet provide the complete lower validated REL reopen or migrate every REL Maintenance entrypoint.


## Publication cohort continuation after f28ab2f

The read-only publication birth-cohort and same-publication scans are moving from the Freshness facade into History through immutable lifecycle sources. Both retain external-publication decoding before Insomnia completion decoding and scan the entire source even after finding a match, so later malformed evidence is not skipped. No publication, scoring or admission policy moves into History. Test compilation and production owner/History guards passed. All **10 cohort tests** and the separate grouped-birth reopen recovery test passed, with zero failures. Full and changed-document policy checks pass. The prior 1,974-test batch belongs to `f28ab2f`; this subsequent move has the focused receipt only.


## Settlement and completion-session continuation after e09f331

Historical settlement validation now composes one History implementation for owner/cut availability, semantic epoch binding, accepted Freshness evidence, legacy physical and singleton births, accepted Archive timeline validation and corpus content binding. Timeline value types and pure algorithms are separated from the higher Freshness planner with public reexports preserved. Scoped corpus reads retain per-key lookup/content/hash order, including MissingContent before source capture and no snapshot for an empty scope. Memory revision and Archive content materialization now accept immutable physical access.

Completion publication consumes an opaque Import-authored value through the concrete lifecycle `ImportSession`. Appends restore the closed admission fence before sync; staged epoch state is installed only after successful durability. The former Cva callback is removed from this completion write path. Compilation and production owner/History guards pass; the combined bounded regression batch is in progress. No M3, Import, settlement-design or format gate is accepted by this continuation.

The [settlement/session receipt](../benchmarks/dependency-convergence-phase3-settlement-session-preparation-2026-10-22.json) records **1,974 passed, zero failed, five ignored and the same four long cases filtered**, in 119.56 seconds. This validates the complete read-only settlement proof and completion-session changes before the next reconstruction/readiness integration. Documentation checks for the combined continuation remain pending.
## REL Maintenance and Import resumed checkpoint

At `d685e0c`: **59/119 closed (49.6%)**, **one active**, **59 pending**. M3/P3 close their original distinct integration/deletion obligations here; W3/R3/I3/S3/C3/D3/A3 remain pending. The detailed scope attribution, deferred tests, final source order and next W3 handoff are in the [realignment continuation](dependency-convergence-phase3-execution-realignment-2026-10-22.md#verified-source-continuation-after-ceaa5b8).

The [REL Maintenance/Import continuation receipt](../benchmarks/dependency-convergence-phase3-rel-maintenance-import-resume-2026-10-22.json) contains source hashes and exact passing-test names grouped into a nonadditive verification matrix. The bounded library run passed **1,975 tests**, zero failed, five ignored and four filtered in **72.04s**. Final Import policy extraction then passed **90 Import tests** in **4.09s**; The final source passed the locked CLI check and **32 CLI tests**. Production owner/History, physical and logical DAG guards pass, including negative fixtures. All six canonical hashes remain unchanged. An earlier restricted run was terminated after filesystem/transport fixture failures; it is not counted as success. No full A3 pass or corpus migration is claimed.

History takes a borrowed replay and returns session-free reconstructed state. The lower lifecycle assembles the physical session into its concrete owner only outside History. Maintenance owns the existing explicit operations; the original facade policy bodies are removed. The added task counts do not substitute for original integration/deletion checks: those are independently enumerated in the P3.3/P3.4 rows and matrix.

Pitlord documentation ownership remains blocked by the existing Arcana manifest error; D3 refresh has not been performed. Documentation and public integration results are recorded in the continuation receipt after their final runs.

Final public integration after the package-only cache rebuild passed **25 suites: 163 tests, zero failed and one ignored**. The first integration build failed before runtime on missing cached linkable dependencies; it is not a pass. Locked final-source CLI check and all **32 CLI tests** passed. Full and changed-document policy checks passed after the realignment schema repair; final Archivist integrity is recorded in the receipt.

## W3 Maintenance rewrite acceptance

Current progress: **67/119 closed (56.3%)** = **47 original accepted + 20 added verified**, **one active (I.03)** and **51 original pending**. Gates **B0/B1/H3/M3/P3/W3 are locally accepted (6/12)**. Next is **P3.6/R3 generic reconciliation**; I3 and settlement/format/final gates remain separate.

The [W3 receipt](../benchmarks/dependency-convergence-phase3-rewrite-2026-10-22.json) pins the source and exact passing rewrite/reconciliation matrix. The bounded library batch passed **1,975 tests**, zero failed, five ignored and the same four long cases filtered, in **73.99s**. All **18 public rewrite contract tests** passed in **1.57s**; locked CLI check and **32 CLI tests** passed in **0.84s**. Owner/physical guards and negative fixtures pass. The new planner effect guard and its negative fixtures pass. All six canonical hashes match the frozen baseline. No format change or D3 source graph refresh occurred.

Maintenance now owns model/error, immutable planning, referential closure, selected semantic and vector emission, and physical file coordination. Public `rewrite` exports and lineage getters remain source-compatible. Concrete lifecycle owners replace production Cva/Phylactery execution inputs. Archive owns ordered local replay facts and publishers; the remaining generic reconciliation adapter uses that contract. The existing graph endpoint validator is under lower lifecycle. New error value ownership and exact immutable DTO/packed-codec contracts are explicit in the source manifest; the 34-node/163-edge logical authority is unchanged.

The fresh rewrite writer validates the enrolled Freshness record transition without retaining a runtime notification cache; the final normal reopen still validates/rebuilds derived state. Existing public variants, error text, copy/clock behavior and refusal ordering are retained. Dead facade Archive rewrite policy and production `src/rewrite/{file,model,plan}.rs` implementations are removed. Earlier moved-test/compile failures remain non-passing attempts. Full A3 long/prefix/corpus acceptance remains pending.


## R3 continuation and S3 measurement preparation

Generic divergent reconciliation now lives in Maintenance over concrete lifecycle owners. Ordered Archive, Memory/Insomnia, Graph, Entity, Relationship and Interaction replay consumes explicit producer parts. Import interval replay calls those lower contracts directly, including read-only History Freshness normalization followed by Import-owned coordinate remapping and validated lifecycle publication. Shared immutable reconciliation results/errors live below both services. Maintenance never dispatches into Import, and Import's copy path uses the lower physical-copy primitive and its own History validation rather than Maintenance. Error cleanup retains the exact created output identity and removes only that output if it still owns the path. Public Cva signatures and physical leases remain unchanged.

The first focused run passed **86 reconciliation tests**, zero failed, in **4.42s**. The subsequent bounded library run passed **1,975 tests**, zero failed, five ignored and the same four deferred long cases, in **89.27s**. Final source review then corrected semantic-event destination-turn rebasing and restored the original Freshness policy persistence order, and narrowed native Import commit's physical/queue access through its concrete lifecycle capability. Final focused verification passed **86 reconciliation tests** in **4.91s** and **90 Import tests** in **3.18s**, with zero failures; the bounded run is not attributed to those later changes. The native commit writer clears its scope on unwind, and the public adapter processes accepted activity notifications on success, failure or unwind. The production owner guard passed before the final capability tightening. R3 acceptance and progress accounting are recorded separately below; I3 remains active because completed-epoch orchestration and historical reconstruction still use Cva adapters.

The read-only [settlement field audit](../benchmarks/dependency-convergence-phase3-settlement-field-audit-2026-10-22.json), generated by `scripts/measure_phase3_settlement.py`, validates all six frozen source hashes, physical framing and settlement checksums. Three REL settlements total **7,415,717 payload bytes**, or **7,415,741 bytes including chunk headers**. Event recipient sets occupy **4,759,100 bytes** and source-node identities **1,943,288 bytes**. The measured full-authority dictionary/prefix/delta/dedup candidate models **1,140,580 bytes**; the event-ID spine with retained postimages models **207,071 bytes**, conditional on framed-epoch reconstruction proofs. Initial unframed settlements have no receipt from which source-node authority can simply be reconstructed. Dropping exact postimages additionally requires a deterministic noncircular historical projection proof. These are candidate-layout measurements, not a selected codec or completed S3 gate. No runtime format or canonical corpus bytes have changed. S3/C3 must resolve those proof obligations before codec implementation, conversion or promotion.


## R3 accepted checkpoint

Current progress is **75/119 closed (63.0%)**: **55 original tasks accepted + 20 added tasks verified**, **I.03 active**, and **43 original tasks pending**. Gates **B0/B1/H3/M3/P3/W3/R3 are locally accepted (7/12)**. The final source passed the focused reconciliation/Import matrix and source guards; CLI verification passed **32 tests**. Full and changed-document policy checks and Archivist integrity passed for the continuation. The bounded full-library receipt remains attributed to its earlier source as explained above; A3 long/oracle obligations are unchanged. No graph refresh or format/corpus migration occurred.

I3 continues with its remaining explicit Cva epoch/historical reconstruction adapters. An operation-specific epoch replay scope now clears the physical write bypass on unwind without accepting a general caller callback. Its last compiler check passed; later I3 extraction/verification remains distinct from R3 acceptance. No original task or added task is credited twice.


Further I3 preparation moved grouped completion hydration/publication and Dream Begin transport into explicit lifecycle owner-part operations. Source and destination compact Dream population proofs still use their own exact bounded physical prefixes. Import callers consume these contracts directly; obsolete internal Cva methods are test-only adapters. The coherent follow-up check passed and **90 Import tests passed in 3.50s**, zero failed; the production owner guard passed. This is recorded as preparation in the R3 receipt without closing I3 or any additional task. Historical reconstruction and completed-epoch ambient Cva access remain the next extraction work.
