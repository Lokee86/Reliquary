# Chronos subsystem plan

Parent index: [Documentation index](INDEX.md)

Decision owner: [ADR 0035](decisions/0035-chronos-shared-temporal-semantics.md)

## Status

Shared-core extraction, parser-independent indication/normalization, the deterministic grammar baseline, explicit resolution status, bounded unresolved-only inference, and Insomnia persistence integration are implemented. Insomnia assesses prepared Memory drafts against authoritative `source_time_ns`, calls the temporal model only for unresolved residue, deterministically verifies every non-empty canonical answer, and persists only verified nondeterministic conclusions. The first detector calibration pass is complete against frozen 14/28-day REL/PHY Memories plus LongMemEval temporal-reasoning answer sessions; Perception integration and measured grammar expansion remain future work.

Chronos owns the shared temporal model, detector, bounded vocabulary normalizer, parser, calendar resolution, recurrence extraction, matcher, and resolution assessment under `chronos*`. `chronos::detect` preserves original indication spans even when parsing fails; `chronos::analyze` returns deterministic products; `chronos::assess` returns detection + analysis + `TemporalResolutionStatus::{NoTemporalMaterial,FullyResolved,Unresolved}` and the exact unresolved indications eligible for bounded inference. Dream preserves its existing public behavior through a thin Memory/source-time adapter and compatibility aliases for the former `DreamTemporal*` public type names.

## Overview

Chronos extracts the existing Dream temporal machinery into one shared deterministic-first temporal layer, then expands its coverage for Insomnia, Dream, and Perception without creating another semantic owner or temporal stack.

## Purpose

Chronos is Reliquary's shared temporal-semantics subsystem. It provides deterministic-first temporal detection, normalization, parsing, resolution, comparison, and valid-time interpretation to Insomnia, Dream, and Perception. Bounded inference handles only temporal meaning unresolved after deterministic analysis.

Chronos owns temporal interpretation mechanics, not Memories, Graph relationships, Entities, Observations, scheduling, or REL/PHY transaction history.

## Temporal axes

```text
source/reference time  when evidence originated; resolves relative language
valid time             when the proposition/state applies in the world
transaction time       when the REL/PHY committed the state
```

Chronos handles source/reference-time resolution and valid-time interpretation. Transaction/knowledge time comes from timestamped global/container versions.

## Shared processing shape

```text
semantic unit + reference chronology
    -> indication detector
    -> bounded normalization
    -> deterministic parser/resolver
    -> temporal assessment
         -> no temporal material: stop
         -> fully resolved: return derived result
         -> unresolved residue + relevant: bounded inference fallback
```

The semantic unit differs by consumer; temporal primitives remain shared.

## Deterministic modules

### Indication detector

High-recall gate for possible world-time information. Detect at least:

- explicit dates/times, months, weekdays, years, seasons, quarters;
- relative tokens and numeric/word-number relative constructions;
- range/boundary language;
- recurrence/frequency and duration language; and
- bounded typo variants of known temporal vocabulary.

A positive indication means interpretation may be needed; it does not assert a temporal fact.

### Normalizer

Build a transient detection view without changing authoritative text. Support deterministic format normalization, word-number recognition, bounded fuzzy matching against temporal vocabulary, and original-span/evidence preservation. Do not perform broad spelling correction.

### Parser/resolver

Generalize the current Dream absolute/relative/recurrence parser. Expand coverage for:

- numeric and word-number days/weeks/months/years relative to reference time;
- explicit ranges and open boundaries;
- recurrence and durations;
- common calendar periods; and
- safe loose calendar forms with one deterministic interpretation.

Resolve against authoritative source/reference chronology. Never substitute Memory creation/import bookkeeping when semantic source time is required.

### Matcher/comparator and validity

Provide deterministic overlap, ordering, recurrence identity, compatibility, and proposition-level valid-time interpretation where syntax and semantic-unit structure make that safe.

The valid-time representation now includes `TemporalInterval` with independently optional start/end bounds and per-bound granularity. Existing bounded `Range` anchors remain as compatibility projections for Dream. Exact standalone durations are represented separately as `TemporalDuration { amount, unit, evidence }`; explicit numeric duration ranges use `TemporalDurationRange`; approximate numeric and qualitative duration language uses `TemporalApproximateDuration` without fabricated bounds; exact event-relative offsets use `TemporalEventRelation` to preserve offset, direction, and textual event referent without inventing an event timestamp; meridiem-qualified standalone clock expressions are represented as `TemporalTimeOfDay` values without inventing a calendar date; and recurrence remains a separate pattern model. Add uncertainty only when actual semantics require it.

Current Dream temporal-match behavior is now implemented behind this seam with compatibility tests.

## Inference fallback

Run only when all are true:

1. temporal indications exist;
2. the consumer needs a temporal answer; and
3. deterministic Chronos cannot safely resolve it.

The model receives original semantic state, reference chronology, deterministic evidence/candidates, and only the narrow unresolved indications. It does not regenerate deterministic anchors already known. Each answer is a standalone canonical temporal expression or an explicit abstention. Chronos reparses every non-empty answer deterministically and rejects it unless it is fully resolvable as the same temporal indication kind as the source residue.

## Consumer contracts

### Insomnia

Insomnia consumes Chronos during Memory processing for newly extracted, imported, user-created, or otherwise newly introduced bodies, plus replacement/correction bodies whose previous temporal inference no longer applies.

```text
Memory body
    -> Chronos deterministic analysis
    -> no indication: stop
    -> fully resolved: continue without model call
    -> unresolved: bounded Chronos inference
```

Accepted inference is bound to the exact `MemoryBodyId` plus authoritative `source_time_ns`. Identical mutation replay must preserve the same explicit inference; a conflicting inference under the same mutation identity is rejected. Lifecycle-only revisions retain inference while that binding still matches; a changed body/reference binding makes the prior inference stale and it is not carried forward.

### Dream

Dream consumes Chronos for temporal candidate discovery, source-relative content-time interpretation, anchor/range overlap, recurrence matching, temporal ordering, and pair-classification context.

Current Dream temporal behavior remains compatible with parser/matcher ownership now in Chronos.

### Perception

Perception consumes Chronos after selecting bounded evidence for Observation synthesis or reconsideration. Chronos may compare supporting Memory temporal evidence, derive Observation validity, expose temporal conflict/ambiguity, or invoke bounded fallback inference.

Perception still owns the Observation being synthesized.

**Memory Freshness design update:** The previously proposed mandatory stale-Memory reinforcement audit and its Chronos event-window gate have been withdrawn. Memory relevance now follows deterministic REL activity and graph events under [Memory Freshness](memory-staleness-plan.md). Chronos still distinguishes source/event, transaction/import and claim-valid times for Memory interpretation and Observation synthesis; none of these times is a Freshness decay clock.

## Persistence policy

Deterministic Chronos products remain derived by default. Do not persist parsed anchors, normalized typos, recurrence patterns, or deterministic intervals merely to avoid cheap recomputation.

Persist only non-deterministically inferred temporal conclusions that a durable consumer actually needs. Memory-owned inference persists the model/contract, original source span/kind/evidence, deterministic-verifiable canonical expression, `MemoryBodyId`, and authoritative `source_time_ns`. The body/reference binding is the staleness key; deterministic anchors/intervals/durations/patterns remain recomputed derived state.

A deterministic cache is permitted as disposable optimization state, never semantic authority.

## Implementation sequence

### A — Shared core — implemented

- `chronos::analyze` is the shared deterministic analysis boundary.
- The former Dream temporal model/parser/calendar/relative/recurrence/matcher modules now live under Chronos ownership.
- Generic `Temporal*` types are canonical; existing `DreamTemporal*` names remain compatibility re-exports.
- Dream resolves authoritative Memory source time and delegates analysis/matching to Chronos.
- Existing Dream temporal tests remain green, with direct Chronos contract tests added.

### B — Detection and deterministic coverage — baseline implemented

- Parser-independent `chronos::detect` now reports explicit/calendar/relative/boundary/recurrence/contextual-duration indications with original byte spans.
- Bounded one-edit/transposition correction now covers selected temporal vocabulary with contextual guards; normalized tokens are transient and corrected parse evidence maps back to the exact original span.
- Numeric and English word-number relative day/week/month/year offsets are implemented through ninety-nine for word forms; month/year shifts use calendar-aware end-of-month clamping.
- Bounded `from … to/through/until …` ranges and open `since`/`until`/`before`/`after`/`starting`/`ending` boundaries are implemented as derived `TemporalInterval` values; bounded intervals also retain compatibility `Range` anchors.
- Boundary endpoints reuse deterministic explicit/relative parsing and support reference-bound bare months/weekdays, including cross-year month ranges.
- Exact standalone durations are implemented for bounded numeric/word-number quantities across seconds/minutes/hours/days/weeks/months/quarters/years without converting calendar units to fixed nanoseconds.
- Cue-bound numeric duration ranges such as `took 10-15 minutes` / `lasted three to five hours` preserve both bounds, while approximate forms such as `about two weeks`, `a few hours`, `a couple of days`, and `several hours` preserve their uncertainty explicitly instead of inventing exact values.
- Exact event-relative offsets such as `30 minutes before cooking` and `two days after the production deployment` are implemented as derived `TemporalEventRelation` values. The event referent remains text for a later semantic owner to resolve; calendar endpoints such as `before Tuesday` remain ordinary Chronos boundary semantics.
- Meridiem-qualified standalone clock expressions such as `6:45 AM` and `6:45:30 pm` are implemented as derived `TemporalTimeOfDay` values with minute/second precision. They are not coerced into an absolute instant from source time. Bare colon-number forms remain unresolved pending precision calibration against technical notation.
- Recurrence now supports word-number intervals and explicit alternation such as `every three weeks`, `every other Tuesday`, and `every other month`, while preserving specific `every month/year on …` precedence. Ambiguous cadence words such as `biweekly` remain parser-unresolved but are detected so later bounded inference can handle them.
- Hemisphere-qualified named seasons are implemented as meteorological three-month `Season` anchors. Northern/southern hemisphere must be explicit; unqualified season language remains detected but unresolved rather than inheriting a machine/user locale assumption.
- Safe loose-calendar parsing now covers common English month abbreviations with explicit years, ordinal `day of Month YYYY`, reference-bound `this|next|last Month`, and `Month day this|next|last year`. Reference-bound forms require authoritative source/reference time.
- Locale-ambiguous numeric dates, unqualified seasons, approximate periods, and other forms without one safe deterministic interpretation remain indication-only for bounded fallback.
- The first detector calibration pass is complete: contextual boundary/recurrence/calendar guards, contextual standalone-year detection, and adjacency-aware fuzzy correction were tuned against frozen REL/PHY Memories and labelled LongMemEval temporal-reasoning answer sessions. Further vocabulary/grammar expansion remains measurement-driven.

### C — Insomnia integration — implemented baseline

- Prepared Insomnia Memory drafts are assessed through `chronos::assess` using authoritative `source_time_ns`; deterministic Chronos products remain unpersisted and do not overwrite Insomnia's category/type/lifecycle metadata contract.
- `TemporalInferencer` is invoked only when `TemporalResolution::needs_inference()` is true and receives only unresolved indications plus already-resolved deterministic evidence.
- Model results are canonical expressions or abstentions. Every non-empty expression is reparsed by deterministic Chronos and must resolve as the same temporal indication kind before publication.
- Verified nondeterministic conclusions persist in the Memory record as `MemoryTemporalInference`, bound to exact body identity plus reference time. REL reconciliation and PHY migration preserve that state.
- RuntimeHost performs the temporal model call outside REL/PHY locks, then reacquires the owner lock and renews/revalidates the active Insomnia lease before publication.
- `models.chronos` is an optional dedicated route; absent configuration falls back to the Insomnia route and then General.

### D — Dream consumer completion — partially implemented

- Parser/matcher ownership and temporal candidate matching now route through Chronos.
- Preserve temporal candidate retrieval and classifier context behavior as deterministic coverage expands.
- Add shared temporal ordering/validity outputs to Dream only when those Chronos primitives are implemented; Dream retains Memory-to-Memory semantic authority.

### E — Perception integration

- Add Chronos input/output to Observation processing contracts; the former stale-Memory reinforcement audit is superseded by the separate deterministic [Memory Freshness design](memory-staleness-plan.md).
- Define deterministic multi-evidence temporal synthesis.
- Use fallback inference only for unresolved semantics.

### F — Validation

Measure deterministic parse coverage, indication recall, fuzzy-detection false positives, inference rate/cost avoided, edit/replacement invalidation, and compatibility with existing Dream temporal retrieval fixtures.

## Open implementation decisions

- final Chronos API surface beyond the implemented detect/analyze/assess/inference boundaries, resolution model, generic `Temporal*` model, and internal matcher;
- indication-vocabulary coverage and calibrated fuzzy thresholds beyond the implemented conservative first pass;
- remaining valid-time representation details beyond implemented optional-bound intervals, especially uncertainty and multi-evidence composition;
- consumer adapters for deterministic validity synthesis, especially Perception Observation composition; and
- whether any deterministic analysis warrants a disposable cache.

## Notes

Transaction/knowledge time is now implemented by the Container/global-version stream. Chronos must consume that boundary where historical belief-state cuts matter but must not duplicate or reinterpret it as valid time.

## Related docs

- [ADR 0035](decisions/0035-chronos-shared-temporal-semantics.md)
- [Dream design and validation record](dream-implementation-plan.md)
- [Perception subsystem plan](perception-subsystem-plan.md)
- [Roadmap](roadmap.md)
- [Versioning, historical cuts, and rollback](version-history-plan.md)
