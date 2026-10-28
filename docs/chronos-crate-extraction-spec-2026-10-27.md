# Chronos crate extraction and deterministic-choice redesign — implementation specification

**Status:** Implementation and regression checks completed; architecture enforcement remains under verification (2026-10-27).
**Branch:** `refactor/crate-first-restart`. **Baseline:** `8943749d` (Owner Lifecycle extraction committed).
**Target:** `crates/reliquary-chronos/`. **Scope:** one combined delivery with **two independent acceptance gates**: (A) behavior-preserving crate extraction, then (B) deterministic candidate generation and decision-model selection.
**Migration policy:** staged hard cut. One canonical implementation at each integrated gate; no legacy runtime readers, duplicate parsers or compatibility adapters. Preserve unrelated Graph evidence worktree changes.

## Purpose

Define the staged extraction and deterministic-choice redesign for Chronos.

## Overview

The implementation is accepted only after the extraction and redesign gates below.

## Problem and desired outcome

Today's Chronos deterministically parses many temporal expressions but asks a `GeneralEndpoint` to *generate* a canonical expression whenever it sees unresolved temporal material. That is unnecessarily open-ended for ambiguities whose possible meanings are already enumerable, and it requires a dedicated `models.chronos` general-model route in the configured runner.

**Target:** Chronos computes the interpretations its grammar can justify; if the evidence selects exactly one, use it without inference. Where there are several defensible candidates, a shared `DecisionEndpoint` chooses one candidate ID or **abstains**. Chronos revalidates the selected candidate before admitting it. If no finite safe set can be generated, retain unresolved temporal material; **never ask a general model to invent additional possibilities**. The shared decision service may use a general model as a transport fallback; that general model still performs only constrained selection.

The user-visible effects are fewer unnecessary inference calls, no model-authored dates/cadences/timezones, one canonical temporal engine, no mandatory Chronos-specific model configuration, and unchanged authority for Memory/Dream/Insomnia.

## Target architecture and ownership

`reliquary-chronos` independently owns:
- Detection with original UTF-8 byte spans, conservative typo normalization, grammar/parsing, resolution and semantic matching; pure calendar and processing-cadence math.
- Enumerating supported candidate interpretations for an unresolved/ambiguous indication, structurally bounded and ordered deterministically. A candidate contains a stable locally assigned ID, source span and kind, canonical expression, and the same verified `TemporalAnalysis` product that would result from deterministic parsing.
- Deterministic validation of candidate eligibility, response choice IDs, selected kind/span/reference, and merging the chosen verified product without losing original evidence.

**Not owned:** provider clients, auth/credentials, Switchboard, `DecisionEndpoint` transport, timestamps sourced from REL/PHY, Memory inference persistence, Dream candidate/ranking/cooldowns, Insomnia extraction/worker scheduling, Perception Observations, Container transaction chronology or a Chronos store. Callers provide semantic input plus authoritative source/reference timestamp; no implicit wall clock or silent timezone/hemisphere assumption.

Only `regex` and `time` are required by the deterministic core; a pure JSON contract may use `serde_json`. No direct Reliquary crate dependencies unless independently justified by an actual lower-level use. The physical crate map's permitted edges are **ceilings**, not instructions to import Container or Switchboard.

## Stage A — behavior-preserving crate extraction

This stage must pass independently **before** changing inference semantics, to establish a meaningful baseline.

1. Freeze HEAD/status, Cargo dependencies, old public APIs, the current deterministic fixtures and the old open-ended inference fixtures. Owner Lifecycle (`8943749d`) is integrated; do not repeat its cuts.
2. Move the canonical `src/chronos/` models, indication/detection, normalization, calendar/absolute/relative/ranges/seasons, recurrence, duration, parser, resolution, matching and their existing tests into `crates/reliquary-chronos/src/`. Move pure `chronos_processing_epoch.rs` arithmetic and tests with it. Do **not** move Dream policy, timestamp choice, wall-clock access or REL/PHY mutation.
3. Separate `chronos_inference.rs` and `chronos_inference_error.rs` from root `GeneralEndpoint` / `GeneralEndpointError`: lower Chronos owns pure request/canonical verification/`analyze_with_inference`, while a single higher adapter performs the endpoint call. The lower inference value structs are canonical; conversion impls involving `reliquary_memory_core` live in one higher boundary module. Preserve the existing `TemporalInferencer` and typed endpoint error during **Stage A only**.
4. Rewire actual Dream/Insomnia/external-source production callers to lower Chronos exports; preserve root `Temporal*` names, `DreamTemporal*` aliases, `chronos::{detect,analyze,assess,analyze_with_inference}` path and the exact old public type shapes. Remove displaced root parser/codec modules and redundant forwarding logic. No duplicate implementation or mirrored model types.
5. **Gate A:** lower crate tests, focused existing Chronos parsing/inference tests, root Dream/Insomnia and REL/PHY temporal-history tests, `cargo check --workspace --locked --offline -j 1` and `cargo fmt --all -- --check` pass. Record the baseline outputs and results in the Chronos extraction evidence. Gate A may be an intermediate verification checkpoint within the same delivery, not a separate long-lived project.

## Stage B — deterministic candidates and shared decision selection

### 1. Candidate admission and enumeration

Change `assess` / resolution internals so an expression with known competing interpretations does **not** get incorrectly marked fully resolved merely because one parser path matched. Preserve current exact analysis on unambiguous input. Introduce a small canonical lower API (for example, `assess_choices(text, reference_ns)`) that returns deterministic resolved products, bounded unresolved spans and finite candidate sets; use existing `TemporalAssessment` for the established public detection/analysis view, extending or adjoining it without duplicating results.

- Candidate rules are **explicit, tested grammar/lexicon rules**, not generated by an LLM or expanded from arbitrary context. Each candidate is individually reparsed/verifiable by existing deterministic Chronos code, consistent with source indication kind and available authoritative reference.
- A known phrase such as *biweekly* may produce “every two weeks” and “twice a week”; both remain ambiguous absent disambiguating context. **Current `TemporalPattern { frequency, interval, weekday, month_day, month, evidence }` cannot distinguish twice-per-week from weekly.** Stage B must add the smallest *derived-only* occurrence-count semantic field/variant and deterministic parsing needed to represent that candidate exactly (unspecified two occurrences, not invented weekdays). Carry the new distinction through dedup, matching, verification and public consumer tests. This is an intentional Stage B derived API evolution, **not** a persisted schema change. Do not silently encode twice-weekly as an ordinary weekly pattern. Other candidates must come from supported grammar and real unresolved fixtures; no speculative grammar expansion.
- One safely implied candidate resolves deterministically with **zero** model calls. Two or more safe, mutually distinguishable candidates invoke selection. A zero-candidate indication stays **unresolved/abstained**, not routed to creative inference.
- Never fabricate a specific instant, date, cadence, locality, timezone or season hemisphere. Missing reference time, contradiction and locale ambiguity block unsupported candidates rather than defaulting to wall time. Candidates can be filtered deterministically only by explicit grounded constraints.
- Stable IDs derive from canonical sorted alternatives and original indication spans, not hash iteration or the model. Deduplicate semantically identical results. Keep the source's exact original evidence even when normalized candidate text differs.
- Bound each semantic unit to **32** unresolved indications, **8** candidates per indication and **64** candidates in one selection request. If the safe set cannot be represented within bounds, preserve the affected indication as unresolved rather than truncating into a misleading selection set. Batch eligible ambiguous indications into one decision request; bounded context/state, no iterative inference loop.

### 2. Decision transport and response contract

Reuse the existing root `DecisionEndpoint::evaluate(state, questions)` contract. Construct choice questions using the existing supported `type: "choice"` / `instructions` / `criteria` shape, with one named question per ambiguous indication; each set includes exactly its enumerated candidate IDs **and an explicit `abstain` option**. Use the source text, authoritative reference, already resolved temporal evidence and candidate descriptions as immutable *state*. Instruct the model to choose only when contextual evidence actually supports one candidate; otherwise it must use **abstain**. A decision model must never return new canonical text.

Accept only the existing validated `answers` response envelope with exact question coverage and allowed choice IDs. Independently check selected IDs, indication/span/kind, candidate/source membership and canonical reparse inside Chronos. Malformed/missing/extra/out-of-set answers and locally duplicated candidate/question IDs fail closed through a typed decision/output error. The current `DecisionEndpoint` returns a parsed `serde_json::Value`, so duplicate *raw JSON object keys* are not observable at the Chronos boundary; do not claim to detect them without a separate transport-level strict-decoding change. Explicit abstentions retain unresolved state without persistence. Do not choose a random/default candidate on provider error, timeout, missing route or ambiguous evidence. A low-confidence or unsupported interpretation is not equivalent to a valid selection.

Define **one new selection request protocol** (e.g. `chronos-temporal-selection-v1`) for decision questions. It is separate from the durable `chronos-temporal-inference-v1` *verified canonical-resolution payload*. The selected candidate can still produce the existing `TemporalInference { model, contract_version, resolutions }` containing a canonical expression and exact indication metadata, so old current-format semantic evidence and deterministic `analyze_with_inference` replay can be preserved without adding another persisted format or legacy runtime reader. Do not store candidate lists or model choice responses; only verified nondeterministic selections required by the consumer.

### 3. Switchboard and product call direction

Canonical route precedence for Chronos selection: **`models.general_decision` → `models.general` (via the existing `GeneralDecisionEndpoint`) → no model, unresolved**. Reuse `configured_runtime_decision.rs`'s existing route selection/construction logic by adding a Chronos consumer to its explicit consumer enum; no new inference router, no shadow fallback in Chronos. `general_decision` selection may use a small decision model; `general` is a *constrained decision adapter*, not permission for free-text interpretation.

**Retire** dedicated `models.chronos`, `ModelCapability::Chronos`, `ModelSwitchboard::chronos/chronos_auth`, configured general endpoint constructors, `ReliquaryRuntimeRoutes::chronos` and any CLI/config setters/docs that exist solely for the retired route. Eliminate configured Insomnia's prior hard requirement for `models.chronos`. Both configured CLI and runtime host must use the same ordered decision route, or behave deterministically without it.

Audit actual saved development configuration artifacts **before** deleting `models.chronos` parsing. Where these files carry the old route, migrate the development corpus/config to the current-only schema, commit migration evidence, then delete temporary migration code. Do not silently preserve the removed field as an unknown extra object, and do not add permanent dual readers. If a released external config dependency is confirmed, isolate that exception with an explicit decision before code changes.

Replace the old `TemporalInferencer<E: GeneralEndpoint>` public entry with a clearly named higher-level `TemporalDisambiguator<E: DecisionEndpoint>` (or a single equivalent final API) in Stage B; migrate all in-repo callers directly and remove the old open-ended adapter. This is an **intentional development API change**, not a promise of source-compatibility; keep the public `Temporal*` data products, historical aliases and verified persistence semantics intact. Typed errors must preserve transport-versus-invalid-choice distinctions for Insomnia retry/backpressure handling.

### 4. Consumer/persistence boundaries

- Insomnia/external-source workflows: run deterministic assessment first, request a decision only for actual ambiguous candidate sets, persist only valid chosen nondeterministic conclusions bound to exact `MemoryBodyId` and source time. Zero candidate / abstention leaves unresolved information; do not turn normal lack of a decision route into a terminal work failure.
- Dream: reuse the single lower parser, comparison and verified inference replay; retain owner source-time lookup, candidate scoring, the six-match limit and order, stale inference rejection, cooldown and processing epoch writes.
- REL/PHY historical opens and reconciliation: passive and repeatable, with **zero model calls**. Persisted conclusions remain authorized only for their exact semantic body and reference and are revalidated deterministically; physical transaction time is still Container-owned.
- Perception/Observation temporal synthesis remains future work; do not use this project to implement it.

## Bounded implementation checkpoints

1. **A1** Freeze baseline and prove the chosen source/type/interface ownership; quantify current model calls and record golden outputs.
2. **A2** Move deterministic model/parse/detection and tests, then matching/processing arithmetic; compile lower and root; eliminate displaced root code.
3. **A3** Cut inference pure core from higher general endpoint and Memory conversions; verify **Gate A** and save its evidence.
4. **B1** Implement canonical candidate grammar and ambiguous-status assessment; fixture tests: none, one, multiple, over-limit, conflict, missing chronology and UTF-8 restoration.
5. **B2** Implement strict question/selection/reverify contract through one `DecisionEndpoint`; test no invented candidate, abstain, malformed responses, batching and fixed call counts.
6. **B3** Add Chronos to shared decision-route selection; rewire configured Insomnia, host and external processing; retire `models.chronos` code/config with current-only development corpus migration if needed.
7. **B4** Update body-bound persisted inference and Dream consumers without changing on-disk structure; remove the old general-model inferencer and obsolete fixtures/docs.
8. **B5** Run full end-to-end tests, update docs/API/architecture/maintainer map/INDEX and save precise evidence; commit only after the integrated Gate B is green. No premature “complete” based solely on Gate A.

## Acceptance gates and regression coverage

**Gate A — extraction parity:** Existing tests for absolute ISO/RFC3339, relative and calendar clamp, partial ranges, seasons/quarters, approximate durations, recurrence, typo original spans, unresolved assessment and match scoring remain equal to baseline. Existing inference protocol results unchanged until Stage B. One lower Chronos implementation consumed by Dream and Insomnia, no forbidden Cargo imports.

**Gate B — redesigned semantics:**
- Resolved and single safe interpretation: **0 decision calls**; multiple supported interpretations: **exactly 1 batched selection request** if a decision route is available; no candidates or missing route: **0 calls** and unresolved output.
- Explicit “biweekly” fixture exposes both justified options **with distinct temporal products** (biweekly vs twice weekly), and context-free fixtures must be able to abstain. A selection never invents the particular weekdays of an unspecified twice-weekly recurrence. Invalid kind, invented date/cadence/zone, out-of-set ID, missing/extra answers, duplicate candidate IDs, wrong spans, unsupported candidate and oversized set must never become admitted/persisted temporal facts.
- Legacy deterministic behavior for unambiguous expressions preserved; wrong early parser match on ambiguous language is corrected only by the intentional new ambiguity rules. Compare against a labeled corpus and record candidate recall, unsupported-candidate count, wrong-selection count, abstention count and model-call counts. Do not claim decision-model accuracy based only on simulated endpoints.
- Switchboard precedence `general_decision → general → unavailable` verified for **both** configured CLI and runtime-host processing; no residual `models.chronos` reader/writer/caller. Provider/backpressure errors still propagate as errors; abstention and lack of configured decision do not manufacture state or cause terminal failure.
- REL and PHY Insomnia, Dream temporal matching/stale Memory inference, external source, append/reopen/rewrite/history all preserve verified semantics. Existing current-format persisted `TemporalInference` replays without model calls. Confirm no on-disk schema change or perform the required explicitly scoped corpus migration instead.

**Integrated checks:** `cargo check -p reliquary-chronos --locked --offline`; lower crate tests; focused `chronos`, `temporal`, `insomnia`, `dream` and `processing_epoch` tests; `cargo check --workspace --locked --offline -j 1`; `cargo fmt --all -- --check`; `cargo test --lib --locked --offline -j 1`; `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`; `ddocs check --indexes --links`. Keep `CVAFRS07` six-file acceptance and fresh architecture graph refresh as separately named gates if not actually run.

**Completion:** One compiled Chronos crate owns the temporal semantics and candidate validation; one shared decision route handles ambiguity; a hard-cut removal of dedicated Chronos routing and free-form inference is integrated; original root data API and durable Memory evidence are coherent; no duplicated path or reverse crate edge; both gates documented and the final implementation committed.

## Out of scope

Expanding the temporal grammar beyond bounded explicit ambiguity enumeration, general-purpose NLP, Perception Observation synthesis, timezone or locale guessing, proprietary new model route, new storage layer, cache, arbitrary rescheduling, open-ended language fallback, and bespoke architecture-proof infrastructure.

## Related docs

- [Chronos subsystem](subsystems/chronos.md)
- [Extraction evidence](chronos-crate-extraction-evidence-2026-10-27.md)

## Notes

This specification records the intended migration; the evidence records verified results.
