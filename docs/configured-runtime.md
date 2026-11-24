# Configured runtime

Parent index: [Documentation](./INDEX.md)

## Purpose

This guide explains how to prepare, protect, inspect and use Reliquary's provider configuration without exposing credentials.

## Overview

The canonical local file is the existing binary `reliquary.cfg` (`CVCFG` 1.0). Root's `ReliquaryConfig` wraps Switchboard's persisted objects; `ConfiguredRuntime::from_path` validates and binds selected routes offline. Configuration supports model routes, encrypted credential objects, archive fragment policy, retrieval weights/limits and Dream inference concurrency. Unknown nonreserved objects are preserved during updates.

Hosts that own inference transports can use `ConfiguredRuntime::hosted(general, embedding, identity)` without opening or creating CVCFG. The `GeneralEndpoint` and `EmbeddingEndpoint` implementations remain host-owned; the identity records non-secret provider/model names, embedding dimensions, shared input semantics and normalization. Root validates the endpoints' reported model, dimensions and normalization against that identity. The supplied general endpoint serves ordinary general/entity-resolution semantic routes and the existing general-backed Chronos, entity-resolution and memory-retrieval decision adapter. Insomnia and Dream keep their existing general fallback, and retrieval settings and worker defaults remain Root-owned. Hosted readiness is offline; `online_probe` is the explicit synthetic provider boundary. Credential values, profile names and worker leases do not enter the processing route identity. Before first binding hosted vector metadata, a host can call the authorized read-only `Instance::has_active_vectors(owner)` summary; an existing vector set with no host identity marker must be treated as unknown provenance.

`Gateway::set_runtime(Some(&runtime))` supplies the Entity route for both initial and automatic cross-REL comparisons. A trusted caller can replace the binding or clear it with `None`. Eligible sessions can complete locally decidable work before inference becomes available. [Cross-REL Entity identity](public-product-api.md#cross-rel-entity-identity)

The same runtime binding starts Perception for authenticated `ManageRel` sessions on writable RELs. Configure General, Embedding and `general-decision` with model `typesafe/jev-1.13`. Observation contribution and Verify use the native shared decision endpoint. Hosted General adapters leave those consumers unavailable. [Same-REL Observations](public-product-api.md#same-rel-observations)

```rust
let identity = HostedInferenceIdentity::new(
    "host-runtime", "general-model", "host-runtime", "embedding-model", 1024,
    HostedEmbeddingInputSemantics::Shared, EmbeddingNormalization::L2,
)?;
let runtime = ConfiguredRuntime::hosted(general, embedding, identity)?;
```

The public Root crate reexports the endpoint traits and errors, `HostedInferenceIdentity`, `HostedEmbeddingInputSemantics`, and `SwitchboardEmbeddingMode` for this boundary. Root's existing `EmbeddingMode` remains the vector-workflow mode.

## Create and validate configuration

Prerequisites: a writable config location and a supported OS credential service. For the standard local profile, `ReliquaryConfig::new(path)` prepares product defaults and route definitions in memory; it creates no credential objects, API keys or OAuth tokens until a credential is added and saved.

```text
reliquary --config reliquary.cfg config show
reliquary --config reliquary.cfg config verify
```

If the file does not yet exist, `config show` displays the standard defaults without saving them. `config verify` opens the file and prints selected-route readiness without contacting a provider. Expected result is JSON containing `verified: true` and a readiness report. Route absence, invalid objects, missing credentials or inaccessible master key are reported as errors; they do not silently become provider success.

### Add credentials

API keys are prompted without echo. `--stdin` reads one bounded line (maximum 64 KiB); use a protected pipe, not a literal argument:

```text
reliquary --config reliquary.cfg config credential add-api-key embedding
reliquary --config reliquary.cfg config credential list
```

The first saved credential creates a random master key in the operating system credential service, bound to the canonical config path, before encrypted credential data is committed. Windows uses Credential Manager, macOS Keychain and Linux Secret Service. A copied or moved config file does not carry the key to the new canonical path. An encrypted config whose key is missing or inaccessible fails closed. `login-codex`, `refresh-codex` and `remove` manage Codex credentials; see the [CLI reference](reference/cli.md).

### Route selection

`config model set-ROUTE` stores a model route; `clear-ROUTE` removes it. Route labels are `general`, `insomnia`, `insomnia-metadata`, `entity-extraction`, `general-decision`, `entity-resolution-decision`, `memory-retrieval-decision`, `entity-resolution`, `dream` and `embedding`. General and embedding routes select `openai-codex` or `openai-ready`; decision routes are OpenAI-compatible URLs. Configure route credential IDs to match entries in `config credential list`.

Fresh `ReliquaryConfig::new` standard model defaults are:

| Slot | Provider/model | Additional default |
| --- | --- | --- |
| General | OpenAI Codex `gpt-5.6-luna` | low reasoning; credential ID `codex-alt` |
| Insomnia | OpenAI Codex `gpt-6-sol` | low reasoning; `codex` |
| Insomnia metadata | OpenAI Codex `gpt-6-luna` | low reasoning; `codex` |
| Entity extraction | OpenAI Codex `gpt-5.6-sol` | low reasoning; `codex-alt` |
| Entity resolution | OpenAI Codex `gpt-5.6-sol` | low reasoning; `codex-alt` |
| Dream | OpenAI Codex `gpt-5.6-luna` | low reasoning; `codex-alt` |
| Entity-resolution decision | OpenAI-compatible `typesafe/jev-1.13` | OpenRouter decisions URL; `embedding` |
| Embedding | OpenAI-compatible `qwen/qwen3-embedding-8b` | OpenRouter embeddings URL; 1024 dimensions; L2; `embedding` |

The standard profile does not create credentials for those IDs. Switchboard selects routes as follows:

| Consumer | Route selection |
| --- | --- |
| Insomnia | `models.insomnia`, then General |
| Insomnia ownership metadata | `models.insomnia_metadata`, then selected Insomnia route, then General |
| Dream | `models.dream`, then General |
| Entity generative resolution | explicit `models.entity_resolution` only; otherwise unavailable |
| Chronos decision | General Decision, then adapted General, then unavailable |
| Entity-resolution decision | dedicated route, then General Decision, then adapted General, then unavailable |
| Memory-retrieval decision | dedicated route, then General Decision, then adapted General, then unavailable |
| Observation contribution and Verify | native General Decision with `typesafe/jev-1.13`, then unavailable |
| Embedding | explicit compatible Embedding route only |

Entity extraction is its own general route and is not an Entity-resolution decision route. A selected route's provider failure is reported; Switchboard does not silently step down to another route after a request fails. Do not infer route readiness from a stored model label alone.

## Product settings and CVCFG contract

Root stores its product settings as three opaque product objects in the Switchboard CVCFG file. These are binary records, not editable text settings. Use `ReliquaryConfig` setters and `save()` to update them; do not hand-edit or construct encrypted config files.

| Product object | Fields in payload order | Default | Validation |
| --- | --- | --- | --- |
| `archive.fragments` | `turns`, `overlap` (`u64` little-endian each) | 8 turns, 2 overlap | `turns` must be positive and `overlap < turns`; otherwise the object is invalid. |
| `retrieval.default` | candidate limit, result limit (`u64` LE), lexical weight, semantic weight (`f64` bit patterns encoded as `u64` LE) | 30 candidates, 10 results, weights 0.45 and 0.55 | Both limits must be positive, results cannot exceed candidates, candidates cannot exceed 1000, and both weights and their sum must be positive and finite. Accepted weights are normalized when used. |
| `runtime.default` | Dream inference concurrency (`u64` LE) | 12 | Must be positive and representable as the platform's `usize`. |

The public Root settings types and methods are `FragmentConfig` / `set_fragments`, `RetrievalConfig` / `set_retrieval`, and `RuntimeSettings` / `set_runtime_settings`. A setter validates before storing its value in the in-memory configuration and returns `ConfiguredRuntimeError::Invalid` with the corresponding object name for invalid settings. `open()` applies the same validation to stored objects; a wrong schema, nonzero flags, wrong payload length, invalid field values, or an integer that cannot fit `usize` rejects that object as `invalid product configuration object`.

`ReliquaryConfig::new(path)` creates the standard model-route profile plus these product defaults in memory; it does not create credentials or write a file. `new_blank(path)` creates the same product defaults without installing model routes. `open(path)` reads an existing file and uses product defaults when any of these product objects are absent; malformed present objects fail to open. Setters and route edits remain in memory until `save()`. Saving first validates route binding, so standard routes that reference credentials not yet present fail with a credential error and are not written. Add credentials or configure routes, then save. `validate()` also validates route binding without saving or contacting a provider.

Example for a blank config with no model routes:

```rust
let mut config = ReliquaryConfig::new_blank("reliquary.cfg");
config.set_fragments(FragmentConfig { turns: 8, overlap: 2 })?;
config.set_retrieval(RetrievalConfig {
    candidate_limit: 30,
    result_limit: 10,
    lexical_weight: 0.45,
    semantic_weight: 0.55,
})?;
config.set_runtime_settings(RuntimeSettings {
    dream_inference_concurrency: 12,
})?;
config.validate()?;
config.save()?;
```

CVCFG 1.0 begins with the 12-byte header `CVCFG\0\r\n\x01\x00\x00\x00`, followed by a little-endian `u32` object count. Each entry contains a little-endian `u16` key length, `u16` schema, `u32` flags, `u32` payload length, four reserved zero bytes, then the UTF-8 key and payload. The reader rejects files over 32 MiB, more than 4096 objects, payloads over 16 MiB, duplicate keys, nonzero reserved bytes, malformed/truncated entries, and trailing data. Unknown nonreserved objects are preserved during updates. These limits and framing apply to the whole CVCFG document; encrypted credentials remain Switchboard-owned objects.

## Security and persisted state

Credential values are encrypted in the config and redacted from readiness/inventory reports. The master key is held by the OS credential service, not a sibling plaintext file. The service record is keyed by config path. A legacy companion `<config-stem>.master-key.json` may be imported only by the guarded migration path: Switchboard authenticates existing ciphertext, imports and rereads the key, authenticates again, and deletes the companion only after successful verification. Failed migration keeps the companion for retry and returns an error. The CVCFG file and ciphertext are not rewritten by this key migration.

`config verify --online` is the explicit provider boundary. It sends synthetic probe inputs through configured routes; `--route LABEL` narrows the probe to one route and requires `--online`. Never use real corpus data for a route probe. `ConfiguredRuntime::probe_embedding_profile` also makes bounded synthetic compatibility probes and should be treated as online work.

## Failure and recovery

- Missing/inaccessible OS key: restore the key through the OS credential service or use the original config path; do not discard encrypted credential objects. Legacy migration failures leave the companion available for retry.
- Missing referenced credential: add/authenticate the credential ID or edit the route to an available credential, then rerun offline `config verify`.
- Invalid route or product object: correct it using `config model` or a known-good config backup; keep the original before edits.
- Provider unavailable: distinguish offline readiness from an online probe failure; inspect the reported route category and retry only after correcting its configured endpoint or credential.

## Related docs

- [CLI reference](reference/cli.md)
- [Operator workflow](operator-workflow.md)
- [Switchboard architecture](architecture/switchboard.md)
- [Configuration restoration evidence](switchboard-configuration-restoration-spec-2026-11-08.md)
- [OS master-key storage evidence](os-master-key-store-spec-2026-11-08.md)

## Notes

Root defaults and product-object validation are in [src/configuration.rs](../src/configuration.rs); fragment and retrieval limits are defined in [archive fragment](../crates/reliquary-archive/src/fragment.rs) and [retrieval policy](../crates/reliquary-retrieval/src/lib.rs). Persisted framing is in [Switchboard's config document](../crates/reliquary-switchboard/src/config_document.rs), while route binding, credential encryption and key migration live in [Switchboard protected configuration](../crates/reliquary-switchboard/src/protected_config.rs). `ConfigObject` is a Switchboard implementation type and is not exported as a Root configuration API. Historical OC02 records and disposable-copy checks are evidence, not current provider availability claims.
