# Switchboard

Parent index: [Reliquary architecture](./INDEX.md)

## Purpose

Explain the owner of runtime configuration, protected provider credentials, route selection, and model endpoint adapters.

## Overview

`reliquary-switchboard` owns configuration documents and Switchboard-reserved model objects, credential lifecycle, configured route resolution, and adapters for general, decision, and embedding endpoints. Product-owned configuration objects can coexist in the document, but Switchboard-reserved keys remain Switchboard-owned. Consumers receive endpoint traits or configured routes; semantic services do not read credential files or select their own provider implementation.

Switchboard does not own product workflows, semantic acceptance criteria, durable REL/PHY state, Gateway authorization, or the external host lifecycle. Application and semantic owners decide what operation to perform; Switchboard resolves which configured endpoint can perform it.

## Routing and state

Configuration is opened from the configured path, parsed into typed objects, and resolved into routes. Chronos, Entity resolution and Memory retrieval prefer a consumer-specific decision route, then the shared decision route, then the general route. Observation contribution and verification require the shared native decision route for `typesafe/jev-1.13`. An unavailable route leaves Observation work pending. Embedding and general routes are also resolved through injected endpoint traits, so route policy is separated from transport implementation. Runtime probes verify configured endpoints before consumers rely on them, and callers receive typed configuration, endpoint, status, and backpressure failures.

Root also exposes a hosted construction path for host-owned general and embedding transports. It binds the general endpoint to the ordinary general and entity-resolution semantic slots and adapts it through the existing `GeneralDecisionEndpoint` for Chronos, Entity resolution and Memory retrieval. The accompanying stable identity includes provider/model names and embedding dimensions, input semantics and normalization; it excludes credentials, profiles and leases. The path does not read persisted configuration, and Root validates reported model/dimension/normalization values before exposing the runtime.

For hosts that use Switchboard's route selection while retaining host-owned transports, `HostedSwitchboardConfig` stores credential-free endpoint references in typed `models.<slot>` objects. `ConfiguredRuntime::hosted_switchboard` resolves these references through caller-supplied, side-effect-free adapters, then applies the same decision fallback selector as standalone configuration. Hosted route objects use their own schema and cannot contain Switchboard credentials; opening a file with credential objects or standalone route records fails clearly. Saving merges pending hosted edits under the existing path lock, preserves unrelated objects, and atomically replaces the CVCFG document. Processing identity includes effective endpoint references and resolved adapter identity; the migration-compatible legacy hash is limited to the prior `chat`/`chat` hosted route shape without processing overrides.

A cross-REL Entity receipt binds the selected Entity route by a digest of its provider, model, endpoint URL and reasoning effort. A hosted runtime binds the host's general provider and model. Changing an unrelated embedding route leaves the Entity comparison eligible for reuse.

Perception binds the configured general, native decision and embedding choices to each work attempt. Its receptor vectors use an independent embedding route identity. The Contribution and Verify consumers share a native transport. Hosted runtimes add that transport and its nonsecret identity with `with_native_perception_decision`. Saved hosted route references retain their existing schema and fallback behavior. Perception owns both versioned question templates and their acceptance rules. [Perception semantics](semantic-processing.md#observation-inference)

Credentials and their protected configuration are managed by Switchboard's protected-config boundary. Secret-bearing credential values are redacted from Debug output and protected key lifecycle is isolated from ordinary route data. Credentials are mutable configuration state, not part of REL/PHY owner files. Do not log or expose secrets through product objects, errors, or diagnostics.

## Failure and recovery

Invalid/reserved keys, missing route adapters, failed authentication/device flow, endpoint status failures, invalid responses, and backpressure are explicit errors. Failure to resolve the configured adapter does not silently substitute an unrelated provider. Configuration reopening reconstructs typed route and credential state; a missing protected key or invalid credential state must be surfaced as an error rather than repaired by an endpoint consumer.

## Code map and tests

- Configuration and route policy: [`crates/reliquary-switchboard/src/lib.rs`](../../crates/reliquary-switchboard/src/lib.rs), [`model_config.rs`](../../crates/reliquary-switchboard/src/model_config.rs), [`configured.rs`](../../crates/reliquary-switchboard/src/configured.rs), and [`routes.rs`](../../crates/reliquary-switchboard/src/protected_config/routes.rs).
- Credential protection/lifecycle and transports: [`protected_config.rs`](../../crates/reliquary-switchboard/src/protected_config.rs), [`credentials.rs`](../../crates/reliquary-switchboard/src/protected_config/credentials.rs), [`key_lifecycle.rs`](../../crates/reliquary-switchboard/src/protected_config/key_lifecycle.rs), and [`transport.rs`](../../crates/reliquary-switchboard/src/transport.rs).
- Focused tests are in [`crates/reliquary-switchboard/src/tests.rs`](../../crates/reliquary-switchboard/src/tests.rs), [`configured_tests.rs`](../../crates/reliquary-switchboard/src/configured_tests.rs), [`protected_config/tests/master_keys.rs`](../../crates/reliquary-switchboard/src/protected_config/tests/master_keys.rs), and [`protected_config/tests/specialty.rs`](../../crates/reliquary-switchboard/src/protected_config/tests/specialty.rs). They cover route precedence, secret handling, protected-key lifecycle, config round trips, endpoint validation, and capability-specific adapters.

## Extension seams

Introduce a new credential kind or route slot in Switchboard, with typed validation and lifecycle tests. Keep semantic intent and fallback policy for a product operation with its Application/semantic owner; keep durable product data in owner stores; keep host credentials and host lifecycle outside this crate.

## Related docs

- [Reliquary source and binary distribution terms](../../README.md#license)
- [System architecture](system.md)
- [Semantic processing](semantic-processing.md)
- [Gateway and host lifecycle](gateway.md)

## Notes

This page describes current implementation ownership. It does not establish that any particular provider is configured or available in a deployment.
