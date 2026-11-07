# G4 — implementation ticket DAG and integration gates

**Status:** accepted; **G4-T01–G4-T09 complete**. Native 2.1 product acceptance is closed; D01 remains a separate unstarted gate.
**Parent:** [G4 product acceptance specification](g4-complete-product-acceptance-spec-2026-11-06.md).
**Non-negotiable scenario oracle:** [G4 2.1 corpus/matrix](g4-generated-2-1-corpus-acceptance-matrix-2026-11-06.md).
**Execution worktree:** `C:\!bin\workspace\Reliquary-rebuild`.
**P01/GM:** accepted prerequisites; do not reopen their architecture without a failing G4 regression.

The tickets below are **verification slices, not a new permanent architecture**. Small commits are preferable, and a fresh agent should be able to finish one ticket with named executable evidence. All lower/service changes require a direct failing case at their owner seam. Canonical G4 acceptance remains a **single gate**, not automatically accepted per ticket.

## Dependency DAG

```text
G4-T01 (freeze authored oracle)
  └─ G4-T02 (generate native 2.1 cohort)
       ├─ G4-T03 (native ingest, import, Dream, Entity)
       │    └─ G4-T04 (vectors, context, retrieval/use)
       └─ G4-T05 (Gateway host/worker/recovery)

G4-T04 + G4-T05
       ├─ G4-T06 (GM on completed cohort copies)
       └─ G4-T07 (full-cohort external consumer and REL CLI)
             [both T06 and T07] → G4-T08 (independent audit)
                                     → G4-T09 (final acceptance)
```

T03 and T05 are **parallel-ready** after T02 where isolated test directories and a reproducible source manifest make them noninterfering. T04 follows T03. Both T06 and T07 require **T04 and T05** so their maintenance/CLI evidence uses the completed product cohort, including accepted-use Freshness and host-recovery receipts, rather than a partially populated owner. T06 and T07 can then proceed in parallel. T08 joins both; T09 is the only final acceptance gate. Do not run parallel Cargo jobs in the same Windows target directory; serialize `-j 1` or use isolated directories.

## Tickets

### G4-T01 — freeze observable scenario oracle

- **Predecessors:** P01, GM.
- **Deliverable:** committed versioned `g4-native-rel21-corpus-v1` scenario manifest, 12 natural conversation scripts, fixed source spans/branch topology and an explicit expected-vs-observed field map for all G4-01..20.
- **Owner/seam:** test fixture contract; no production code mutation.
- **Checks:** hand-authored source references, principal/owner routing and negative result types are fixed independently of runtime output; 2 REL/2 PHY owner aliases, 8+4 conversations, ≥40 turns and ≥3 branch forks; no real secrets/old corpus data; static schema validation rejects missing/duplicate script IDs, unclear owner grants and inconsistent parent refs.
- **Evidence:** manifest revision/digest and passing fixture-schema test.
- **State:** complete.
- **Verified evidence:** Fixture `tests/fixtures/g4-native-rel21-v1.json`, seed `g4-rel21-v1`, SHA-256 `FF996170B657EDEC07CD96DD6EE65D8FB6D6DA6643360D160DB0687AC267E2CD`; schema tests pass in the final workspace suite.

### G4-T02 — build a replayable native 2.1 cohort

- **Predecessors:** G4-T01.
- **Deliverable:** small test-only builder with explicit stable seed and deterministic model/host/VCS substitutes connected through **existing** interfaces. Fresh directory per run, generated REL/PHY owners and a read-only witness exporter.
- **Owner/seam:** public `Rel`/`Phylactery` creation and accepted Gateway/owner-session fixture entry points. Do not create a new product `Corpus` service or write physical records to fabricate required success paths.
- **Checks:** four owners are independently reopened; owner IDs/scope, source-turn hashes, frozen script counts/branches and mutable output isolation pass; deterministic rerun yields identical normalized semantic witnesses, with generated UUID aliases compared consistently instead of forced hardcodes; failure cleans only test-owned files.
- **Coverage:** G4-01 and initial G4-02.
- **Evidence:** targeted generated-corpus smoke + fresh-reader source witnesses; record exact test command and counts.
- **State:** complete.
- **Verified evidence:** The joined native cohort checks 12 conversations, 48 exact authored turns, three forks and all four reopened owners; two independently regenerated semantic witnesses match.

### G4-T03 — prove source-grounded ingest, native import, Dream and Entity on one cohort

- **Predecessors:** G4-T02.
- **Deliverable:** end-to-end accepted turn → Episode → Insomnia candidate → REL/PHY publication → Dream canonical/duplicate/first pass → resolved or ambiguous Entity cases; also run a supported native input normalization/import epoch on Cascade with exact retry and exclusive settlement. Preserve produced files for the next ticket **only through regenerated deterministic fixture inputs**, not an untracked handoff corpus.
- **Owner/seam:** Gateway/Instance and existing Application evaluator and owner-session commit APIs; use existing named workflows. Add no generic owner-state escape hatch.
- **Checks:** exact source spans, ownership routing, owner privacy, idempotent receipt, historical revision and Dream/Freshness birth; completed native import/receipt, same-input dedup and post-reopen state. Invalid Episode, forged source, contradictory duplicate, stale Entity proof, unsupported native profile and incomplete/conflicting import epoch reject without mutation; independent read and reopen match authored expected witness.
- **Coverage:** G4-02–05 and **all** G4-10.
- **Evidence:** targeted composed test and prior V01/V02/V03 tests; failing tests before any semantics correction.
- **State:** complete.
- **Verified evidence:** Joined source/provenance, private routing, Dream/Entity and native-import witnesses plus negative outcomes pass; independent owner reopen and process transcript checks agree.

### G4-T04 — prove vectors, context, retrieval and confirmed use

- **Predecessors:** G4-T03.
- **Deliverable:** same cohort with accepted Memory/Archive vector profiles, pinned VCS attachment, bounded context, passive+agentic candidate selection, durable exact host-confirmed delivery and Freshness receipt.
- **Owner/seam:** Application evaluation and authenticated Gateway, with fake embedding/decision/host endpoints at existing ports. No inference under broad owner locks.
- **Checks:** exact selected source IDs and body revisions; fixed F03 ID vectors; repeat embedding reuse; VCS historical bytes/authorization; no credit before externally confirmed consumption; no second receipt after retry or reopen. Negative provider shape/stale-cut, unauthorized PHY, revocation, wrong manifest and unknown host produce unchanged owner authority.
- **Coverage:** G4-06–09.
- **Evidence:** composed test output + independent post-reopen receipts and history; no model calls after idempotent replay.
- **State:** complete.
- **Verified evidence:** Joined vectors/context/VCS/retrieval/confirmed-use checks and all six F03 identity goldens pass; reopened receipt and vector evidence recorded in the acceptance report.

### G4-T05 — prove Gateway and trusted-host lifecycle together

- **Predecessors:** G4-T02.
- **Deliverable:** shared-owner queue/branch and host-operation scenarios across a fresh process reopen; worker drain/restart, private generation checkpoints and stream resubscription.
- **Owner/seam:** P01 public `Gateway`/`Instance` where supported, internal Gateway test seam for existing privileged worker controls; direct queue state is **observation**, not a second owner writer.
- **Checks:** finalize/cancel race single winner, foreign submitter denied, incomplete append tail detected, unknown host Held and never implicitly accepted, checkpoint-before-visibility, valid resume, wrong-grant/revoked read and writer-lease exclusivity. Tests identify both durable receipts and absence of unauthorized content.
- **Coverage:** G4-11, G4-12, G4-19 relevant security.
- **Evidence:** deterministic fault fixture and process-reopened public state; prior G3 queue/stream tests remain green.
- **State:** complete.
- **Verified evidence:** Completed-cohort queue, host, worker, checkpoint and restart checks pass; separate consumer verifies committed/cancelled/Held/race state against accepted Archive.

### G4-T06 — run supported GM history/maintenance on cohort copies

- **Predecessors:** G4-T04 and G4-T05 (the completed native corpus includes import, Dream/Insomnia, vector, retrieval/accepted-use and host/worker evidence).
- **Deliverable:** verified-copy rewrite (REL/PHY), terminal prune, profile repair, proven reclamation and same-owner prefix/divergent reconciliation using **copies of the completed product cohort**, plus historical comparison before/after.
- **Owner/seam:** public `reliquary_memory` maintenance/history where available, accepted Application/owner-session GM seams for exact deep replay evidence.
- **Checks:** before/after source digest and cut, selected evidence closure, fixed Identity/History generation, independently reopened destination, exact no-replace collision, reclaim second no-op, preserved legitimate Dream/Insomnia/Freshness and rejection of unsupported conflicting divergence. No active owner file modified by offline candidate transformation.
- **Coverage:** G4-13–17.
- **Evidence:** separate read-only verifier after all writer leases drop, typed negative cases and GM regression rerun.
- **State:** complete.
- **Verified evidence:** Completed-cohort maintenance checks pass: all four terminal payload domains, profile repair, actual orphan reclamation with exact second no-op, supported rewrite policies and retained-authority reconciliation. Prior GM rewrite exclusions retain typed no-output evidence.

### G4-T07 — independent public consumer and REL CLI process replay

- **Predecessors:** G4-T04 and G4-T05.
- **Deliverable:** separate Cargo consumer and fresh CLI process open an **independently reopened, fully populated** current-format cohort owner and compare observed bounded public results with authored scenario invariants. CLI must use P01 exported library only.
- **Owner/seam:** Root `reliquary-memory`; standalone `cli/Cargo.toml` and `tests/external-consumer` (no direct lower-crate imports).
- **Checks:** public management title/provenance/health, user/privacy gates and quiesce/reopen; CLI info/verify/history proof, dry-run, supported copy command and fail-closed wrong kind/2.0/unknown flag/output collision; no in-process test double masquerading as CLI result.
- **Coverage:** G4-18, G4-19 and part of G4-01.
- **Evidence:** command+exit JSON snapshots and independent process reopen, with byte-unchanged source.
- **State:** complete.
- **Verified evidence:** Actual CLI/root-consumer process script exits 0; one explicitly enabled external integration test passes; 25 command observations include 23 CLI commands and two consumer inspections, with eight expected CLI refusals.

### G4-T08 — independent cross-scenario oracle and negative audit

- **Predecessors:** G4-T04, G4-T05, G4-T06, G4-T07 (and therefore G4-T03).
- **Deliverable:** complete row-by-row G4-01..20 evidence matrix comparing authored expected observations, captured actual values and independently reopened values **without reusing in-memory projections**.
- **Owner/seam:** read-only fresh owner readers, separate process CLI/public consumer and scenario manifest. Reuse prior owner historical proof logic, not a duplicate mutable store.
- **Checks:** no missing mandatory row, no fake zero-row success, wrong-kind/format, unauthorized/corrupt/missing/tombstoned/pruned states separated; source bytes preserved after negative/maintenance runs; confirmed-use credit monotonic and no read-side reinforcement; every regression detected during G4 has a red/green test and tracked fix commit.
- **Coverage:** cross-check G4-01..19.
- **Evidence:** independently signed-off review section in `Reliquary-rebuild/docs/g4-product-corpus-acceptance-2026-11-06.md` (create report here, but **do not mark accepted** yet).
- **State:** complete.
- **Verified evidence:** Independent Luna review `g4_lifecycle_review` found no discrepancy between the fixed manifest and captured fresh W/P/C values; the report records all 20 case IDs, executable/evidence pointers and correction commits.

### G4-T09 — final integration gate and canonical status update

- **Predecessors:** G4-T08.
- **Deliverable:** accepted G4 evidence record, committed tests, clean rebuild worktree and canonical tracker `G4=complete`; explicit handoff to D01.
- **Owner/seam:** full product workspace, CLI, external Rust consumer and production dependency checker.
- **Checks:** run the complete locked/offline test/check/format commands from the parent spec; exact G4 case inventory all `PASS`; ensure 23 production packages/76 edges or an explicitly approved newer DAG; no introduced 2.0 runtime reader, `Cva` alias, mutable Root owner bag, production test fixture API or obsolete dual path. Record source HEAD, fixture hash, test totals and negative cases; preserve GM and P01 acceptance.
- **Evidence:** reviewed acceptance report and Git commits (rebuild + planning tracker); no source release or archived-corpus activation.
- **State:** complete.
- **Verified evidence:** All prescribed locked/offline gates pass at source `31e7adb2`; accepted rebuild evidence commit `a3c3a9643f494bced51faf92ba057b16bcdac8a7` is clean. Canonical G4 is complete. D01 may next begin its separately scoped offline conversion prerequisites; D01–D03/G5 are not accepted or activated here.

## Completion and escalation rules

- A **specific missing integration primitive** may justify a minimal change at the correct owner/workflow/Gateway boundary, never a general compatibility adapter or a new crate for convenience. Document its failing externally observable test.
- Unsupported 2.0 data, future Ego/Observation inference, arbitrary cross-owner merge and provider-quality benchmarking remain **excluded** and cannot be used to claim a blocked mandatory accepted scenario.
- If a G4 scenario reveals a real previously accepted GM/P01 regression, fix it and rerun those specific gates before acceptance. Do not rewrite the original gate report to conceal the regression; add a correction record.
- Preserve untracked user files and unrelated modifications. Stage only G4 changes. Commit any independent tickets as verified small slices; never claim G4 accepted before T09.
- G4 closes the **native 2.1 full product** only. D01–D03 and G5 are distinct unstarted gates and must not be marked complete by this plan.

## Accepted execution record

- Report: `Reliquary-rebuild/docs/g4-product-corpus-acceptance-2026-11-06.md` (rebuild commit `a3c3a9643f494bced51faf92ba057b16bcdac8a7`).
- Successful generated bundle: `Reliquary-rebuild/.test-tmp/g4-completed-776c0bd2d3a54d2aa0b55bfdab33db9e`, plus deterministic repeat.
- The report records exact commands/exits/totals, the unchanged 23-package/76-edge production graph, independent review, inherited documentation-tool limitations and the source-preserving negative cases.
- Handoff: D01 remains unstarted; this gate authorizes neither archived-corpus conversion nor release activation.

- Documentation status check: the required policy check with `--changed-from origin/main` exits 1 with exactly the same 117 findings as the pre-edit baseline; no new finding was introduced. Existing index failures are recorded in the acceptance report.
