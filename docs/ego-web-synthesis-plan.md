# Ego Memory-Web summary plan

Parent index: [Documentation index](INDEX.md)

## Purpose

Record the future-only policy, rationale, measurements, and calibration work for Ego Memory-Web Summary construction without presenting unimplemented behavior as current architecture.

The filename retains the earlier `ego-web-synthesis-plan.md` name for link stability. The target artifact is a **Memory-Web Summary**, and the broad audit that discovers candidate areas is not itself a summarization pass.

## Overview

The target pipeline is now explicitly staged:

1. derive a bounded, user-relative **working Memory Web** from shared REL state;
2. perform a broad reasoning **audit** over that working Web with mandatory coverage and full-Web `memory_search`;
3. perform separate **candidate selection** over the areas/threads discovered by the audit;
4. summarize only the selected candidates from their exact source Memories; and
5. reconcile those candidate summaries into the cached Memory-Web Summary.

The design remains Memory-first and Perception-optional. Entity/Observation state may later improve selection or organization when available, but Ego must remain semantically complete without Perception.

The target project summary is user-relative. Shared evidence remains REL-owned, while the perspective used to choose and summarize currently relevant project state is keyed to PHY identity.

## Status

This document records the current design direction and the measurements that motivated it. The durable Ego substrate exists, but the working-Web selector, audit controller, candidate-selection pass, summary inference, reconciliation, activity accounting, and refresh scheduling described here are not yet implemented.

## Pre-established Ego pieces

- Identity is PHY-owned and high-inertia.
- Personality is PHY-owned and lower-inertia.
- Anchors are durable PHY/REL Ego inputs.
- Cached Memory-Web synthesis persistence already exists in the current substrate.
- Cross-chat context is a separate Ego layer with its own deterministic selection/budget policy.
- REL-local activity is preferred over wall-clock age for project recency.
- Chronos owns deterministic temporal interpretation from Memory text plus authoritative source chronology.
- Memory vectors, Community routing, and deterministic Community sub-centroids already exist.
- Dream Graph/Community structure is derived routing/organization state, not semantic authority.
- REL conversation records carry PHY identity, allowing user-specific recent conversation tails to be selected from shared REL state.
- Ego must not require Perception; Entities/Observations are optional enrichment only.

## Scope

This plan primarily covers project-facing Memory-Web Summary construction and refresh, and records the owner-specific scheduling distinction where PHY-local Ego state cannot correctly inherit the REL activity policy.

It does not define:

- Identity synthesis: Identity is explicit, PHY-owned, and high-inertia rather than synthesized;
- final multi-owner context assembly, hierarchy, prompt budgeting, or Warlock injection;
- Observation synthesis or other Perception-owned inference;
- a project-wide/team-wide shared summary artifact; or
- the final persistence migration/schema needed to move the target user-relative project summary fully under PHY ownership.

Personality ownership and evidence boundaries are recorded here because they are part of Ego, but Personality synthesis mechanics remain a separate follow-up.

## Governing principles

The full Memory Web remains the durable source of detail. The Memory-Web Summary is a compact current-orientation layer, not a replacement for retrieval.

A Memory may be absent from the current working Web or final summary without being false, obsolete, or unimportant. Cold state remains retrievable through `memory_search`.

Deterministic machinery may narrow and route the working set using explicit lifecycle state, temporal state, user-relative activity, embeddings, graph structure, and calibrated thresholds. These mechanics are **selection/routing**, not semantic authority.

The controller owns coverage. The model owns semantic judgement.

For audit population construction and candidate discovery, two starting invariants apply:

1. **No scale-driven pruning or batching occurs until the complete Memory Web exceeds the calibrated maximum single audit-batch size.** While the whole Web fits, the whole Web is presented to the audit.
2. **Give the model as much of the Web as economically practical, and only start taking information away when scale requires it.**

Once scale requires narrowing, pruning proceeds in an explicit deterministic priority order; batching is the final escalation after acceptable pruning can no longer bring the audit population within the single-batch maximum.

The strongest practical coverage guarantee is:

> Every Memory admitted to the working Web must be surfaced as mandatory core material in at least one audit batch before the audit can complete.

This does not guarantee internal model attention to every token. It does prevent retrieval choice or early completion from leaving selected Memories wholly unseen.

## Ownership: shared evidence, user-relative perspective

The relevant project evidence is REL-owned:

- conversation records/tails;
- Memories;
- Memory vectors;
- Graph relationships;
- Communities; and
- authoritative source chronology plus Chronos-derived temporal interpretation.

REL conversation records carry PHY identity. Ego can therefore derive recent activity for one user without moving conversation state into the PHY:

```text
shared REL conversation state
        ↓ filter by active PHY identity
that user's recent conversation tails
        ↓
recent-activity semantic centroids
        ↓
relevance over the shared REL Memory Web
```

Two users attached to the same large/team REL may legitimately activate different portions of the same Memory Web and therefore receive different Memory-Web Summaries.

That makes the **target summary perspective PHY-owned**, even though its project evidence is REL-owned. Existing REL cached-synthesis persistence is current substrate, not the intended semantic ownership of this user-relative result. Exact PHY persistence/keying by REL remains implementation design work.

A future shared project/team synopsis, if needed, should be a distinct artifact rather than an Ego summary.

## What the measurements ruled out

The deterministic baseline already established is:

- exclude archived Memories;
- exclude explicitly `historical` Memories;
- collapse duplicate copies already identified by Dream; and
- retain `future` Memories.

This cleanup is useful but does not solve scale.

### REL measurements

The frozen 14-day REL reduces to 924 eligible Memories, 146,738 title/body characters, or roughly 36.7k tokens.

An earlier cut of the 28-day REL reduced to 2,079 eligible Memories, 298,130 characters, or roughly 74.5k tokens. After later Insomnia draining, the same experiment fixture contains 2,398 eligible Memories, 342,921 characters, or roughly 85.7k tokens. The later figure is the current stress measurement.

Category/type filtering is not a safe primary scaling mechanism:

- in the earlier 28-day cut, `decision + fact + constraint` represented about 83% of filtered REL text;
- `project` represented about 91% of filtered REL text; and
- the dominant intersections were `decision × project`, `fact × project`, and `constraint × project`.

Those categories are the project state the summary is supposed to preserve, so dropping them to meet a budget would be arbitrary.

### PHY measurements

The 28-day PHY reduces to 194 eligible Memories, 27,194 characters, or roughly 6.8k tokens.

Its substance is similarly concentrated rather than safely discardable:

- `instruction + preference` represented about 88% of filtered text; and
- `process + communication` represented about 89%.

This measurement remains relevant to PHY-local Ego/user-state scheduling and Personality calibration. The project-facing Memory-Web Summary remains user-relative over REL evidence; the PHY mutation clock described below is an owner-specific freshness policy, not a claim that PHY and REL summaries share one evidence model.

## Rejected primary scaling approaches

### Whole-Web one-shot summarization

Rejected as the default strategy.

The problem is not merely context capacity. Even a Web that fits comfortably in context can encourage a model to form an early global impression, focus on salient clusters, and move toward completion before the long tail has been deliberately processed.

The Ellis audit reinforced that narrower inspect/explore cycles can discover coherent areas well, but an unconstrained exploratory run does not guarantee that every relevant Memory will be surfaced before completion.

### Retrieval-only audit

Rejected as the baseline coverage mechanism.

`memory_search` is valuable for following a thread outside the current batch, but the model must not control which baseline Memories ever become visible. Mandatory coverage is controller-owned.

### Full community-by-community summarization

Rejected as a semantic boundary.

Communities are useful derived routing/coverage structure, but a coherent summary area may cross Community boundaries. Measured Ellis examples showed this directly: the strict Eastwood core spanned four Communities while remaining one connected thread.

Communities and graph neighbourhoods may help order batches, derive overlap, and expand promising threads; they do not define candidate summary areas.

### Category/type exclusion

Rejected. Categories and types may help structure reasoning but are not safe primary relevance filters.

### Calendar recency

Rejected. A dormant project must not lose current state merely because wall-clock time passed.

### Global user activity recency

Rejected for project state. Activity in another REL must not age this REL.

## User-relative recent activity

REL-local activity remains the recency boundary, but the recent semantic signal should originate from **recent REL conversation tails**, not from recently created Memories.

Recent Memories are already a transformed/selected representation. Conversation tails more directly represent what the user is actually working on now.

Let `W` be the recent REL-local activity window used to choose conversation tails. `W` is measured in **incoming logical turns**, not wall-clock time.

REL activity has one durable monotonic clock: `rel_turn_count`. It advances exactly once when a previously unseen logical **user or agent turn** is successfully accepted into that REL. This clock is semantic ingest metadata, not a count reconstructed from physical storage:

- a distinct incoming user turn advances it once;
- a distinct incoming agent/assistant turn advances it once, including durable automated-task output;
- an idempotent retry/replay of an already-seen logical turn identity advances it zero times;
- two distinct turn identities with identical content still advance it twice;
- tool calls, tool results, reasoning steps, internal workers, and provider retries do not independently advance it unless they arrive as first-class user/agent turns;
- storage de-duplication, compaction, repacking, or record representation must not alter the count; and
- activity in another REL does not advance this REL.

This gives the REL a durable activity statistic as well as Ego's scheduling clock. A dormant REL cannot age because its turn count does not move.

The recent-tail window `W` and summary refresh cadence are related to the same activity clock but are **not the same policy**. `W` bounds the recent conversation-tail signal used for recency centroids. REL summary refresh instead uses percentage growth in the durable turn count until an absolute maximum interval is reached.

After a successful REL-backed summary generation, persist the represented `source_turn_count`. Steady-state refresh becomes due when:

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

A due refresh does **not** run Ego immediately. Crossing the threshold marks the current Episode and queues Ego work through that Episode. The processing coordinator must first finish all Insomnia work required through the marked Episode and all Dream work made necessary by that Insomnia output. Only after that prerequisite semantic work is terminal may the queued Ego generation begin. The marked Episode is a readiness boundary, not a requirement to chase a continuously moving activity head; later activity must not starve an already-due Ego refresh.

The first REL-backed summary does not use turn-percentage growth from zero. It becomes eligible when the REL contains **25 eligible durable Memories**. Once that bootstrap threshold is reached, the current Episode is marked and the first Ego generation is queued through the same prerequisite-settlement path.

The important separation is therefore:

- **recent relevance:** `W` recent incoming turns;
- **REL bootstrap:** 25 eligible durable Memories; and
- **REL steady-state freshness:** percentage turn-count growth capped by a maximum turn delta.

## Recency centroids and working-Web selection

For the active PHY identity:

1. select the most recent conversation tails inside the REL-local activity window;
2. embed those tails using the compatible semantic profile;
3. derive multiple recent-activity centroids rather than one global centroid;
4. score current REL Memory vectors against those centroids;
5. combine that semantic activity signal with hard eligibility and temporal/state keep lanes; and
6. admit the resulting Memories to the working Web.

Conceptually:

```text
REL conversation records
        ↓ PHY identity
recent conversation tails
        ↓
multiple recent-activity centroids
        ↓
semantic relevance over REL Memory vectors
        +
hard lifecycle/temporal rules
        ↓
working Memory Web
```

Multiple centroids are important for large projects/teams where several workstreams may be active simultaneously. Existing Community sub-centroid machinery demonstrates the value of representing multimodal semantic regions rather than compressing everything into one centroid, although the exact recency-centroid construction does not have to be identical.

The relevance score/threshold is deterministic derived routing state. It must not be treated as proof that excluded Memories are irrelevant. Excluded state remains cold and searchable.

Chronos should contribute temporal interpretation where useful, especially for explicit future/current/historical material. Source chronology and content-valid time remain distinct.

Memory-owner Freshness is an additional deterministic, graded *contextual relevance* input for working-Web selection: each REL Memory has a signed -100..+100 counter, lazy REL-turn decay, access/new-linkage reinforcement and graph-distance/Community-local propagation. Freshness is not an assertion of factual truth and does not wait for a Perception/Chronos audit. When the entire Web fits economically, include it; at larger scale retain mandatory broad audit and protected keep lanes. Ego consumes the derived score and remains usable without Perception. See [Memory Freshness](memory-staleness-plan.md) and its [implementation contract](generic-staleness-implementation-plan.md).

Exact centroid count, clustering method, activity weighting, similarity threshold, temporal weighting, and fallback behaviour when recent conversation evidence is sparse remain calibration work.

## Mandatory broad Memory-Web audit

The working Web is then processed by a reasoning audit in controlled batches.

The audit is **not a summarization pass**.

Its job is simpler:

> Find bodies of Memories that are coherent enough to assemble a plausible single-subject summary of state and recent history.

The audit discovers those bodies and their tentative boundaries. It does not rank generic importance, decide final summary prose, or infer new higher-order state.

The controller, not the model, owns the mandatory traversal:

```text
working Web
    ↓
mandatory batch
    ↓
audit reasoning
    ↙        ↘
local Web    memory_search across full REL Memory Web
    ↓
candidate/thread state
    ↓
next mandatory batch
```

Required properties:

- every working-Web Memory appears as **core** material in at least one mandatory batch;
- the model cannot finish the audit while mandatory core coverage remains incomplete;
- `memory_search` can explore anywhere in the REL Memory Web, including cold state outside the default working set;
- search results supplement the mandatory sweep but do not replace it;
- candidate/thread state survives across batch boundaries; and
- not every Memory must belong to a candidate area.

### Batch sizing

Batching is for **attention control and repeated engagement**, not primarily for fitting the context window.

The current starting hypothesis is relatively large batches:

- roughly 100–200 core Memories for Ellis-like density;
- adaptive sizing based on working-graph size, up to a calibrated hard maximum; and
- overlap/context in addition to the core batch.

For the Ellis working set of roughly 490 eligible Memories, a starting point around 125–150 new/core Memories per pass is more plausible than very small 40-Memory batches.

Exact sizes must be calibrated. Memory count alone is not enough; token density and graph shape may eventually participate.

Only the new/core Memories advance coverage. Repeated context does not.

### Overlap and graph context

Overlap should likely become graph-informed rather than simple positional repetition.

A batch may contain:

- mandatory new/core Memories;
- selected neighbours from the previous/current graph boundary;
- compact state for already-discovered candidate threads; and
- on-demand search results.

Community membership can help route or construct graph context, but does not constrain candidate membership.

## Stage 1: broad audit

For each controlled batch, the audit should:

- inspect the mandatory core material;
- recognize possible coherent state/work/process threads;
- use `memory_search` when the local batch suggests related state elsewhere;
- record or extend discovered thread/area state;
- leave sparse/incidental Memories unassigned when appropriate; and
- advance only after the batch has been processed.

The audit should not write summary prose. Its output is discovery state: potential subjects, evidence/member handles, boundary questions, and links to existing candidate threads.

A dedicated audit prompt plus purpose-built tools is the initial architecture. A `SKILL.md` is not required merely to make the procedure valid. It can be evaluated later as packaging/versioning or if measured adherence improves, but controller-enforced exposure is the actual hard mechanism.

## Stage 2: candidate selection

Candidate selection is separate from both broad audit and summarization.

It decides which discovered areas are coherent and useful enough to pass forward and resolves their source membership/boundaries.

A candidate area is conceptually:

```text
CandidateArea {
    area_id
    subject
    memory_ids: [MemoryId, ...]
}
```

Desired properties:

- exactly one coherent summary subject;
- explicit Memory membership;
- incidental keyword/entity overlap is insufficient;
- categories/types may vary inside one area;
- state progression, constraints, decisions, history, and future actions may all participate when they concern the same durable subject;
- the area should be independently summarizable;
- sparse state may remain unassigned; and
- overlap policy between candidate areas remains to be calibrated rather than assumed.

Candidate selection should favour **coherent shared state that can support a plausible single-subject summary of state and recent history**, not generic importance. Compression value remains useful as a downstream reason to summarize a coherent body, but it is not the auditor's semantic objective.

It still does **not** produce the summary.

## Stage 3: summary generation

Only selected candidates proceed to summary generation.

Each summarization pass receives the candidate's exact source Memories plus required provenance/context and produces a faithful lossy compression of already-existing Memory state.

A Summary may:

- merge redundancy;
- compact state progression;
- preserve current constraints/decisions/future state; and
- omit detail that remains retrievable from the source Web.

A Summary must not invent a new higher-order proposition.

If the output asserts something that is not already collectively stated by its sources, it has crossed into Observation territory and belongs to Perception rather than Ego summary.

Provenance from the summary back to its source Memory set must remain available.

## Stage 4: reconciliation, sizing, and persisted summary tiers

Candidate summaries then undergo bounded reconciliation to:

- remove redundant summary coverage;
- reconcile ordering/priority for current orientation;
- preserve distinct active threads; and
- produce one grounded rich summary representation suitable for controlled further compression.

Reconciliation must not become a hidden Observation pass. It composes already-source-grounded summaries.

The refresh pipeline then produces **three persisted Memory-Web Summary sizes** from that grounded representation. Each tier has a configured **model target** and a larger **controller hard ceiling**. Both are fixed by configuration/policy and are not calculated ad hoc from the context window at conversation start. The model is always instructed to target below the controller's maximum, leaving deliberate headroom for tokenizer differences and imperfect model length control.

Conceptually:

```text
candidate summaries
      ↓
reconciled grounded representation
      ↓
large stored summary
      ↓ controlled compression
medium stored summary
      ↓ controlled compression
small stored summary
```

The exact tier names and token budgets remain calibration/configuration work, but all three are materialized and stored during the same summary refresh. Runtime context assembly only selects the largest already-stored tier that fits the available Ego allocation. It does **not** rerun audit, candidate selection, summarization, or compression when a conversation starts.

Budget compliance is controller-owned. A model is not expected to reliably hit an arbitrary exact output size, and token counts may vary across tokenizers. The model therefore receives a target that is deliberately below the tier's controller ceiling. The controller measures the produced artifact and only requires that it remain at or below that ceiling; if it exceeds the ceiling, another bounded compression pass may run. Outputs below the model target are acceptable and are never padded merely to consume budget.

Smaller tiers are derived from the richer grounded summary state rather than independently re-auditing or independently summarizing the Memory Web, so the stored projections remain semantically aligned and share provenance.

## Refresh policy

**Summary regeneration** uses owner-specific source-progress clock math and a separate Ego scheduling/completion policy: REL project summaries follow accepted REL turns, while PHY-local summaries follow the PHY Memory mutation version. The implemented typed source evaluator may be reused here, but a cross-target durable review store is not yet a requirement or a substitute for Ego's accepted-source-cut and Episode-readiness rules. Summary regeneration is independent of per-Memory deterministic Freshness and processing cooldown epochs. See [Unified Freshness and retained source-progress infrastructure](generic-staleness-implementation-plan.md) and [Memory Freshness](memory-staleness-plan.md).

### REL-backed project summary

The REL is usage-sensitive, so refresh cadence is based on the durable incoming-turn clock rather than semantic mutation density.

Bootstrap:

```text
if no REL-backed summary
and eligible_REL_memory_count >= 25:
    queue first Ego generation
```

Steady state:

```text
required_turn_delta =
    min(
        ceil(source_turn_count * rel_refresh_fraction),
        rel_max_refresh_turn_delta
    )

if current_rel_turn_count - source_turn_count >= required_turn_delta:
    queue Ego refresh
```

This means young RELs refresh after proportionally small amounts of new activity, while mature RELs eventually settle at the configured maximum turn interval. There is deliberately no wall-clock age backstop.

### PHY-local summary

PHY freshness follows semantic mutation rather than REL activity. The PHY should become eligible immediately when it first has semantic state worth summarizing; there is no 25-Memory bootstrap gate.

Use the **Memory version/mutation index** as the primary PHY refresh clock, not raw Graph version. Graph work is derived and may fan one meaningful Memory mutation into several Graph mutations, so Graph churn must not independently make the PHY summary stale.

Conceptually:

```text
if no PHY summary
and first eligible PHY Memory mutation exists:
    queue first Ego generation

required_memory_delta =
    min(
        ceil(source_memory_version * phy_refresh_fraction),
        phy_max_refresh_memory_delta
    )

if current_phy_memory_version - source_memory_version >= required_memory_delta:
    queue PHY Ego refresh
```

This intentionally permits several cheap summary cycles near the beginning of a new PHY, then widens the interval as the PHY matures until the absolute mutation cap takes over.

### Queued execution

For either owner, becoming due means **queueing**, not immediate execution. The queued generation runs only after the semantic processing required by its source boundary has settled. For REL-backed work the concrete readiness path is:

```text
REL bootstrap or turn-growth threshold crossed
        |
mark current Episode E
        |
queue Ego refresh through E
        |
Insomnia terminal through E
        |
Dream terminal for all work required by Insomnia through E
        |
run Ego once against the now-settled source state
        |
record actual source cuts + successful source turn count
```

The marked Episode is the minimum prerequisite boundary. Activity and processing after that Episode do not indefinitely postpone the queued run. Duplicate due signals while a refresh is already pending must not create parallel or repeated Ego generations; they coalesce into the pending work according to scheduler policy.

Memory, Graph, Entity, and global/container versions remain useful source-state watermarks and diagnostics for REL-backed generations even though REL scheduling is activity-driven. PHY scheduling is the deliberate exception: its Memory version is itself the freshness clock.

## Stress fixture

The current 28-day experiment remains a harsh activity/scaling calibration case:

`../reliquary-fixtures/local/authoritative/chatgpt-first28d/project.prj.rel`

The current recovered 28-day Entity-v9 fixture is at Memory version **5,665**, Graph version **25,136**, Entity version **671**, and global/container version **52,407**. Its full-Dream pre-Perception baseline was Memory **5,665**, Graph **23,227**, and global/container **49,827**. Entity processing therefore advanced Graph by **1,909** and the global clock by **2,580** without changing the Memory version at all. The authoritative Ellis pre-Relationship fixture is at Memory version **1,358**, Graph version **6,007**, Entity version **199**, and global/container version **11,328**. These measurements remain useful for understanding source-state scale, but refresh cadence is now intentionally tied to REL-local user activity rather than any one semantic/storage mutation clock. Bulk import/backfill can still create large processing runs; queueing Ego behind the marked Episode's required Insomnia and Dream settlement prevents those runs from exposing a prematurely summarized Web.

The REL spans 29 Pacific calendar dates from 2026-05-15 through 2026-06-12 and contains 16,465 unique ingested conversation turns. Those unique logical turns match the activity-clock semantics: idempotent replays are not additional activity, while distinct turn identities count independently even if content is duplicated. Using a 30-minute gap only as a diagnostic estimate of engaged conversation time, 23 of 29 dates exceed six engaged hours and 17 exceed eight hours; elapsed engaged-time estimates are not part of the scheduler clock.

The newest three source dates currently have no extracted eligible REL Memories, reinforcing why the recent activity signal should originate from conversation state rather than Memory extraction density.

Cumulative eligible Memory input ending on the latest Memory-bearing day, 2026-06-09, is:

| Recent active dates | Memories | Rough tokens |
| ---: | ---: | ---: |
| 1 | 171 | 5.7k |
| 3 | 487 | 16.3k |
| 5 | 799 | 26.1k |
| 7 | 1,080 | 35.7k |
| 10 | 1,337 | 44.9k |
| 14 | 1,627 | 54.0k |
| 21 | 2,291 | 82.5k |
| 26 | 2,398 | 85.7k |

A fixed “last 30 active days” rule is still too coarse for a heavily used REL. Recent-tail centroids plus semantic routing should allow activity recency to activate old-but-currently-relevant durable state without rereading the entire REL on every refresh.

## Ellis audit evidence

The manual Ellis audit provides useful design evidence but was not an exhaustive controlled run.

The audit operated over a 490-Memory eligible REL set through external search/filter/inspection rather than loading the entire Web into one prompt. It discovered coherent areas such as Eastwood financial-literacy work, publishing, garden state, Dell XPS reliability, household finance, meal planning, and recovery threads.

That exploration pattern worked well; the failure was coverage. Nothing forced the run to continue until all 490 eligible Memories had been surfaced.

Graph measurement on two strict lexical cores also showed:

- Eastwood: 34 Memories, four Communities, 82.4% in one dominant Community, one induced connected component, pairwise full-graph median two hops and maximum four;
- Dell XPS 13: three Memories, one Community, all pairwise one hop.

This supports using graph/Community structure for routing and overlap while allowing candidate areas to cross Community boundaries.

The target batched audit is intended to preserve the successful inspect/search/follow-thread behaviour while adding controller-enforced coverage.

## Personality boundary

Personality remains:

- PHY-owned and persisted only in the PHY;
- synthesized only from PHY-owned Memory state;
- never derived from REL/project state;
- lower inertia than Identity;
- user-editable;
- optionally self-adaptive; and
- not semantic authority over factual Memory state.

Personality should be a behavioural projection of the user, not a second generic user summary. Intended evidence lanes remain communication/process/relationship preferences and recurring behavioural patterns.

Generic user facts are not Personality evidence merely because they live in PHY. Identity-like facts, education, employment history, possessions, location, and other factual biography remain ordinary PHY Memory state unless they directly encode behavioural preference.

Still undecided:

- exact behavioural evidence-selection rules and category/type mapping;
- Personality activity/window and resynthesis policy;
- treatment of user-authored Personality edits during later automatic synthesis; and
- output shape and budget.

Do not implement Personality inference by simply reusing the project Memory-Web Summary pipeline.

## Implementation architecture and order

Implement the planned architecture directly in dependency order. Do not build a temporary single-batch path, bypass final ownership, or preserve provisional seams that will later require a cutover. Unimplemented downstream behaviour should fail explicitly rather than being bypassed.

The target owner is one deep orchestration module:

```text
EgoWebSummaryEngine
    |
    +-- AuditPopulationBuilder
    |    +-- complete Memory Web
    |    +-- controller-side size measurement
    |    +-- ordered pruning escalation after overflow
    |    +-- batching only as the final escalation
    |
    +-- AuditController
    |    +-- mandatory coverage
    |    +-- audit-batch progression
    |    +-- full-Web memory_search
    |    +-- persistent discovered-body state for the run
    |
    +-- CandidateResolver
    |    +-- explicit CandidateArea membership
    |
    +-- CandidateSummarizer
    |    +-- exact-source, provenance-carrying candidate digests
    |
    +-- SummaryReconciler
    |    +-- one rich grounded project-summary representation
    |
    +-- SummaryTierBuilder
    |    +-- large model target / controller ceiling
    |    +-- medium model target / controller ceiling
    |    +-- small model target / controller ceiling
    |
    +-- EgoSummaryStore
         +-- PHY-owned summary bundle keyed by REL
```

### Phase 1 - final contracts and ownership

Define the permanent types and interfaces first. The exact field spelling remains implementation work, but the architecture should expose equivalents of:

```text
EgoSummaryPolicy
    max_single_audit_tokens
    pruning_policy
    audit_batch_limit
    recent_activity_window_turns
    rel_bootstrap_eligible_memories = 25
    rel_refresh_fraction
    rel_max_refresh_turn_delta
    phy_refresh_fraction
    phy_max_refresh_memory_delta
    large: SummaryTierBudget
    medium: SummaryTierBudget
    small: SummaryTierBudget

SummaryTierBudget
    model_target_tokens
    hard_ceiling_tokens

CandidateArea
    id
    subject
    memory_ids

CandidateDigest
    candidate_id
    text
    memory_ids

EgoWebSummaryBundle
    rel_owner_id
    source_turn_count
    source_memory_version
    source_graph_version
    generation
    policy_version
    large
    medium
    small

EgoRefreshWatermark
    RelTurnCount(u64)
    PhyMemoryVersion(u64)

QueuedEgoRefresh
    rel_owner_id: optional for PHY-local work
    phy_owner_id
    through_episode_id: optional when no REL Episode boundary applies
    due_watermark: EgoRefreshWatermark
```

The existing single-string `EgoWebSynthesis` is not the target storage shape. Change the model before building inference machinery around it.

The target project Memory-Web Summary is PHY-owned and keyed by stable REL owner identity. Do not extend REL-owned synthesis persistence as a temporary architecture and migrate it later.

Candidate and summary provenance remains mandatory while a generation is running: candidate membership and digest source Memory IDs must remain available through reconciliation so every produced summary is grounded in the selected evidence. Those intermediate provenance records are run-local and are discarded after the final bundle is successfully produced. The persisted REL-backed bundle retains its scheduling/source identity through the successful `source_turn_count` plus actual Memory/Graph source cuts; it does not persist candidate membership or per-summary Memory provenance. PHY-local summary persistence should analogously retain the successful PHY Memory-version watermark used by its mutation-driven scheduler.

### Phase 2 - audit population construction

Implement the two audit-stage invariants directly:

1. no scale-driven pruning or batching while the complete Web fits the configured single-audit maximum; and
2. give the model as much of the Web as economically practical, taking information away only when scale requires it.

The population builder therefore behaves conceptually as:

```text
complete Web
    |
    +-- fits single-audit maximum? --> one untouched batch
    |
    +-- overflow
          |
          +-- ordered deterministic pruning
                  |
                  +-- fits? --> one batch
                  |
                  +-- continue acceptable pruning
                          |
                          +-- still too large? --> batching
```

Batching is the terminal scaling mechanism, not a competing early strategy.

The exact pruning order still requires calibration/design, but Archived, Historical, Superseded, and Duplicate state are the initial explicit lanes under consideration.

The population builder should return an inspectable audit plan rather than only a vector of Memories. It should record at minimum the source Memory watermark, complete population size, admitted IDs, exclusions with reasons, and final batches.

### Phase 3 - audit controller

Implement the real audit controller against the audit plan rather than a special one-batch implementation.

The auditor's semantic job is:

> Find bodies of Memories that are coherent enough to assemble a plausible single-subject summary of state and recent history.

The controller, not the model, owns:

- mandatory core coverage;
- batch progression;
- run-local discovered-body/thread state;
- access to full-Web `memory_search`; and
- completion eligibility.

A Web that fits in one batch naturally completes through one audit call. Larger Webs use the same controller and contracts across multiple batches.

Pruning removes material from mandatory/default exposure only. It does not make the full REL Web unavailable to `memory_search`.

### Phase 4 - candidate resolution

Candidate resolution is a distinct stage between audit discovery and summarization.

Input consists of discovered bodies plus their exact referenced Memories and any required audit boundary state. Output is zero or more explicit `CandidateArea` records.

This stage may:

- resolve exact membership;
- merge duplicate candidate bodies;
- split bodies that actually contain multiple subjects;
- reject bodies that are not coherent enough to summarize as one subject; and
- resolve overlap according to the eventual overlap policy.

It still produces no summary prose.

### Phase 5 - candidate summaries

Hydrate each accepted candidate from its exact Memory IDs and summarize that closed evidence set.

The contract is to produce a plausible single-subject summary of state and recent history already represented by those Memories.

Each digest retains its source Memory IDs/provenance. Candidate summarization compresses existing propositions; it must not create a new higher-order proposition. New inference remains Perception/Observation territory.

### Phase 6 - reconciliation

Reconcile candidate digests into one rich grounded project-summary representation.

This stage should:

- eliminate redundant candidate coverage;
- preserve distinct active subjects;
- reconcile current state with useful recent history;
- retain source provenance; and
- avoid imposing the final small/medium/large token budgets yet.

The result is the stable rich representation from which stored tiers are derived.

### Phase 7 - three persisted summary tiers

Build all three summary tiers during the same refresh generation.

Each tier has:

```text
model_target_tokens < controller_hard_ceiling_tokens
```

The model aims below the ceiling. The controller counts the actual result with the configured token counter.

- if actual size is at or below the ceiling, accept it;
- if it exceeds the ceiling, run another bounded semantic compression pass;
- never truncate merely to satisfy the budget; and
- never pad an undersized grounded summary merely to consume tokens.

Large, Medium, and Small belong to one coherent generation so runtime cannot mix tiers from different source states.

### Phase 8 - persistence

Persist the complete tier bundle in the PHY keyed by REL identity.

The persisted generation should include at least:

- REL owner identity;
- successful source REL incoming-turn count;
- actual source Memory and Graph version cuts used by the generation;
- summary generation;
- policy/contract version;
- all three summary texts; and
- measured token sizes.

Audit, candidate, and source-Memory provenance state remains run-local and is discarded after a successful bundle commit unless interrupted-build recovery later demonstrates a need to persist it.

### Phase 9 - runtime tier selection

Conversation startup performs no audit, candidate selection, summarization, or compression.

Runtime context assembly should only:

```text
available Ego allocation
      |
load current stored bundle
      |
large fits? ------> large
      |
      no
      |
medium fits? -----> medium
      |
      no
      |
small ------------> small
```

If no stored tier is valid/current enough for policy, handle that state explicitly rather than silently launching summary generation in the conversation-start path.

### Phase 10 - refresh scheduling

Only after the summary engine and persistence path work end-to-end should automatic refresh be attached.

The scheduler owns:

- reading the **Archive-owned** durable `rel_turn_count` and historical REL activity cuts, as provided by the separate [REL activity clock Phase 1 implementation plan](rel-activity-clock-implementation-plan.md);
- REL first-summary eligibility at 25 eligible durable Memories;
- REL steady-state due calculation by the generic source-progress staleness evaluator over percentage turn-count growth capped by `rel_max_refresh_turn_delta`;
- PHY first-summary eligibility on the first eligible PHY Memory mutation;
- PHY steady-state due calculation from percentage Memory-version growth capped by `phy_max_refresh_memory_delta`;
- marking the current Episode/source boundary when Ego becomes due;
- one pending Ego refresh per applicable owner/perspective; and
- handing pending refresh work to the processing coordinator rather than invoking Ego immediately.

A queued REL refresh is not runnable until Insomnia is terminal through its marked Episode and Dream is terminal for every piece of work required by that Insomnia output. The coordinator owns that readiness check. Once the marked boundary is settled, Ego may run even if newer Episodes exist; continuous activity must not starve a due summary. PHY-local queued work must likewise wait for the semantic processing required by its triggering mutation watermark rather than running against partially processed state.

The scheduler's semantic responsibility is only to decide **when Ego becomes due** and establish the prerequisite source boundary. The processing coordinator decides when that queued work is safe to execute. `EgoWebSummaryEngine` continues to own summary semantics.

### Implementation rule

> Build every seam in its final ownership location. Unimplemented downstream behaviour fails explicitly; nothing is temporarily bypassed.

This keeps incremental implementation testable without introducing architecture that is already known to be temporary.

## Open calibration

Still unresolved:

- exact recent-turn window `W` used for conversation-tail selection;
- REL steady-state `rel_refresh_fraction`;
- REL absolute `rel_max_refresh_turn_delta`;
- PHY steady-state `phy_refresh_fraction`;
- PHY absolute `phy_max_refresh_memory_delta`;
- exact pending-refresh coalescing policy while the marked Episode/source boundary is waiting for prerequisite settlement;
- recent conversation-tail length/selection;
- number/construction of recent-activity centroids;
- semantic relevance threshold/weighting and any temporal weighting;
- exact ordered pruning policy after single-batch overflow;
- single-audit maximum and final batch-size function;
- graph-derived overlap/context budget;
- candidate-area overlap policy;
- candidate-selection threshold for coherent summarizability;
- the fixed model targets and controller hard ceilings for the three persisted summary tiers;
- initial/bootstrap behaviour when recent user-specific conversation evidence is sparse; and
- exact PHY persistence structure for REL-keyed project summary bundles.

## Related docs

- [Roadmap](roadmap.md)
- [Current limitations](current-limitations.md)
- [Architecture](architecture.md)
- [Ego Cross-chat context plan](ego-cross-chat-context-plan.md)
- [Ego cross-REL summary inclusion plan](ego-cross-rel-summary-inclusion-plan.md)
- [Memory scope design record](reliquary-phylactery-memory-scope-plan.md)
- [Dream design and validation record](dream-implementation-plan.md)
- [Chronos subsystem plan](chronos-subsystem-plan.md)
- [Perception subsystem plan](perception-subsystem-plan.md)
- [Activity staleness and Memory revalidation](memory-staleness-plan.md)

## Notes

When this pipeline ships, move implemented behaviour into current-state architecture/API/storage owners and retain only unresolved calibration/follow-up work here.
