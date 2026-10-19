# Decision Endpoint Inheritance — Implementation Specification
Date: 2026-10-19
Status: Implemented and verified; environment limits recorded in the execution plan

## Purpose

Define and track compatibility-preserving configured decision endpoint inheritance.

## Overview

Specialized routes override a shared decision service, which overrides a structured General adapter. Runtime failures do not change the selected tier, and the General adapter never manufactures confidence.

## Implemented behavior

The private configured-runtime selector and General decision adapter implement this specification. General choice output excludes all probability fields because the General interface exposes no calibrated likelihood evidence. Memory Retrieval passes its existing top-level approval/rewrite schemas through unchanged. The separate hosted Entity resolver route remains a prerequisite. Current reference owners are [Configuration](configuration.md), [Architecture](architecture.md#decision-endpoint-inheritance), and [Behavioral contracts](behavioral-contracts.md#decision-inheritance-contracts); verification is recorded in the [linear execution plan](decision-endpoint-inheritance-linear-plan-2026-10-19.md).

## Problem Statement
Entity Resolution and agentic Memory Retrieval each configure a dedicated decision-model route. Without that route the runtime presently has no common decision endpoint, despite a configured general model being capable of structured JSON completion. Users must configure specialized endpoints rather than simply configuring one general model. Inheritance belongs to the switchboard/runtime composition boundary, not to each cognition consumer.

## Target Architecture
For each decision consumer, resolve at runtime construction in this exact order:
1. Its operation-specific decision endpoint.
2. Shared `models.general_decision` endpoint.
3. `models.general` adapted to `DecisionEndpoint`.
4. No route only if all three are absent; retain the consuming operation's existing unavailable-route policy.

This is **configuration precedence**, not automatic retry/failover on transport failures. Once selected, one endpoint handles the request. Avoid copying this precedence into individual consumers.

`models.general_decision` uses existing `DecisionModelEndpoint` configuration schema and transport. `models.general` retains `GeneralModelEndpoint`, provider authentication, provider-specific transport, optional reasoning effort, and its current JSON-completion capability. A single reusable adapter wraps an `Arc<dyn GeneralEndpoint>` (or equivalent) and implements `DecisionEndpoint::evaluate(state, questions)` by requesting schema-constrained JSON matching the decision-response contract.

Call direction: switchboard/config -> configured runtime endpoint construction -> runtime route injection -> Entity Resolution / Memory Retrieval. Cognition consumers remain unaware of which tier was selected. The adapter must not call the decision HTTP protocol on a general endpoint URL.

## User / Developer Outcomes
- A valid `models.general` alone supplies both decision routes without requiring specialized configuration.
- `general_decision` supplies both unless individually overridden.
- Existing explicit specialized routes remain authoritative.
- Future decision consumers reuse the same resolution helper.
- No duplicated credentials, inheritance settings, or consumer-specific fallback ladders.

## Implementation Decisions
1. Add optional `general_decision: Option<DecisionModelEndpoint>` to `ModelSwitchboardConfig`, corresponding known config object `models.general_decision`, using existing decision schema V1, validation and API-key credential policy. Preserve old config readability; absent field is None. No config-wide schema bump solely for adding an optional known object. Update CLI display/set/clear and tests.
2. Centralize selection in the configured runtime/switchboard boundary with a route-identifying enum or small function; it returns a resolved *kind* (dedicated decision endpoint or adapted general endpoint), not a misleading `&DecisionModelEndpoint` for general-model configurations. Authentication is resolved from the endpoint actually selected.
3. Reuse `ConfiguredDecisionEndpoint` for specialized and shared decision routes. Prefer one constructor taking the selected decision route over new duplicated HTTP clients. Keep existing request payload `{model,state,questions}`.
4. Provide a single `GeneralEndpoint`-to-`DecisionEndpoint` adapter. Use `complete_json` with a response schema built from the supplied questions; require `answers`, keyed question IDs, answer types, selected choice and validated allowed choices. Do not fabricate model-derived choice probabilities or numerical confidence. If the general completion cannot supply reliable probabilities required by a particular consumer, return a safe nonterminal/invalid decision outcome according to that consumer's established policy; do not silently mark a decision 0.95 confident. Explicitly test this with Entity Resolution's threshold and margin rules.
5. Keep Entity Resolution's distinction between a terminal high-confidence decision fast path and its existing general-generative resolver. Preserve deterministic rejection guards, lexical/normalized constraints, and its general resolver fallback. The decision route hierarchy does not replace the latter.
6. Keep Memory Retrieval's decision response validation and existing iteration/error behavior; confirm its actual call site and contract during implementation. No change to retrieval strategy or admission semantics.
7. Never fall back from a configured specialized decision endpoint to another tier *because of runtime failure*. Existing consumer error handling may handle failure or decide an independent operation-level fallback; this is not route substitution.
8. Existing `models.entity_resolution` is a distinct *general resolver* configuration; do not rename or conflate it with `models.entity_resolution_decision`. Do not change unrelated model routes.
9. Preserve real provider-specific authorization paths; `general_decision` accepts API-key auth like current specialized decisions, while general-model fallback uses its existing provider auth (including Codex support).

## Migration Strategy
Compatibility-preserving external config extension; internally adopt one canonical route-resolution implementation in a single change. Existing saved configurations remain loadable and preserve explicit route priority. Do not add legacy aliases or compatibility wrappers for new optional settings.

## Implementation Sequence
1. Add constant/config-field, binary persistence, validation, credential checks, CLI commands/display, and serialization round-trip tests for `general_decision`.
2. Implement canonical decision-route selection including provenance of specialized / general_decision / general, and endpoint construction with correct authentication; test all precedence combinations.
3. Build the reusable general-model decision adapter and validate schema, parsing, invalid choices, missing answers, malformed JSON, transport errors and confidence handling with simulated general endpoints.
4. Wire Entity Resolution and Memory Retrieval routes through central selection. Preserve entity fast-path fallback and retrieval failure semantics; test both using general-only configuration.
5. Run targeted and whole-repository verification; update user-facing model configuration documentation.

## Testing Decisions
- `model_switchboard_tests`, `config_route_tests`, configuration round-trip: each tier, missing endpoint, precedence, credential type, old config decoding, set/clear.
- `entity_resolution_decision` and `entity_resolution_engine`: high-confidence dedicated model bypasses general resolver; ambiguous and invalid general-derived decision does not force identity merges; creation/unresolved/reject cannot be treated as existing matches; deterministic guards precede inference.
- Memory Retrieval route and decision-controller tests: decision endpoint available under general-only and general-decision-only setups, structured output validated, errors not silently retried on lower tier.
- `runtime_host_route_tests` and full runtime construction: effective models, auth headers, route attachment and no accidental provider mismatch.
- Contract tests for decision and general JSON transport with simulated endpoints, without real network credentials.
- Compile + targeted tests after each logical patch, then full `cargo test` and relevant CLI/integration suites at final gate. Document unrelated baseline failures rather than masking them.

## Out of Scope
- Inference model benchmarking and cost optimization.
- Changing Entity admission/reconciliation rules.
- Reworking agentic retrieval algorithm.
- A general automatic transport-retry or runtime model failover framework.
- Adding new arbitrary cognition capabilities.
- Changing the binary decision protocol.

## Completion Gates
1. Every decision consumer uses the same precedence rule; no duplicated consumer fallback chain.
2. General-only runtime configuration supplies valid selected decision endpoints for both operations.
3. General-decision and specialized overrides behave as specified.
4. General-model adapter never invents confidence; unsafe merges are prevented and appropriate fallback/error behavior is tested.
5. Old saved configurations load; new config round-trips; CLI and credential checks match the resolved endpoints.
6. No silent failover after runtime errors.
7. Targeted and whole-repository tests pass or pre-existing failures are precisely recorded.
8. Arcana structural re-check where the repository snapshot is current, plus direct source review confirming no stale selection paths.

## Further Notes / Verified Baseline
- `src/runtime_inference/model_switchboard.rs` defines distinct `DecisionModelEndpoint` and `GeneralModelEndpoint` and separate Entity Resolution/Memory Retrieval decision configuration.
- `src/runtime_inference/configured_decision.rs` posts `{model,state,questions}` directly to a dedicated decision service.
- `src/runtime_inference/configured_general.rs` already supports provider-specific `complete_json`.
- `src/runtime_inference/configured_runtime.rs` currently only constructs decision routes for explicitly configured specialized endpoints.
- `src/perception/entity_resolution_decision.rs` requires selected-choice probability >=0.95 and margin >=0.15 before terminal fast resolution. This makes the general adapter's probabilistic contract the key safety seam.
- `src/config_security/config.rs`, `src/runtime_inference/model_auth.rs` and `cli/src/config_cmd.rs` own persisted config, auth, and CLI route representation.
- Arcana stats was accessible on 2026-10-19; structural claims above were also checked against source, not Arcana alone.

## Related docs

- [Local configuration](configuration.md)
- [Architecture](architecture.md#decision-endpoint-inheritance)
- [Behavioral contracts](behavioral-contracts.md#decision-inheritance-contracts)

## Notes

Implementation evidence and remaining verification are tracked in the linear execution plan. Existing explicit Entity resolver prerequisites remain unchanged.
