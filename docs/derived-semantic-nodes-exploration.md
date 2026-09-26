# Derived semantic nodes exploration

Parent index: [Documentation index](INDEX.md)

## Status

Exploratory design record — 2026-09-08. This is historical design input, not current implementation authority.

The terminology in this record predates the accepted Perception distinction. Where it loosely groups Entity and Observation as equivalent "semantic nodes," current ADRs supersede that wording: **Memories and Observations are propositional nodes; Entities are referential/traversal nodes that carry identity and routing structure but do not themselves assert propositions.**

## Overview

This retained exploration captured the first-class Entity/Observation idea before Perception was accepted. The accepted direction now lives in ADR 0033, ADR 0034, ADR 0036, and the Perception subsystem plan. Preserve this file as design history rather than retroactively treating its earlier terminology as current architecture.

## Purpose

Record the useful architectural idea identified while comparing Reliquary with Hindsight before the separate Honcho reasoning-model investigation.

The useful idea is not Hindsight's retrieval stack or its Observation implementation directly. It is the possibility that Reliquary may benefit from first-class semantic nodes beyond Memories: especially canonical Entities and synthesized Observations that can become high-value hubs in the Memory Web.

## Current baseline

Current Reliquary Graph authority is Memory-to-Memory. Memories already carry semantic authority, provenance, lifecycle, revision history, supersession, and owner-local Graph relationships. Dream discovers relationships among existing Memories and projects accepted Graph state into Memory lifecycle.

Nothing in this record implies that existing Memory or Graph semantics are deficient, or that the current Graph should immediately become a generic node store.

## Candidate semantic node classes

### Entity

An Entity represents a canonical referent shared by multiple Memories, for example a person, organization, project, component, technology, document, or other repeatedly referenced subject.

An Entity could become a semantic hub rather than requiring many pairwise Memory relationships around the same referent.

Entity identity is semantic state, not merely a retrieval cache. Incorrect merges can contaminate many downstream relationships, so canonicalization must be conservative and reversible enough to support later merge, split, alias, and correction operations.

### Observation

An Observation represents a synthesized proposition supported by one or more Memories but not necessarily stated by any single Memory.

Example:

```text
Memory A: Keep retrieval simple unless benchmarks prove otherwise.
Memory B: Complex multi-channel retrieval is not justified by current results.
Memory C: Add temporal search only where measured scale requires it.

Observation: Reliquary intentionally adds retrieval complexity only when measured failure modes justify it.
```

The source Memories are related but are not duplicates. Selecting one source Memory as the representative would lose the fact that the higher-level proposition is synthesized from several distinct pieces of evidence.

An Observation can therefore act as both a semantic statement and a graph hub for the evidence and concepts surrounding that statement.

## Semantic authority

Entities and Observations are not required to remain non-authoritative caches.

A semantic node may carry authority in its own right. The important distinction is how that authority was established, not whether the node originated from deterministic/user input or Dream synthesis.

Candidate origins include:

- `user`: explicitly created or edited by the user;
- `dream`: synthesized or canonicalized by Reliquary;
- future bounded system origins only if they have explicit publication and lifecycle semantics.

User authorship is required. A Knowledge Interface must be able to create and edit Observations and Entities and, where appropriate, perform explicit merge/split/alias/support operations. A user edit to a model-derived node should establish explicit user authority rather than remaining silently overwriteable by later model maintenance.

Exact authority kinds, lifecycle states, and mutation rules remain undecided.

## Derivation provenance

This is the strongest argument found so far for retaining derivation provenance.

For an ordinary Dream relationship between two existing semantic objects, persisting the model's original reasoning/evidence has no demonstrated operational requirement. The accepted Graph relationship is itself the semantic result, and current pair context can normally be reconstructed if the relationship is reconsidered.

A derived semantic node is different because the derivation creates a new semantic object.

For a Dream-created Observation:

```text
Memory A ----\
Memory B -----+--> Observation O
Memory C ----/
```

O's relationship to A/B/C is part of O's grounding and potentially part of its lifecycle. Retaining that lineage can support:

- determining whether O requires reconsideration when supporting evidence changes;
- exposing the evidence behind O in the Knowledge Interface;
- distinguishing independent corroboration from repeated or derivative evidence;
- merging or refining Observations without discarding their support history;
- recognizing that an Observation remains supported, becomes contradicted, or loses necessary support;
- rebuilding or re-evaluating synthesized knowledge without treating model-generated text as unexplained authority.

Entity derivation provenance has a related but different role. Evidence supporting entity identity, aliasing, or canonical merges can make later merge/split correction safer and more inspectable.

### Working principle

> Persist derivation provenance when a derivation creates a new semantic object whose meaning or lifecycle depends on its inputs. Do not persist model reasoning merely because a derivation discovered a relationship between already-existing semantic objects.

The provenance should describe sources/evidence and deterministic publication facts, not hidden chain-of-thought or unrestricted model reasoning traces.

## Derived does not mean non-authoritative

`derived` should describe origin and recomputability, not automatically imply weak semantic status.

A Dream-created Observation may be accepted semantic knowledge while still retaining source dependencies. A user-created Observation may have direct authority and no derivation set. A user-edited derived Observation may preserve historical derivation lineage while the current semantic statement becomes user-authored.

This requires keeping several concepts separate:

```text
node kind       Memory | Entity | Observation
origin          source/user | Dream | future bounded producer
authority       how the semantic claim is authorized
provenance      what source/evidence/derivation produced or supports it
lifecycle       active/current/superseded/etc. as later defined
```

Do not collapse these axes into one `derived` boolean.

## Graph architecture question

Current Reliquary Graph endpoints are deliberately `MemoryId -> MemoryId`. The reusable Arcana mechanics are generic, but Reliquary owns the semantic endpoint and relation vocabulary.

There are at least two implementation directions if this exploration survives validation:

1. extend Reliquary Graph authority to typed semantic-node identities; or
2. introduce a separate semantic-node owner/projection that composes with the Memory Graph without weakening existing Memory-to-Memory invariants.

No decision is made here. The existing Graph format must not be generalized simply because Arcana can represent generic topology.

Any persistent implementation must define:

- stable node identity;
- owner-local and cross-owner identity rules;
- derivation/support relationship ownership;
- user mutation authority;
- merge/split/retraction semantics;
- lifecycle interaction with supersession and canonicalization;
- invalidation/reconsideration rules;
- retrieval/traversal behavior;
- reconciliation and historical-version behavior; and
- whether Entities and Observations share one semantic owner or remain purpose-built owners.

## Potential graph value

The hypothesis is that Entities and Observations could become useful high-degree semantic hubs as a Memory Web grows.

Instead of dense repeated pairwise relations around a recurring referent:

```text
Memory A --- Memory B
    |          |
Memory C --- Memory D
```

a canonical Entity can express the shared referent:

```text
Memory A --\
Memory B ---+--> Entity E
Memory C ---+
Memory D --/
```

Likewise several distinct Memories can support an Observation without pretending they are duplicates or choosing an arbitrary representative Memory.

If this improves graph structure, traversal, inspection, or semantic compression, it could strengthen the existing Reliquary principle of making the knowledge substrate intelligent enough that retrieval can remain simple.

This remains a hypothesis and should be measured rather than assumed.

## Hindsight boundary

This exploration came from examining Hindsight's use of Entities and Observations. Reliquary should not copy Hindsight's architecture wholesale.

The useful extracted ideas are:

- canonical referents can function as semantic graph nodes;
- synthesized propositions can be first-class knowledge rather than being forced into duplicate/canonical-Memory semantics;
- source/support lineage becomes operationally meaningful when a new synthesized semantic object is created; and
- user authorship must coexist with model-derived state.

Hindsight does not establish that Reliquary needs its specific `proof_count`, consolidation, retrieval, or freshness mechanisms.

## Honcho boundary

The Honcho reasoning-model teardown has not yet been completed.

Honcho's explicit/deductive/inductive/abductive derivation model and premise-to-conclusion lineage may overlap with the provenance problem described here. Do not design a final generalized derivation schema until that investigation determines whether Observation support lineage and reasoning-premise lineage should share one primitive or remain distinct.

## Open questions

- Are Entity and Observation sufficient node classes, or are these examples of a more general typed semantic-node mechanism?
- Should a synthesized Observation be immutable like a Memory body, replaced by a new semantic identity when its proposition changes, or revised through its own explicit history model?
- Does evidence change mark an Observation dirty, reduce support, create a successor, or merely trigger bounded reconsideration?
- How should user edits interact with retained historical derivation lineage?
- What constitutes independent support rather than duplicate evidence?
- Should an Entity's evidence set be persisted, derived from incident relationships, or both?
- Are Entity merges semantic history that must be reversible through explicit split operations?
- Should retrieval target semantic hubs directly or continue to retrieve Memories and use hubs only during traversal?
- Does introducing hub nodes materially reduce graph density or improve retrieval/inspection on realistic Reliquary corpora?
- Can one provenance/derivation primitive serve Hindsight-style Observation grounding and Honcho-style reasoning lineage without collapsing different semantics?

## Guardrails

- Do not store hidden chain-of-thought as provenance.
- Do not generalize the current Graph endpoint format before node ownership and lifecycle semantics are decided.
- Do not treat model-derived state as immune to user correction.
- Do not add hub nodes merely to reduce edge count; require a semantic/product benefit.
- Do not import Hindsight retrieval complexity as part of this exploration.
- Preserve owner-local authority and explicit cross-owner identity rules.

## Next investigation

Complete the source-level Honcho reasoning-model teardown, then compare:

```text
Hindsight-inspired semantic node lineage
    source Memories -> Entity/Observation

Honcho reasoning lineage
    premises -> deduction -> induction -> abduction
```

Only then decide whether Reliquary needs a shared derivation-provenance primitive and where it belongs.

## Related docs

- [Perception subsystem plan](perception-subsystem-plan.md)
- [ADR 0033 — Perception Entities, Observations, and ambiguity](decisions/0033-perception-entities-observations-and-ambiguity.md)
- [ADR 0034 — cross-owner relational Observations and active-PHY privacy](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md)
- [ADR 0035 — Chronos shared temporal semantics](decisions/0035-chronos-shared-temporal-semantics.md)

## Notes

Retain this document for design provenance. Where it conflicts with the accepted Perception/Relationship decisions, the ADRs and current subsystem plan take precedence.