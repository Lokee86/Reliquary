# Chronos future work

Parent index: [Documentation index](INDEX.md)

Current implementation: [Chronos](subsystems/chronos.md) · Decision owner: [ADR 0035](decisions/0035-chronos-shared-temporal-semantics.md)

## Purpose

Track only unfinished Chronos work. Implemented parser, detector, normalization, assessment, verified inference, Dream integration, and Insomnia persistence live in the current subsystem documentation.

## Overview

Chronos is already the shared temporal-semantics layer. Remaining work is deliberately narrower:

1. complete Perception/Observation integration;
2. expand deterministic grammar only from measured unambiguous cases;
3. improve detector/fuzzy calibration where real false positives or misses justify it;
4. finish multi-evidence valid-time composition semantics; and
5. consider disposable caching only after measurement shows recomputation is material.

None of these items reopens Dream ownership, transaction-time ownership, or the deterministic-first inference contract.

## Current baseline

The current subsystem already provides:

- parser-independent temporal indication detection with exact source spans;
- bounded normalization and conservative typo handling;
- absolute, relative, recurrence, range/boundary, duration, event-relative, time-of-day, season, and selected loose-calendar analysis;
- `TemporalAssessment` with explicit unresolved residue;
- bounded inference only for unresolved indications;
- deterministic verification of every non-empty inferred canonical expression;
- Insomnia persistence of verified nondeterministic conclusions bound to exact Memory body/reference time; and
- Dream temporal matching through shared Chronos primitives.

See [Chronos](subsystems/chronos.md) for the exact current contract.

## C1 — Perception integration

Perception needs temporal interpretation for Observation synthesis and reconsideration without making Chronos the Observation owner.

Remaining work:

- define the Observation-facing input/output seam over already selected propositional evidence;
- distinguish source/reference time, claim-valid time, and transaction time in Observation evidence;
- compose multiple intervals/recurrences without manufacturing precision;
- surface conflict/ambiguity to Perception rather than forcing one answer;
- bind any persisted nondeterministic conclusion to the exact Observation/evidence identity that required it; and
- add focused tests proving that Perception lifecycle/reasoning remains outside Chronos.

### Exit

Perception can consume deterministic Chronos products and bounded fallback results while retaining all Observation authority.

## C2 — Measured grammar expansion

Do not add broad language coverage by intuition.

For each proposed grammar expansion:

1. collect concrete unresolved/misparsed examples;
2. label the safe deterministic interpretation;
3. prove locale/hemisphere/context assumptions are explicit;
4. add detector/parser tests;
5. measure false-positive changes on retained corpora; and
6. retain unresolved behavior when more than one plausible interpretation remains.

Priority candidates should come from production misses, not synthetic completeness goals.

## C3 — Detector and fuzzy calibration

Current correction is intentionally bounded.

Measure:

- indication recall;
- false positives in technical text;
- fuzzy correction collisions;
- contextual year/range/recurrence errors; and
- percentage of cases that unnecessarily invoke inference.

Any threshold/vocabulary change requires before/after corpus evidence.

## C4 — Multi-evidence validity composition

Perception will eventually need deterministic helpers for combining several temporal facts.

Define:

- compatible overlap;
- disjoint/conflicting intervals;
- recurrence compatibility;
- open-boundary composition;
- granularity preservation;
- uncertainty/approximation propagation; and
- abstention when composition would invent specificity.

This remains a derived interpretation API, not a persisted Chronos semantic object.

## C5 — Disposable cache decision

No cache is required by architecture.

Only add one if profiling shows material repeated deterministic work. Any cache must be rebuildable, body/reference keyed, semantically non-authoritative, and safe to drop on upgrade/reopen.

## Non-goals

- replacing Archive/global transaction chronology;
- owning Observation validity state;
- making temporal inference a generic semantic reasoning service;
- persisting cheap deterministic parse products by default;
- using wall time as a substitute for source/reference time; or
- broad locale guessing.

## Verification

Each completed item must update [Chronos](subsystems/chronos.md), API/behavioral documentation where public contracts change, and focused deterministic/inference/consumer tests.

## Related docs

- [Chronos current behavior](subsystems/chronos.md)
- [Perception future work](perception-subsystem-plan.md)
- [Architecture](architecture.md)
- [ADR 0035](decisions/0035-chronos-shared-temporal-semantics.md)

## Notes

Historical implementation sequencing was removed from this document. Git history retains it.
