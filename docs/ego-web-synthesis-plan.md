# Ego Memory-Web Summary design

Parent index: [Documentation index](INDEX.md)

## Purpose

Record only the currently decided design for Ego's project-facing Memory-Web Summary.

This is a design record, not an implementation specification. Mechanics that have not been explicitly settled remain open and must not be promoted into architecture merely because they are convenient to specify.

## Status

Future behavior. The durable Ego substrate exists, but Memory-Web Summary generation is not implemented.

The design below supersedes premature execution detail that had been treated as settled architecture before its semantics were actually designed.

## Overview

Ego audits the whole eligible Memory Web while it fits, uses Freshness to prune mandatory audit exposure only when scale requires it, separately selects candidates, summarizes each as provenance-linked bullets, reconciles those bullets, and materializes three fixed stored summary sizes. Freshness is used twice: first as the raw-Memory overflow-pruning signal with Fresh as the final floor, then as averaged source-Memory priority for pruning summary bullets. Cold Memories remain searchable, and unresolved downstream mechanics stay explicitly open.

## Governing model

The full REL Memory Web remains the durable source of detail. The Memory-Web Summary is a lossy current-orientation layer used to make useful project state fit economically into routine context.

Absence from the current summary does not mean that a Memory is false, deleted, archived, or unreachable. Cold state remains available through authorized retrieval/search.

The pipeline is deliberately multi-stage:

1. build the routine working Web;
2. broadly audit it for plausible subject areas;
3. perform a separate candidate-selection pass;
4. summarize each selected candidate as provenance-linked bullet points;
5. reconcile candidate summaries into one grounded summary representation;
6. materialize three fixed stored summary sizes.

Audit is discovery. Candidate selection decides what is worth summarizing. Summary generation compresses source Memories. Reconciliation assembles the result. These are distinct responsibilities.

## Ownership

Project evidence remains REL-owned:

- Memories and their lifecycle state;
- Memory Freshness;
- source chronology;
- Dream relationships and derived Community structure; and
- authorized retrieval/search over the REL.

The project summary is user-relative. The target persisted summary perspective belongs to the active PHY and is keyed by REL identity.

Cross-chat selection, cross-REL inclusion, Personality synthesis, and final all-layer Ego context assembly remain separate designs.

## Objective lifecycle cleanup

Before Freshness is used, apply already-established objective cleanup:

- exclude Archived Memories from routine current-orientation input;
- exclude explicitly Historical Memories from routine current-orientation input;
- remove superseded copies when authoritative lifecycle state identifies a replacement;
- collapse Dream-recognized duplicates to the appropriate active/canonical representation; and
- do not discard Future Memories merely because their temporal status is Future.

This cleanup is semantic/lifecycle normalization, not relevance ranking.

## Routine working-Web Freshness pruning

After objective lifecycle cleanup, present the **entire eligible Memory Web** to the mandatory audit while it fits the configured audit budget. Do not prune merely because some Memories are Stale or Dormant.

Only when the eligible Web exceeds the audit budget does Freshness become the pruning control. Reduce mandatory audit exposure from the coldest Memories upward until the population fits. The Fresh band is the final pruning floor: if scale still requires narrowing, only Fresh Memories remain mandatory audit input.

Conceptually:

```text
eligible Web fits
    -> audit all eligible Memories

overflow
    -> prune lowest-Freshness Memories first
    -> continue until audit population fits
    -> never prune below the Fresh band solely for routine scale control
```

Freshness bands remain:

```text
Fresh:   +1 .. +100
Stale:  -50 .. 0
Dormant: -100 .. -51
```

Pruned Stale/Dormant Memories remain searchable through authorized full-Web `memory_search`. Freshness is contextual relevance, not factual truth or lifecycle authority.

The current design does **not** establish a generic "protected keep lane." In particular, there is no accepted blanket rule that "current/future/required" material bypasses Freshness. Future state survives lifecycle cleanup, but whether any temporal/lifecycle class should bypass Freshness pruning is still an open design question.

Every Memory retained in the working Web after any required pruning must receive mandatory audit exposure. If that retained population is still too large for one audit call, batch it rather than silently dropping additional admitted Memories.

## Broad audit

The broad audit exists because the Ellis experiment showed that inspect/search/follow-thread reasoning could discover coherent subject areas, while an unconstrained run did not guarantee coverage.

The auditor's job is:

> identify broad candidate areas that appear coherent enough to support a useful single-subject summary, and associate the relevant source Memories with those areas.

The audit does **not** write the summary.

The controller owns coverage:

- every working-Web Memory must be shown as mandatory core material at least once;
- larger working Webs may be processed in multiple controlled batches;
- the auditor has bounded authorized `memory_search` over the full REL Memory Web, including Stale and Dormant state;
- search supplements the mandatory sweep but does not make an unseen working-Web Memory count as covered; and
- discovered candidate-area state may carry across batches.

Community/graph structure may help routing or search, but Communities are not summary boundaries.

The settled audit output is intentionally simple: broad proposed subjects/areas plus the source Memory references that led to them. Earlier speculative contracts around explicit join/split/boundary-note records are not accepted design.

## Candidate selection

A separate candidate-selection stage is required after the broad audit.

Its decided responsibility is to take the audit's proposed areas and determine which coherent areas should actually be summarized, with an exact source-Memory set for each accepted candidate.

That stage is necessary because audit discovery and final summary membership are different judgments.

The detailed mechanics are **not yet designed**. There is currently no accepted algorithm or schema for merge/split/reject policies, overlap policy, boundary repair, or a `CandidateResolver` interface. Those details must be designed and tested rather than inferred from the existence of the stage.

Candidate selection still produces no summary prose.

## Candidate summary shape

Each accepted candidate is summarized from its closed source-Memory set.

The target summary shape is a compact bullet list rather than an opaque prose paragraph.

Conceptually:

```text
CandidateArea
  subject
  source Memory IDs
      |
      v
CandidateSummary
  - grounded summary point 1  -> source Memory IDs
  - grounded summary point 2  -> source Memory IDs
  - grounded summary point 3  -> source Memory IDs
```

Each bullet point must remain linked to the Memory IDs that support it while the generation is in progress.

The summarizer may compress redundant wording and state progression, but it must not invent a higher-order project fact that is not supported by the source Memories.

Candidate summarization uses the candidate's closed evidence set. It does not perform open-ended retrieval to silently change that evidence set.

## Summary-point Freshness

Freshness is also used after summarization.

For each provenance-linked summary bullet, derive a bullet Freshness value from the average Freshness of the source Memories supporting that bullet at the generation's source state:

```text
bullet_freshness = average(source Memory Freshness scores)
```

That score is a relevance/pruning signal for the summarized representation.

This gives Freshness two distinct roles:

1. **raw Memory level:** determine the routine mandatory audit population using the Fresh threshold;
2. **summary-point level:** prioritize provenance-linked bullets when a smaller stored summary must omit material.

Freshness still does not establish truth, correctness, supersession, or lifecycle state.

Exact rounding, tie-breaking, and handling of unusual provenance sets remain calibration/implementation details.

## Reconciliation

Candidate summaries are reconciled into one richer project-summary representation before final size reduction.

The decided purpose is narrow:

- avoid obvious duplicate summary coverage;
- preserve distinct subjects;
- keep bullet provenance available through reconciliation; and
- provide one grounded bullet-oriented source from which the stored summary sizes can be produced.

No `Essential/Core/Context/Detail` retention taxonomy is accepted design.

No general conflict-resolution algorithm has been decided. If reconciliation requires semantic conflict policy beyond existing authoritative lifecycle/chronology state, that policy must be designed explicitly rather than assumed.

## Three fixed stored summary sizes

The system stores **three fixed summary sizes** so runtime can select an appropriate amount of Ego context without launching summary inference when a conversation starts.

Exact names and token budgets remain configuration/calibration work.

For each size:

- the model target is below a controller hard ceiling;
- output at or below the ceiling is accepted;
- an over-ceiling result may be recompressed;
- output is never hard-truncated to satisfy the budget; and
- an undersized grounded result is not padded merely to consume tokens.

All three sizes are produced from the same reconciled provenance-linked summary state during one refresh.

When reducing the summary, provenance-linked bullets are prioritized using their averaged source-Memory Freshness. The exact secondary tie-breaks and compression behavior are not yet decided.

Runtime selects the largest already-stored summary size that fits the available Ego allocation. Conversation startup does not rerun audit, candidate selection, summarization, or compression.

## Provenance lifetime

Source provenance is required during candidate summarization, reconciliation, and summary-size construction.

The current decision is **not** to persist per-bullet/per-candidate provenance indefinitely merely for Ego. Once the fixed summary bundle has been successfully produced, run-local candidate/bullet provenance may be discarded.

The persisted bundle still needs the source/generation metadata required to identify what REL state it represents.

## Refresh scheduling

REL-backed project-summary scheduling remains owner-activity based rather than wall-clock based.

Decided behavior:

- first REL summary becomes eligible at 25 eligible durable Memories;
- steady-state REL refresh uses accepted incoming-turn growth from the last successful summary, using a percentage threshold capped by an absolute maximum turn delta;
- becoming due queues work rather than synchronously invoking Ego;
- the queued source boundary waits for required Insomnia and Dream settlement;
- later activity must not indefinitely move that boundary and starve an already-due refresh; and
- there is no wall-clock age backstop.

PHY-local summary freshness is separate. It uses PHY Memory mutation/version progress rather than borrowing the REL activity clock; raw Graph churn is not itself the PHY refresh clock.

Exact percentages and caps remain calibration.

## Evidence behind the audit design

The manual Ellis exploration used a roughly 490-Memory eligible REL set and discovered coherent areas including Eastwood financial-literacy work, publishing, garden state, Dell XPS reliability, household finance, meal planning, and recovery.

The useful result was that inspect/search/follow-thread reasoning could identify plausible summary subjects.

The failure was coverage: nothing forced the exploratory run to continue until the intended population had been deliberately exposed.

That evidence supports mandatory audit coverage plus full-Web search. It does not by itself justify a complicated candidate-boundary schema.

Graph measurements also showed that coherent topics can cross Community boundaries, so Community membership remains routing/organization evidence rather than semantic summary ownership.

## Explicitly unresolved

The following are not settled architecture and must remain open until designed or calibrated:

- whether any lifecycle/temporal class bypasses the Freshness gate;
- exact audit batch sizes and overlap/context policy;
- candidate-selection criteria and output schema beyond subject + exact source membership;
- candidate overlap policy;
- reconciliation behavior for unresolved contradictions;
- secondary bullet-pruning signals/tie-breaks beyond averaged Freshness;
- exact three-size token targets and ceilings;
- exact REL refresh percentage/cap values;
- exact PHY refresh percentage/cap values; and
- final binary persistence encoding for the PHY-owned per-REL summary bundle.

## Explicit non-decisions

Do not treat the following removed speculative machinery as accepted design:

- generic protected keep lanes;
- a `CandidateResolver` merge/split/reject framework;
- `SummaryUnit` / `ReconciledSummaryLedger` as required internal types;
- mandatory evidence-accounting dispositions for every candidate Memory;
- `Essential/Core/Context/Detail` retention classes;
- nested semantic-unit tier plans;
- an invented conflict-resolution policy; or
- implementation-step sequencing presented as if the downstream semantics were already settled.

These ideas may be reconsidered later only if an actual design problem and evidence justify them.

## Related docs

- [Memory Freshness policy](memory-staleness-plan.md)
- [Unified Memory Freshness implementation plan](generic-staleness-implementation-plan.md)
- [REL activity clock Phase 1 spec](rel-activity-clock-implementation-plan.md)
- [Ego Cross-chat context plan](ego-cross-chat-context-plan.md)
- [Ego cross-REL summary inclusion plan](ego-cross-rel-summary-inclusion-plan.md)
- [Roadmap](roadmap.md)

## Notes

When implementation begins, write an execution plan from this decided design at that time. Do not pre-author implementation contracts for unresolved semantics.
