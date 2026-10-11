# Ego Memory-Web Summary implementation plan

Parent index: [Documentation index](INDEX.md)

Status: **future-only implementation specification; no stages in this document are claimed implemented unless current-state documentation says so.**

## Purpose

Specify the implementation of Ego's user-relative REL Memory-Web Summary pipeline from deterministic audit-population construction through persisted summary tiers and refresh scheduling.

This document is the execution contract for the Memory-Web Summary component of Ego. The design rationale, corpus measurements, rejected alternatives, and calibration background remain in [Ego Memory-Web summary plan](ego-web-synthesis-plan.md). Cross-chat selection and cross-REL inclusion remain separate plans.

## Overview

Implementation proceeds linearly from permanent data/ownership contracts through deterministic audit-population construction, mandatory broad audit, candidate resolution, exact-source candidate summarization, reconciliation, persisted tier construction, atomic PHY publication, inference-free runtime tier selection, and finally automatic refresh scheduling. Audit, candidate resolution, candidate summarization, reconciliation, and compression remain distinct inference responsibilities. Numeric calibration values remain versioned policy rather than architectural blockers.

## Completion target

A completed implementation must be able to:

1. capture one coherent REL/PHY summary-generation input state;
2. construct the largest practical audit population without scale-driven pruning while it fits;
3. guarantee mandatory audit coverage of that population while allowing the auditor to search the full authorized REL Memory Web;
4. resolve discovered bodies into explicit summary candidates without writing summary prose;
5. summarize each accepted candidate from its exact source Memories;
6. reconcile candidate digests into one rich grounded representation;
7. derive and atomically persist large, medium, and small summary tiers for the active PHY keyed by REL;
8. select an already-persisted tier at runtime without triggering inference; and
9. schedule refresh from owner-local semantic activity while respecting Insomnia/Dream prerequisite settlement.

The pipeline is intentionally multi-stage and multi-call. It must not collapse audit, candidate resolution, candidate summarization, reconciliation, and tier compression into one inference call.

## Governing invariants

1. **Small Webs stay whole.** After deterministic lifecycle/canonical cleanup, no scale-driven pruning or batching occurs while the complete eligible Web fits the configured single-audit ceiling.
2. **Take information away only when scale requires it.** The controller presents as much of the eligible Web as economically practical.
3. **Pruning controls mandatory exposure, not reachability.** Every audit inference call has bounded search access to the full authorized, version-consistent REL Memory search surface. Search is not restricted to the working/audit population.
4. **Controller-owned coverage.** Every Memory admitted to the working audit population appears as mandatory core material in at least one audit batch before audit completion.
5. **Freshness is relevance, not truth.** Freshness can rank or route Memories when scale requires narrowing. It cannot invalidate, archive, supersede, or make a Memory factually false.
6. **Protected lanes survive relevance pruning.** Explicit current/future/required state and other policy-protected material cannot disappear merely because semantic activity or Freshness is low.
7. **Communities route; they do not define subjects.** Community and graph structure may order exposure, overlap, and search, but candidate boundaries are semantic.
8. **Audit discovers; it does not summarize.** Candidate resolution is also separate from summarization.
9. **Summaries compress existing Memory state.** A candidate digest or reconciled summary must not introduce a new higher-order proposition. New inference belongs to Perception/Observation.
10. **Exact candidate evidence.** Every accepted candidate has explicit Memory membership. Candidate summarization is hydrated from that closed source set, not from an open-ended retrieval query.
11. **One coherent generation.** All persisted tiers in a bundle come from the same captured source cuts and policy version.
12. **No conversation-start inference.** Runtime selects an already-stored tier; missing/stale summary state is explicit and is handled by background scheduling.
13. **No partial publication.** A failed generation leaves the prior committed bundle intact.
14. **Stable ownership.** REL owns the source evidence; the user-relative summary bundle is PHY-owned and keyed by stable REL owner identity.

## Scope

This plan implements the project-facing, active-PHY-relative Memory-Web Summary.

Included:

- audit-population construction;
- lifecycle/canonical cleanup;
- recency/Freshness/semantic routing for overflow;
- mandatory broad audit;
- full-Web audit search;
- candidate resolution;
- exact-source candidate summarization;
- rich-summary reconciliation;
- three stored summary tiers;
- PHY-owned REL-keyed bundle persistence;
- runtime tier selection; and
- queued refresh scheduling.

Not included:

- Personality synthesis;
- Cross-chat materialization;
- cross-REL summary selection;
- final all-layer Ego prompt/context assembly;
- Perception/Observation synthesis;
- a shared team-wide project synopsis;
- user-facing summary editing policy; or
- host-specific prompt formatting.

## Permanent component boundary

```text
EgoWebSummaryEngine
    |
    +-- AuditPopulationBuilder
    |    +-- source-cut capture
    |    +-- deterministic lifecycle/canonical cleanup
    |    +-- protected keep lanes
    |    +-- overflow relevance routing
    |    +-- batching as final escalation
    |
    +-- AuditController
    |    +-- sequential mandatory coverage
    |    +-- full-Web memory_search
    |    +-- run-local discovered-body state
    |
    +-- CandidateResolver
    |    +-- exact CandidateArea membership
    |    +-- merge / split / reject
    |
    +-- CandidateSummarizer
    |    +-- closed source hydration
    |    +-- bounded parallel candidate jobs
    |
    +-- SummaryReconciler
    |    +-- rich grounded representation
    |
    +-- SummaryTierBuilder
    |    +-- large
    |    +-- medium
    |    +-- small
    |
    +-- EgoSummaryStore
         +-- PHY-owned bundle keyed by REL
```

The engine is an Ego orchestration owner, not a new semantic authority.

## Core types and contracts

Exact Rust names may vary, but implementation must expose equivalent semantics.

```text
EgoSummaryPolicy
    policy_version
    max_single_audit_tokens
    max_audit_batch_tokens
    max_audit_core_memories
    audit_overlap_tokens
    recent_activity_window_turns
    overflow_selection_policy
    rel_bootstrap_eligible_memories = 25
    rel_refresh_fraction
    rel_max_refresh_turn_delta
    phy_refresh_fraction
    phy_max_refresh_memory_delta
    max_parallel_candidate_summaries
    resolver_input_ceiling
    candidate_summary_input_ceiling
    reconciliation_input_ceiling
    large: SummaryTierBudget
    medium: SummaryTierBudget
    small: SummaryTierBudget

SummaryTierBudget
    model_target_tokens
    hard_ceiling_tokens

EgoSummarySourceCut
    rel_owner_id
    phy_owner_id
    rel_turn_count
    memory_version
    graph_version
    community_generation_or_validation_identity
    compatibility_profile_id
    policy_version

AuditPlan
    source_cut
    complete_eligible_count
    complete_eligible_tokens
    admitted_memory_ids
    exclusions: [AuditExclusion]
    batches: [AuditBatch]

AuditExclusion
    memory_id
    reason

AuditBatch
    sequence
    core_memory_ids
    overlap_memory_ids

DiscoveredBody
    run_local_id
    tentative_subject
    referenced_memory_ids
    unresolved_boundary_notes

CandidateArea
    candidate_id
    subject
    memory_ids

CandidateDigest
    candidate_id
    text
    memory_ids

ReconciledSummary
    text
    candidate_ids

EgoWebSummaryBundle
    rel_owner_id
    phy_owner_id
    generation
    source_cut
    large: PersistedSummaryTier
    medium: PersistedSummaryTier
    small: PersistedSummaryTier

PersistedSummaryTier
    text
    measured_tokens

QueuedEgoRefresh
    rel_owner_id
    phy_owner_id
    through_episode_id
    due_watermark
```

Candidate/source membership is mandatory run-local provenance through reconciliation. The persisted bundle need not retain every candidate-to-Memory mapping unless later product requirements justify that storage cost; it must retain the exact source cuts and generation identity.

## Source-cut consistency

A generation must not mix arbitrary live states.

At run start, after prerequisite semantic processing is ready, capture the source identities needed to describe the generation: REL turn count, Memory version, Graph version, current validated Community identity/generation when used, compatibility profile, active PHY, and policy version.

All deterministic population construction, search, candidate hydration, and generation metadata must refer to that coherent run state. If the current APIs cannot provide a consistent read at the required cuts, the implementation must fail or restart the uncommitted generation rather than silently combine incompatible states.

A newer REL mutation may occur while inference is running. It does not mutate the in-flight generation's declared source identity. A subsequent refresh accounts for later state.

## Deterministic audit-population policy

### Baseline cleanup

Baseline cleanup is semantic/current-orientation cleanup, not scale-driven pruning. Apply the already accepted rules before deciding whether the Web fits:

- exclude Archived Memories from normal current-orientation exposure;
- exclude explicitly Historical Memories from normal current-orientation exposure;
- remove superseded copies when the authoritative current chain already identifies a replacement;
- collapse Dream-recognized duplicate copies to the appropriate active/canonical representation; and
- retain future/current state according to the existing lifecycle/Chronos policy.

Excluded state remains available through the existing authorized retrieval/search surface where that surface permits it.

### Whole-Web rule

Measure the complete eligible population after baseline cleanup.

If it fits `max_single_audit_tokens`, produce one audit batch containing the entire eligible population. Do not apply Freshness thresholds, recency thresholds, category filtering, community quotas, or arbitrary top-K truncation merely to make a small Web smaller.

### Overflow routing

Only when the complete eligible population exceeds the single-audit ceiling may the controller narrow default exposure.

The overflow selector must combine, without treating any one signal as semantic authority:

- active-PHY recent REL conversation-tail relevance;
- multiple recent-activity semantic centroids;
- Memory Freshness;
- explicit lifecycle/Chronos protected lanes;
- graph/Community structure for coverage/diversity; and
- stable deterministic tie-breaking.

Requirements:

- protected keep lanes are admitted before relevance-ranked discretionary material;
- Freshness is graded and does not automatically remove Dormant state;
- no category/type class may be globally discarded merely because it is common;
- Communities may provide coverage quotas/order only as a routing aid;
- the admitted set and every exclusion reason are inspectable;
- cold/excluded Memories remain searchable; and
- batching is attempted only after acceptable deterministic narrowing still cannot fit one audit call.

Exact numeric weights, thresholds, and centroid counts are configuration/calibration values, not architectural decisions.

### Batching

When batching is required:

- core coverage is disjoint for accounting purposes: each admitted Memory becomes new mandatory core in at least one batch;
- graph/semantic overlap may repeat already-covered Memories as context;
- only core IDs advance mandatory coverage;
- batch ordering is deterministic from the captured audit plan;
- overlap cannot crowd out required core below the configured minimum;
- batch size obeys both token and Memory-count ceilings; and
- no batch construction can make a Memory unreachable to full-Web search.

## Inference execution contract

The stages below are separate inference responsibilities.

### Audit inference

Execution shape:

```text
for batch in audit_plan.sequence:
    hydrate mandatory core + bounded overlap
    provide compact discovered-body state
    expose bounded full-Web memory_search
    run one audit reasoning call
    validate returned body/member IDs
    merge discovered-body state
    mark batch core covered
```

Audit batches execute sequentially because later calls may extend or correct discovered bodies from earlier calls.

The model may search outside the mandatory batch whenever the local evidence suggests related state elsewhere. Search results supplement the sweep; they do not satisfy mandatory core coverage for Memories not yet exposed as core.

The auditor cannot declare the run complete while uncovered mandatory core remains.

Audit output is discovery state only: tentative subjects, source references, possible joins/splits, and boundary questions. Summary prose is rejected at this stage.

### Candidate resolution inference

Candidate resolution begins only after audit coverage is complete.

Responsibilities:

- turn discovered bodies into explicit `CandidateArea` records;
- merge duplicate discovered bodies;
- split multi-subject bodies;
- reject bodies that are too sparse or incoherent to summarize independently;
- resolve exact source membership; and
- resolve candidate overlap according to policy.

It writes no summary prose.

If the entire discovery set fits `resolver_input_ceiling`, use one resolver call. If it does not fit, partition discovered bodies deterministically, resolve partitions independently, then run a bounded final merge/overlap-resolution call over the resulting candidate metadata. Every output Memory ID is validated against the source cut.

### Candidate summarization inference

Each accepted candidate is an independent summary job over a closed evidence set.

For a candidate that fits `candidate_summary_input_ceiling`:

1. hydrate the exact candidate Memory IDs at the run source cut;
2. provide source chronology/provenance needed for faithful state progression;
3. run one candidate-summary call;
4. produce one `CandidateDigest`; and
5. retain the candidate's source Memory IDs with the digest.

Candidate jobs may execute concurrently up to `max_parallel_candidate_summaries`. Concurrency must not affect output ordering, persistence order, or generation identity; normalize results by stable candidate ID before reconciliation.

If one candidate is too large for one inference call, do not silently truncate its source set. Deterministically partition that candidate's evidence into bounded source chunks, produce grounded intermediate digests, then run one candidate-finalization call over those intermediate digests plus the candidate identity. The final digest still records the complete original candidate Memory set.

Candidate summarization may compact redundancy, state progression, current constraints, decisions, and future state. It must not invent a proposition not collectively supported by its source Memories.

### Reconciliation inference

Reconciliation consumes candidate digests, not the raw full Web.

Its job is to:

- remove redundant candidate coverage;
- preserve distinct active subjects;
- establish useful ordering/current orientation;
- preserve important recent history needed to understand current state; and
- produce one rich grounded representation.

It must not infer new project facts.

If all candidate digests fit `reconciliation_input_ceiling`, run one reconciliation call. If they do not, deterministically partition candidate digests, reconcile each partition into grounded fragments, then run a final bounded reconciliation call over those fragments. Candidate IDs remain attached through the process so overlap removal is auditable.

No final small/medium/large budget is imposed yet beyond a safety ceiling for the rich representation.

### Tier-building inference

Build all three tiers during the same generation:

```text
ReconciledSummary
      |
      +--> build Large to large.model_target_tokens
                |
                +--> if > large.hard_ceiling_tokens: bounded recompress
                |
                +--> build Medium from the accepted richer state
                          |
                          +--> if > medium.hard_ceiling_tokens: bounded recompress
                          |
                          +--> build Small from the accepted richer state
                                    |
                                    +--> if > small.hard_ceiling_tokens: bounded recompress
```

Requirements:

- model targets are below hard ceilings;
- controller/token counter owns compliance;
- oversize output triggers bounded semantic recompression, never byte/token truncation;
- undersize output is acceptable and never padded;
- each smaller tier must preserve the important state of the richer tier while dropping detail;
- all tiers share the same generation/source cut;
- a tier-build failure prevents publication of the new bundle.

The implementation must cap recompression attempts. Exhaustion returns a generation failure and preserves the previous committed bundle.

## Inference authority and validation

Model calls propose audit bodies, candidate boundaries, and compressed prose within their stage contract. Deterministic controller code owns:

- source identity;
- authorization;
- mandatory coverage;
- valid Memory/candidate IDs;
- exact candidate hydration;
- token ceilings;
- stage progression;
- retry limits;
- bundle generation identity; and
- atomic publication.

The first implementation does not create a second semantic verifier that itself becomes a competing Observation authority. Semantic support is enforced by closed-source prompts/contracts plus corpus acceptance tests. If measured production failures later justify a verifier, add it as a separate explicitly designed capability.

## Failure, retry, cancellation, and recovery

### Before publication

All audit bodies, candidate records, intermediate candidate digests, reconciliation fragments, and tier drafts are provisional.

If any stage fails:

- do not mutate the current committed bundle;
- retain the queued refresh as pending/retryable under normal worker policy;
- discard or explicitly invalidate incomplete run-local state; and
- retry from a coherent source cut.

A retry may use a newer fully settled source cut if the previous uncommitted cut is no longer directly readable, but the generation must accurately record the cut it actually used. Never claim that a retry represents an earlier cut when it does not.

### Cancellation

Cancellation between inference calls stops the generation with no publication.

If provider cancellation occurs during a call, treat the call as failed and discard its partial output unless the provider contract supplies an explicitly complete validated result.

### Publication

The complete `EgoWebSummaryBundle` is one atomic semantic publication. Large/Medium/Small tiers from different generations must never be observable as one current bundle.

On reopen, only fully committed bundles are current.

### Existing bundle behavior

A failed or pending refresh does not delete the last good bundle. Runtime policy can decide whether an older bundle is acceptable, stale-but-labeled, or omitted; it must not represent it as freshly regenerated.

## Persistence contract

The target project summary lives in the active user's PHY and is keyed by durable REL owner ID.

Do not extend REL-owned cached synthesis as a temporary final architecture.

The persisted bundle must include at least:

- REL owner ID;
- PHY owner ID or implicit owning PHY identity;
- generation;
- policy version;
- successful source REL turn count;
- Memory source version;
- Graph source version;
- compatibility profile identity needed to interpret selection provenance;
- Large text + measured token count;
- Medium text + measured token count; and
- Small text + measured token count.

Exact binary record encoding belongs in [Storage format](storage-format.md) when implemented.

A successful replacement is latest-wins for that PHY/REL summary key while historical storage follows the container's normal version semantics.

## Runtime tier selection

Runtime context assembly performs no Memory-Web inference.

Given a valid current bundle and the Memory-Web allocation:

1. choose Large if it fits;
2. otherwise choose Medium if it fits;
3. otherwise choose Small if it fits;
4. otherwise omit the Memory-Web Summary or use an explicitly defined minimal fallback outside this plan.

Do not dynamically recompress a tier during conversation startup.

Tier selection must account for the actual target provider/token counter or a conservative compatible estimate. The persisted token count is an optimization/record; a provider-specific counter may remeasure if required.

## Refresh scheduling

### REL-backed project summary

Bootstrap:

- first generation becomes due at 25 eligible durable REL Memories.

Steady state:

```text
required_turn_delta =
    min(
        ceil(source_turn_count * rel_refresh_fraction),
        rel_max_refresh_turn_delta
    )

refresh_due =
    current_rel_turn_count - source_turn_count
        >= required_turn_delta
```

When due:

- mark the current Episode/source boundary;
- maintain at most one pending refresh for the PHY/REL perspective;
- do not invoke Ego immediately;
- wait until Insomnia is terminal through the marked boundary;
- wait until all Dream work required by that Insomnia output is terminal; and
- then allow generation to start.

Newer activity must not indefinitely move the readiness boundary forward and starve an already-due refresh.

There is no wall-clock refresh backstop.

### PHY-local follow-on

PHY-local summary scheduling remains distinct from the REL-backed project summary. If/when implemented, use PHY Memory mutation/version progress rather than pretending the PHY owns the REL activity clock. Its exact summary artifact is outside the project-summary completion gate unless separately added.

## Step 1 — Final types, ownership, and storage seam

Implement permanent types/interfaces before model orchestration.

Work:

- replace the single-string target shape with the REL-keyed PHY bundle model;
- add policy and tier-budget types;
- add source-cut/run identity types;
- add audit plan/exclusion/batch types;
- add discovered-body/candidate/digest/reconciled types;
- expose a read/write store seam whose final owner is PHY;
- define atomic bundle replacement;
- define explicit errors for unsupported downstream stages rather than temporary bypasses.

Acceptance:

- type/model unit tests;
- REL-keyed isolation between two REL summaries in one PHY;
- two PHYs cannot observe each other's bundle through owner-local APIs;
- atomic replacement/reopen test;
- no new inference path yet.

## Step 2 — AuditPopulationBuilder

Implement source-cut capture, baseline cleanup, whole-Web measurement, overflow selection, and batch planning.

Work:

- build complete eligible Memory population;
- record baseline exclusions and reasons;
- preserve whole eligible Web when it fits;
- integrate recent conversation-tail centroids;
- integrate Freshness as graded overflow signal;
- implement protected lanes;
- implement stable overflow ranking and tie-breaking;
- implement graph/Community-aware overlap and deterministic batching;
- emit inspectable `AuditPlan`.

Acceptance:

- small Web produces exactly one untouched eligible batch;
- lowering the single-audit ceiling changes routing/batching, not semantic storage;
- Dormant Memories remain included when the whole eligible Web fits;
- protected state survives overflow routing;
- every admitted ID occurs as core in at least one batch;
- no excluded ID accidentally becomes mandatory core;
- exclusions have deterministic reasons;
- repeated construction from same cut/policy is byte-for-byte/stably equivalent where serialization exists.

## Step 3 — AuditController

Implement sequential mandatory audit with full-Web search.

Work:

- hydrate core + overlap;
- expose source-cut-consistent `memory_search`;
- maintain discovered-body state across calls;
- validate returned IDs;
- prevent completion before mandatory coverage is complete;
- bound search calls/results/tokens per inference;
- expose run metrics: batches, core coverage, searches, discovered bodies.

Acceptance:

- one-batch Web completes in one audit inference call;
- multi-batch Web cannot finish early;
- auditor can recover a relevant Memory outside the working population through search;
- search result does not mark that Memory as mandatory-core covered;
- invalid/stale IDs are rejected;
- batch N+1 receives compact prior discovery state without replaying the entire previous prompt;
- cancellation leaves no persistent summary state.

## Step 4 — CandidateResolver

Implement the separate candidate-boundary stage.

Work:

- normalize discovered bodies;
- merge/split/reject with explicit result reasons;
- validate every candidate Memory ID;
- ensure each candidate has one subject and non-empty exact membership;
- implement deterministic overflow partition/final merge path;
- retain overlap information for calibration/inspection.

Acceptance:

- no summary prose output is required or accepted as the stage artifact;
- duplicate bodies can converge to one candidate;
- a mixed body can split into separate candidates;
- incoherent/sparse bodies can be rejected;
- candidate membership cannot reference a Memory outside the captured REL/source surface;
- large discovery sets take the bounded multi-call resolver path without changing the output contract.

## Step 5 — CandidateSummarizer

Implement exact-source candidate summaries.

Work:

- hydrate each candidate's exact source set;
- preserve chronology/provenance context;
- run bounded parallel candidate jobs;
- normalize outputs by stable candidate ID;
- implement oversized-candidate hierarchical summarization without source truncation;
- attach complete candidate source IDs to each final digest.

Acceptance:

- unrelated candidate jobs may run concurrently;
- scheduling order cannot alter committed candidate ordering;
- no retrieval/search tool is exposed as an open evidence source during candidate summarization;
- oversized candidates cover every original source Memory through intermediate chunks;
- failed one-candidate job fails the generation rather than silently dropping that candidate;
- corpus tests score support, current-state preservation, decision/constraint preservation, and hallucinated higher-order propositions.

## Step 6 — SummaryReconciler

Implement rich-summary assembly.

Work:

- consume candidate digests;
- remove duplicate coverage;
- retain distinct active subjects;
- maintain useful current-state/recent-history ordering;
- carry candidate IDs through reconciliation;
- implement bounded hierarchical reconciliation for oversized digest populations.

Acceptance:

- repeated coverage does not duplicate final rich-summary sections;
- distinct active subjects remain represented;
- no raw full-Web search/retrieval occurs in reconciliation;
- every reconciled factual section is traceable to one or more candidate digests during the run;
- oversized digest populations use the same semantic contract through hierarchical reconciliation.

## Step 7 — SummaryTierBuilder

Implement Large/Medium/Small materialization.

Work:

- configure target/ceiling per tier;
- tokenize/measure actual outputs;
- bounded semantic recompression on oversize;
- persist measured size metadata in the draft bundle;
- ensure one source generation for all tiers.

Acceptance:

- each tier <= its hard ceiling;
- no string/token truncation path exists;
- undersize tier is accepted unchanged;
- recompression exhaustion fails the generation;
- Small preserves designated must-retain current constraints/decisions/future state from the rich source according to corpus gates;
- tiers cannot be mixed across generations.

## Step 8 — EgoSummaryStore publication

Wire full generation to atomic PHY persistence.

Work:

- generate new bundle off to the side;
- atomically publish only after all stages succeed;
- preserve previous committed bundle on failure;
- reopen and read latest bundle;
- reject REL/PHY ownership mismatches;
- update current-state docs/storage format when implementation lands.

Acceptance:

- crash/failure before commit preserves old bundle;
- committed bundle survives reopen;
- two REL keys update independently;
- generation/source-cut metadata round-trips;
- no run-local candidate/provenance scratch state is accidentally presented as durable authority.

## Step 9 — Runtime tier selection

Expose inference-free read selection.

Work:

- load current PHY/REL bundle;
- evaluate validity/staleness policy;
- select largest fitting tier;
- expose source/generation metadata to downstream context assembly;
- return explicit missing/stale states.

Acceptance:

- no model/provider call occurs;
- Large -> Medium -> Small fallback is deterministic;
- no dynamic compression occurs;
- missing bundle does not synchronously trigger generation;
- stale labeling cannot be lost when returned downstream.

## Step 10 — Refresh scheduling and end-to-end acceptance

Attach automatic due calculation only after Steps 1–9 work end to end.

Work:

- REL bootstrap at 25 eligible Memories;
- percentage/capped turn-delta due calculation;
- pending-refresh coalescing;
- Episode/source boundary capture;
- Insomnia/Dream prerequisite readiness;
- worker retry/cancellation integration;
- end-to-end metrics and corpus calibration.

Acceptance:

- below bootstrap: no generation;
- crossing bootstrap queues exactly one refresh;
- steady-state threshold queues exactly one pending refresh;
- retries/replays do not duplicate pending work;
- due work waits for required Insomnia/Dream settlement;
- newer activity cannot starve an already-marked refresh;
- failed generation leaves the prior bundle and pending retry state intact;
- successful generation advances the source watermark;
- conversation startup remains inference-free.

## Verification matrix

### Deterministic/controller tests

Must cover:

- lifecycle/canonical cleanup;
- whole-Web no-pruning invariant;
- protected keep lanes;
- Freshness ranking without truth/lifecycle mutation;
- stable batch construction;
- complete mandatory coverage;
- full-Web search access outside the working population;
- source-cut/ID validation;
- bounded retry/recompression;
- atomic publication;
- owner/privacy isolation; and
- scheduling/idempotency.

### Corpus semantic tests

Use at least:

- Ellis;
- frozen 14-day REL;
- frozen 28-day REL; and
- purpose-built adversarial candidate fixtures.

Measure:

- audit subject/thread recall;
- fraction of useful candidates missed by mandatory-only exposure but recovered by search;
- candidate coherence and overmerge/oversplit rates;
- source-membership precision;
- candidate-summary source support;
- preservation of current constraints, decisions, unresolved work, and future state;
- hallucinated higher-order proposition rate;
- reconciliation redundancy;
- active-thread retention;
- tier semantic retention versus size;
- inference call count/cost; and
- wall/runtime latency separately from semantic quality.

Calibration may change numeric policy values only after these measurements. It must not weaken the governing invariants to improve a benchmark.

## Metrics and observability

Per generation record non-content operational metrics sufficient to diagnose scale/cost:

- eligible Memory count/tokens;
- baseline exclusion counts by reason;
- admitted/excluded overflow counts;
- batch count and sizes;
- audit search count/result count;
- discovered-body count;
- accepted/rejected candidate count;
- candidate summarization call count;
- hierarchical summarization/reconciliation usage;
- tier build/recompression attempts;
- total inference calls by stage;
- stage durations; and
- final measured tier sizes.

Do not log Memory bodies, summary content, credentials, or other private content merely for observability.

## Calibration knobs that remain intentionally open

The following are configuration/calibration values, not blockers to implementation structure:

- recent conversation-tail window;
- tail length;
- number/construction of recent-activity centroids;
- exact semantic/Freshness weighting in overflow;
- exact protected-lane ordering among simultaneously protected material;
- single-audit and per-batch token ceilings;
- core Memory-count ceiling;
- overlap budget;
- candidate overlap tolerance;
- resolver/candidate/reconciliation input ceilings;
- candidate-summary parallelism;
- Large/Medium/Small model targets and hard ceilings;
- REL refresh fraction and max turn delta; and
- bounded retry/recompression counts.

Every default must be explicit, versioned by policy, and replaceable without changing durable semantic ownership.

## Explicit non-solutions

Do not implement:

- one-shot whole-Web final summarization;
- retrieval-only auditing;
- Community-as-summary-boundary behavior;
- category/type exclusion as the primary scale control;
- wall-clock recency for project state;
- global-user activity aging of an inactive REL;
- a model-controlled decision to skip mandatory audit batches;
- candidate summarization with open-ended retrieval;
- final tier generation at conversation start;
- hard token truncation of generated summaries;
- a new Ego truth/Observation authority; or
- temporary REL-owned project-summary persistence that must later migrate to PHY.

## Completion definition

Memory-Web Summary implementation is complete when Steps 1–10 pass their focused tests and the full repository regression/documentation gates, the Ellis/14-day/28-day semantic evaluation has recorded acceptable results for the selected policy, and a live host can consume a persisted tier without any synchronous Ego summary inference.

Completion of this plan does **not** imply completion of Personality, Cross-chat, cross-REL inclusion, or final all-layer Ego context assembly.

## Related docs

- [Ego Memory-Web summary design and calibration record](ego-web-synthesis-plan.md)
- [Ego Cross-chat context plan](ego-cross-chat-context-plan.md)
- [Ego cross-REL summary inclusion plan](ego-cross-rel-summary-inclusion-plan.md)
- [Unified Memory Freshness implementation plan](generic-staleness-implementation-plan.md)
- [REL activity clock Phase 1 spec](rel-activity-clock-implementation-plan.md)
- [Roadmap](roadmap.md)
- [Current limitations](current-limitations.md)
- [Architecture](architecture.md)

## Notes

This is a future-only execution specification. As stages ship, move current behavior into Architecture/API/Storage/Behavioral contracts as required by repository documentation policy, remove completed work from future-only planning, and retain this document only for remaining implementation/calibration obligations.
