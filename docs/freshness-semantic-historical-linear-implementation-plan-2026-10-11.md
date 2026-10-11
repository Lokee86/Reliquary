# Freshness semantic reinforcement + historical reconstruction — linear implementation plan

Parent: [Documentation index](INDEX.md).

## Purpose

Provide one sequential, implementation-ready execution order for live semantic Freshness reinforcement (F8) and the separate historical import reconstruction (H1–H8).

## Overview

Execute live F8 through its independent release gate before historical reconstruction. Assign Luna to bounded work under frozen policy and Sol to ownership, policy, durability, and recovery decisions. Step 1 baseline evidence is recorded in [the dated baseline](freshness-semantic-historical-baseline-2026-10-11.md).

**Date:** 2026-10-11  
**Status:** implementation-ready sequencing document; no step is complete merely because it appears here.  
**Scope:** F8/S1–S6 live semantic Freshness reinforcement followed by H1–H8 historical import reconstruction.  
**Source specs:** [Freshness completion execution plan](freshness-completion-execution-plan-2026-10-03.md), [historical import reconstruction spec](freshness-historical-import-reconstruction-spec-2026-10-03.md), [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md), [Memory Freshness policy](memory-staleness-plan.md).

## Related docs

- [Step 1 source and test baseline](freshness-semantic-historical-baseline-2026-10-11.md)
- [Memory Freshness policy](memory-staleness-plan.md)
- [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md)

## Model assignment

- **Luna** — use when the policy/interface is already frozen and the task is bounded implementation, fixtures, mechanical integration, targeted debugging, or verification.
- **Sol** — use when the step changes ownership boundaries, durable/event semantics, policy constants, reconciliation/recovery behavior, or requires choosing among materially different implementations.
- Do not downgrade a **Sol** step merely because the code diff looks small; the assignment is based on semantic risk.
- A **Luna** implementation step that reveals a new architectural decision stops and hands that decision to **Sol** before continuing.

## Execution rule

Execute strictly in order. **Steps 1–12 complete live F8 first. Steps 13–25 implement historical reconstruction only after live F8 has its own passing release gate. Step 26 is the combined production-readiness decision.** Do not interleave historical propagation into the live semantic path before Step 12 is green.

Normal live semantic reinforcement remains **local and non-propagating**. Historical import semantic reinforcement is the separate exception and may propagate over the completed authorized graph.

## Sequential steps

| # | Model | Work | Done when |
| ---: | :---: | --- | --- |
| **1 — complete** | **Luna** | **Freeze the current baseline.** Inspect current `main`, exact Freshness owner/storage APIs, accepted-turn ingestion seams, semantic/vector search APIs, fragment/vector availability, reconciliation code and existing R1–R7 tests. Record the source/test files F8 will touch and the currently passing focused baseline. | A dated baseline note lists the exact seams, current test commands/results and confirms no F8 code already exists. |
| **2** | **Sol** | **Freeze the live semantic policy interface.** Define the semantic policy/version object, eligible owner/lifecycle/profile rules, self-source exclusion, one-credit-per-Memory-per-turn identity, deterministic ties, candidate limit behavior, and how the semantic event is represented without pretending it is +25 accepted use or +50 Dream linkage. | A narrow policy/API contract is written and no unresolved ownership or event-identity decision remains. |
| **3** | **Luna** | **Build the calibration/oracle harness.** Make threshold and similarity-to-bonus curves parameterized; measure compatible-vector coverage, nearest-neighbor counts, false-match neighborhoods, candidate-search cost and resulting score trajectories on synthetic fixtures plus already-retained corpora. Do not require new REL/PHY inference. | Reproducible calibration commands produce comparable results for several thresholds/bonus curves. |
| **4** | **Sol** | **Select and freeze F8 constants.** Review Step 3 evidence and choose the production cosine threshold, monotone local bonus curve, per-turn cap/candidate limits and policy-version transition rules. The old 0.75 value remains tentative until this step. | Selected constants/rationale are recorded in a semantic calibration document and the policy version is frozen. |
| **5** | **Luna** | **Implement the pure semantic policy module.** Add policy types/functions for thresholding, bonus calculation, compatibility checks, deterministic dedupe/tie behavior and direct local effect calculation. Keep it graph-independent. | Focused unit tests cover below/at/above threshold, monotonicity, cap, ties, incompatible profile/generation and invalid inputs. |
| **6** | **Sol** | **Define and implement the accepted-turn semantic producer seam.** Bind a semantic event to owner + exact accepted Archive conversation/node identity + REL activity position + profile/policy evidence. Decide the authoritative point at which genuine ingested fragments are available and exclude generated retrieval/summary echoes. | One explicit producer seam exists with stable replay identity and no ambiguity about source ownership or accepted-turn timing. |
| **7** | **Luna** | **Implement bounded candidate search and target eligibility.** Reuse existing compatible semantic/vector search, filter owner/lifecycle/profile/self-source rules, dedupe all fragments to the strongest target match for the accepted turn, and guarantee future/invalid candidates cannot consume the usable candidate budget. | Deterministic tests prove one target credit per turn, multi-fragment dedupe, profile mismatch behavior and bounded search. |
| **8** | **Sol** | **Extend Freshness durability for live semantic events.** Add the minimum versioned receipt/event identity needed for exactly-once semantic application using existing Freshness storage/replay machinery. Do not create a separate semantic ledger or Memory semantic revision. Define late-event canonical ordering relative to Dream/use events. | Retry, reopen and duplicate receipt resolution are deterministic and storage ownership remains inside existing Freshness durability. |
| **9** | **Sol** | **Wire live semantic reinforcement into accepted-turn processing.** After genuine fragment evidence is ready, run Step 7 search and apply only the direct local semantic effect at the accepted REL cut. Preserve lazy-decay remainder, score caps, Dormant reactivation and existing Dream/use principals. **No graph propagation.** | A real accepted turn can produce local semantic Freshness updates end-to-end; neighboring Memories remain unchanged. |
| **10** | **Luna** | **Complete live semantic correctness fixtures.** Cover threshold boundaries, many fragments, self-exclusion, no/empty vectors, incompatible profiles/generations, negative and Dormant targets, +100 saturation, partial ten-turn remainder, independent real +25 accepted-use after semantic reinforcement and exact worker/order determinism. | Focused F8 tests pass and explicitly prove semantic-neighbor non-propagation. |
| **11** | **Sol** | **Close live semantic recovery and transformation semantics.** Extend/review reconciliation, copy/repack/reclamation, delayed-event replay, profile rotation and destination-turn rebasing. Reject unsupported provenance rather than inventing activity positions. | Reopen, retry, physical transforms and twice-reconciled histories retain exactly-once local semantic effects. |
| **12** | **Luna** | **Run the standalone F8 release gate.** Execute all new F8 tests plus affected core R7/root/CLI/docs checks, measure search/event throughput and bounded receipt behavior, update API/storage/current-limitations only for shipped behavior, and write the semantic release verification record. | **Live F8 is independently green and committed. Historical reconstruction has not yet changed live semantics.** |
| **13** | **Sol** | **Freeze the historical reconstruction manifest and timeline policy.** Define canonical source timestamp/ancestry tie rules, effective Memory availability, grouped birth cohorts, final graph/Community snapshot proof, import identity/hash, same-position event ordering, missing-provenance behavior and REL-only scope. | H1 policy is fixed; no unresolved timestamp, eligibility, ownership, or manifest rule remains. |
| **14** | **Luna** | **Implement the historical timeline/provenance planner.** Deduplicate source turns, build canonical timestamp/ancestry order, resolve source/authority/grounding/Episode evidence, compute each Memory's approximate active-Web admission position and birth cohort, and emit bounded coverage/errors. | H2 fixtures pass for ties, multiple conversations, shared branches, missing/bad provenance, late evidence, isolated active Memories and owner isolation. |
| **15** | **Sol** | **Enforce Archive-clock compatibility.** Compare reconstructed chronology with actual first-accepted Archive activity order. For the first release, explicitly reject incompatible already-ingested histories unless a separately proven conversion exists; do not persist synthetic positions as real REL cuts. | Mismatched order fails explicitly and matching imports have one authoritative decay clock. |
| **16** | **Luna** | **Implement historical semantic event planning.** Reuse the now-shipped F8 policy/search machinery, but evaluate source turns at reconstructed historical positions and collect eligible semantic roots/bonuses. Reuse retained vectors or a separately requested bounded embedding-only pass; never rerun Insomnia/Dream. | Deterministic historical semantic events are generated with source position, policy/profile proof and target eligibility. |
| **17** | **Sol** | **Implement the import-only propagation adapter.** Reuse the deterministic Freshness frontier with variable semantic root bonuses and maximum contribution per recipient for the entire source turn. Traverse the complete authorized graph—including historically future/ineligible intermediate Memories—but apply historical time eligibility only when awarding recipient credit. Do not weaken normal +25/+50 principal validation or enable live semantic propagation. | H3 propagation tests prove future-node bridging, per-turn max aggregation, cycles, Community attenuation, authorization and 1-vs-N worker equality. |
| **18** | **Sol** | **Implement approximate historical first-cycle Dream event production.** Prefer actual new-corpus first-cycle proofs; otherwise derive one multi-root +50 event per qualifying source Memory from completed direct Dream-origin links to older canonical targets. Preserve duplicate authority, verified no-link outcomes and source/cohort credit exclusions. | H4 Dream fixtures prove no edge-by-edge multiplication, no pure-rewire event, correct canonical duplicate target and no fabricated historical +25 use. |
| **19** | **Luna** | **Add recipient-only eligibility and bridge oracles.** Exercise both historical semantic and Dream events where an ineligible future Memory is the strongest traversal bridge. Verify the bridge receives no historical credit while an eligible downstream Memory can. | The exact approximation chosen in the spec is executable and regression-protected. |
| **20** | **Luna** | **Implement the sparse chronological scorer.** Group contributions by recipient, sort by historical position + stable event ID, and reuse existing `FreshnessRecord::created/admit/reinforce_with_policy/settle_with_policy`. Preserve -100/+100 caps at each event, ten-turn remainder and lazy tail decay. | H5 mathematical oracles pass, including +100 saturation, -100 then +50 => -50, early-vs-late divergence, Dormant recovery and byte-identical parallel output. |
| **21** | **Sol** | **Design and implement the atomic superseding import checkpoint.** Freeze the manifest/input cut, build final records off to the side, and atomically replace provisional import birth/Dream **scoring** for the cohort while preserving Dream proof/journal authority. Define how post-cut legitimate live events are retained. | H6 proves no half-published state, no provisional Dream double-credit, stable retry identity and clean continuation on the normal REL clock. |
| **22** | **Luna** | **Close checkpoint recovery/idempotence.** Add crash-prefix, reopen, repeated-import, due-index rebuild and post-checkpoint accepted-use tests. | Every crash/retry prefix converges to the same authoritative projection; repeated identical reconstruction is a no-op. |
| **23** | **Sol** | **Close historical reconciliation and physical-transform semantics.** Handle copy/repack/reclamation, regeneration of already-enrolled Archives, divergent merge/reorder and policy/manifest conflicts. Recompute/rebase only from verified source IDs; never transplant meaningless numeric anchors. | H7 transformation matrix passes or explicitly rejects unsupported cases without silent score invention. |
| **24** | **Luna** | **Wire the one-shot import coordinator.** Orchestrate corpus-complete barrier → manifest freeze → timeline planner → semantic/Dream event generation → propagation → sparse scoring → atomic checkpoint. Add the optional bounded embedding-only fallback as an explicit opt-in operation with coverage reporting. | A complete REL import can run reconstruction once without staged Insomnia/Dream inference and without leaving a permanent event queue. |
| **25** | **Luna** | **Run H1–H8 end-to-end acceptance and measurement.** Execute mandatory historical fixtures, live-vs-import divergence tests, core regression, full repo/docs gates and retained-corpus performance/coverage measurements. Publish a dated historical reconstruction verification record. | Historical reconstruction is green for the supported REL-local import shape, with CPU/memory/vector/edge/checkpoint measurements and explicit unsupported cases. |
| **26** | **Sol** | **Final combined production review and roadmap activation.** Audit F8 + H behavior against ADR 0039, privacy/authorization, runtime ownership, storage/reconciliation and importer contracts. Decide whether H remains optional/experimental or becomes default for eligible imports; explicitly leave PHY Freshness deferred unless separately designed. Update roadmap/status docs accordingly. | One production disposition is recorded; no spec says “implemented” unless Steps 1–25 evidence supports it. |

## Step ownership by original phase

| Original phase | Linear steps |
| --- | --- |
| **F8 / S1** | 2, 5, 6, 8 |
| **F8 / S2** | 3, 4 |
| **F8 / S3** | 7, 9, 10 |
| **F8 / S4** | 13–25 |
| **F8 / S5** | 8, 11, 21–23 |
| **F8 / S6** | 12, 25, 26 |
| **H1** | 13 |
| **H2** | 14–15 |
| **H3** | 17, 19 |
| **H4** | 16, 18 |
| **H5** | 20 |
| **H6** | 21–22 |
| **H7** | 23–24 |
| **H8** | 25–26 |

## Execution notes

1. **Do not parallelize sequential semantic ownership changes.** In particular Steps 6, 8, 9, 11, 17, 18, 21 and 23 can touch overlapping Freshness/runtime/reconcile authority and should be performed one at a time in the same convergence branch.
2. Luna may prepare **independent test fixtures** for a later Sol step only if that does not mutate the same source/storage modules and the frozen interface is already explicit.
3. Each step should end with a focused test command, `git diff --check`, a clean statement of remaining failures, and a commit before moving to the next source-mutating step.
4. Do not run new expensive model-generated REL/PHY calibration merely to select a semantic baseline. Use synthetic fixtures and existing retained corpora first.
5. Historical import reconstruction remains **REL-local** until a separate PHY activity/ownership design is approved.
6. The normal live invariant is permanent unless a new ADR changes it: **semantic reinforcement is local and non-propagating.**

## Notes

The current per-step status is recorded in the first column of the sequence table. Step completion requires the evidence named in that row; a listed step is not implicitly complete.
