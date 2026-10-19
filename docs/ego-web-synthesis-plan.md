# Ego Memory-Web Summary design

Parent index: [Documentation index](INDEX.md)

## Purpose

Record only the currently decided design for Ego's project-facing Memory-Web Summary.

This is a design record, not an implementation specification. Mechanics that have not been explicitly settled remain open and must not be promoted into architecture merely because they are convenient to specify.

## Status

Future behavior. The durable Ego substrate exists, but Memory-Web Summary generation is not implemented.

The design below supersedes premature execution detail that had been treated as settled architecture before its semantics were actually designed.

## Overview

Ego produces one maximum-detail, itemized Memory-Web Summary per source REL in each active PHY. The exact controller pipeline is **Candidate Selection → Candidate Audit → Summary Extraction → Summary Creation → Storage → Budgeting → Injection**. The controller owns Memory IDs, provenance, metadata, Freshness calculations, and selection; a final text-creation LLM receives extracted summary content, not raw `memoryID` identifiers. Persisted point provenance permits non-reinforcing, current-Freshness evaluation at injection time. There are no three pre-generated summary sizes.

Candidate Selection applies the whole-Web-first, ordered-reduction, mandatory-coverage and batching policy below. Cold source Memories remain searchable; absence from a selected summary is not deletion or loss of source authority.

## Governing model

The full REL Memory Web remains the durable source of detail. The Memory-Web Summary is a lossy current-orientation layer used to make useful project state fit economically into routine context.

Absence from the current summary does not mean that a Memory is false, deleted, archived, or unreachable. Cold state remains available through authorized retrieval/search.

The pipeline has seven stages:

1. **Candidate Selection**: admit the whole Web if it fits; otherwise apply the ordered exclusion policy and generate `memoryID` sets/batches. This stage only prepares the working Web and audit batches; it does not produce a `SummaryProposal`.
2. **Candidate Audit**: inspect selected Memories in mandatory-coverage batches, trim proposed source sets to the evidence worth carrying forward, and retain the identity/provenance mapping in the controller.
3. **Summary Extraction**: extract content to be summarized into `SummaryCandidates` (blocks associated with their pre-existing sections); maintain precise source-`memoryID` provenance as controller metadata.
4. **Summary Creation**: generate `SummaryPoint` text from extracted content and populate the existing `SummarySection`s from `SummaryProposal`. Do **not** recreate or re-identify sections. The final creation LLM does not need raw `memoryID`s, but the controller retains their associations.
5. **Storage**: persist one maximum-detail itemized summary per (PHY, source REL), with sections, points, provenance, generation/source-cut metadata, and any token-cost metadata needed for budgeting.
6. **Budgeting**: retrieve stored items and evaluate current source-Memory Freshness through non-reinforcing reads. Recalculate point and section averages and select coherent items under the *actual* runtime token budget, without summary inference.
7. **Injection**: project the budgeted content into Ego context.

All downstream operations retain `memoryID` metadata even when an LLM need not see it. Subject grouping is inherited from `SummaryProposal`, not invented during Summary Creation.

## Ownership

Project evidence remains REL-owned:

- Memories and their lifecycle state;
- Memory Freshness;
- source chronology;
- Dream relationships and derived Community structure; and
- authorized retrieval/search over the REL.

The project summary is user-relative. The target persisted summary perspective belongs to the active PHY and is keyed by REL identity.

Cross-chat selection, cross-REL inclusion, Personality synthesis, and final all-layer Ego context assembly remain separate designs.

## Working-Web admission and ordered reduction

**Whole-Web-first invariant:** if the complete REL Memory Web fits the audit budget, admit and audit it unchanged. Do not perform lifecycle cleanup or Freshness pruning simply because a Memory is Archived, Historical, duplicate, superseded, Stale, or Dormant.

Only if the Web does not fit, apply the following reductions **in strict order**, checking whether the remaining Web fits after each stage and stopping reduction as soon as it does:

1. **Archived:** exclude Archived Memories.
2. **Historical:** exclude explicitly Historical Memories.
3. **Duplicates:** collapse Dream-recognized duplicates to the appropriate canonical representation.
4. **Superseded:** exclude Memories superseded according to authoritative lifecycle state.
5. **Dormant:** exclude the Dormant Freshness band in full, if further reduction is needed.
6. **Stale:** exclude the Stale Freshness band in full, if further reduction is needed.
7. **Batching:** if the remaining population, including **all Fresh Memories**, still exceeds one audit window, process it in multiple controlled batches. Batching is not an additional exclusion step.

Do not gradually rank and drop individual Memories within the Dormant or Stale bands to fill a single audit window. These are successive band-exclusion stages, and **Fresh Memories are never excluded merely to fit a single audit window**. Every Memory admitted after the reductions must receive mandatory audit exposure, across batches where necessary.

Freshness bands remain:

```text
Fresh:   +1 .. +100
Stale:  -50 .. 0
Dormant: -100 .. -51
```

Excluded Memories remain available through authorized full-Web `memory_search`; that access does not count as mandatory audit coverage. Freshness controls routine exposure, not factual truth or lifecycle authority. Future Memories are not excluded solely for being Future; no generic protected keep lane is established.

The ordering above is audit-input preparation, not the later candidate-selection stage.

## Summary stages and data contracts

### Candidate Selection and `SummaryProposal`

Candidate Selection forms initial Memory references and broad proposed subjects. A proposed subject becomes a `SummarySection` identity at this stage; that identity is carried forward **only if** audited/extracted `SummaryCandidates` are produced for it. The selected set is prepared with whole-Web-first admission, the ordered reductions above and mandatory batches. Batching is a processing boundary, never a forced subject boundary.

### Candidate Audit

Candidate Audit examines selected `memoryID` sets, trims them down based on suitability for summary, and may inspect/search through authorized full-Web retrieval. Every admitted Memory gets mandatory coverage; search of other Memories does not count as that coverage. Audit does not author final summary prose. Exact batch size, cross-batch context and audit decision criteria remain to be specified.

### Summary Extraction and `SummaryCandidates`

Extraction transforms audited evidence into candidate content blocks for creation, retaining the association between each block, its section identity and its exact supporting `memoryID`s. Provenance metadata is maintained **outside** the final LLM's prose-input contract; the controller must be able to attach it to the resulting points without guessing. Extraction does not silently widen the audited source set.

### Summary Creation and `SummaryPoint`s

Creation transforms the extracted blocks into grounded, independently understandable `SummaryPoint` text under the pre-existing `SummarySection` subjects. The `SummarySection` is not regenerated or renamed in this stage. Controller-side identity/provenance associations survive creation, even though the final LLM does not receive raw `memoryID` identifiers. Do not invent project conclusions unsupported by source Memories.

### Point and section Freshness

Both averages are **ordinary arithmetic means**, not weighting, propagation, model scores, or a new Freshness mechanism:

- `SummaryPoint.freshness` = average of the current Freshness values of the distinct source Memories referenced by that point.
- `SummarySection.freshness` = average of the Freshness values of its `SummaryPoint`s. Each point contributes equally, regardless of its provenance count.

These values may be recorded as derived/cached metadata, but are not the authoritative source. During budgeting, they can be recalculated from the source REL without regenerating summary text. The retrieval path for Memory, provenance and Freshness metadata must be **non-reinforcing**: no access boost, propagation, persistent mutation, or activity-clock advancement merely from inspection. Lazy score projection is permitted. Source-Memory Freshness is a relevance signal, not truth or lifecycle authority.

### Storage, Budgeting and Injection

Store **one maximum-detail itemized summary per source REL in each PHY**, preserving section identity, standalone points, supporting `memoryID` provenance, source-version/generation cut and enough deterministic formatting/token data for budgeting. Do not discard point provenance after publication.

At runtime Budgeting selects from these same persisted points and sections for any available budget, using their **current** Freshness; there are no fixed small/medium/large summary variants and no summary LLM call on startup. Section and point scores support prioritization, while validation of superseded/archived source state remains distinct from Freshness. Exact handling of item dependencies, ties, lifecycle invalidation, token packing and section representation is still open. Injection simply assembles selected context, without counting internal retrieval as accepted external Memory use.

### Reconciliation limits

Avoid obvious redundant coverage while preserving distinct subjects and provenance. No speculative `CandidateResolver`, general merge/split engine, mandatory per-Memory disposition taxonomy, or invented contradiction resolution is accepted. If semantic contradictions cannot be resolved from authoritative source state, their handling requires a separate design decision.

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

- whether a future explicitly defined temporal exemption is warranted beyond the currently accepted whole-Web-first and ordered-reduction rules;
- exact audit batch sizes and overlap/context policy;
- precise candidate audit trimming and extraction criteria, plus controller provenance-to-point association;
- candidate/subject overlap policy and cross-batch subject continuity;
- reconciliation behavior for unresolved contradictions;
- section-versus-point budgeting priority, item dependencies, lifecycle invalidation and deterministic tie-breaks;
- maximum-detail summary generation ceiling and actual-budget item-packing rules;
- exact REL refresh percentage/cap values;
- exact PHY refresh percentage/cap values; and
- final binary persistence encoding for the PHY-owned, per-REL itemized summary and its persistent provenance.

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