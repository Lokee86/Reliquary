# Working with Memories and Entities

Parent index: [Reliquary operator manual](INDEX.md)

## Purpose

Show how to create, revise, inspect, and connect the two currently durable Memory/Entity owners while preserving their different semantic roles.

## Overview

Memory is source-grounded **proposition state**. Entity is canonical **referential/traversal state** for a continuing identity. They share typed Graph/topology machinery, but an Entity does not itself assert a proposition or become standalone Observation evidence.

A stable identifier alone does not make something an Entity. Individual commits/revisions, builds, benchmark/test runs, snapshots, requests/responses, transactions, deployment instances, and one-off sessions normally remain in Memory/provenance.

Fine-grained implementation referents are also not promoted merely because they can be named precisely. A first mention of a function, field, constant, input action, minor file, UI control, small helper, or similar implementation detail normally stays as an extracted Memory mention plus `Pending(recurrence_required)`. A second compatible Memory can promote the referent; the earlier pending mention is then eligible to resolve to the same Entity. Repositories, continuing files/directories, services, components, people, projects, tools, and other identities that can recur and accumulate observations are appropriate Entity candidates.

## Publish a Memory

REL:

```text
Cva::publish_memory(id, expected_revision, draft)
```

PHY exposes the corresponding `publish_memory` surface with stricter provenance rules.

For a new Memory:

- `id` may be omitted.
- stable identity derives from the mutation identity.
- replaying the same mutation and draft is idempotent.

For an existing Memory:

- pass its stable ID.
- pass the exact current revision.
- a stale revision is a conflict, not an instruction to overwrite.

REL Memory drafts may carry REL-local source provenance. Direct PHY publication must not carry REL-local Episode/node/conversation provenance.

## Publish an Entity

REL and PHY expose:

```text
publish_entity(id, expected_revision, draft)
entity(id)
entities()
entity_version()
entity_stats()
```

For a new Entity, `id` may be omitted and is derived deterministically from the creation mutation ID. Later metadata revisions retain the same Entity ID and original creation timestamp.

Entity metadata currently includes canonical name, aliases, kind, summary, mutation identity, and revision/time bookkeeping.

## Candidate lookup

`entity_candidates_for_surface(surface, limit)` provides the exact canonical-name/alias lane through a derived one-to-many surface index. `entity_candidates_for_normalized_surface(surface, limit)` is a looser candidate-only lane that ignores separators/punctuation; it exists to surface lexical variants such as CamelCase versus spaced or hyphenated names, not to decide identity.

For a real Insomnia mention, use `entity_candidates_for_mention(key, config)`. It adds two bounded context lanes without making an identity decision:

- lexical-Memory context: search current owner-local Memory text and collect Entities already associated with those Memories;
- Dream-neighbour context: collect Entities attached to first-hop Memory neighbours in the Memory Graph projection.

Each returned candidate retains separate evidence flags/counts plus bounded supporting Memory IDs. Exact matches are ranked first, then normalized-surface variants, then corroborated lexical/structural evidence is used only to decide which candidates fit under the resolver cap. When the resolver positively confirms an existing Entity, the observed mention surface may be learned as an alias for future exact lookup.

Important rules:

- one surface may return multiple Entities;
- one Entity may have many aliases;
- exact or normalized lexical equality creates candidates only;
- contextual proximity creates candidates only;
- neither lexical equality nor Memory-Web proximity proves identity.

Entity centroid/vector routing is still deferred until measurement shows that the deterministic lanes leave meaningful retrieval misses.

## Associate a Memory with an Entity

Once identity is known:

```text
set_entity_association(
    memory_id,
    entity_id,
    active,
    expected_graph_version
)
```

Both endpoints must already exist in the same owner. The relation is directional `Memory -> Entity` and Perception-owned.

Use the full current `graph_version()` for optimistic publication.

Read associations with:

```text
entity_associations_for_memory(memory_id)
memories_for_entity(entity_id)
semantic_graph_relations()
```

## Revising vs replacing

Revise a Memory when the stable proposition identity remains the same and the API contract permits that metadata/body evolution.

Revise an Entity when the referent remains the same but its canonical name, aliases, kind, or summary changes.

Do not “fix” an identity mistake by mutating one Entity into a different referent. Merge/split semantics remain future Perception work.

## Unresolved mentions

An unresolved mention means:

> this particular mention does not yet have an Entity assignment.

It does **not** mean “create an unresolved Entity.”

The existing unresolved-state machinery should be treated conservatively until the real bootstrap/convergence loop supplies measured retry/retention behavior.

## Related docs

- [Rust API](../api.md)
- [Perception subsystem plan](../perception-subsystem-plan.md)
- [Graph and Communities](graph-and-communities.md)
- [Current limitations](../current-limitations.md)

## Notes

False Entity merges are more damaging than temporary duplicates or unresolved mentions. Candidate generation and identity judgment must remain separate.