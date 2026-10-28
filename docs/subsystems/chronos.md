# Chronos

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe the implemented shared temporal-semantics subsystem and the boundary between deterministic temporal interpretation, bounded inference, and consumer-owned semantic state.

## Overview

Chronos is Reliquary's shared deterministic-first temporal layer. It detects possible world-time material, normalizes bounded syntax, parses and resolves supported expressions, compares temporal products, and exposes unresolved residue for optional model assistance.

Chronos does not own Memories, Graph relationships, Entities, Observations, scheduling, or transaction history.

## Temporal axes

Reliquary distinguishes:

- **source/reference time** — when evidence originated and the reference for relative language;
- **valid time** — when a proposition or state applies in the world; and
- **transaction time** — when Reliquary committed state.

Chronos interprets source/reference-relative and valid-time semantics. Container/global publication history owns transaction time.

## Processing contract

```text
semantic text + authoritative reference chronology
  -> indication detection
  -> bounded normalization
  -> deterministic parse/resolution
  -> TemporalAssessment
       no material -> stop
       fully resolved -> return derived products
       unresolved residue -> deterministic candidate sets -> selection or abstention
```

`chronos::detect` preserves original indication spans even when parsing fails. `chronos::analyze` returns deterministic products. `chronos::assess` combines detection, analysis, resolution status, and the exact unresolved indications eligible for inference. `analyze_with_inference` accepts only already-verified inference results.

## Deterministic coverage

Current deterministic support includes absolute calendar expressions, relative day/week/month/year expressions, bounded/open ranges, recurrence, exact and approximate durations, event-relative offsets, meridiem-qualified times of day, selected season forms, and conservative loose-calendar syntax.

Ambiguous locale-dependent forms remain unresolved rather than inheriting machine locale. Bounded typo normalization preserves original evidence spans. Calendar month/year arithmetic uses calendar-aware clamping rather than fixed-duration substitution.

## Candidate selection

The lower `reliquary-chronos` crate enumerates supported alternatives for unresolved indications. `biweekly` offers every two weeks and twice weekly; `bimonthly` offers every two months and twice monthly. Distinct derived occurrence counts avoid inventing weekdays or month days. Unsupported expressions remain unresolved. A shared decision endpoint receives the original text, reference time, resolved evidence and locally enumerated choices. It may select an ID or abstain; Chronos reparses and revalidates every selection. Invalid, missing, extra or unknown answers fail closed.

Configured selection uses `models.general_decision`, then `models.general` through the constrained decision adapter. If neither exists, no decision is made. The former dedicated `models.chronos` route is rejected. Candidate sets and model responses are not stored.

Deterministic products remain derived. Only a consumer that needs a nondeterministic conclusion may persist it, bound to the exact semantic body and authoritative reference time.

## Consumers

### Insomnia

Prepared Memory drafts are assessed against authoritative `source_time_ns`. Deterministically resolved content requires no temporal model call. Verified nondeterministic conclusions are bound to the exact `MemoryBodyId` and reference time; replacement content invalidates stale inference.

### Dream

Dream uses Chronos for temporal candidate interpretation, overlap/matching, recurrence, and classifier context while retaining Memory-to-Memory relationship authority.

### Perception

Perception currently owns Observation semantics and is the future consumer for broader multi-evidence validity composition. Existing Chronos primitives may be consumed without making Chronos the Observation owner.

## Failure and recovery

- Missing reference time leaves reference-dependent forms unresolved.
- Model failure or invalid canonical output cannot overwrite deterministic results.
- Persisted inference is stale when its body/reference binding changes.
- Chronos never substitutes transaction time for semantic source/valid time.
- Deterministic caches, if introduced, are disposable optimization state only.

## Code Map

- `crates/reliquary-chronos/src/**` (temporal semantics and candidate validation)
- `src/chronos/**` (decision transport and Memory boundary)
- `src/runtime_inference/insomnia/*temporal*`
- `src/runtime_inference/configured_runtime*`
- `src/runtime_inference/model_switchboard*`

## Related docs

- [Architecture](../architecture.md)
- [Rust API](../api.md)
- [Perception](perception.md)
- [Dream](dream.md)
- [ADR 0035](../decisions/0035-chronos-shared-temporal-semantics.md)
- [Chronos future work](../chronos-subsystem-plan.md)

## Notes

The current implementation is broader than the original Dream-only temporal parser. Remaining work is kept in the future-only Chronos plan rather than mixed into this document.
