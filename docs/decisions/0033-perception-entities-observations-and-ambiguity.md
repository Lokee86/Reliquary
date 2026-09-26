# ADR 0033: Perception entities, observations, and ambiguity handling

Parent index: [Architectural decisions](INDEX.md)

Implementation plan: [Perception subsystem plan](../perception-subsystem-plan.md)

## Status

Accepted — 2026-09-08; relational-Observation model amended 2026-09-25; propositional-anchor, Exploitation/Exploration candidate-lane, staggered scheduling, and staged reasoning architecture amended 2026-09-26. Entity pass 1 is implemented; the pre-amendment Relationship persistence foundation exists under ADR 0034; Observation/ambiguity synthesis remains staged.

Amends ADR 0012 by adding Insomnia metadata enrichment after Memory extraction, and ADR 0025 by allowing multi-resolution Community structure as derived routing infrastructure without making Communities the primary Observation-selection boundary or turning processing windows into semantic Communities. ADR 0034 defines cross-owner relational semantics as a specialized Observation form over owner-qualified Entity references without changing Dream's owner-local Memory-Graph authority. ADR 0035 assigns Observation temporal interpretation to the shared Chronos subsystem rather than a Perception-local temporal stack.

## Context

Insomnia and Dream already have distinct semantic responsibilities. Insomnia extracts source-grounded Memories. Dream organizes those Memories by discovering durable semantic relationships, canonical/corroborating structure, lifecycle effects, and derived owner-local Communities.

A separate problem remains: some useful semantic structure is not itself a remembered proposition and is not merely a relationship between two Memories.

Perception owns two first-class durable object categories with different semantic roles:

- **Entities** are durable canonical **referential/traversal nodes** such as a person, organization, place, project, or other thing that may be mentioned by many propositions under changing contexts. An Entity carries identity and routing structure; it does not itself assert a proposition.
- **Observations** are contingent higher-order **propositional nodes** synthesized from semantic evidence, such as a recurring pattern, apparent authority, tendency, architectural relationship, social relationship, or other conclusion whose useful semantic structure is not exhausted by retaining the source propositions alone.

Memories remain the source-grounded propositional nodes owned by Insomnia. Observation synthesis therefore reasons over Memories and Observations, while Entity identity is used to connect, route, constrain, and structure those propositions.

ADR 0034 defines **Relationships as a specialized relational Observation form**, not a third semantic object family. A relational Observation carries structured owner-qualified Entity participants, optional participant roles, and optional relation classification because those fields improve routing, inspection, privacy composition, and validation. Its proposition, support lineage, lifecycle, reconsideration, ambiguity, temporal semantics, and authority remain Observation semantics.

Entities and Observations require different lifecycle rules. An Entity remains the same referent even when its metadata, relationships, or relevance change. An Observation is contingent propositional knowledge whose current usefulness or currentness may change, whose support can change, and whose semantics may need reconsideration.

Perception must not become a second full-corpus inference pass over every Memory. Large Project or Organization Reliquaries make all-to-all Observation comparison unbounded in both cost and latency. Candidate discovery therefore needs deterministic structure around narrowly scoped inference calls.

## Decision

Reliquary introduces a separate **Perception** subsystem after Dream.

The normal processing order is:

```text
Archive/source
    -> Insomnia Memory extraction
    -> Insomnia Entity-mention enrichment
    -> Dream Memory-Web organization
    -> Perception
```

Perception does not take over Dream's Memory-to-Memory relationship synthesis. Dream remains responsible for organizing the Memory Web. Perception consumes that organized Web to materialize implicit semantic structure. Relational propositions among Entities are synthesized as Observations under the same authority/lifecycle model as other Observations, with ADR 0034 supplying their structured participant and cross-owner privacy rules. Observation synthesis/reconsideration may consume shared Chronos temporal analysis under ADR 0035; Perception does not own a separate temporal parser or temporal-inference stack.

### Insomnia Entity enrichment and deterministic Memory lexical routing

Insomnia gains an additional pass after authoritative Memory extraction. For each newly created Memory, that pass extracts only bounded exact Entity mentions from the final Memory title/content. A mention must carry enough identity in its surface form to be reusable across Memories; anonymous/context-only references such as `the server`, `the file`, `the agent`, or `the repository` are excluded, and descriptive action/architecture phrases ending in `flow`, `lane`, `mechanism`, `request`, `response`, `seam`, `state`, or `status` are deterministically screened out. Stable descriptive identities remain valid when a modifier or relation consistently distinguishes the referent within owner scope.

Lexical routing is deliberately separate from inference. REL and PHY derive a disposable owner-local lexical index directly from complete current Memory title/content, using the same deterministic lexical tokenizer/index/scoring machinery as Archive search. The index is candidate-generation state only: it is not persisted, is rebuilt after reopen or Memory-version changes, and requires no model call.

The Entity enrichment pass does **not** resolve, create, merge, or disambiguate Entities and does not synthesize Observations. Those are Perception responsibilities. The deterministic Memory lexical index supplies lexical locality for later candidate/context construction without adding probabilistic routing metadata.

## Perception pass 1: Entity synthesis, association, and disambiguation

Perception consumes the Entity mentions extracted by Insomnia against the post-Dream Memory Web.

For each mention, Perception may:

- associate it with an existing durable Entity;
- create a new Entity when no existing referent is plausible and the mention still carries identifiable reusable identity; or
- preserve an unresolved ambiguity when identity cannot safely be established.

Candidate discovery should be cheap and deterministic where possible. Mention/context embeddings may retrieve candidate Entities, after which existing Entity semantic centroids and the Memories/structure already attached to the Entity provide contextual evidence. A bounded inference call is used at the uncertain seam to confirm association, reject it, or mark ambiguity.

Entities are durable referents. Their aliases, metadata, semantics, relationships, and relevance may change, but loss of current relevance does not by itself delete the Entity. For example, an Entity representing Sarah remains Sarah even if Sarah later leaves an organization or disappears from active conversation.

An Entity may need multiple semantic centroids rather than one averaged representation when the same referent legitimately occurs across distinct contexts.

## Observation processing

After Entity resolution, Observation processing is defined over **propositional nodes**: Memories and Observations. Entities are durable referential/traversal nodes. They provide canonical identity, participant structure, and deterministic routing between propositions, but they do not themselves assert propositions and are not independent synthesis evidence.

Any newly created propositional node may become an Observation-processing anchor once it has completed its own required processing. A Memory becomes eligible after Dream organization and Entity resolution. A newly published Observation becomes eligible after required post-publication work such as receptor generation. Entity creation alone is not an Observation-processing trigger.

This allows higher-order derivation chains while preserving exact lineage:

    Memory -> Observation -> higher-order Observation

Potential reasoning cascades are bounded by candidate/context/inference budgets, duplicate admission, and verification. Hard cascade-depth limits are deferred until measured behavior justifies them.

Observation processing has three separate semantic call contracts:

1. **existing-Observation contribution/reconsideration input**;
2. **new ordinary-Observation synthesis**; and
3. **new relational-Observation synthesis**.

They share candidate infrastructure and provenance accounting but should not be collapsed into one overloaded model call.

### Existing-Observation contribution

Each eligible propositional anchor may be routed to existing Observations through high-recall receptors and exact structural indexes. Exhaustive anchor-by-all-Observations inference remains forbidden.

The contribution judgment is pairwise:

    propositional anchor P <-> existing Observation O

P may be a Memory or an Observation. When an Observation is the anchor, bounded exact support/derivation lineage may accompany it so the judgment can evaluate what the derived proposition rests on without treating transitive overlap as independent evidence.

The call decides support, challenge/contradiction, weakening, qualification, ambiguity, or irrelevance. Relevant results accumulate as mutations against the target Observation and may later trigger that target's semantic reconsideration. Reconsideration is target-lifecycle work rather than a new Observation-synthesis call.

Routing receptors and deterministic Entity/dependency/participant routes are candidate-generation metadata only. They are never evidentiary authority.

## New-Observation synthesis: Exploitation and Exploration

Exploitation and Exploration are orthogonal **discovery modes**. Either may derive an ordinary Observation or a relational Observation.

For configurable cadence C, Exploitation runs immediately for a new fully processed anchor and then every C; Exploration is phase-offset by C / 2 and then also runs every C. With a 30-day cadence, the intended shape is Exploitation at days 0/30/60 and Exploration at days 15/45/75. Missed epochs do not accumulate as backlog.

Lower-level Graph, Entity, Community, and metadata mutations do not recursively trigger synthesis. They change the structure visible at the next eligible anchor round.

### Bootstrap

When the complete authorized propositional Web fits the synthesis context budget, Perception may compare the whole Web. Community structure is therefore not a bootstrap prerequisite.

At larger scale, one processing round may emit multiple bounded comparison jobs. Candidate-generation budgets and model-context/inference budgets are distinct: mature Webs may expose more plausible candidates than can be hydrated or reasoned over in one round.

### Exploitation

Exploitation requires an **independently established reason for comparison**. Outside whole-Web bootstrap, embedding similarity alone is not enough.

Initial Exploitation lanes are:

- **Entity bridge** — traverse durable Entity references from the anchor to other propositional nodes referencing the same canonical Entity/set;
- **source neighbourhood** — same Episode, conversation, document, import unit, transcript region, or another concrete provenance-local region;
- **Web-structure / regional neighbourhood** — direct/short-path Dream/Graph structure plus coherent Community/subcommunity locality;
- **derivation neighbourhood** — accepted Observation support/derivation topology such as parents, children, or shared-support siblings.

Communities are therefore one regional structure inside the Web-structure lane, not the primary Observation-selection mechanism and not a hard inference boundary.

Generated candidates that lose the current comparison/context budget may enter a bounded Exploitation-overflow reservoir. They are not treated as permanently processed.

### Relational synthesis rules

Relational Observations use the same discovery modes but a specialized candidate/output contract.

Before relational synthesis:

- the anchor must reference at least one durable Entity;
- the candidate context must introduce at least one other distinct durable Entity; and
- relational Observations are excluded as evidence/anchors for **new relational-Observation synthesis**, although they may contribute to ordinary higher-order Observations and may themselves receive contribution/reconsideration evidence.

Publication requires at least two distinct canonical Entity participants. This is deterministically enforceable structure; whether the evidence actually establishes a meaningful relation remains semantic judgment.

A cheap Decision-style/Jev relatedness call may gate relational candidates before heavier synthesis. Its confidence is routing metadata only and cannot establish the relationship.

### Exploration

Exploration deliberately searches outside the strongest established comparison routes for emergent concepts.

Initial Exploration lanes are:

- **Exploitation overflow**;
- **semantic similarity**;
- **lexical similarity**;
- **cross-region / cross-Community sampling**; and
- **genuinely random sampling**.

Exploration does not use a permanent settled-pair ledger. Both Exploitation and Exploration record bounded recent comparison/context history. Exploration excludes or strongly deprioritizes candidates recently compared in either mode. Old history falls away so propositions may be reconsidered later under a changed Web.

This deliberately avoids using the entire theoretical/raw Exploitation-eligible pool as a permanent Exploration exclusion set. On mature Webs, even Exploitation candidate generation itself may need trimming; excluding every theoretically reachable candidate could make some comparisons permanently unreachable.

Weak exploratory contexts may use a cheap Decision-style/Jev relatedness probe before heavier synthesis. Probe output remains routing metadata.

### Multiple jobs and two-stage selection

A single anchor round may emit multiple narrow LLM comparison jobs rather than one mixed context. Initial lane-based selection only needs to reveal a plausible reasoning opportunity.

If a proposal call emits a candidate Observation, a second **reasoning-directed evidence-expansion** stage searches specifically for the support, contradiction, qualification, counterexamples, complementary premises, or competing explanations required by that candidate and reasoning mode.

Thus:

    fully processed propositional anchor
        -> Exploitation or Exploration candidate lanes
        -> one or more bounded discovery contexts
        -> candidate Observation proposal
        -> reasoning-directed evidence expansion
        -> reasoning/derivation
        -> independent verification
        -> publication or rejection

### Reasoning taxonomy

The intended derivation modes are:

- **deductive** — the proposition follows from the supplied premises under ordinary logical/general reasoning;
- **inductive** — repeated examples/evidence justify a scoped pattern or generalization;
- **abductive** — the proposition is a plausible explanation of the evidence and remains explicitly uncertain;
- **abstractive** — multiple grounded propositions are semantically consolidated/lifted into a useful higher-level representation without materially extending beyond what they already establish.

Abstractive reasoning is not a catch-all for cases that do not fit the other three. Its exact algorithm and boundary against generalized deduction/induction remain open calibration work; the system should abstain rather than force an unsupported derivation class.

Reasoning-specific evidence expansion and verification are required. A generic model-confidence score does not substitute for validating the actual derivation.

Evidence is many-to-many: one Memory or Observation may contribute to multiple Observations, and one Observation may depend on multiple propositional nodes. Exact support lineage must preserve transitive overlap so the same underlying source is not miscounted as independent corroboration.

## Perception pass 4: Observation routing-receptor generation

Every newly created Observation, and any Observation whose semantic content is materially rewritten, receives a separate receptor-generation pass.

The pass asks what kinds of future facts could materially:

- support the Observation;
- challenge or contradict it;
- qualify it; or
- otherwise indicate that its current interpretation should be reconsidered.

It emits multiple prospective-evidence descriptions. Each receptor is embedded separately and indexed back to the Observation. They must not be collapsed into a single centroid because evidence capable of bearing on one Observation may occupy very different semantic regions.

For example, an Observation such as "Sarah has hiring authority at the Vancouver office" might generate receptors covering hire approval, recruiting authorization, staffing-budget authority, role changes, departure from the organization, transfer of hiring authority, or another person assuming that authority.

Receptor generation should favor **recall over precision**. A false-positive receptor match costs one bounded contribution comparison; a false negative can hide relevant evidence from that Observation until another routing path exposes it.

The exact vector-search implementation is an indexing choice, not semantic authority. The architectural requirement is that routine contribution inference remain bounded rather than growing linearly with the total Observation population.

## Observation lifecycle and reconsideration

Observations are retained historical semantic objects. Wall time or lack of confirming evidence must not silently invalidate or delete them.

Normal Observation reconsideration may be triggered when relevant contribution mutations accumulate past a configured threshold.

Wall-time currentness is **policy-sensitive**, not necessarily universal. Observations whose propositions can silently become outdated may receive a wall-time reconsideration checkpoint. Durable/historical propositions, including some Relationship classes, may be exempt from staleness-by-time alone. Exact staleness applicability remains a class/temporal-semantics design decision and must not be conflated with Chronos valid-time closure.

For a wall-time-sensitive Observation, a checkpoint may deterministically inspect whether relevant mutations occurred since the Observation was last semantically considered:

- if **no** relevant mutation occurred, the policy may mark the Observation **stale** and move it out of priority/current consideration without inference;
- if **one or more** relevant mutations occurred, run semantic reconsideration inference.

The existence of a mutation only establishes that reconsideration is warranted. It does not imply that the Observation remains active/current. Reconsideration may still conclude that an Observation is stale because the new evidence is old, weak, irrelevant to current validity, or otherwise insufficient to maintain priority.

`stale` is a priority/currentness state, not a statement that the Observation is false. A stale, superseded, historical, or otherwise inactive Observation should normally be retained rather than removed. Actual contradiction, supersession, or invalidation requires supporting semantic evidence.

## Entity and Observation ambiguity

Perception may persist unresolved ambiguity for Entities and Observations rather than forcing an unsafe semantic decision.

The initial clarification mechanism is intentionally limited to these two Perception-owned ambiguity classes. It is not a general curiosity/open-question subsystem.

When a later user turn deterministically intersects an unresolved ambiguity, the runtime may inject an instruction-bearing Perception addendum into the conversational agent context. The addendum tells the agent not to assume the unresolved identity/interpretation and, when the current turn does not already resolve it, to seek the minimum clarification needed from the user.

The resulting answer returns to Perception for resolution or continued ambiguity. This permits Perception to discover an ambiguity after the original Memory was created and opportunistically resolve it the next time the subject becomes conversationally relevant.

## Consequences

The semantic responsibilities remain separated:

```text
Insomnia   = extract source-grounded Memory propositions and exact Entity-mention metadata
Dream      = organize the Memory Web
Chronos    = provide shared temporal interpretation when invoked
Perception = resolve durable referents and materialize derived Observation propositions
```

Perception contains one implemented Entity-processing subsystem plus several narrow Observation-processing call types:

1. Entity synthesis / association / disambiguation;
2. propositional-anchor-to-existing-Observation contribution;
3. new ordinary-Observation synthesis;
4. new relational-Observation synthesis; and
5. routing-receptor generation for new/materially rewritten Observations.

Exploitation and Exploration are scheduling/discovery modes used by the two synthesis call types rather than additional Observation classes. Either discovery mode may derive ordinary or relational Observations subject to their respective gates.

Observation reconsideration and ambiguity clarification are lifecycle/runtime mechanisms around those calls rather than additional full-population scans.

The design deliberately spends inference where semantic judgment is required while pushing candidate discovery, scheduling, recent-comparison suppression, mutation checks, receptor lookup, Entity traversal, source/Web/derivation routing, and clarification activation into deterministic machinery.

## Rejected alternatives

### Put Entity and Observation synthesis into Dream

Rejected. Dream's responsibility is Memory-to-Memory relationship/canonical organization. Perception consumes that organization for referential Entity resolution and derived Observation reasoning.

### Run Observation synthesis directly over every Memory before Dream

Rejected. Observation discovery benefits from Dream structure, durable Entity resolution, source organization, and existing Observation derivation topology. Memory creation alone is not the final Observation-processing boundary.

### Treat Entities as Observation evidence

Rejected. An Entity is durable identity/traversal structure, not a proposition. Entity references may route candidates and constrain relational participants, but some Memory/Observation proposition must carry the evidentiary claim.

### Compare every propositional anchor against every existing Observation

Rejected. Contribution cost would grow linearly with Observation population. High-recall receptors plus exact structural routes exist specifically to avoid a full scan.

### Use embedding similarity as ordinary Exploitation eligibility

Rejected. Exploitation must have an independently established reason for comparison outside whole-Web bootstrap. Semantic similarity is useful for Exploration or targeted evidence expansion, but using it as primary Exploitation selection would feed fuzzy semantic relevance back into itself.

### Make Communities the primary Observation selector

Rejected. Communities are valuable regional organization and can justify one Web-structure lane, but they can hide cross-domain emergent concepts and cannot define the boundary of what Perception may reason over.

### Use the entire raw/theoretical Exploitation pool as permanent Exploration exclusion

Rejected. On mature Webs, Exploitation candidate generation itself may need trimming and cannot guarantee eventual coverage of every structurally plausible candidate. Permanent exclusion would make some comparisons unreachable. Bounded recent comparison history creates novelty pressure without claiming complete Exploitation coverage.

### Maintain a permanent completed-pair ledger for Observation discovery

Rejected. Unlike Dream's pair classification, Observation usefulness can change as surrounding evidence and higher-order propositions accumulate. Perception retains only bounded recent comparison/context history.

### Collapse contribution, ordinary synthesis, and relational synthesis into one model call

Rejected. They ask materially different semantic questions and have different candidate/output constraints. Shared scheduling/candidate infrastructure does not justify overloading one inference contract.

### Allow relational Observations to recursively synthesize more relational Observations

Rejected. Relationship-class nodes may contribute to ordinary higher-order reasoning and receive reconsideration evidence, but relational-to-relational synthesis creates unnecessary recursive relational amplification. New relational synthesis is restricted to Memories and ordinary Observations.

### Use one routing-vector centroid per Observation

Rejected. Different forms of support, contradiction, qualification, and change may be mutually dissimilar even though each bears on the same Observation.

### Force Leiden to split until every context fits the inference budget

Rejected. A cohesive Community may have no meaningful semantic subdivision, and Community hierarchy is no longer the primary Observation-selection mechanism. Processing bounds belong to candidate/context budgets, not fabricated semantic partitions.

### Persist arbitrary processing partitions as Communities

Rejected. Temporary candidate contexts and evidence-expansion windows are consumer work units, not semantic Community authority.

### Treat Abstractive reasoning as an "other" bucket

Rejected. Abstractive derivation must have a narrow positive contract. If a candidate cannot be justified under a supported reasoning algorithm, the system should abstain rather than publish it through a catch-all label.

### Expire or delete old Observations automatically

Rejected. Time without new evidence does not establish falsity. Class-sensitive currentness/staleness policy may alter retrieval priority; semantic invalidation requires evidence.

## Open implementation questions

This ADR fixes the semantic boundaries and high-level processing shape but intentionally leaves the two major behavioral algorithms for dedicated design/calibration:

- **candidate selection:** exact algorithms and priority for the four Exploitation lanes and five Exploration lanes; candidate-generation caps; comparison-job grouping; context/inference budgets; overflow retention; recent-ledger depth/fingerprints; fairness/rotation; and budget spillover;
- **reasoning:** exact deductive, inductive, abductive, and abstractive derivation contracts; reasoning-directed evidence-expansion strategies; verification criteria; abstention; and the boundary preventing Abstractive from becoming a catch-all.

Additional implementation questions include:

- persisted schemas for Observations, relational-Observation profiles, receptor metadata, ambiguity state, recent comparison history, overflow state, and exact derivation/support sets;
- receptor-index implementation and retrieval limits;
- relational participant role/cardinality vocabulary and class-sensitive staleness applicability;
- Decision/Jev probe route, thresholds, and whether separate probes are needed for generic exploratory relevance vs relational relevance;
- Observation mutation accounting and reconsideration thresholds;
- model-route/capability naming for Perception;
- exact Chronos integration for Observation synthesis/reconsideration under ADR 0035; and
- exact convergence of the pre-amendment ADR 0034 Relationship persistence foundation with unified Observation persistence/routing.

## Verification

Implementation must protect at minimum:

- Insomnia metadata extraction without Entity/Observation semantic publication;
- durable Entity identity across metadata/relevance changes;
- Entity nodes used for identity/traversal rather than standalone proposition evidence;
- ambiguity preservation rather than forced Entity association;
- Memory and Observation anchors only after their own required processing completes;
- bounded receptor generation/indexing and propositional-anchor-to-existing-Observation contribution;
- separate existing-Observation contribution, ordinary synthesis, and relational synthesis contracts;
- immediate Exploitation and half-cadence-offset Exploration for fully processed propositional anchors;
- both modes recurring on the same per-mode cadence without accumulated missed-epoch debt;
- whole-Web bootstrap only while the authorized propositional Web fits the synthesis budget;
- Exploitation restricted to Entity/source/Web-structure/derivation justification outside bootstrap;
- Communities treated as one regional signal rather than the primary or hard inference boundary;
- Exploration lanes for Exploitation overflow, semantic similarity, lexical similarity, cross-region sampling, and random sampling;
- bounded recent comparison history across both modes and no permanent pair-completion ledger;
- no theoretical/raw Exploitation-eligibility blacklist that can permanently suppress Exploration;
- distinct candidate-generation vs hydrated context/inference budgets;
- multiple narrow comparison jobs permitted per anchor round;
- discovery selection followed by reasoning-directed evidence expansion;
- reasoning-specific derivation and independent verification rather than generic confidence;
- Abstractive reasoning not used as a fallback category;
- many-to-many evidence contribution across Memories and Observations with transitive-overlap accounting;
- at least two distinct canonical Entity participants for every relational Observation;
- no relational Observation used as evidence/anchor for new relational-Observation synthesis;
- exact Observation support/derivation lineage;
- no independent Relationship proposition/lifecycle authority outside Observation;
- Observation temporal interpretation delegated to shared Chronos rather than duplicated inside Perception;
- wall-time staleness/currentness applied only where the Observation's class/policy permits it;
- semantic reconsideration when contribution mutations cross the configured threshold; and
- deterministic clarification injection only for relevant unresolved Entity/Observation ambiguity.

.
