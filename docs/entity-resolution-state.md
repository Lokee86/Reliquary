# Entity mention resolution state

Parent index: [Documentation index](INDEX.md)

## Purpose

Describe the implemented per-mention Entity-resolution state owner, deterministic lifecycle rules, and how the production-capable Entity processor uses that state.

## Overview

Entity resolution is tracked per extracted Memory mention. A Memory may therefore contain any mixture of resolved, unresolved, and rejected Entity mentions.

An Entity is a **continuing identity**, not every uniquely referable occurrence. Admission requires a referent that can reasonably recur across Memories and accumulate state, observations, or relationships over time.\n\nAdmission also distinguishes **immediate semantic identities** from **recurrence-gated implementation identities**. Projects, repositories, tools, services, major subsystems, major domain identities, and clearly architectural components may be promoted on first mention. Fine-grained implementation artifacts—functions/methods, fields, constants, enum members, config keys, input actions, ordinary code types, minor files/directories, individual UI controls/nodes/effects, small helper components, and similar details—normally remain `Pending` with reason `recurrence_required` until another compatible Memory demonstrates that the same identity is actually useful across Memories. Immutable historical occurrences and one-off instances—such as individual commits/revisions, builds, benchmark/test runs, snapshots, requests/responses, transactions, deployment instances, or session instances—remain in Memory/provenance rather than becoming durable Entity nodes by default.

The persistence/retry machinery is used by `resolve_entity_mention(...)` and the production `EntityResolutionEngine`. Zero-candidate mentions use the existing generative Admission path because that call also materializes the new Entity metadata. Non-empty candidate sets first pass through a deterministic transient-occurrence gate and then the bounded decision endpoint; only a high-confidence existing-candidate match is terminal there. All other choices, low-confidence results, malformed responses, and decision-endpoint failures fall through to the existing generative V4 resolver. The processor then deterministically persists Entity creation/reuse, `Memory -> Entity` association, and mention state. The frozen 41-query zero-Entity bootstrap converges to its complete expected state and survives reopen. The long-lived runtime host schedules this Entity pass automatically after Dream for both REL and attached PHY.

## Ownership boundary

Insomnia owns mention extraction through `MemoryRoutingMetadata.entity_mentions`.

Entity resolution never rewrites, combines, or expands those spans. Each resolution record is keyed by the exact Memory ID, field, and byte range of one extracted mention.

Candidate generation has two surface lanes. The exact lane uses case-insensitive canonical-name/alias equality. A second candidate-only normalized lane removes separators/punctuation so surfaces such as `PlayerHuePresenter`, `Player Hue Presenter`, and `player-hue-presenter` can be compared by the resolver. Normalized equality never merges Entities and does not prove identity. Confirmed existing-Entity resolutions may learn the mention surface as an alias so future mentions can use the exact lane.

Unresolved mentions are **not Entities** and do not receive an `EntityId`.

Current resolution state must reference current Entity identities. During reopen, after the rest of owner rebuild/validation succeeds, a resolution reference to a known retired Entity is deterministically canonicalized to its active replacement and the repaired revision is persisted before strict resolution-reference validation. Pending/Dormant candidate-set fingerprints are cleared when this happens so later retry preparation cannot treat stale candidate evidence as unchanged. A reference that is neither active nor traceable through the retired-Entity replacement chain still fails closed.

Entity audits include resolved targets and Pending/Dormant candidate targets; either class pointing at a non-current Entity is a finding.

## States

- `Resolved(EntityId)` — terminal durable association.
- `Rejected` — terminal while the source Memory remains active.
- `Pending` — unresolved and active for reconsideration.
- `Dormant` — unresolved but removed from the active retry set.

`create_new` is not a durable resolution state. After an Entity is actually created, the mention is persisted as `Resolved(new_entity_id)`.

When a recurrence-gated mention later appears in another compatible Memory, Admission may create the Entity. The earlier pending mention then sees a changed candidate/context fingerprint and can resolve to the new Entity, retroactively associating the earlier Memory without introducing a provisional Entity node.\n\nA repeated unresolved result with unchanged candidate/context fingerprints is idempotent. The runtime may receive a wake event, but it rebuilds the candidate/evidence state first and makes no model call, appends no record, and does not increase the attempt count when both hashes are unchanged. The persisted field names remain `candidate_set_fingerprint` and `context_fingerprint`; semantically they are hashes of candidate identity/state and the bounded resolution evidence supplied to Admission/V4.

## Retention

Pending state is deliberately bounded:

- after 30 days without a changed-evidence retry, Pending becomes Dormant;
- after 180 days dormant, the resolution state is tombstoned and removed from active state;
- pending/dormant state is purged immediately when its source Memory is archived or superseded;
- rejected state is also purged when its source Memory becomes inactive;
- resolved associations remain durable.

The original extracted mention stays on the Memory after unresolved state is purged, so later event-driven discovery can reconsider it without retaining an unresolved pseudo-Entity indefinitely.

The REL/PHY container is append-only, so old state records remain historical bytes until a migration/repack rewrites the container. They are not active semantic objects after a tombstone.

## Retry contract

`resolve_entity_mention(...)` calls the retry policy before model resolution. An absent resolution record is eligible. Pending or dormant state is eligible only when the candidate-set fingerprint or relevant-context fingerprint changed. Resolved and rejected mentions are not eligible. Admission context participates in the relevant-context fingerprint, so changed same-surface evidence can make a previously unresolved zero-candidate mention eligible again. Candidate-attached Memory evidence likewise participates, so a new association/evidence Memory on a previously considered candidate can make that mention eligible.

The resolution-state owner itself does not invoke a model; it stores and deterministically governs lifecycle. The Perception runtime scheduler owns *when* mention keys are reconsidered. Its wake sources are bounded deterministic events: post-Dream source/affected Memories, same-surface mention evidence, and changes to evidence associated with previously considered candidate Entities. The scheduler then prepares a fresh snapshot. Obvious historical/transient occurrences are rejected before either decision endpoint or generative fallback. With existing candidates, the configured decision endpoint may resolve an existing Entity only when the selected candidate probability is at least 0.95 and exceeds the second-highest choice by at least 0.15; `create_new`, `unresolved`, and `reject` are never terminal decision-endpoint outcomes. Otherwise the engine invokes the generative fallback. Decision-endpoint errors also fail open to that fallback rather than deferring resolution. Commit still occurs only if the durable owner identity and Memory/Entity/Graph/resolution versions match the prepared snapshot.

## Configured decision inheritance

Runtime composition selects `entity_resolution_decision`, then `general_decision`, then adapted `general`. The engine remains unaware of the selected tier. The General adapter rejects probability fields because text completions do not establish calibrated likelihoods; its choices therefore proceed through the existing generative resolver. This operation-level fallback is independent of switchboard selection and does not retry another decision tier. Hosted processing still requires the separate `entity_resolution` resolver route.

## Related docs

- [Perception subsystem plan](perception-subsystem-plan.md)
- [Architecture](architecture.md)
- [Rust API](api.md)
- [Manual: Working with Memories and Entities](manual/memories-and-entities.md)

## Notes

The zero-Entity bootstrap loop, continuing-identity Admission v4 contract with recurrence-gated promotion, normalized-surface candidate lane, learned aliases, automatic runtime scheduling, and bounded decision fast path are implemented. The switchboard keeps `entity_resolution_decision` separate from the generative `entity_resolution` fallback; this allows Jev-style decision APIs without pretending they are chat-completion endpoints. Pending→Dormant and Dormant→tombstone timing remains lifecycle housekeeping only; wall time does not itself trigger another model attempt. Reconsideration is evidence/event driven.