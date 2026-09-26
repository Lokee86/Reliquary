# ADR 0033: Perception entities, observations, and ambiguity handling

Parent index: [Architectural decisions](INDEX.md)

Implementation plan: [Perception subsystem plan](../perception-subsystem-plan.md)

## Status

Accepted — 2026-09-08; relational-Observation model and Observation extrapolation scheduling/context architecture amended 2026-09-25. Entity pass 1 is implemented; the pre-amendment Relationship persistence foundation exists under ADR 0034; Observation/ambiguity synthesis remains staged.

Amends ADR 0012 by adding Insomnia metadata enrichment after Memory extraction, and ADR 0025 by requiring recursive Community subdivision for bounded Perception synthesis without turning processing windows into semantic Communities. ADR 0034 defines cross-owner relational semantics as a specialized Observation form over owner-qualified Entity references without changing Dream's owner-local Memory-Graph authority. ADR 0035 assigns Observation temporal interpretation to the shared Chronos subsystem rather than a Perception-local temporal stack.

## Context

Insomnia and Dream already have distinct semantic responsibilities. Insomnia extracts source-grounded Memories. Dream organizes those Memories by discovering durable semantic relationships, canonical/corroborating structure, lifecycle effects, and derived owner-local Communities.

A separate problem remains: some useful semantic structure is not itself a remembered proposition and is not merely a relationship between two Memories.

This ADR establishes two first-class Perception semantic object families:

- **Entities**: durable canonical referents such as a person, organization, place, project, or other thing that may be mentioned by many Memories under changing contexts; and
- **Observations**: contingent higher-order semantic propositions synthesized from semantic evidence, such as a recurring pattern, apparent authority, tendency, architectural relationship, social relationship, or other conclusion whose useful semantic structure is not exhausted by retaining the source Memories alone. Ordinary Observations normally combine multiple pieces of evidence; a relational Observation may also be justified by one explicit durable relational Memory when canonical participant/role structure itself is the additional semantic value.

ADR 0034 defines **Relationships as a specialized relational Observation form**, not a third semantic object family. A relational Observation carries structured owner-qualified Entity participants, optional participant roles, and optional relation classification because those fields improve routing, inspection, privacy composition, and validation. Its proposition, support lineage, lifecycle, reconsideration, ambiguity, temporal semantics, and authority remain ordinary Observation semantics.

Entities and Observations require different lifecycle rules. An Entity remains the same referent even when its metadata, relationships, or relevance change. An Observation is contingent knowledge whose current usefulness can become stale, whose support can change, and whose semantics may need reconsideration.

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

## Perception pass 2: Existing-Observation contribution

Each newly integrated Memory is checked for possible effect on existing Observations, but Perception must not compare it against every Observation.

Candidate Observation routing is driven primarily by pre-generated **routing receptors** attached to existing Observations. Each receptor describes one kind of future fact that could materially bear on its Observation and is embedded independently. New Memory vectors query the receptor-vector index to retrieve a bounded candidate Observation set. Deterministic Entity/dependency indexes may contribute additional candidates where exact structural routing exists.

The actual semantic judgment remains strictly **pairwise**:

```text
Memory M <-> Observation O
```

The pairwise pass decides whether the Memory materially supports, challenges/contradicts, weakens, qualifies, creates ambiguity around, or is irrelevant to the Observation. It does not extrapolate new Observations.

Routing receptors are candidate-generation metadata only. They are never evidentiary authority and do not themselves support an Observation.

## Perception pass 3: New-Observation extrapolation

New Observation discovery is independent of pass 2. A Memory may support an existing Observation and still combine with nearby knowledge to establish a distinct new Observation.

The ordinary discovery trigger is Memory processing itself. Reliquary does not recursively schedule new-Observation synthesis from lower-level Graph, Entity, Community, or Observation mutations. Those mutations change the semantic Web that future eligible Memory passes observe.

Pass 3 has two lanes:

- **Exploitation** runs immediately after a new Memory has passed Dream and Entity resolution, and may re-run when that Memory becomes eligible under the same provenance-anchored cooldown-epoch model used by Dream maintenance.
- **Exploration** is delayed: a new Memory is not explored immediately and first becomes eligible at the next processing/cooldown epoch. Later Exploration uses the same recurring epoch cadence.

Exploitation exists to reason from the strongest structure the Web already provides. Exploration exists to probe outside that structure so weak, cross-domain, or currently unrepresented connections are not permanently hidden by Community boundaries or Graph topology.

Exploration must not treat Memory pairs as permanently settled. It retains only bounded recent comparison/context history sufficient to discourage immediate repetition; older exploratory combinations may naturally become eligible again as surrounding evidence changes.

Observation extrapolation is bounded multi-Memory inference rather than pairwise Memory-to-Memory classification. Its initial context is derived from Dream's structural organization, but the reasoner may traverse beyond that starting context within an explicit budget while gathering evidence.

### Bootstrap and context scaling

Community hierarchy is not required to bootstrap Observation synthesis. When the relevant owner-local Web fits the synthesis budget, Exploitation simply processes the whole Web.

As the Web grows, context narrows hierarchically:

1. whole relevant Web when it fits;
2. otherwise the smallest genuine containing Community that fits;
3. otherwise a genuine containing subcommunity;
4. otherwise an ephemeral bounded neighbourhood around the anchor Memory.

Community structure is therefore a scaling mechanism, not a hard inference boundary. The reasoner may cross Community boundaries through bounded traversal when graph structure, shared Entities, lexical routes, semantic routes, or exploratory bridges indicate relevant evidence.

Exploration constructs separate bounded cross-Web comparison contexts. Random sampling is an explicit admissible routing signal, alongside diversity/novelty signals. A cheap Decision-style relatedness probe may reject low-value exploratory contexts or promote promising ones for wider evidence expansion. Probe confidence is routing metadata only and never semantic evidence.

### Recursive Community narrowing

ADR 0025's current single-level Community snapshot is insufficient as a direct Perception context boundary because a large Reliquary can contain an enormous Community.

Community structure therefore needs recursive semantic subdivision:

1. run the existing deterministic Leiden/community algorithm for the owner-local Graph;
2. recursively run Community detection inside a Community while Leiden finds meaningful subcommunities; and
3. stop when a Community no longer divides into genuine semantic subcommunities.

The hierarchy is dynamic rather than fixed-depth. Small graphs may need one level; large graphs may have several.

Leiden is not required to force a split. A tightly cohesive Community may remain semantically irreducible even when it is too large to place directly into an inference context.

### Oversized irreducible Communities

Reliquary must **not** arbitrarily chop an irreducible semantic Community into durable fake subcommunities merely to satisfy an inference budget.

When the deepest genuine Community containing the new/changed Memory is still too large, Perception constructs an ephemeral bounded processing neighbourhood around that Memory. The neighbourhood may use deterministic graph proximity, strongest relationships, shared Entities, lexical locality from the owner-local Memory lexical index, and other bounded structural signals.

That neighbourhood is a processing window only. It is not persisted as a semantic Community and does not claim semantic authority.

Observation extrapolation then uses the bounded multi-Memory starting context to propose candidate higher-order propositions that are not explicitly represented by an individual Memory.

Formation is staged: candidate proposal, bounded evidence expansion, reasoning/derivation, independent verification, then publication. Evidence expansion may traverse the authorized Web beyond the initial context when needed. Reasoning may use synthesis/abstraction, deduction, induction, or bounded hypothesis/abduction, with reasoning-specific validation rather than one generic confidence threshold.

Evidence is many-to-many: one Memory may materially contribute to multiple Observations, and one Observation may depend on multiple semantic objects. Any created Observation retains exact typed support/derivation lineage back to the semantic objects that established it.

## Perception pass 4: Observation routing-receptor generation

Every newly created Observation, and any Observation whose semantic content is materially rewritten, receives a separate receptor-generation pass.

The pass asks what kinds of future facts could materially:

- support the Observation;
- challenge or contradict it;
- qualify it; or
- otherwise indicate that its current interpretation should be reconsidered.

It emits multiple prospective-evidence descriptions. Each receptor is embedded separately and indexed back to the Observation. They must not be collapsed into a single centroid because evidence capable of bearing on one Observation may occupy very different semantic regions.

For example, an Observation such as "Sarah has hiring authority at the Vancouver office" might generate receptors covering hire approval, recruiting authorization, staffing-budget authority, role changes, departure from the organization, transfer of hiring authority, or another person assuming that authority.

Receptor generation should favor **recall over precision**. A false-positive receptor match costs a bounded pairwise comparison in pass 2; a false negative can permanently hide relevant evidence from that Observation.

The exact vector-search implementation is an indexing choice, not semantic authority. The architectural requirement is that routine contribution inference remain bounded rather than growing linearly with the total Observation population.

## Observation lifecycle and reconsideration

Observations are retained historical semantic objects. Wall time or lack of confirming evidence must not silently invalidate or delete them.

Normal Observation reconsideration may be triggered when relevant mutations accumulate past a configured threshold.

Observations also have a wall-time reconsideration checkpoint. At that checkpoint, deterministic machinery inspects whether any relevant mutations occurred since the Observation was last semantically considered:

- if **no** relevant mutation occurred, mark the Observation **stale** and move it out of priority consideration without inference;
- if **one or more** relevant mutations occurred, run semantic reconsideration inference.

The existence of a mutation only establishes that reconsideration is warranted. It does not imply that the Observation remains active. Reconsideration may still conclude that an Observation is stale because the new evidence is old, weak, irrelevant to current validity, or otherwise insufficient to maintain priority.

`stale` is a priority/lifecycle state, not a statement that the Observation is false. A stale, superseded, or otherwise inactive Observation should normally be archived/retained rather than removed. Actual contradiction, supersession, or invalidation requires supporting semantic evidence.

## Entity and Observation ambiguity

Perception may persist unresolved ambiguity for Entities and Observations rather than forcing an unsafe semantic decision.

The initial clarification mechanism is intentionally limited to these two Perception-owned ambiguity classes. It is not a general curiosity/open-question subsystem.

When a later user turn deterministically intersects an unresolved ambiguity, the runtime may inject an instruction-bearing Perception addendum into the conversational agent context. The addendum tells the agent not to assume the unresolved identity/interpretation and, when the current turn does not already resolve it, to seek the minimum clarification needed from the user.

The resulting answer returns to Perception for resolution or continued ambiguity. This permits Perception to discover an ambiguity after the original Memory was created and opportunistically resolve it the next time the subject becomes conversationally relevant.

## Consequences

The semantic responsibilities remain separated:

```text
Insomnia   = extract source-grounded Memories and exact Entity-mention metadata
Dream      = organize the Memory Web
Chronos    = provide shared temporal interpretation when invoked
Perception = materialize implicit Entities and Observations
```

Perception has four routine inference passes:

1. Entity synthesis / association / disambiguation;
2. pairwise Memory-to-existing-Observation contribution;
3. bounded multi-Memory new-Observation extrapolation; and
4. routing-receptor generation for new/materially rewritten Observations.

Observation reconsideration and ambiguity clarification are lifecycle/runtime mechanisms around those passes rather than additional routine full-population scans.

The design deliberately spends inference where semantic judgment is required while pushing candidate discovery, scheduling, mutation checks, Community narrowing, receptor lookup, and clarification activation into deterministic machinery.

## Rejected alternatives

### Put Entity and Observation synthesis into Dream

Rejected. Dream's existing responsibility is relationship/canonical Memory-Web organization. Perception depends on that work and materializes a different class of semantic structure.

### Run Perception directly over every Memory before Dream

Rejected for Observation synthesis. Observation extrapolation benefits from the graph, canonical structure, and Communities Dream has already established. Only bounded Entity-mention extraction belongs directly after Insomnia; lexical locality is derived deterministically from Memory text.

### Compare every new Memory against every existing Observation

Rejected. Inference cost grows linearly with Observation population and becomes unsuitable for large Project/Organization Reliquaries. Receptor-vector and deterministic structural routing exist specifically to avoid this full scan.

### Use Observation-text embedding similarity as the only contribution route

Rejected. A fact can materially support or challenge an Observation while being lexically and semantically distant from the Observation's wording. Prospective-evidence receptors expand the searchable semantic surfaces of an Observation before future evidence arrives.

### Use one routing-vector centroid per Observation

Rejected. Different forms of support, contradiction, qualification, and change may be mutually dissimilar even though each bears on the same Observation.

### Force Leiden to split until every leaf fits the inference budget

Rejected. A cohesive Community may have no meaningful semantic subdivision. Forcing a partition would create false semantic boundaries. Oversized irreducible Communities use ephemeral bounded processing neighbourhoods instead.

### Arbitrarily persist processing partitions beneath oversized Communities

Rejected. Processing bounds are consumer concerns, not semantic Community authority. Temporary local neighbourhoods may overlap and change without creating new Community identity.

### Expire or delete old Observations automatically

Rejected. Time without new evidence does not establish falsity. Wall-time maintenance controls priority; semantic invalidation requires evidence.

## Open implementation questions

This ADR fixes the semantic boundaries and processing shape but intentionally leaves several mechanics for implementation work:

- persisted schemas for Observations, relational-Observation profiles, receptor metadata, ambiguity state, and exact derivation/support sets; Entity persistence is already implemented;
- receptor-index implementation and retrieval limits;
- Community-recursion thresholds and the exact bounded-neighbourhood growth policy;
- Exploitation/Exploration budgets, Exploration sampling/diversity metrics, Decision-probe model/threshold, and bounded recent-history depth;
- reasoning-mode algorithms and verification acceptance criteria;
- Observation mutation accounting and wall-time intervals;
- model-route/capability naming for Perception and Insomnia metadata enrichment;
- exact Chronos integration for Observation synthesis/reconsideration under ADR 0035; and
- exact convergence of the pre-amendment ADR 0034 Relationship persistence foundation with unified Observation persistence/routing. ADR 0034 preserves owner-qualified participant visibility/composition rules but no longer defines independent Relationship semantic authority and does not create cross-owner Memory-Graph authority.

## Verification

Implementation must protect at minimum:

- Insomnia metadata extraction without Entity/Observation semantic publication;
- durable Entity identity across metadata/relevance changes;
- ambiguity preservation rather than forced Entity association;
- receptor generation/indexing and bounded candidate retrieval;
- strict pairwise contribution inference;
- new-Observation extrapolation independent of existing-Observation support;
- immediate Exploitation for newly integrated Memories and delayed first-epoch Exploration;
- provenance-anchored recurring eligibility for both lanes without accumulated missed-epoch debt;
- no permanent Exploration settled-pair ledger; bounded recent context history only;
- whole-Web bootstrap when the relevant Web fits the synthesis budget;
- recursive Community subdivision without fabricated semantic children;
- Community boundaries used for scaling rather than as hard inference boundaries;
- bounded traversal beyond the initial context when evidence warrants it;
- bounded ephemeral neighbourhoods for oversized irreducible Communities;
- staged proposal, evidence expansion, reasoning/derivation, verification, and publication;
- many-to-many evidence contribution across Memories and Observations;
- exact Observation support/derivation lineage, including relational Observations;
- no independent Relationship proposition/lifecycle authority outside Observation;
- Observation temporal interpretation delegated to shared Chronos rather than duplicated inside Perception;
- deterministic wall-time stale transition when no relevant mutation exists;
- semantic reconsideration when a relevant mutation exists; and
- deterministic clarification injection only for relevant unresolved Entity/Observation ambiguity.
