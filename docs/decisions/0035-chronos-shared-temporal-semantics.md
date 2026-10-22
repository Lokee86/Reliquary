# ADR 0035: Chronos shared temporal semantics

Parent index: [Architectural decisions](INDEX.md)

Implementation planning: [Chronos subsystem plan](../chronos-subsystem-plan.md) and [Roadmap](../roadmap.md)

## Status

Accepted — 2026-09-09. Shared deterministic core extraction, indication/normalization, the conservative grammar baseline, explicit resolution status, bounded unresolved-only inference, Memory-bound inferred-state persistence/staleness, and the first corpus-calibrated detector precision pass are implemented. Insomnia invokes Chronos over prepared Memory drafts, runs temporal inference only for unresolved residue, and persists only same-kind deterministically verified nondeterministic conclusions. Perception integration remains future work.

The former deterministic `dream_temporal*` parser/model/calendar/recurrence/matcher implementation is now owned by `chronos*`. Chronos also exposes parser-independent indication detection with original byte spans and bounded contextual typo correction over temporal vocabulary. Dream retains a thin owner-specific source-time adapter and compatibility aliases rather than a second temporal stack.

## Context

Reliquary already derives explicit/relative content-time anchors, ranges, calendar periods, recurrence, and temporal matches from Memory bodies plus authoritative source chronology. Those products are intentionally unpersisted because they are cheap, deterministic, and reproducible.

Temporal semantics are now needed at three different semantic boundaries:

- Insomnia while processing individual Memories;
- Dream while organizing Memory-to-Memory structure; and
- Perception while synthesizing or reconsidering Observations.

The current parser is also narrower than the new requirement. Temporal processing must not be skipped merely because an expression is unsupported or contains an obvious typo such as `three mnoths ago`.

Knowledge/system time is separate: wall-clock transaction time belongs on the global/container version stream, not in Chronos.

## Decision

### Chronos is the shared temporal subsystem

**Chronos** owns reusable temporal detection, normalization, parsing, resolution, comparison, validity interpretation, and bounded temporal-inference contracts.

Chronos is not a REL/PHY owner, semantic object store, scheduler, or independent processing pipeline. Insomnia, Dream, and Perception decide when to invoke it.

```text
                 Chronos
        detection / normalization
        parsing / resolution
        comparison / validity
        bounded inference fallback
             /      |       \
            /       |        \
     Insomnia      Dream    Perception
      Memories     pairs    Observations
```

### Deterministic extraction is primary

Chronos must derive as much temporal information as practical without inference.

The deterministic path should cover:

- explicit dates/times and calendar periods;
- relative expressions resolved against authoritative source/reference time;
- numeric and word-number durations such as `3 months ago` and `three months ago`;
- ranges and open boundaries such as `from`, `until`, `since`, `before`, `after`, `starting`, and `ending`;
- recurrence/frequency;
- duration expressions;
- safe loose calendar language; and
- temporal overlap/order/comparison.

Existing Dream temporal parsing/resolution is generalized into Chronos rather than reimplemented beside it.

### High-recall indication detection gates inference

Chronos adds a cheap deterministic detector whose question is broader than parsing:

> Could this semantic unit contain meaningful world-time information?

A positive indication may resolve completely through deterministic parsing or merely justify bounded inference when deterministic interpretation remains incomplete.

Inference must not run over every Memory, pair, or Observation by default.

### Typo tolerance is bounded and non-destructive

Temporal indication detection must tolerate obvious misspellings of a bounded temporal vocabulary, for example:

```text
mnoths     -> months
Febuary    -> February
Wendesday  -> Wednesday
```

This is not a general spellchecker. Fuzzy recognition is restricted to temporal vocabulary and conservative deterministic rules.

Normalization is transient and never rewrites authoritative Memory/source text.

### Source, valid, and transaction time remain distinct

Chronos distinguishes:

```text
source time       when the evidence originated
valid time        when the represented proposition/state applies
transaction time  when the REL/PHY committed the state
```

Chronos owns source/reference-time resolution and valid-time interpretation. Transaction/knowledge time belongs to timestamped global/container versions.

### Deterministic products remain derived

Deterministically reproducible Chronos output remains derived state and should not be persisted merely for convenience. It may be recomputed or cached behind ordinary invalidation rules.

Only temporal conclusions that cannot be reproduced deterministically require durable semantic persistence when a consumer needs them. Persist the semantic conclusion and required input/version identity, not duplicate deterministic intermediates.

### Inference is a bounded fallback

Model temporal interpretation is permitted only when:

1. deterministic indication detection found relevant temporal material;
2. the consumer actually needs temporal resolution; and
3. deterministic Chronos cannot safely resolve the remaining semantics.

Inference receives the original semantic unit, authoritative source/reference time, and deterministic candidates/evidence. It resolves only the remaining temporal ambiguity.

Any inferred result that becomes durable must be bound to the semantic body/version that justified it so replacement makes the inference stale.

### Consumer ownership remains unchanged

**Insomnia** invokes Chronos for newly processed Memory bodies, including extracted/imported/user-created content and replacement/correction bodies. Insomnia remains Memory authority.

**Dream** consumes Chronos for temporal candidate discovery, recurrence, ordering, overlap, and pair-classification context. Dream remains Memory-to-Memory relationship authority.

**Perception** consumes Chronos while synthesizing or reconsidering Observations from bounded evidence. Perception decides what Observation exists; Chronos supplies temporal interpretation for it.

## Consequences

Reliquary gains one temporal vocabulary/resolution engine across single-Memory, pairwise-Memory, and Observation processing.

Most temporal work remains deterministic and storage-free. Model cost is paid only for unresolved residue.

The former Dream temporal machinery is preserved under shared Chronos ownership; Dream-specific code now supplies only owner-local reference chronology and consumer context.

Together with wall-clock global-version timestamps, Chronos provides valid-time semantics while the version stream provides transaction/knowledge time.

## Rejected alternatives

### Keep temporal semantics inside Dream

Rejected. Insomnia and Perception now require the same machinery, and separate implementations would drift.

### Add an independent temporal processing daemon

Rejected. Chronos is a reusable capability invoked at existing semantic processing boundaries.

### Run model temporal classification over every object

Rejected. Most temporal forms are deterministic; inference is gated by indications plus unresolved need.

### Persist all parsed anchors and patterns

Rejected. Cheap deterministic analysis can be reproduced from authoritative content plus reference chronology.

### General-purpose typo correction

Rejected. Chronos only needs bounded fuzzy recognition of temporal vocabulary.

## Open implementation questions

- final Chronos APIs beyond the implemented detection/analysis/assessment/inference boundaries, resolution model, generic `Temporal*` types, and internal matcher;
- future indication-vocabulary or fuzzy-match expansion beyond the measured corpus-calibrated baseline;
- expanded deterministic grammar;
- remaining valid-time representation details beyond implemented optional-bound intervals, especially recurrence composition, granularity synthesis, and uncertainty; and
- exact multi-evidence Observation temporal-synthesis contract.

## Verification

Implementation must protect:

- deterministic replay for unchanged semantic input + reference time;
- no mandatory persistence of deterministic output;
- bounded typo tolerance without mutating source text;
- no inference when deterministic resolution is sufficient or no indication exists;
- stale invalidation of body-bound inferred results after replacement;
- preservation of current Dream temporal candidate behavior through Chronos;
- Perception consumption without transferring Observation ownership; and
- strict separation of source time, valid time, and transaction time.

## Related docs

- [Chronos subsystem plan](../chronos-subsystem-plan.md)
- [Dream design and validation record](../subsystems/dream.md)
- [Perception subsystem plan](../perception-subsystem-plan.md)
- [ADR 0012 — Insomnia Memory authority](0012-deterministic-episodes-and-insomnia-memory-authority.md)
- [ADR 0024 — owner-local Dream processing](0024-owner-local-dream-processing.md)
- [ADR 0033 — Perception entities/observations](0033-perception-entities-observations-and-ambiguity.md)
- [Roadmap](../roadmap.md)
