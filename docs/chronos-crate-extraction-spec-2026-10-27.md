# Chronos crate extraction — implementation specification

**Status:** Specified, implementation not started (2026-10-27).
**Branch:** refactor/crate-first-restart. **Reference HEAD:** 390095ec.
**Target:** crates/reliquary-chronos/ (currently a placeholder).
**Prerequisite:** Finish and commit the ongoing Owner Lifecycle extraction first. Do not mix its uncommitted worktree changes into this extraction.
**Mode:** Correction-only hard cut: move current Chronos semantics without changing temporal grammar, inference behavior, storage or provider configuration.

## Goal and ownership

Chronos independently compiles and owns reusable temporal detection, bounded normalization/typo tolerance, calendar/relative/boundary/recurrence/duration parsing, explicit resolution status and unresolved residue, overlap matching, deterministic processing-cadence math, and deterministic verification/application of canonical temporal inference. Callers supply authoritative reference time and choose whether to infer.

It does **not** own Memory or Archive source time, transaction clocks, Dream pair/candidate policy, Dream scheduling, Insomnia extraction, persisted Memory inference, Perception Observations, REL/PHY storage, provider credentials or model routing. No durable Chronos store is introduced. The crate map permits Container and Switchboard only as a maximum dependency ceiling; the actual deterministic code needs neither. Use regex/time and, if needed for pure inference JSON validation, serde_json. Do not add an internal Reliquary crate dependency just because the map permits it.

**Authorities:** docs/decisions/0035-chronos-shared-temporal-semantics.md, docs/subsystems/chronos.md, docs/architecture/crate-map.json, docs/crate-first-layout-spec-2026-10-25.md.

## Concrete source cut

**Move lower, preserving original behavior and one canonical implementation:**
- src/chronos/chronos.rs: detect, analyze, assess, analyze_with_inference and temporal_matches.
- chronos_model.rs, chronos_detection_model.rs and chronos_resolution_model.rs: canonical public Temporal* types and values.
- Deterministic chronos_{absolute,absolute_calendar,boundary,boundary_endpoint,calendar,detection,detection_context,detection_explicit,duration,edit_distance,event_relative,fuzzy,loose_calendar,loose_explicit,loose_relative,match,normalize,number,parser,recurrence,recurrence_interval,relative,relative_offset,resolution,season,time_of_day,vocabulary}.rs.
- Pure functions in chronos_inference_verify.rs, chronos_inference_apply.rs and chronos_processing_epoch.rs. Move matching chronos_*_tests.rs with these implementations; do not duplicate them in root.

**Keep higher:**
- The GeneralEndpoint and GeneralEndpointError transport types from src/runtime_inference/general_endpoint.rs, configured runtime/ModelSwitchboard routes and network clients.
- Root-facing TemporalInferencer<E: GeneralEndpoint> as a **thin endpoint adapter** to lower-owned pure inference request/response validation. Preserve public generic signature, prompt/contract constants, and typed TemporalInferenceError (Endpoint(GeneralEndpointError) versus InvalidOutput(String)).
- Conversion impls between Chronos TemporalInference/TemporalInferenceResolution/TemporalIndicationKind and reliquary_memory_core equivalents. Move these to one higher Memory/Chronos boundary module, not into lower Chronos.
- src/runtime_inference/insomnia/temporal.rs: draft source time, optional call admission, consumer persistence, MemoryBodyId and body/reference staleness.
- src/runtime_inference/dream_temporal.rs, dream_owner_candidates.rs and dream_cooldown.rs: owner Memory read, authoritative source chronology, Dream candidate scoring, cooldown, cadence version, wall-time choice, REL/PHY processing-epoch writes.
- Perception valid-time/Observation synthesis remains outside Chronos.

## Mandatory architecture corrections

1. **No reverse endpoint dependency.** Currently chronos_inference.rs imports root GeneralEndpoint, and chronos_inference_error.rs imports root GeneralEndpointError. Split at the existing endpoint.complete_json call: lower owns the exact schema/prompt, bounded unresolved item/ID construction, canonical output validation, deterministic verification and application; root TemporalInferencer invokes the endpoint and preserves its typed errors. Prefer a narrow concrete request/acceptance API over callbacks or a new generalized inference framework.
2. **No Memory Core dependency.** chronos_inference_model.rs currently contains conversion impls referring directly to reliquary_memory_core. The lower crate owns canonical Chronos value definitions; the higher root owns inter-domain conversions. Preserve existing persisted bytes and public type identity, including the 32-indication cap and the chronos-temporal-inference-v1 contract.
3. **Existing model selection unchanged.** Configured Insomnia requires the dedicated models.chronos route: OpenAI Codex or OpenAI-ready, with no implicit general_decision fallback. Runtime host routes.chronos() is optional; when absent, InsomniaExtractor.temporal_endpoint() falls back to the **main Insomnia endpoint**. Do not silently conflate these pathways.
4. **API identity unchanged.** Root re-exports original Temporal* types and DreamTemporal* aliases; preserve current root chronos::{detect,analyze,assess,analyze_with_inference} module path. Move temporal_matches into lower but expose only the narrow symbol Dream actually consumes; do not retain a second parser or borrowed/owned shadow types.
5. **Temporal invariants.** Original evidence spans and Unicode-safe offsets survive typo repair; deterministic normalization, dedup, ordering, locale abstention, UTC/RFC3339, time precision, calendar-aware month/year clamp, open range boundaries, recurrence, matching scores/maximum six matches, and absence-of-reference behavior are unchanged. No new grammar or calibration while extracting.
6. **Inference invariants.** Zero model calls when resolved; at most one bounded unresolved-only model request. Strict matching of IDs, kind, count, coverage and canonical expression; mismatched/invalid/non-resolvable outputs reject. Abstention cannot invent facts. Pure canonical reparse may not mutate root or persist deterministic results.
7. **Persistence/clock boundary.** Only consumer-owned nondeterministic inference may become durable and must remain bound to exact MemoryBodyId/reference timestamp; Dream rejects stale bindings. ProcessingCadence functions preserve i128 overflow/saturation and u64 behavior, while Dream owns source timestamp, 30-day policy, cadence version and processing state writes; Chronos does not read wall time or Container transaction time.

## Bounded implementation sequence

1. **Freeze after Owner Lifecycle.** Confirm HEAD, status and committed lower crates. Enumerate actual root public Temporal*/DreamTemporal* re-exports and callers; audit the inference transport, conversion and runtime route paths. Capture current deterministic output and inference fixture behavior. Preserve unrelated docs/graph-crate-extraction-evidence-2026-10-26.md.
2. **Pure parser/model cut.** Move public models, detector/normalizer/parser/calendar modules and their tests into the lower crate. Preserve existing root chronos module path using re-export, not duplicated executable code. Compile lower and root before continuing.
3. **Resolution/matching/cadence cut.** Move deterministic resolution, match and processing-epoch arithmetic. Rewire Dream owner candidate and cooldown helpers, leaving Dream policy and state above.
4. **Inference cut.** Separate pure request/schema/verification/application from GeneralEndpoint call and error mapping; move Chronos inference value definitions and Memory conversion impls to appropriate layers. Rewire Insomnia, external-derived processing and configured/runtime-host callers without changing model selection.
5. **Hard cut cleanup.** Remove displaced src/chronos owner modules, duplicate root module registrations and obsolete forwarding code, retaining only genuinely higher endpoint/conversion and consumer adapters. Do not introduce a second durable cache.
6. **Verify and commit.** Update Cargo/lock and detached CLI lock as necessary. Update docs/architecture.md, docs/api.md, docs/maintainer-map.md, docs/subsystems/chronos.md and INDEX.md. Record exact results in docs/chronos-crate-extraction-evidence-2026-10-27.md and commit only scoped changes once green.

## Acceptance matrix

**Lower crate:** cargo check -p reliquary-chronos --locked --offline; cargo test -p reliquary-chronos --locked --offline. Confirm no root/Memory/Dream/Insomnia/Perception direct dependency.

**Pure temporal:** Original exact outputs for ISO dates and timestamps, natural calendars/quarters/seasons, relative dates with and without reference, calendar clamping, intervals, durations/approximations, recurrence, fuzzy correction spans, unresolved material, six-match cap and ordering. No inference on fully resolved input.

**Inference and transport:** Typed endpoint error parity; invalid/missing/duplicate/extra resolution IDs, wrong kind, unsafe canonical output and abstention; same request schema/contract. Configured dedicated Chronos route and host fallback remain distinct. Body/reference-bound inferred data still valid only for exact source content and cannot be regenerated by replay.

**Consumers:** Dream pair matching, REL/PHY processing epochs and stale inferred data; Insomnia prepared draft semantics and persistence, external-source processing, historical Memory replay/current-format parity and root public alias compatibility. Run focused chronos, temporal, dream, insomnia and processing_epoch tests.

**Integration:** cargo check --workspace --locked --offline -j 1; cargo fmt --all -- --check; cargo test --lib --locked --offline -j 1; cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1; ddocs check --indexes --links. Do not claim an unrun CVAFRS07 corpus gate or independent architecture refresh.

**Complete only when:** One compiled Chronos is used by actual Dream/Insomnia paths; root retains only higher endpoint/model persistence adapters, type identity and routing preserved, no reversed Cargo edge or duplicate parser, tests and evidence green, extraction committed.

## Exclusions

No new temporal grammar, Observation/Perception feature work, timezone/locale guesses, new scheduling/wall-time API, Switchboard extraction, model/provider change, durable Chronos store, legacy reader, generalized compatibility layer or custom architecture-proof machinery.
