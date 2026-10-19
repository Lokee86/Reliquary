# Decision Endpoint Inheritance — Linear Execution Plan
Date: 2026-10-19
Status: Complete — 22/22 steps closed; verification limits recorded below
Source: docs/decision-endpoint-inheritance-spec-2026-10-19.md

## Purpose

Define and track compatibility-preserving configured decision endpoint inheritance.

## Overview

Specialized routes override a shared decision service, which overrides a structured General adapter. Runtime failures do not change the selected tier, and the General adapter never manufactures confidence.

## Execution rules
Execute strictly in order. Each step changes one bounded responsibility and has a clear acceptance gate. Mark a step complete only after its specified tests/checks pass; record commit, affected files and any baseline failures. Do not expand scope, add compatibility aliases, silently route around failed configured endpoints, or invent decision probabilities. Keep implementation narrowly scoped; no model benchmarks, entity policy changes, or retrieval redesign.

## A — Configuration (Steps 1–6)
- [x] **1. Switchboard field.** Add optional `general_decision: Option<DecisionModelEndpoint>` to `ModelSwitchboardConfig`, and read-only access in `ModelSwitchboard`. Gate: compile; absence remains `None`. Files: `src/runtime_inference/model_switchboard.rs`.
  - Commit: none (uncommitted working-tree change).
  - Verification: `cargo check` passed; `entity_routes_are_explicit_and_do_not_fall_back` confirms unset shared route remains `None` (also covered by the focused switchboard suite).
  - Blockers: none.
- [x] **2. Config binary codec.** Add `models.general_decision` key with the existing decision schema, load/save. Gate: new config round-trip and old config without key still loads. Files: `src/runtime_inference/model_switchboard_codec.rs`, `src/config_security/config.rs`, config tests.
  - Commit: none (uncommitted working-tree change).
  - Verification: `general_decision_config_round_trips_and_old_configs_leave_it_unset` passed (1 test); `config_tests` passed (7 tests). Old configs decode with `general_decision == None`.
  - Blockers: none.
- [x] **3. Route validation.** Apply existing decision endpoint URL/model validation to `general_decision`; avoid changing specialized policies. Gate: good config accepted; malformed configuration rejected. Files: `model_switchboard.rs`, tests.
  - Commit: none (uncommitted working-tree change).
  - Verification: `general_decision_route_validation_matches_existing_decision_routes` passed for a valid endpoint and rejected blank model and non-HTTP URL scheme.
  - Blockers: none.
- [x] **4. Credential validation.** Validate shared-decision API-key credential and expose resolved auth without altering provider-based general auth. Gate: missing/wrong-kind credentials rejected; valid API key accepted. Files: `src/runtime_inference/model_auth.rs`, switchboard tests.
  - Commit: none (uncommitted working-tree change).
  - Verification: `general_decision_requires_api_key_and_resolves_its_authentication` passed for missing credential, OAuth credential, and API-key bearer auth; general provider auth tests remain green.
  - Blockers: none.
- [x] **5. CLI configuration commands.** Add set/clear/show for `general_decision` using existing CLI model-command patterns. Gate: CLI route tests cover all actions. Files: `cli/src/config_cmd.rs`, relevant CLI command parsing/tests.
  - Commit: none (uncommitted working-tree change).
  - Verification: focused CLI command parse and set/show/clear tests passed (2 tests).
  - Blockers: none.
- [x] **6. Config integration check.** Update config literals/tests impacted by new field, verify persisted config and unchanged specialized routes. Gate: targeted config, switchboard and CLI suites pass, or precise pre-existing failure recorded.
  - Commit: none (uncommitted working-tree change).
  - Verification: `model_switchboard_tests` passed (24 tests), `config_tests` passed (7 tests), full CLI suite passed (32 tests), and `rustfmt --check` passed for all changed Rust files. Specialized entity decision routes remained intact in persistence and CLI tests. Baseline repository warnings remain; no new failures observed.
  - Blockers: none.

## B — Selection (Steps 7–10)
- [x] **7. Selection representation.** Introduce a private route-selection kind describing selected dedicated/shared decision config versus general model. No network instantiation yet. Gate: compile; no consumer behavior changes.
- [x] **8. Precedence function.** Central function returns operation-specific → general_decision → general → unavailable. Gate: table-driven tests for both consumers, all three tiers, mixed overrides, and no routes.
- [x] **9. Decision-service construction.** Reuse the existing `ConfiguredDecisionEndpoint` transport for both specialized and shared decision routes, with selected credential. Gate: endpoint model/URL/auth tests; no extra HTTP protocol.
- [x] **10. Selection integration check.** Exercise selector plus endpoint construction using simulated routes, including invalid-config and missing-credential outcomes. Gate: targeted route tests green; no calls switched yet.

Selection verification (steps 7–10): private `configured_runtime_decision.rs`; `cargo check` passed at step 7; 4 selector/construction tests passed, including all precedence masks, mixed overrides, selected URL/model/auth/wire payload, invalid configuration and missing credentials. Switchboard suite passed 24 tests after stricter shared URL validation. Commit: none (working tree). Blockers: none.

## C — General-model adapter (Steps 11–15)
- [x] **11. Adapter shell.** Add reusable `GeneralEndpoint`→`DecisionEndpoint` adapter with delegated `model()` and injected general endpoint. Gate: compile and model identity test.
- [x] **12. Structured decision request.** Map `state` and `questions` to `complete_json` prompt/payload/schema, preserving question IDs, types, and allowed choices; reuse provider-specific general endpoint and credentials. Gate: simulated general endpoint receives expected schema/request.
- [x] **13. Structured response validation.** Validate the required `answers` object, per-question type, declared choice and permitted values; reject malformed/unknown answers. Gate: positive/negative contract tests. Do not change dedicated decision protocol.
- [x] **14. Confidence safety.** Never fabricate or inflate probabilities. A general completion without demonstrably valid probabilities cannot terminally satisfy Entity Resolution's >=0.95 confidence and >=0.15 margin; it must proceed through its existing safe generative fallback. Gate: low/missing/ambiguous confidence tests plus no unsafe identity merge.
- [x] **15. Adapter failures.** Translate general endpoint transport/schema errors without retrying a lower tier. Gate: deterministic failure tests; confirm both OpenAI-ready and Codex general transports are exercised at their existing seam.

Adapter verification (steps 11–15): private `general_decision.rs` wraps `Arc<dyn GeneralEndpoint>`, delegates model identity, builds strict choice-answer schemas without probabilities, validates answer IDs/types/choices and rejects missing, unknown or extra fields; embedded Memory Retrieval schema is passed through untouched. `cargo test --lib general_decision::tests` passed 6 tests; `cargo test --lib seam_tests` passed 4 local-loopback provider tests through `ConfiguredGeneralEndpoint`, covering Ready JSON schema and malformed JSON plus Codex OAuth/account/reasoning/SSE and failed SSE response. `cargo test --lib general_model_choice_without_calibrated_confidence` passed the Entity no-unsafe-merge regression. `cargo fmt --check` passed. Commit: none (working tree). Blockers: none.

## D — Consumer wiring (Steps 16–19)
- [x] **16. Runtime composition.** `ConfiguredRuntime::runtime_routes` selects once per consumer, reuses the configured dedicated/shared transport, and wraps the already-constructed general route for General selection. The existing `Arc<dyn DecisionEndpoint>` route fields are unchanged. `runtime_routes_select_each_tier_with_consumer_specific_auth_and_contracts` passed for both consumers and all three tiers, asserting model, URL, credential, and request shape. Commit: none (working tree). Verification: 1 integration test passed (six route/consumer cases). Blockers: none.
- [x] **17. Entity Resolution tests.** `entity_engine_uses_selected_general_or_shared_decision_with_existing_resolver` passed for general-only and general_decision-only configurations paired with a separate existing resolver; the choice without confidence remains unresolved and does not merge. The existing `entity_resolution_engine::tests` suite passed 22 tests, covering guards, exact/normalized restrictions, decision fast path, and resolver fallback. The hosted Entity worker still requires its separately configured `models.entity_resolution` resolver route. Commit: none (working tree). Verification: integration test plus 22 engine tests passed. Blockers: none.
- [x] **18. Memory Retrieval tests.** `memory_selection_and_rewrite_controllers_use_selected_general_or_shared_route` passed for general-only and general_decision-only configurations. It exercised the actual selection and rewrite controllers and asserted versioned state, schema pass-through, selected owner-qualified approval, and complete-query rewrite. Existing `selection_tests` passed 3 tests and `rewrite_tests` passed 2 tests. Commit: none (working tree). Verification: integration test plus 5 controller tests passed. Blockers: none.
- [x] **19. Runtime failure policy.** `selected_dedicated_or_shared_failure_does_not_retry_a_lower_tier` passed for a failing Entity dedicated route and a failing Memory shared route with lower tiers configured; each selected endpoint received exactly one request and no lower-tier request was made. Commit: none (working tree). Verification: 1 integration test passed for both consumer-specific cases. Blockers: none.

## E — Integration / closeout (Steps 20–22)
- [x] **20. Configuration documentation.** Document the three-tier inheritance, `general_decision`, specialized overrides, difference between selection and operation-level fallback, and decision probabilities caveat. Gate: docs match actual CLI/config schema. Verification: current configuration, CLI, architecture/API, Entity state, behavioral contracts, coverage/maintainer navigation and limitations updated against source; both documentation compliance modes passed. Commit: none (working tree).
- [x] **21. Full verification.** Run formatting, compilation, focused suites, and applicable full workspace/integration/CLI suites. Gate: formatting and compilation passed for library/CLI; all 32 CLI tests and all 157 integration tests passed. Corrected library rerun passed 1,518 tests with five ignored and two unchanged long recovery tests filtered; those two tests passed in the preceding complete run. The only discovered assertion regression was an outdated diagnostic expectation, corrected and verified. Unrestricted all-target example build hit Windows paging-file exhaustion; serial correctness compilation and suites passed. Commit: none (working tree).
- [x] **22. Structural audit and handoff.** Direct source search and Arcana if current: ensure both consumers share the canonical selector, no duplicated precedence/legacy adapters, auth paths remain correct. Update spec status and record implementation commits plus verification. Gate: direct source review confirms one canonical selector invoked once per decision consumer, one reusable General adapter, selected credential paths, and no runtime cross-tier retry. Spec status and current docs updated. Pitlord passed 23 rules on the retained graph; fresh Lexicon/Arcana analysis did not complete and no fresh-graph pass is claimed. Commit: none (working tree).

## Constraints and acceptance
A valid `models.general` alone must select an effective decision endpoint for both consumers. `general_decision` overrides `general`; dedicated operation endpoints override both. Selection happens once during runtime composition, not dynamically per failed inference. The general adapter may never assert confidence that wasn't actually supplied/validated, and must not allow unsafe Entity merges. Old config records continue loading. The existing dedicated decision API wire format remains untouched.

## Tracking
Each checkbox is a single executable step. Implementation owner should append `Commit:`, `Verification:`, `Blockers:` under any step only once performed. The plan had 22 steps, zero completed at creation. All 22 execution steps are closed with the verification results and environment limits below. No implementation commit was created.

## Related docs

- [Local configuration](configuration.md)
- [Architecture](architecture.md#decision-endpoint-inheritance)
- [Behavioral contracts](behavioral-contracts.md#decision-inheritance-contracts)

## Completion report

Implementation remains uncommitted in the existing working tree; no unrelated user changes were reset. Luna subagents implemented configuration/selection and the adapter/runtime integration under the plan's ordered gates.

### Verification record

- Library and CLI formatting: passed. Library `cargo check` and locked CLI check: passed.
- Locked CLI suite: 32 passed, zero failures.
- All 24 integration targets compiled with `cargo test --tests --no-run -j 1`; their freshly compiled binaries passed all 157 tests, zero failures/ignored.
- The initial complete library run had 1,519 passed, five ignored, and one stale diagnostic-string assertion. Both long historical crash-prefix recovery tests passed. The assertion was corrected to the new generic decision-endpoint diagnostic; no production behavior changed. After the test-only correction, `cargo test --lib -j 1` with the two already-passing long recovery cases skipped passed 1,518 tests, zero failed, five ignored, two filtered (130.01 seconds). Thus all 1,520 non-ignored library cases have passing evidence against the unchanged production implementation; the diagnostic test passes with its corrected assertion.
- Structural and `--changed-from origin/main` documentation checks: both passed after current-state docs and plan/spec navigation were updated.
- Both ownership source guards passed; Pitlord policy validated and enforcement passed 23 rules against the retained graph. The required fresh Lexicon/Arcana attempt did not produce a usable current source snapshot during the bounded run and was stopped. Direct source review verifies the two composition calls share one selector, one General adapter, selected auth, and no failure-driven tier retry. No current graph pass is claimed.

### Environment limits

Unrestricted `cargo test` could not complete its parallel all-target build: Windows returned paging-file exhaustion (OS error 1455) while mapping the large library archive. A serial all-target retry reached roughly 12 GB in an unrelated tuning-example debug linker and was stopped. Serial compilation of all library/integration test targets and direct execution of the full correctness suites provided the test coverage above. Example-wide build completion and refreshed semantic-graph enforcement remain explicit limitations; they are not represented as passing checks.

### Documentation impact

- Inspected: required architecture, invariants, storage format, maintainer map, limitations and roadmap; configuration, CLI, API, Entity state, behavioral contracts and coverage; source specification and linear plan.
- Updated: configuration/CLI references, architecture/API decision boundaries, Entity state, limitations, behavioral tests matrix, coverage/maintainer navigation, docs index, source spec and this execution record.
- Not affected: REL/PHY semantic storage, version clocks, credential encryption format, retrieval strategy, Entity admission policy and background scheduling prerequisites.
- Compliance check: both required documentation modes passed.
- Known documentation gaps: pre-existing truncation markers in architecture/API prose remain outside this feature; this change adds current decision references without reconstructing unrelated missing text.

### Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: configured runtime owns the canonical precedence and construction; the private General adapter owns request/answer adaptation. Consumers retain existing semantic validation and operation-level error handling. No mutable semantic owner or durable state was added.
- Pitlord enforcement impact: existing ownership areas/policy retained; 23 rules pass on the retained graph. Fresh source graph remains unverified as stated above.
- Other verification impact: selector matrices, URL/auth/config compatibility, CLI routes, real Ready/Codex transport fixtures, confidence safety, actual consumer integration and no-cross-tier-retry tests added; all integration and CLI suites passed.
- Known architectural gaps: fresh Lexicon/Arcana coverage could not be established. Hosted Entity processing deliberately still requires its separate `entity_resolution` resolver route.

## Notes

Implementation evidence and remaining verification are tracked in the linear execution plan. Existing explicit Entity resolver prerequisites remain unchanged.
