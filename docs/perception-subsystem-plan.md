# Perception subsystem plan

Parent index: [Documentation index](INDEX.md)

Decision owners: [ADR 0033](decisions/0033-perception-entities-observations-and-ambiguity.md) and [ADR 0034](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md)

Shared temporal dependency: [ADR 0035 — Chronos](decisions/0035-chronos-shared-temporal-semantics.md)

## Status

Accepted architecture; implementation is in progress. Insomnia Entity-mention enrichment and Perception Milestone B's Entity owner, typed Graph association, bounded candidate retrieval, zero-candidate Admission, calibrated V4 identity resolution, deterministic persistence, per-mention resolution state, and long-lived post-Dream runtime scheduling are implemented. The full 41-case frozen zero-Entity bootstrap converges from no preconstructed Entities and survives reopen. Observation synthesis remains planned; ADR 0034 now defines Relationships as a relational Observation specialization. The existing Relationship persistence foundation is transitional storage to converge with the Observation owner before synthesis ships.

This document owns the implementation shape for Perception. ADR 0033 owns the architectural decision and rationale. Current shipped/proven behavior is also reflected in the architecture, API, manual, and limitation docs.

## Overview

Perception is the planned post-Dream semantic layer for durable Entities and synthesized Observations. Relational knowledge is represented as a structured Observation specialization over owner-qualified Entity participants rather than as a third semantic object family. Perception uses bounded deterministic routing and inference rather than full-corpus reasoning.

## Purpose

Perception materializes semantic structure implicit in the organized Memory Web that is neither a directly remembered proposition nor an ordinary Dream relationship.

Initial semantic objects:

- **Entities** — durable canonical referents whose metadata and relationships may change without changing identity.
- **Observations** — contingent higher-order propositions synthesized from existing semantic evidence.

A **relational Observation** is an Observation whose proposition describes a durable connection among two or more Entities. It may carry structured owner-qualified participants, roles, and relation classification for routing/inspection/privacy, but it uses the same support, lifecycle, ambiguity, temporal, and reconsideration semantics as every other Observation.

Perception is not another full-corpus reasoning layer. Deterministic routing, Dream structure, mutation state, and wall time bound its inference.

## Pipeline

```text
Archive/source
    -> Insomnia semantic extraction + final Memory wording
    -> Insomnia Entity-mention enrichment
       -> Chronos Memory-level temporal assessment/inference when indicated
    -> durable Memory publication
    -> Dream Memory-Web organization (consumes Chronos)
    -> Perception 1: Entity synthesis/association/disambiguation
    -> Perception 2: existing Observation contribution
       -> includes relational Observations
    -> Perception 3: new Observation extrapolation
       -> immediate Exploitation around the incoming Memory
       -> delayed cooldown-epoch Exploration outside established locality
       -> includes sparse relational-Observation synthesis
       -> Chronos higher-order temporal analysis when indicated
    -> Perception 4: Observation receptor generation
```

Observation reconsideration and ambiguity clarification are lifecycle/runtime mechanisms around these four routine passes.

## Insomnia enrichment prerequisite

After final authoritative Memory wording, Insomnia runs a separate metadata pass that extracts exact Entity mentions from title/content, persisted as field + UTF-8 byte span + verbatim text. A mention must be an **identity-bearing reusable referent**: its surface form must carry enough identity to be recognized again in another Memory without reconstructing the sentence that produced it. Bare/context-only references such as `the server`, `the file`, `the agent`, or `the repository` are excluded. Descriptive action/architecture phrases ending in `flow`, `lane`, `mechanism`, `request`, `response`, `seam`, `state`, or `status` are also screened out unless identity is carried by a code/API artifact rather than the prose phrase itself. Stable owner-local descriptive identities such as `write server`, `devtools window`, `the Vancouver office`, or `my brother` remain eligible when the modifier/relation consistently identifies the referent.

The implemented record is `MemoryRoutingMetadata`, bound to exact `MemoryId + MemoryBodyId`. Entity mentions are capped at 64 items and each emitted text value at 512 UTF-8 bytes. Write/reopen validation requires every mention span to match the immutable Memory body exactly. The metadata is clock-neutral and remains valid across metadata/lifecycle revisions because the Memory body cannot mutate in place.

Lexical locality is deliberately **not** model-extracted metadata. REL and PHY derive a disposable owner-local Memory lexical index directly from full current Memory title/content using the same deterministic tokenizer, inverted index, and coverage+density score as Archive lexical search. It rebuilds lazily from `memory_version`, persists no semantic state, and is the lexical routing/context-construction primitive Perception should use.

It does **not** resolve/create Entities, decide identity, assign Entity IDs/types, synthesize Observations, or create semantic authority beyond the Memory itself. Perception pass 1 remains the sole owner of Entity synthesis/association/disambiguation.

## Chronos temporal dependency

Perception does not own a separate temporal parser or temporal-inference stack. It consumes the shared Chronos subsystem defined by ADR 0035 after Perception has selected the bounded evidence relevant to an Observation synthesis or reconsideration.

Chronos may compare supporting Memory intervals/patterns, derive Observation valid-time semantics deterministically, expose temporal conflict/ambiguity, or invoke bounded temporal inference when deterministic interpretation is insufficient. Perception remains responsible for deciding what Observation proposition exists.

Deterministic Chronos products remain derived and need not be persisted. Any non-deterministically inferred temporal conclusion that becomes part of durable Perception state must be tied to the semantic inputs/version that justified it.

## Pass 1 — Entity synthesis, association, disambiguation

Perception consumes Insomnia-extracted Entity mentions against the post-Dream Memory Web.

Current Entity flow:

1. Start from one exact Insomnia-extracted mention keyed by Memory ID, field, and UTF-8 byte range.
2. Build a bounded owner-local candidate set from exact canonical-name/alias matches, any existing source-Memory association, Entities attached to lexically local Memories, and Entities attached to first-hop Dream Memory neighbours. These signals generate candidates; they never prove identity.
3. With **zero candidates**, run **Entity Admission v2**. Admission receives bounded same-surface owner-local Memory context, uses a narrow deterministic guard for known bare-generic surfaces, and decides whether the current mention should `create_new`, remain `unresolved`, or be `reject`ed. On creation it also materializes stable Entity kind/summary metadata. Context may inform metadata only when clearly compatible with the same referent; same-surface evidence that could describe a second identity must not be merged.
4. With **one or more candidates**, run the calibrated **V4 identity resolver** against the current Memory plus each candidate's Entity metadata and bounded evidence, prioritizing Memories already associated with that Entity before discovery evidence. V4 decides `resolve_existing`, `create_new` for a distinct identity, `unresolved`, or `reject`.
5. Deterministically persist a successful result as Entity state where needed, a `Memory -> Entity` `EntityAssociation`, and terminal per-mention resolution state. V4 `create_new` uses a separate metadata-only materialization call so the calibrated V4 contract remains unchanged.
6. Persist unresolved/rejected state under the existing lifecycle policy. Candidate/context fingerprints make unresolved reconsideration evidence-driven rather than repeated unchanged inference.

No Entity vector/centroid lane is required by the current implementation. That remains a measurement-driven future routing option.

Entities are durable referents. Sarah remains the same Entity through employer, role, location, relevance, and conversational changes.

Entity metadata/relationships are therefore mutable independently of Entity existence. The design should support multiple semantic centroids or equivalent multi-context representation where one referent spans materially different contexts.

## Relational Observation specialization — sparse cross-owner relational state

After Entity resolution, Observation synthesis may create or update relational Observations under ADR 0034.

A relational Observation is not created for every Entity or every possible Entity pair. Candidate discovery is bounded to affected resolved Entities and evidence. Explicit durable relational facts may justify immediate synthesis; otherwise repeated or convergent evidence may be required before a useful higher-order relational proposition exists.

Relational Observation participants are owner-qualified Entity references and may span currently mounted REL/PHY Memory Webs. The Observation itself remains owned by one REL or PHY. Visibility follows that Observation owner, not the visibility of participant Entities.

The runtime privacy rule is strict: only the active PHY contributes private relational Observations; authorized active RELs contribute their shared relational Observations. Non-active PHY state is never traversed as a bridge through shared Entities.

Participant roles, relation classification, and compact presentation state are structured Observation metadata. They do not create a nested semantic owner, separate lifecycle, separate provenance model, independent Dream graph, Communities, or recursive Perception scheduler.

This specialization does not alter Dream's same-owner Memory-to-Memory relationship authority.

## Pass 2 — Existing Observation contribution

This pass determines whether a new Memory materially bears on an existing Observation.

The semantic judgment is strictly pairwise:

```text
Memory M <-> Observation O
```

Possible results include support, challenge/contradiction, weakening, qualification, ambiguity, or irrelevance. This pass never synthesizes a new Observation.

### Candidate routing

Exhaustive `new Memory x all Observations` inference is forbidden because cost grows linearly with Observation population.

Existing Observations expose separately generated **routing receptors**: prospective-evidence descriptions of future facts that could materially bear on the Observation.

Each receptor is embedded independently and indexed back to its Observation. A new Memory queries that index for a bounded candidate set.

Exact deterministic routes may supplement receptor search, especially:

- shared Entity IDs;
- direct dependency/support links after an existing support object mutates.

Candidate Observation IDs are deduplicated before pairwise inference.

Routing receptors are routing metadata only. They are never evidentiary authority.

## Pass 3 — New Observation extrapolation

This pass asks whether the Memory Web around an eligible Memory now supports a higher-order proposition that no individual Memory explicitly represents.

It runs independently of pass 2: one Memory may support an existing Observation and also help establish another. New Memory publication is the ordinary discovery trigger; lower-level Graph, Entity, Community, and Observation mutations do not recursively trigger new-Observation synthesis. Those changes alter the Web that a later eligible pass sees.

Pass 3 has two scheduling lanes:

- **Exploitation** runs immediately after a new Memory has completed Dream and Entity resolution, then becomes eligible again on the same provenance-anchored cooldown-epoch cadence used by Dream maintenance.
- **Exploration** does not run immediately. A new Memory first becomes eligible for Exploration at its next processing/cooldown epoch, then remains periodically eligible on the same cadence.

The two lanes share the epoch model but maintain separate Perception processing history. A successful pass satisfies only the current epoch; missed epochs do not accumulate as backlog.

Exploitation re-runs because the Memory body is immutable while its useful surrounding Web is not. Later Memories, Dream structure, Entity resolution, and Community evolution may make an Observation inferable that was not inferable on the Memory's first pass.

Exploration deliberately probes outside the strongest existing organization so that Community boundaries and current Graph topology cannot permanently hide cross-domain or weakly connected structure. It must not use Dream's permanent settled-pair ledger. Instead it keeps only bounded recent Exploration history per Memory/run so recently repeated comparison contexts can be deprioritized and then fall away naturally.

### Bootstrap and exploitation context construction

Community hierarchy is a scaling mechanism, not a bootstrap prerequisite.

For an eligible anchor Memory:

1. if the currently relevant owner-local Memory Web fits the synthesis context budget, Exploitation uses the whole Web;
2. otherwise select the smallest genuine containing Community that fits;
3. if the containing Community is too large, descend through genuine subcommunities containing the anchor until one fits;
4. if the deepest genuine containing Community is still too large, build an ephemeral bounded neighbourhood around the anchor.

No unrelated Memories are added merely to fill unused context budget. Strong boundary evidence may still be reached through bounded traversal during reasoning.

The practical scaling path is therefore:

```text
whole relevant Web
    -> containing Community
    -> containing subcommunity
    -> ephemeral bounded neighbourhood
```

### Exploration context construction

Exploration builds bounded cross-Web comparison contexts using signals deliberately different from ordinary locality. Randomness is an explicit routing signal, alongside future diversity/novelty signals that prove useful.

A cheap Decision-style probe may judge whether an exploratory Memory combination is meaningfully connected enough to justify expansion. The probe is routing metadata only: its confidence does not establish an Observation or semantic relationship. Low-value probes are discarded for that run; promising probes expand context around their endpoints and enter the normal synthesis/evidence pipeline.

Exploration records bounded recent run/context history, not permanent unordered-pair completion. The same Memories may therefore be reconsidered later under different surrounding evidence without causing the same comparison groups to dominate every epoch.

Exact exploration sampling weights, probe model/threshold, and recent-history depth are calibration parameters rather than semantic authority.

### Multi-resolution Community hierarchy

The current flat Community layer is insufficient as an inference boundary for large RELs/PHYs.

Communities are **derived semantic structure**. They are inferred from the organization of the semantic Graph and therefore carry meaningful information about how Memories cohere, but they do not create, override, or replace Memory, Entity, or Observation authority.

Community processing should expose multiple useful resolutions:

1. compute the normal owner-local Leiden partition;
2. recursively run Leiden inside a Community while genuine subcommunities exist;
3. build a Community meta-graph from cross-Community semantic relationships and derive coarser super-communities where genuine higher-level structure exists;
4. stop subdivision or aggregation when another level does not add meaningful structure.

Depth is dynamic in both directions. Leiden must not be forced to invent a split or grouping solely to satisfy an inference or presentation budget.

The hierarchy must be derived from persisted Graph/Community state, not from the temporary shard/reduction tree used by scan-and-merge. Execution topology is not semantic hierarchy.

Persisted sub-/super-Community structure may be named, assigned lineage, traversed, surfaced in the Knowledge Interface, and used for coarse -> normal -> fine retrieval or Perception routing. It remains descriptive organization: a Community-level node is not itself a proposition and cannot become an independent relationship authority.

### Oversized irreducible leaves

If the deepest genuine Community is still too large, Perception builds an ephemeral bounded processing neighbourhood around the new/changed Memory instead of persisting fake child Communities.

Neighbourhood construction may use bounded combinations of:

- strongest graph relationships;
- graph distance/locality;
- shared Entities;
- lexical locality from the deterministic owner-local Memory lexical index;
- semantic similarity as a backstop.

The neighbourhood is a processing window only, with no Community identity or semantic authority.

The same bounded-neighbourhood mechanism supports Exploitation when genuine Community structure remains too large. Existing-Observation contribution remains the separate receptor-routed pairwise pass.

### Bounded traversal during formation

The initially selected context is a starting region, not an assertion that all relevant evidence is already present.

During candidate formation and evidence gathering, the reasoner may traverse the authorized Memory Web within an explicit search/context budget. Traversal may follow Graph neighbours, Community parent/child boundaries, shared Entities, deterministic lexical routes, semantic-search backstops, or promising Exploration bridges. Traversal may cross the initial Community when the evidence justifies it.

Only semantic objects actually used to establish or challenge the proposition become support/derivation inputs. Temporary traversal context has no authority of its own.

### Synthesis and verification contract

New-Observation formation is staged rather than one monolithic inference call:

1. **proposal** — inspect the bounded starting context and propose zero or more higher-order candidate propositions;
2. **evidence expansion** — for each candidate, deliberately gather support, contradiction, qualification, and alternative evidence through bounded traversal;
3. **reasoning/derivation** — apply the appropriate reasoning mode to determine what the gathered evidence actually warrants;
4. **verification** — independently judge the candidate against its claimed derivation and exact evidence;
5. **publication** — persist only candidates that pass verification, with exact lineage.

Reasoning modes may include synthesis/abstraction, deduction, induction, and bounded hypothesis/abduction. Their exact algorithms and acceptance criteria are separate calibration work; a generic confidence score must not substitute for reasoning-specific validation.

A candidate must add useful semantic structure beyond a raw restatement of supplied evidence. Ordinary Observations should be abstractions, patterns, implications, or other higher-order propositions rather than paraphrases of one Memory. A relational Observation may be supported by one explicit durable relational Memory when canonical Entity participants/roles supply the additional structured semantics; the source Memory remains the grounding authority.

Evidence is many-to-many. One Memory may materially contribute to multiple Observations, and one Observation may depend on multiple Memories or other authorized semantic objects. Contribution is typed per Observation/evidence relation; reuse of one Memory across Observations does not create independent corroboration within any single Observation.

Persisted Observations retain exact support/derivation lineage. Independent support paths remain distinguishable rather than collapsing to a count.

## Pass 4 — Observation routing-receptor generation

Every new Observation, and any materially rewritten Observation, receives a separate receptor-generation pass.

The pass generates prospective future facts that could materially:

- support it;
- challenge/contradict it;
- qualify it;
- indicate that its current interpretation warrants reconsideration.

Each receptor is embedded independently and mapped back to the Observation. Receptors must not be collapsed into one centroid because relevant evidence can occupy very different semantic regions.

Generation favors recall over precision: a false positive costs one bounded pairwise comparison; a false negative may hide relevant evidence indefinitely.

## Observation lifecycle

Observations are retained historical semantic objects and should normally be archived rather than deleted.

### Mutation-driven reconsideration

Relevant mutations accumulate against an Observation. Crossing the configured threshold triggers semantic reconsideration.

Mutation only establishes that inference is warranted. Reconsideration may keep the Observation active, qualify/supersede it, expose contradiction/ambiguity, or mark it stale.

### Wall-time reconsideration

Observations also receive wall-time checkpoints for propositions that may silently become outdated.

At a checkpoint, deterministic machinery asks whether any relevant mutation occurred since the last semantic reconsideration:

- **none** -> mark stale and remove from priority consideration without inference;
- **one or more** -> run semantic reconsideration.

A mutation does not imply freshness. Old supporting evidence may still lead reconsideration to mark an Observation stale.

`stale` means non-priority/currentness-uncertain, not false. Wall time alone never invalidates or deletes an Observation; contradiction/supersession requires evidence.

## Entity and Observation ambiguity

Perception may persist unresolved Entity or Observation ambiguity instead of forcing an unsafe decision.

When a later user turn deterministically intersects one, the runtime injects an instruction-bearing Perception addendum that:

- identifies the ambiguity;
- prohibits assuming the unresolved identity/interpretation;
- requires the agent to seek minimum clarification when the current turn does not already resolve it.

The user's answer returns to Perception for resolution or continued ambiguity.

Initial scope stops here. Do not generalize this into a universal curiosity/open-question system until Entity/Observation ambiguity demonstrates that need.

## Ownership invariants

- Insomnia owns source-grounded Memory extraction and Entity-mention metadata extraction; it consumes Chronos during eligible Memory processing. Lexical Memory routing is deterministic derived indexing, not Insomnia model output.
- Dream owns Memory-to-Memory semantic relationship authority and Community organization; it consumes Chronos for temporal candidate/context reasoning.
- Chronos owns shared temporal detection, normalization, parsing, resolution, comparison, and bounded temporal-inference mechanics; it owns no semantic objects or transaction clock.
- Perception owns Entity and Observation semantic objects plus Entity/Observation ambiguity state; it consumes Chronos for Observation temporal interpretation.
- Relationship is a relational Observation specialization, not a sibling semantic authority.
- Relational Observation participants are owner-qualified Entity references; Observation ownership and visibility are independent of participant-Entity visibility.
- Only the active PHY may contribute private relational Observations to normal runtime composition; authorized REL relational Observations may be shared/portable across users.
- Community detection and hierarchy remain derived semantic organization, never independent semantic authority.
- Sub-/super-Community levels may be persisted, named, lineaged, traversed, and used for routing, but cannot create or override Memory/Entity/Observation truth.
- Scan-and-merge reduction intermediates are execution machinery and must not be promoted into semantic hierarchy without an independent graph-derived hierarchy pass.
- Processing neighbourhoods are ephemeral and never fake Communities.
- New Memory publication is the ordinary Pass-3 discovery trigger; lower-level semantic mutations do not recursively schedule new-Observation synthesis.
- Exploitation runs immediately for new Memories and may re-run on provenance-anchored cooldown epochs; Exploration begins no earlier than the next epoch and then uses the same cadence.
- Exploration is bounded and may use stochastic/diversity routing, but it does not persist a permanent settled-pair ledger; only bounded recent comparison/context history is retained for repetition control.
- Community structure narrows Exploitation at scale but is not an inference boundary; bounded traversal may cross Communities when evidence warrants it.
- Routing receptors are metadata, never evidence.
- Pairwise Observation contribution and multi-Memory extrapolation remain separate contracts.
- Observation support lineage resolves to real owner-qualified semantic objects.
- Entity identity is durable; Observation currentness is contingent.

## Implementation sequence

### A — Entity metadata + deterministic Memory lexical index — implemented 2026-09-17

- `MemoryRoutingMetadata` defines bounded exact Entity mentions over immutable Memory text; lexical routing comes from the disposable Memory lexical index over full Memory title/content.
- Insomnia contract `v3-4` owns the post-wording Entity enrichment pass; configured/runtime-host execution uses `models.insomnia_metadata` when present and otherwise effective main Insomnia. Extraction targets identity-bearing reusable referents rather than generic noun phrases; a narrow deterministic guard drops known bare/context-only generic surfaces plus multiword action/architecture phrases headed by `flow`, `lane`, `mechanism`, `request`, `response`, `seam`, `state`, or `status`. The model may select one occurrence when identical surface text has mixed semantics, but durable routing metadata remains exact title/content spans only.
- REL Project results embed Entity routing metadata atomically in `CVAINSC5`; PHY/User results persist the same clock-neutral attachment beside the routed Memory.
- New Entity routing attachments use `CVAMRTE2`; legacy `CVAMRTE1` remains readable and its obsolete model-generated lexical terms are discarded on decode.
- REL and PHY expose disposable Memory lexical search over complete current non-archived title/content, using the same deterministic lexical machinery as Archive search and no model call.
- Reopen validates `MemoryId + MemoryBodyId` binding and exact Entity source text. Migration/reconciliation replay the attachment, identical writes are idempotent, and conflicts fail closed.

Milestone **B — Entity owner and pass 1** is now implemented as an explicit owner API, validated zero-Entity bootstrap path, and automatic long-lived runtime lane after Dream. ADR 0036 establishes the shared typed semantic-Graph direction: Arcana remains the graph kernel, the Graph catalogue addresses typed semantic nodes, and `EntityAssociation` adds the first Entity-backed relation family without moving Entity payload/lifecycle authority into Graph.

### B — Entity owner and pass 1

- **Implemented:** durable Entity IDs, revisions/persistence, normalized one-to-many aliases, mutable semantic metadata, REL/PHY reopen, migration/reconciliation replay, and resolution-reference validation.
- **Implemented:** typed Graph relation endpoints/persistence over the existing Arcana kernel, with full semantic traversal plus an isolated Memory-only Dream/Community projection.
- **Implemented:** directional `Memory -> Entity` `EntityAssociation` topology, REL/PHY reopen, migration, divergent reconciliation, reverse lookup, and projection-watermark isolation.
- **Implemented:** bounded deterministic Entity candidate retrieval for one exact Memory mention: indexed exact canonical-name/alias lookup, source-Memory association recovery, owner-local lexical-Memory context, first-hop Dream-neighbour context, bounded evidence Memory IDs, deterministic ranking, REL/PHY parity, and reopen-safe derived indexes. These signals generate candidates only and do not assert identity.
- **Implemented:** zero-candidate **Entity Admission v2** with bounded same-surface context, narrow bare-generic rejection, and stable source-grounded initial Entity metadata. Admission and identity resolution remain separate judgments.
- **Implemented:** calibrated **V4 identity resolution** for non-empty candidate sets using the same prompt/payload/schema as the frozen calibration harness, with Entity-attached Memories hydrated before incidental discovery evidence.
- **Implemented:** deterministic `resolve_entity_mention(...)` processor persistence across Entity creation/reuse/split, `Memory -> Entity` association, resolved/unresolved/rejected mention state, partial-write recovery through source associations, REL/PHY parity, and reopen.
- **Proven:** the frozen 41-query organic bootstrap starts from zero durable Entities and reaches 24 first-occurrence creates, 13 repeat resolves, one distinct same-surface split, one unresolved, two rejects, and 25 durable Entities after reopen without weakening gold assertions.
- **Deferred pending measurement:** Entity vector/centroid routing. No all-Entity semantic scan or per-query Entity re-embedding is used as a substitute.
- **Implemented:** long-lived runtime scheduling after Dream for REL and attached PHY. Dream completion enqueues mentions on the processed source and bounded affected candidate Memories; same-surface and candidate-Entity evidence events wake relevant unresolved mentions through derived reverse indexes. Startup performs one recovery scan of active Dream-processed Memories, terminal states are not rerun, unchanged candidate/context fingerprints suppress inference, model calls occur outside owner locks, and stale prepared snapshots are rejected before persistence.

### B2 — Relational Observation persistence convergence

- **Implemented transitional foundation:** durable Relationship IDs/revisions, dense owner-local `relationship_version`, open classification string, n-ary owner-qualified `EntityRef` participants with optional roles, owner-qualified `MemoryRef` evidence, bounded compact summary, REL/PHY reopen, global-version validation, migration, divergent reconciliation, physical reclamation, and deterministic query-by-participant.
- **Implemented transitional foundation:** local references validate against the containing owner while cross-owner references may remain dangling; Entity merge retargets only participant refs owned by the merging owner, leaving foreign-owner refs untouched.
- **Semantic amendment:** these records no longer define a separate Relationship semantic authority. They are provisional structured storage/indexing for the relational Observation profile.
- **Before synthesis:** converge this persistence with the Observation owner, either by folding relational fields into Observation records or by keying a tightly coupled relational-profile record to one Observation identity.
- **Remaining synthesis:** relational propositions are produced through bounded Observation extrapolation, not an independent Relationship synthesis lane and never an all-Entity-pairs path.
- **Remaining runtime:** active-PHY privacy and authorized-REL relational-Observation composition fixtures/view.

### C — Multi-resolution Communities and local neighbourhoods

- Extend Community representation to a derived multi-resolution hierarchy.
- Add recursive sub-Communities only while genuine internal structure remains.
- Build a Community meta-graph from cross-Community semantic relationships and derive coarser super-Communities where meaningful higher-level structure exists.
- Keep the hierarchy graph-derived; do not persist scan-and-merge reduction intermediates as semantic structure.
- Preserve or extend lineage, semantic naming, and stable inspection semantics at each persisted hierarchy level.
- Expose coarse -> normal -> fine routing for Perception and later retrieval/Ego integration.
- Detect oversized irreducible leaves.
- Implement ephemeral bounded local-neighbourhood construction for leaves that still exceed inference budgets.

### D — Observation owner and pass 3

- Define Observation persistence, support lineage, lifecycle, and chronology.
- Implement bounded multi-Memory extrapolation.
- Integrate Chronos over the bounded support set for deterministic valid-time synthesis and unresolved temporal fallback.
- Reconcile semantic duplicates without losing independent support paths.

### E — Receptors and pass 2

- Define receptor records/vector bindings.
- Implement receptor generation for new/materially rewritten Observations.
- Build bounded receptor-vector retrieval.
- Implement strict pairwise Memory-to-Observation contribution.

### F — Reconsideration and ambiguity runtime

- Add mutation accounting and thresholds.
- Add deterministic wall-time checks/stale transition.
- Add semantic reconsideration inference.
- Add Entity/Observation clarification triggers and runtime injection.

### G — Scale and quality validation

- Measure receptor recall and false positives on cross-domain evidence cases.
- Measure inference cost as Memory/Entity/Observation populations grow.
- Validate recursive Community/local-neighbourhood coverage on large RELs.
- Validate stale lifecycle without false deletion/invalidation.
- Tune thresholds only from measured workloads.

## Open implementation decisions

- Persistent schemas for Observation, relational-Observation profile, ambiguity, and receptor state; Entity persistence is implemented and the existing Relationship persistence is transitional input to this design.
- Relational participant role/cardinality policy and materialization thresholds; owner-qualified `EntityRef` representation and dangling-reference behavior are already implemented in the transitional Relationship substrate.
- Dedicated Perception model route vs General/Dream fallback.
- Entity candidate thresholds and multi-centroid maintenance.
- Multi-resolution Community criteria: recursive split stopping rules, Community meta-graph construction/weighting, super-Community resolution/stopping rules, and processing-neighbourhood budget.
- Observation derivation/inference taxonomy beyond support lineage.
- Receptor vector-index implementation and candidate limits.
- Mutation-accounting boundaries and wall-time policy.
- Observation duplicate/canonical reconciliation mechanics.
- Retrieval/Ego treatment of Entities and Observations once implemented.

## Notes

Perception is partially implemented: Entity pass 1 is production code, and the existing Relationship persistence foundation is production storage now designated as transitional relational-Observation substrate. Observation synthesis, relational-profile convergence/runtime composition, receptors, and ambiguity remain staged. The earlier derived-semantic-nodes exploration is retained as design history; ADRs 0033–0035 and this plan own the current accepted direction.

## Related docs

- [ADR 0033](decisions/0033-perception-entities-observations-and-ambiguity.md)
- [ADR 0034 — cross-owner relational Observations and active-PHY privacy](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md)
- [ADR 0035 — Chronos shared temporal semantics](decisions/0035-chronos-shared-temporal-semantics.md)
- [Chronos subsystem plan](chronos-subsystem-plan.md)
- [ADR 0012 — Insomnia memory authority](decisions/0012-deterministic-episodes-and-insomnia-memory-authority.md)
- [ADR 0025 — derived Graph communities](decisions/0025-owner-local-derived-communities.md)
- [ADR 0030 — Dream maintenance](decisions/0030-provenance-anchored-dream-maintenance.md)
- [ADR 0032 — Community lineage](decisions/0032-community-lineage-and-name-continuity.md)
- [Dream design and validation record](dream-implementation-plan.md)
- [Roadmap](roadmap.md)
