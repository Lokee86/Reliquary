# Model and provider runtime

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe current model endpoint abstraction, route selection, authentication, and OpenAI-compatible provider execution used by semantic subsystems.

## Overview

Reliquary separates semantic operation ownership from model transport. `reliquary-switchboard` is the independently compiled, no-internal-Reliquary-dependency owner of model routes, credentials, General/Decision contracts, provider transports, and low-level config-document I/O. The root product reexports its canonical types and composes endpoints with semantic services; Switchboard does not own prompts, decision criteria, or storage authority.

## Endpoint classes

The runtime exposes:

- `GeneralEndpoint` for structured/general inference;
- `DecisionEndpoint` for constrained choices without fabricated calibrated confidence;
- `EmbeddingEndpoint` for vector generation; and
- capability-specific route selection for Insomnia, Entity work, Memory retrieval, Chronos, Dream, and Embedding.

## Route inheritance

Configured decision routes use the documented precedence:

```text
operation-specific decision
  -> general_decision
  -> adapted general endpoint
  -> unavailable
```

General-model adaptation validates the constrained output contract. Runtime failure of a chosen endpoint is not permission to silently switch to another model after semantic work has begun.

General capability routes use their own explicit inheritance contracts; absent dedicated routes may fall back only where the capability contract says so.

## Authentication

Credentials are resolved by provider/auth kind before runtime use. Authorization headers are constructed without exposing secret material through Debug/log output.

OpenAI-ready API-key and Codex/ChatGPT OAuth paths validate the credential kind expected by the selected provider. Account IDs are carried only where required.

## Provider transports

`crates/reliquary-switchboard/src/` contains OpenAI-ready General and Embedding transport and Codex Responses, device-auth and refresh support. Provider-specific response parsing remains below endpoint interfaces. The product retains a single adapter for vector normalization.

Provider retries or remote operation recovery must not change semantic operation identity.

## Failure behavior

- missing/wrong credential kind fails before request execution;
- unavailable route remains explicit;
- structured output is validated by the owning endpoint contract;
- provider auth secrets are redacted from debug surfaces; and
- a provider transport cannot mutate REL/PHY semantic state directly.

## Code Map

- `crates/reliquary-switchboard/src/**` — routes, contracts, credentials, config document, and provider transports
- `src/config_security/config.rs`, `config_object.rs`, `runtime_config.rs` — product config wrapper and domain payload validation
- `src/runtime_inference/configured_runtime*.rs` — product runtime composition
- `src/runtime_inference/embedding_endpoint.rs` — vector normalization boundary adapter
- `src/runtime_inference/runtime_vector_step.rs`

## Related docs

- [Configuration](../configuration.md)
- [Architecture](../architecture.md)
- [Rust API](../api.md)
- [Agentic Memory retrieval](agentic-memory-retrieval.md)
- [Chronos](chronos.md)

## Notes

Model routing is replaceable infrastructure. Semantic authority remains with the calling subsystem.
