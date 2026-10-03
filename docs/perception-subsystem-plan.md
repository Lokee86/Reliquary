# Perception subsystem plan

Parent index: [Documentation index](INDEX.md)

Decision owners: [ADR 0033](decisions/0033-perception-entities-observations-and-ambiguity.md) and [ADR 0034](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md)

Shared temporal dependency: [ADR 0035 — Chronos](decisions/0035-chronos-shared-temporal-semantics.md)

## Status

Accepted architecture; implementation is in progress. Insomnia Entity-mention enrichment and Perception Milestone B's Entity owner, typed Graph association, bounded candidate retrieval, zero-candidate Admission, calibrated V4 identity resolution, deterministic persistence, per-mention resolution state, and long-lived post-Dream runtime scheduling are implemented. The full 41-case frozen zero-Entity bootstrap converges from no preconstructed Entities and survives reopen. Observation synthesis remains planned; ADR 0034 now defines Relationships as a relational Observation specialization. The existing Relationship persistence foundation is transitional storage to converge with the Observation owner before synthesis ships.

This document owns the implementation shape for Perception. ADR 0033 owns the architectural decision and rationale. Current shipped/proven behavior is also reflected in the architecture, API, manual, and limitation docs.

## Overview

Perception is the planned post-Dream layer for durable Entity identity and synthesized Observations. Memories and Observations are propositional nodes. Entities are referential/traversal nodes that canonicalize what propositions are about and connect propositions through durable identity.

Relational knowledge is represented as a structured Observation specialization over owner-qualified Entity participants rather than as a third semantic authority. Perception uses bounded deterministic routing plus narrow inference calls rather than full-corpus reasoning.

## Purpose

Perception materializes useful structure implicit in the organized Web that is neither an ordinary Dream Memory-to-Memory relationship nor already represented by one source-grounded Memory.

Its durable roles are:

- **Entities** — canonical referents used for identity, traversal, routing, participant binding, and inspection. They are not propositional evidence by themselves.
- **Observations** — contingent higher-order propositions synthesized from Memories and/or other Observations with exact derivation lineage.

A **relational Observation** is an Observation whose proposition describes a durable connection among two or more canonical Entity participants. It may carry structured owner-qualified participants, roles, and relation classification for routing/inspection/privacy, but it uses the same support, ambiguity, temporal, reconsideration, and ownership semantics as other Observations.

Perception is not another full-corpus reasoning layer. Deterministic routing, candidate/context budgets, recent comparison history, mutation state, and configurable processing cadence bound its inference.

## Pipeline

```text
Archive/source
    -> Insomnia semantic extraction + final Memory wording
    -> Insomnia Entity-mention enrichment
       -> Chronos Memory-level temporal assessment/inference when indicated
    -> durable Memory publication
    -> Dream Memory-Web organization (consumes Chronos)
    -> Perception Entity processing
       -> Entity synthesis / association / disambiguation
    -> fully processed propositional anchor
       -> existing-Observation contribution
       -> ordinary Observation synthesis
          -> Exploitation and later Exploration
       -> relational Observation synthesis when deterministic Entity gates pass
          -> Exploitation and later Exploration
    -> accepted Observation publication
       -> receptor generation
       -> newly fully processed Observation may itself become a future anchor
```

Existing-Observation contribution may create mutations that trigger reconsideration of the target Observation. Observation reconsideration and ambiguity clarification remain lifecycle/runtime mechanisms rather than full-population scans.

The word **pass** refers to a concrete processing sweep/model call, not to one monolithic Perception phase. One anchor may therefore generate several narrow calls in one processing round.

## Insomnia enrichment prerequisite

After final authoritative Memory wording, Insomnia runs a separate metadata pass that extracts exact Entity mentions from title/content, persisted as field + UTF-8 byte span + verbatim text. A mention must be an **identity-bearing reusable referent**: its surface form must carry enough identity to be recognized again in another Memory without reconstructing the sentence that produced it. Bare/context-only references such as `the server`, `the file`, `the agent`, or `the repository` are excluded. Descriptive action/architecture phrases ending in `flow`, `lane`, `mechanism`, `request`, `response`, `seam`, `state`, or `status` are also screened out unless identity is carried by a code/API artifact rather than the prose phrase itself. Stable owner-local descriptive identities such as `write server`, `devtools window`, `the Vancouver office`, or `my brother` remain eligible when the modifier/relation consistently identifies the referent.

The implemented record is `MemoryRoutingMetadata`, bound to exact `MemoryId + MemoryBodyId`. Entity mentions are capped at 64 items and each emitted text value at 512 UTF-8 bytes. Write/reopen validation requires every mention span to match the immutable Memory body exactly. The metadata is clock-neutral and remains valid across metadata/lifecycle revisions because the Memory body cannot mutate in place.

Lexical locality is deliberately **not** model-extracted metadata. REL and PHY derive a disposable owner-local Memory lexical index directly from full current Memory title/content using the same deterministic tokenizer, inverted index, and coverage+density score as Archive lexical search. It rebuilds lazily from `memory_version`, persists no semantic state, and is the lexical routing/context-construction primitive Perception should use.

It does **not** resolve/create Entities, decide identity, assign Entity IDs/types, synthesize Observations, or create semantic authority beyond the Memory itself. Perception pass 1 remains the sole owner of Entity synthesis/association/disambiguation.

## Chronos temporal dependency

Perception does not own a separate temporal parser or temporal-inference stack. It consumes the shared Chronos subsystem defined by ADR 0035 after Perception has selected the bounded evidence relevant to an Observation synthesis or reconsideration.

Chronos may compare supporting Memory intervals/patterns, derive Observation valid-time semantics deterministically, expose temporal conflict/ambiguity, or invoke bounded temporal inference when deterministic interpretation is insufficient. Perception remains responsible for deciding what Observation proposition exists.

Deterministic Chronos products remain derived and need not be persisted. Any non-deterministically inferred temporal conclusion that becomes part of durable Perception state must be tied to the semantic inputs/version that justified it.

Chronos remains responsible for interpreting occurrence and valid time in **Observation** synthesis and reconsideration. The earlier proposal for a mandatory Chronos-qualified Perception audit before a Memory can regain relevance has been **superseded**: [Memory Freshness](memory-staleness-plan.md) now derives attention/context relevance deterministically from REL turns, direct access and existing Dream graph activity. Perception does not own this counter or its lifecycle.

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

## Observation processing model

After Entity resolution, Perception operates over **propositional nodes**:

- **Memories** are source-grounded propositions.
- **Observations** are derived propositions with exact support/derivation lineage.
- **Entities are referential/traversal nodes, not propositional evidence nodes.** They connect propositions that refer to the same durable thing, supply canonical participants/roles, and provide strong deterministic routing signals, but an Entity by itself asserts nothing.

A newly created propositional node becomes an Observation-processing **anchor only after that node has completed its own required processing**. For a Memory, that means Dream organization plus Entity resolution. For an Observation, that means successful publication plus required post-publication work such as receptor generation. Entity creation does not itself create an Observation-processing anchor because its grounding proposition has already entered through a Memory or Observation.

This permits recursive higher-order reasoning:

    Memory -> Observation -> higher-order Observation -> ...

Such cascades are possible but are expected to be self-limiting through bounded candidate/context budgets, duplicate admission, reasoning-specific verification, and the requirement that each published Observation add useful semantic structure. Do not add a hard cascade-depth rule before measurement. Validation should measure cascade depth, fan-out, accepted descendants per anchor, duplicate rejection, and inference cost.

Observation processing has three distinct inference paths. They share scheduling, candidate-discovery infrastructure, budgets, provenance accounting, and verification machinery, but remain separate model contracts rather than one overloaded call:

1. **existing-Observation contribution** — determine whether the anchor materially bears on one already-published Observation;
2. **new ordinary-Observation synthesis** — determine whether the anchor plus selected propositional evidence supports a new non-relational Observation; and
3. **new relational-Observation synthesis** — determine whether the anchor plus selected propositional evidence supports a new Relationship-class Observation.

Relational Observations use the same Observation lifecycle and may contribute to ordinary higher-order Observations. They are **not eligible as evidence for synthesizing another relational Observation**. Memories and ordinary Observations may contribute to either ordinary or relational synthesis when the relevant gates pass.

### Existing-Observation contribution and reconsideration

Contribution is routed primarily through separately generated **routing receptors** attached to existing Observations. Each receptor describes one kind of future proposition that could materially bear on its Observation and is embedded independently.

The semantic judgment is pairwise at the proposition/Observation boundary:

    propositional anchor P <-> existing Observation O

P may be a Memory or an Observation. When P is itself an Observation, the contribution call may hydrate its exact derivation/support lineage as bounded context so the model can understand what the derived proposition rests on. Provenance accounting must not double-count transitive overlap as independent corroboration.

Possible contribution results include support, challenge/contradiction, weakening, qualification, ambiguity, or irrelevance. A contribution judgment never creates a new Observation.

Candidate Observation IDs may come from receptor-vector retrieval, exact shared-Entity routing, exact support/dependency topology, and relational-participant indexes where relevant.

Exhaustive anchor-by-all-Observations inference is forbidden. Receptors and deterministic routes are routing metadata only and never evidentiary authority.

Relevant contribution results accumulate as mutations against the target Observation. Crossing its mutation threshold triggers the target Observation's separate **semantic reconsideration**. Reconsideration belongs to the target Observation's lifecycle; it is not another mandatory call charged to the anchor's routine processing path.

## Observation synthesis scheduling: Exploitation and Exploration

**Exploitation** and **Exploration** are discovery/scheduling modes, not Observation classes and not reasoning algorithms. Either mode may produce an ordinary Observation or a relational Observation.

For a configurable processing cadence C:

- Exploitation for a new fully processed anchor runs immediately, then becomes eligible every C.
- Exploration is phase-offset by C / 2, so its first opportunity occurs at the next Exploration phase and it then recurs every C.

For an illustrative 30-day cadence:

    day 0   Exploitation
    day 15  Exploration
    day 30  Exploitation
    day 45  Exploration
    day 60  Exploitation

Each mode therefore still has a 30-day cooldown in that example, while Perception work is distributed at roughly 15-day intervals. The cadence remains configurable. Missed epochs do not accumulate as backlog.

Exploitation exists to follow **already-established reasons for comparison**. Exploration exists to compare propositions outside or beyond those strong routes specifically to search for emergent structure that the current Web does not already encode.

Lower-level Graph, Entity, Community, and metadata mutations do not recursively trigger synthesis by themselves. They change the structure that the next eligible anchor processing round sees.

### Bootstrap behavior

When the complete authorized propositional Web fits comfortably inside the synthesis context budget, Perception may process the whole Web. This is the natural zero/small-Web bootstrap and avoids inventing routing complexity before scale requires it.

Once the Web no longer fits, Exploitation uses strong candidate-selection lanes and one round may emit **multiple bounded comparison jobs**. Candidate generation and LLM context are both budgeted independently: a mature Web may contain far more structurally plausible candidates than can be hydrated or compared in one background round.

### Exploitation candidate-selection lanes

Outside whole-Web bootstrap, an Exploitation comparison must have an independently established reason to exist before the synthesis model sees it. Embedding similarity alone is not sufficient.

The initial lanes are:

1. **Entity bridge**
   - Start from durable Entity references attached to the anchor proposition.
   - Traverse those Entities to other Memories/Observations that reference the same canonical Entity or relevant Entity set.
   - The Entities themselves do not enter the synthesis evidence set; the selected propositional nodes do.
   - Example: M1 -> {Sarah, Acme}, M7 -> {Sarah, Vancouver}, and O3 -> {Acme, Vancouver} can justify comparing M1, M7, and O3 because canonical Entity identity supplies a concrete bridge.

2. **Source neighbourhood**
   - Select propositions with concrete provenance locality: the same Episode, conversation, document, import unit, transcript region, or other source-local region.
   - Large sources use bounded physical/chronological windows rather than treating an entire document or conversation as one unlimited context.
   - For an Observation anchor, its support lineage may lead back to the source neighbourhoods of its grounding Memories.

3. **Web-structure / regional neighbourhood**
   - Use Dream/Graph structure that already exists independently of the Observation comparison: direct strong edges, bounded short paths, and coherent Community/subcommunity locality.
   - Communities are one useful regional structure inside this lane, **not the primary or exclusive Observation selector** and never a hard inference boundary.

4. **Derivation neighbourhood**
   - Traverse already-established Observation support/derivation topology: parents, children, shared-support siblings, or other propositions linked by accepted derivation structure.
   - This lane becomes increasingly important as higher-order Observations accumulate.

Each lane may generate more candidates than fit. Candidate-generation limits prevent pathological traversal; a separate context/inference budget limits what is hydrated and sent to reasoning models. The scheduler may rotate candidates across epochs using recent comparison history so one dense local region does not permanently monopolize the available budget.

Candidates that were actually generated through a valid Exploitation lane but were not compared because of candidate/context/inference budget may be retained in a bounded **Exploitation overflow reservoir** for later Exploration.

### Community hierarchy as structural infrastructure

The existing owner-local Community layer remains valuable derived organization, but it is not the governing Observation-selection algorithm.

Where measured scale/routing value justifies it, Community processing may expose multiple genuine resolutions:

1. compute the normal owner-local Leiden partition over the Memory projection;
2. recursively run Leiden inside a Community while genuine subcommunities exist;
3. build a Community meta-graph and derive coarser super-communities where genuine higher-level structure exists; and
4. stop subdivision or aggregation when another level does not add meaningful structure.

Leiden must never be forced to invent a split or grouping solely to satisfy an inference budget. The hierarchy must derive from persisted Graph/Community state, not the temporary scan-and-merge reduction tree.

Perception may consume this hierarchy in two ways:

- **Exploitation Web-structure/regional lane:** Community/subcommunity locality is one concrete pre-existing reason to compare propositions.
- **Exploration cross-region lane:** separate Communities/regions provide deliberate boundaries to sample across when looking for emergent concepts.

If a genuine Community/leaf is too large for a regional comparison job, Perception may construct an ephemeral bounded local neighbourhood using Graph locality, Entity bridges, source/derivation structure, or other accepted lane signals. Such a processing window is never persisted as a fake Community.

### Relational-synthesis gate

Relational synthesis has additional deterministic gates before any expensive model call:

- the anchor proposition must reference at least one durable Entity;
- the candidate set must introduce at least one **other distinct** durable Entity; and
- a relational Observation used as the anchor/evidence is ineligible for another relational-synthesis job.

A published relational Observation must reference at least **two distinct canonical Entity participants**. This is a structural invariant, not proof that a meaningful relationship exists.

A cheap Decision/Jev-style probe may gate candidate relational contexts with a narrow question such as whether the supplied propositions indicate a potentially meaningful relationship among at least two referenced Entities. Its score is routing metadata only. Passing the probe merely justifies spending the heavier relational synthesis/verification budget.

### Exploration candidate-selection lanes

Exploration is intentionally broader. Its job is to test comparisons the established Web may never prioritize and to search for emergent concepts across otherwise separate regions.

Initial lanes are:

1. **Exploitation overflow** — structurally justified candidates generated during Exploitation but omitted from actual comparison because of budget.
2. **Semantic similarity** — embedding-near propositions that lack a stronger current Exploitation route.
3. **Lexical similarity** — propositions sharing unusually informative terms, identifiers, phrases, or names without a stronger route.
4. **Cross-region sampling** — deliberately compare propositions from different Communities/structural regions, especially regions not recently explored together.
5. **Random sampling** — genuinely stochastic eligible comparison for broad long-run coverage.

Exploration does **not** maintain a permanent pair-completed ledger. Instead, both Exploitation and Exploration write to bounded recent-comparison history. Exploration excludes or heavily deprioritizes candidates/contexts that were recently compared in either mode. Old entries naturally fall away so the same propositions may be reconsidered later under changed surrounding evidence.

The recent ledger creates novelty pressure; it does not assert that a comparison can never become useful again. Exact ledger depth, context fingerprinting, per-lane sampling weights, and spillover policy remain calibration work.

A cheap Decision/Jev-style relatedness probe may be used on weak exploratory contexts before heavier synthesis. As with relational gating, probe confidence is routing information only and never semantic evidence.

### Multiple comparison jobs per round

One eligible anchor is not required to produce one blended LLM context. A single Exploitation or Exploration round may emit several narrow comparison jobs from different lanes.

For example:

    anchor M42
      -> Entity-bridge context        {M42, M8, O3}
      -> source-neighbourhood context {M42, M11, M12}
      -> derivation context           {M42, O9, O14}

These contexts may overlap. Deduplication should prevent identical work, but a proposition may legitimately appear in more than one context when the surrounding evidence and reason for comparison differ.

The exact per-lane priorities, candidate caps, number of comparison jobs, context budgets, and unused-budget spillover rules are not yet settled. They are the remaining candidate-selection design/calibration work.

### Two rounds of candidate selection

Initial candidate selection is not required to assemble a complete proof. It only needs to surface a plausible reasoning opportunity.

New-Observation formation therefore has two distinct selection stages:

1. **discovery selection** — Exploitation/Exploration lanes construct one or more bounded contexts and a proposal call emits zero or more candidate Observations;
2. **reasoning-directed evidence expansion** — once a candidate proposition and likely reasoning mode exist, targeted traversal gathers the additional support, contradiction, qualification, counterexamples, complementary premises, or alternative explanations needed to evaluate that specific candidate.

Evidence expansion may use authorized Graph traversal, Entity bridges, source neighbourhoods, derivation topology, lexical routes, semantic search, Communities, and other bounded retrieval primitives. This second stage is goal-directed evidence retrieval, not another generic Exploration round.

### Reasoning and verification contract

The intended reasoning taxonomy is:

- **Deductive** — the candidate follows from the supplied propositions under ordinary logical/general reasoning. This does not require a predeclared rule engine; the algorithm must nevertheless identify the premises that make the conclusion follow.
- **Inductive** — multiple observations/examples support a broader pattern or generalization. Expansion should seek additional instances and counterexamples, and the published scope must not exceed what the evidence warrants.
- **Abductive** — the candidate is a plausible explanation for the evidence rather than an entailed conclusion. Expansion should seek competing explanations and discriminating evidence, and the resulting Observation must retain its uncertainty.
- **Abstractive** — several grounded propositions are consolidated/lifted into a useful higher-level semantic representation without materially extending beyond what those propositions already establish.

Abstractive reasoning is **not** a generic escape hatch for cases that do not fit the other three. Its exact algorithm and boundary against generalized deduction/induction still require calibration. If a candidate cannot be cleanly justified by a supported reasoning contract, the system should abstain rather than force it into Abstractive.

Illustrative reasoning distinctions:

    Deduction:
      "Only admins may deploy."
      "Maya is not an admin."
      -> "Maya is not authorized to deploy under that rule."

    Induction:
      several independent incidents show the same failure after the same condition
      -> a scoped recurring-pattern Observation, subject to counterexamples.

    Abduction:
      several failures begin immediately after one dependency change
      -> "The dependency change may explain the failures,"
         while alternative causes remain part of verification.

    Abstraction:
      multiple grounded propositions describe different facets of one larger
      semantic structure
      -> one higher-level representation of that structure without adding
         a broader pattern, causal explanation, or unsupported consequence.

The Abstractive example above is intentionally schematic because its precise acceptance boundary is one of the remaining reasoning-algorithm design tasks.

After evidence expansion, the reasoning-specific derivation is independently verified against the exact evidence. A generic confidence score must not substitute for reasoning-specific validation.

Publication persists the Observation proposition, ordinary vs relational class/profile, derivation/reasoning mode, exact typed support/derivation lineage, any support/challenge/qualification relations, Chronos temporal semantics where applicable, and optionally the discovery mode/route as diagnostic provenance without treating routing metadata as semantic authority.

Evidence is many-to-many. One Memory or Observation may contribute to multiple Observations, and one Observation may depend on multiple propositional nodes. Reuse of the same underlying source through several derived paths must not be counted as independent corroboration.

## Pass 4 — Observation routing-receptor generation

Every new Observation, and any materially rewritten Observation, receives a separate receptor-generation pass.

The pass generates prospective future facts that could materially:

- support it;
- challenge/contradict it;
- qualify it;
- indicate that its current interpretation warrants reconsideration.

Each receptor is embedded independently and mapped back to the Observation. Receptors must not be collapsed into one centroid because relevant evidence can occupy very different semantic regions.

Generation favors recall over precision: a false positive costs one bounded pairwise comparison; a false negative may hide relevant evidence indefinitely.

## Historical note — proposed stale-Memory audit withdrawn

An earlier design proposed a periodic Perception pass that validated recent source-specific work, with Chronos-verified event time, before resetting a Memory's staleness baseline. **That pass is no longer part of the planned Memory Freshness architecture.** The accepted [unified Freshness design](generic-staleness-implementation-plan.md) replaces periodic per-Memory reviews with deterministic -100..+100 scores, REL-turn decay, access and new-Memory linkage events, graph-distance propagation and fixed originating-Community locality.

This change does **not** remove Perception's Observation synthesis, semantic reconsideration, independent contradiction analysis or Chronos temporal responsibilities. A future optional Perception evidence event must be explicitly designed if there is a demonstrated need; it is not a prerequisite, default inference pass or automatic Freshness reset.

## Observation lifecycle

Observations are retained historical semantic objects and should normally be archived/retained rather than deleted.

### Mutation-driven reconsideration

Relevant contribution mutations accumulate against an Observation. Crossing the configured threshold triggers semantic reconsideration.

Mutation only establishes that inference is warranted. Reconsideration may keep the Observation active, qualify/supersede it, expose contradiction/ambiguity, or change its priority/currentness state.

### Wall-time currentness policy

Wall-time staleness is not assumed to apply uniformly to every Observation.

Current-state propositions may receive wall-time checkpoints because they can become silently outdated. Durable/historical propositions may be exempt from staleness-by-time alone. Relationship classes make this distinction especially visible: a current employment/authority relation may need a freshness policy, while a historical or permanently established relation may remain semantically useful without periodic confirming evidence.

For an Observation whose policy enables wall-time checks, deterministic machinery may ask whether any relevant mutation occurred since the last semantic reconsideration:

- **none** -> policy may mark stale and remove the Observation from priority/current consideration without inference;
- **one or more** -> run semantic reconsideration.

A mutation does not imply freshness. Old supporting evidence may still lead reconsideration to mark an Observation stale.

`stale` means non-priority/currentness-uncertain, not false. Wall time alone never contradicts, deletes, or invalidates an Observation. Chronos valid-time and Observation currentness remain separate concerns.

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
- Perception owns durable Entity referential state, Observation propositional state, and Entity/Observation ambiguity state; it consumes Chronos for Observation temporal interpretation.
- Memories and Observations are propositional nodes eligible for Observation reasoning; Entities are referential/traversal nodes and never count as standalone propositional evidence.
- A newly created Memory or Observation becomes an Observation-processing anchor only after its own required processing has completed. Entity creation is not an Observation-processing anchor.
- Relationship is a relational Observation specialization, not a sibling semantic authority.
- Every published relational Observation has at least two distinct canonical Entity participants.
- Relational Observations may contribute to ordinary higher-order Observations and may be reconsidered, but they cannot themselves participate as evidence/anchors in new relational-Observation synthesis.
- Relational Observation participants are owner-qualified Entity references; Observation ownership and visibility are independent of participant-Entity visibility.
- Only the active PHY may contribute private relational Observations to normal runtime composition; authorized REL relational Observations may be shared/portable across users.
- Community detection and hierarchy remain derived organization, never independent semantic authority.
- Communities are one Web-structure/regional candidate route, not the primary Observation selector and not an inference boundary.
- Sub-/super-Community levels may be persisted, named, lineaged, traversed, and used for routing, but cannot create or override Memory/Entity/Observation truth.
- Scan-and-merge reduction intermediates are execution machinery and must not be promoted into semantic hierarchy without an independent graph-derived hierarchy pass.
- Processing neighbourhoods are ephemeral and never fake Communities.
- Exploitation follows independently established Entity/source/Web/derivation structure. Exploration deliberately reaches beyond those strong routes for emergent concepts.
- Exploitation runs immediately for a new fully processed propositional anchor and then on cadence C; Exploration is phase-offset by C / 2 and then uses the same per-mode cadence.
- Both modes maintain bounded recent-comparison history; no permanent pair-completion ledger exists. Exploration suppresses recent comparisons from either mode rather than assuming Exploitation will eventually cover every structurally plausible candidate.
- Candidate-generation budgets and model-context/inference budgets are separate. Mature Webs may trim candidates before inference without making the trimmed region permanently ineligible for Exploration.
- Routing receptors and Decision/Jev probe scores are routing metadata, never evidence.
- Existing-Observation contribution, ordinary synthesis, and relational synthesis remain distinct semantic call contracts.
- New-Observation formation uses discovery selection followed, when needed, by reasoning-directed evidence expansion.
- Observation support lineage resolves to real owner-qualified propositional objects and preserves transitive overlap so repeated derivations do not fabricate independent corroboration.
- Entity identity is durable; Observation currentness is contingent. Wall-time staleness applicability may vary by Observation semantics/class while valid-time remains a separate Chronos concern.

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

- **Implemented transitional foundation:** durable Relationship IDs/revisions, dense owner-local relationship version, open classification string, n-ary owner-qualified EntityRef participants with optional roles, owner-qualified MemoryRef evidence, bounded compact summary, REL/PHY reopen, global-version validation, migration, divergent reconciliation, physical reclamation, and deterministic query-by-participant.
- **Implemented transitional foundation:** local references validate against the containing owner while cross-owner references may remain dangling; Entity merge retargets only participant refs owned by the merging owner, leaving foreign-owner refs untouched.
- **Semantic amendment:** these records no longer define a separate Relationship semantic authority. They are provisional structured storage/indexing for the relational Observation profile.
- **Before synthesis:** converge this persistence with the Observation owner, either by folding relational fields into Observation records or by keying a tightly coupled relational-profile record to one Observation identity.
- **Remaining synthesis:** implement a specialized relational-synthesis call path under the same Exploitation/Exploration discovery modes as ordinary Observation synthesis. Enforce the deterministic two-distinct-Entity participant invariant and exclude relational Observations from relational-to-relational synthesis.
- **Remaining lifecycle:** define which relational classes are subject to wall-time staleness versus durable/historical currentness semantics without creating a separate lifecycle authority.
- **Remaining runtime:** active-PHY privacy and authorized-REL relational-Observation composition fixtures/view.

### C — Candidate discovery, scheduling, and comparison history

- Implement propositional anchors for fully processed Memories and Observations.
- Implement immediate Exploitation plus Exploration staggered by half the configurable cadence while preserving the same per-mode cooldown length.
- Implement whole-Web bootstrap while the authorized propositional Web fits the synthesis budget.
- Implement the four Exploitation lanes: Entity bridge, source neighbourhood, Web-structure/regional neighbourhood, and derivation neighbourhood.
- Keep Communities inside the Web-structure/regional lane rather than using them as the primary selector.
- Separate candidate-generation limits from hydrated context/inference budgets.
- Add a bounded Exploitation-overflow reservoir for generated-but-uncompared candidates.
- Implement bounded recent-comparison ledgers covering both Exploitation and Exploration; use them for rotation/novelty pressure rather than permanent pair completion.
- Implement the Exploration lanes: Exploitation overflow, semantic similarity, lexical similarity, cross-region sampling, and random sampling.
- Add optional Decision/Jev routing probes for weak exploratory contexts and relational-candidate gating; probe confidence is never semantic evidence.
- Allow multiple narrow comparison jobs per anchor round and suppress only genuinely duplicate work.

### D — Observation owner, synthesis, and reasoning

- Define unified Observation persistence, exact support/derivation lineage, lifecycle, chronology, and ordinary-vs-relational profile.
- Implement separate ordinary-Observation and relational-Observation synthesis contracts.
- Implement discovery selection -> candidate proposal -> reasoning-directed evidence expansion -> reasoning-specific derivation -> independent verification -> publication.
- Define and calibrate deductive, inductive, abductive, and abstractive reasoning algorithms.
- Keep Abstractive reasoning narrow; it must not become a fallback label for otherwise unclassified associations.
- Integrate Chronos over the bounded evidence set for deterministic valid-time synthesis and unresolved temporal fallback.
- Reconcile semantic duplicates without losing independent support paths or double-counting transitive source overlap.
- Measure reasoning-cascade depth/fan-out before adding hard depth governors.

### E — Receptors, contribution, and reconsideration inputs

- Define receptor records/vector bindings.
- Implement receptor generation for new/materially rewritten Observations.
- Build bounded high-recall receptor retrieval over Memory and Observation anchors.
- Implement strict pairwise propositional-anchor-to-existing-Observation contribution.
- For Observation anchors, hydrate bounded exact support/derivation context when needed without double-counting transitive provenance.
- Route relevant contribution results into target-Observation mutation accounting and reconsideration.

### F — Reconsideration and ambiguity runtime

- Add contribution-mutation accounting and thresholds.
- Add semantic reconsideration inference for threshold-crossing target Observations.
- Add class-sensitive wall-time currentness policy; do not stale durable/historical classes merely because time passes.
- Keep Chronos valid-time separate from Observation currentness/staleness.
- Add Entity/Observation clarification triggers and runtime injection.

### F2 — Withdrawn historical step: stale-Memory revalidation

The former mandatory audit, provenance/Chronos freshness-reset gate and generic `MemoryReconsideration` schedule are **superseded**. Implement the [unified Memory Freshness plan](generic-staleness-implementation-plan.md) in the Memory/Graph activity owner, not as a new Perception pass. Observation-specific temporal reasoning and reconsideration remain under steps E/F.

### G — Scale and quality validation

- Measure receptor recall and false positives on cross-domain evidence cases.
- Measure inference cost as Memory/Entity/Observation populations grow.
- Validate recursive Community/local-neighbourhood coverage on large RELs.
- Validate stale lifecycle without false deletion/invalidation.
- Tune thresholds only from measured workloads.

## Open implementation decisions

The two major unresolved **behavioral-design** pieces before Observation implementation is roadmap-ready are:

1. **candidate-selection specifics** — exact per-lane candidate-generation algorithms, lane priority/rotation, comparison-job grouping, candidate/context/inference budgets, Exploitation-overflow retention, recent-ledger depth/fingerprinting, and unused-budget spillover;
2. **reasoning algorithms** — exact deductive, inductive, abductive, and abstractive contracts; reasoning-directed evidence expansion; reasoning-specific verification; abstention behavior; and especially the narrow boundary that prevents Abstractive reasoning from becoming a catch-all.

Additional implementation/calibration decisions remain:

- persistent schemas for Observation, relational-Observation profile, ambiguity, receptor, recent-comparison, and overflow state; Entity persistence is implemented and the existing Relationship persistence is transitional input;
- relational participant role/cardinality policy and materialization thresholds; owner-qualified EntityRef representation and dangling-reference behavior are already implemented in the transitional Relationship substrate;
- dedicated Perception model routes vs General/Dream fallback, including the Decision/Jev probe route;
- Entity candidate thresholds and multi-centroid maintenance;
- multi-resolution Community criteria for routing/retrieval value, without making Community hierarchy the primary Observation selector;
- receptor vector-index implementation and candidate limits;
- mutation-accounting boundaries and class-sensitive wall-time currentness policy;
- Observation duplicate/canonical reconciliation mechanics and transitive-support overlap accounting;
- reasoning-cascade measurement and whether any hard governor is eventually necessary; and
- Retrieval/Ego treatment of Entity routing and Observation proposition/currentness state once implemented.

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
