# Chronos explicit-definition resolution — correction specification

**Status:** Specified, not implemented (2026-10-27).  
**Target:** `crates/reliquary-chronos` existing temporal core; root consumers stay thin.  
**Baseline:** `refactor/crate-first-restart`, Chronos commits `69460fe6`, `abc62e99`, `ae2d555a`.  
**Related:** [Chronos extraction and decision redesign](chronos-crate-extraction-spec-2026-10-27.md).  
**Delivery:** One bounded correction in the already-extracted crate, not another crate extraction or architectural phase. **Do not mix or stage ongoing Switchboard work or the Graph evidence modification.**

## Purpose

Close the verified semantic gap: an ambiguous recurrence with an **explicit, local, supported definition** still requests model disambiguation even though deterministic evidence identifies a unique interpretation. Prevent a model-selected alternative from contradicting that explicit definition.

## Diagnosis — exact current implementation

- `crates/reliquary-chronos/src/chronos_choices.rs::assess_choices` calls `chronos::assess`, then independently produces two fixed candidate interpretations for unresolved `biweekly` and `bimonthly`. It neither reads the meaning of adjacent explicit definitions nor filters the candidates by those definitions.
- `chronos_resolution.rs::assess_resolution` derives resolved indications using `TemporalAnalysis` evidence strings. `resolved_spans` finds **every** matching substring occurrence through `text.match_indices`; it does **not** carry a source-span relationship between an ambiguous term and a phrase defining it. The normalized temporal model also deduplicates identical recurrence semantics, so matching evidence text or counting the patterns is insufficient proof of binding.
- `chronos_choices.rs::accept_selection` reconstructs the same unconstrained choices and verifies an ID/canonical parser result, not whether that result violates a source-local definition. `chronos_inference_apply.rs::apply_inference` likewise validates persisted source indication and reparsable canonical text, but not a conflicting explicit definition.
- `chronos::analyze` and `chronos::assess` derive analysis independently of `assess_choices`. `src/runtime_inference/dream_temporal.rs` uses `analyze` directly; `src/runtime_inference/insomnia/temporal.rs` uses `assess` before optional inference and simply returns if no decision endpoint is configured. A patch only to `assess_choices` would leave Dream and no-model consumers inconsistent.
- Existing tests prove two unqualified alternatives and valid response envelopes, **not** explicit-definition binding, model-call avoidance when context is sufficient, or contradiction rejection. Prior 71 lower-crate, 1,580 root and 32 CLI successes prove no regression at that checkpoint, **not** this behavior.

**Reproduction fixture to add first:** `Review biweekly, meaning every two weeks.`. The current `assess_choices` still offers two candidates; `TemporalDisambiguator::select` calls its endpoint if available. This behavior is established from the source paths above and must be recorded by a failing executable regression before the fix.

## Target behavior and interface

For a supported ambiguous recurrence, **deterministic interpretation is authoritative where there is exactly one explicitly bound, verified meaning**.

| Input | Expected outcome | Decision requests |
| --- | --- | --- |
| `Review biweekly, meaning every two weeks.` | Every two weeks, fully resolved | 0 |
| `Review biweekly (twice weekly).` | Twice weekly; no weekdays invented | 0 |
| `Review bimonthly, i.e. twice monthly.` | Twice monthly; no month days invented | 0 |
| `Review biweekly.` | Both supported candidates still available | 1 if configured, else unresolved |
| `Review biweekly. Audit every two weeks.` | Separate assertions, **not** a definition | 1 if configured, else unresolved |
| `Review biweekly, not every two weeks.` | Not a positive definition; unresolved unless separately proven | No deterministic selection |
| Conflicting or uncertain definitions | Remain unresolved, **do not** force a choice | 0 for blocked contradictory units |
| `Review biweekly (every two weeks); audit bimonthly.` | First resolved; second remains eligible for decision | 1 batched request containing only the second |

- Keep existing public `chronos::{detect,analyze,assess,analyze_with_inference}` and `chronos_choices::{assess_choices,selection_request,accept_selection}` signatures if possible. No new endpoint route, storage record, generated inference marker or public AST is needed.
- The one **lower-crate** deterministic analysis path must supply the same resolved `TemporalPattern` and resolution status to `analyze`, `assess`, `assess_choices`, Dream and Insomnia, **even with no model configured**.
- If all detected temporal material is accounted for, `TemporalResolutionStatus::FullyResolved`; if unrelated unsupported material remains, retain `Unresolved` with only the genuine residue. `TemporalChoices.sets` excludes explicit-definition-resolved indications and retains independently ambiguous ones. Zero eligible sets produces no `DecisionEndpoint` request.
- A definition-resolved result is **deterministic, derived data**, not `TemporalInference`. Only actual accepted model selections can produce `TemporalInference` for Memory Core, still bound to exact `MemoryBodyId` and reference time. No format/version change.

## Deterministic binding rule — intentionally conservative

1. **Identify exact terms:** initially the existing ambiguous lexicon `biweekly` and `bimonthly` only, using `TemporalDetection`'s exact original UTF-8 byte spans and indication kind. Compare case-insensitively without changing original offsets.
2. **Recognize explicit definitions only:** bounded, same-clause, same-field apposition/definition structures such as `TERM, meaning PHRASE`, `TERM (PHRASE)`, `TERM, i.e. PHRASE`, `TERM, that is PHRASE` and `TERM = PHRASE`. Optionally the exact reversed parenthetical `PHRASE (TERM)` when unambiguous. A free-floating later phrase, punctuation-only neighbor, sentence break, semicolon, newline between title/content, or mere matching recurrence elsewhere **must not** bind. Recognize the link from captured **source spans**, not nearest-value guessing or global string equality.
3. **Verify meaning through the existing grammar:** the bound `PHRASE` must parse as one supported recurrence interpretation using existing `verify_canonical_expression`/parser; it must be **semantically identical** to exactly one enumerated candidate (frequency, interval, occurrences-per-period, weekday/month-day/month constraints). Do not convert unsupported phrasing into a new candidate. If extra unspecified days/times would be needed, abstain.
4. **Filter only when sound:** one positively asserted, unnegated and unhedged, uniquely bound candidate produces deterministic resolution. Two different definitions, multiple plausible bindings, an explicit contradiction, negation over the linking clause, `or`/hypotheticals, or malformed/incomplete parentheses prevent auto-resolution. When incompatible explicit definitions are detected, treat the affected unit as **blocked unresolved**, not as two freely selectable alternatives. Avoid claiming generic English semantic contradiction analysis.
5. **Preserve source provenance:** link the exact ambiguous-term span to the exact defining-phrase span in the *internal calculation*; do not infer links from `TemporalPattern.evidence`, `match_indices`, or normalized-string reuse. The caller-visible derived recurrence must be canonical and source-backed, and the original term remains visible with its original bytes in detection. Explicitly handle semantic dedup where both the ambiguous term and defining phrase denote the same recurrence; the resolution must count both source spans as explained, not delete one and leave false unresolved residue. No duplicate semantic pattern is created merely to retain evidence.
6. **Keep existing bounds:** retain max 32 unresolved indications, max 8 alternatives/indication and max 64 submitted candidates; no unbounded scan, recursive parser/model loop or opportunistic history reads. UTF-8 boundaries and case/punctuation variants must not panic. Use only the source input and provided reference timestamp, never locale or wall clock.

**Implementation seam:** use one small private in-crate explicit-definition matcher that returns span-grounded proof of a unique candidate, no proof, or a blocked contradictory unit; integrate that result into the shared deterministic assessment/analysis pipeline. Prefer extending the existing `TemporalAssessment` computation privately rather than adding a public inference-state type, second parser or free-text rule engine. Use the existing normalization and match semantics. An internal span-proof representation may be necessary because `TemporalAnalysis` itself carries evidence text, not source offsets.

## Selection and replay correctness

- Recompute the same deterministic constraints in `accept_selection`, **before** validating a model answer against eligible questions. A model cannot choose an eliminated candidate, and an unsolicited answer for a now-deterministically-resolved unit is extra/invalid.
- `analyze_with_inference` / `apply_inference` must **reject** a persisted canonical choice that contradicts a locally proven explicit definition, while continuing to accept compatible current-format inference. Existing root Dream fallback to deterministic results must remain safe. Validate this against retained inference fixtures; do not silently invalidate unrelated historical conclusions or introduce a new persisted format.
- Input-specific model question IDs remain deterministic and unique for surviving ambiguous units; no bogus question emitted for a resolved or blocked unit. The current `DecisionEndpoint` shape, `models.general_decision → models.general → unavailable` precedence and typed transport/backpressure errors remain unchanged.
- Non-definition cases preserve prior parser, matcher, status, model-call and persistence behavior. The correction is **not** a generic reasoning engine, an expansion of temporal grammar, or a change to model routing.

## Small implementation sequence

1. **C1 — red evidence:** add targeted lower-crate regression cases for both definition meanings and unrelated proximity, plus a mock decision call-count check. Capture the failing baseline on the current Chronos crate alone; do not run unrelated Switchboard compilation as a prerequisite to finding the defect.
2. **C2 — one canonical core:** add the private span-safe definition matcher; integrate it where `analyze` and `assess` share their deterministic result and where `assess_choices` derives outstanding sets. Explicitly verify semantic dedup and multiple occurrences with identical text.
3. **C3 — fail-closed selection/replay:** apply exactly the same binding constraints to selection answer validation and persisted inference application. Confirm no new persistence shape; non-conflicting current inference still replays. Keep root adapters untouched except a genuinely required call-site adjustment.
4. **C4 — consumer verification:** lower tests, zero-call and mixed batch via `TemporalDisambiguator`, no-route Insomnia assessment, exact body/source-time persistence and Dream direct `analyze` parity. Test conflicted/negated/hedged/duplicate phrases and original UTF-8 spans; rerun focused external processing error mapping.
5. **C5 — final gate and docs:** after Switchboard's concurrent work is at a coherent integration checkpoint, run the locked offline Chronos tests, workspace all-target compile, root library, detached CLI, formatting and `ddocs check --indexes --links`. Update `docs/subsystems/chronos.md`, `docs/current-limitations.md`, API/behavioral contracts as required; remove the limitation when the implementation and gates actually pass. Preserve independent CVAFRS07/Arcana checks as unclaimed unless executed. Commit only the correction and directly owned docs.

## Completion evidence

**Must pass:** both supported alternatives for both terms; case variants and UTF-8 prefix; `i.e.` / `meaning` / parentheses boundaries; multiple identical terms with one definition; title-content newline isolation; negated/hedged/contradictory definitions; unrelated same-sentence and neighboring-sentence recurrence; no model route; mixed independently ambiguous material; malicious choice ID and stale persisted inference; reparse/normalization and matching parity.

Record per fixture: candidate count before/after narrowing, link span evidence, resolution status, number of decision calls, accepted/rejected persisted conclusion and Dream/Insomnia agreement. Assert **zero unsupported automatic resolutions** and **zero model calls** for fully explicit, supported definitions. Simulated endpoint tests prove contract behavior, not real-model accuracy.

**Architecture acceptance:** one Chronos owner and one deterministic path, no new crate/dependency edge, no reverse reference to Memory or Switchboard, no temporary duplicate/compatibility wrapper. Only current local grammar results become derived facts. A fresh architecture analyzer or new policy infrastructure is **not** part of this correction.

## Out of scope

Broad natural-language entailment, arbitrary cross-sentence coreference, timezone inference, free-text model interpretation, new temporal variants, chronology/storage changes, new semantic model route, Perception Observations, and unrelated Switchboard extraction work.
