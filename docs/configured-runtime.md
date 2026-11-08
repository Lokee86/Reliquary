# Configured provider runtime

**Operational target for OC02.** See the [Switchboard configuration restoration specification](switchboard-configuration-restoration-spec-2026-11-08.md). The canonical local configuration is the existing binary `reliquary.cfg` plus its companion `reliquary.master-key.json`. The interim JSON/environment bootstrap is superseded and is not a supported product format.

## Loading configuration

The public Root API opens the existing configuration directly and constructs the provider runtime from its model routes and encrypted credential objects.

```rust
use reliquary_memory::ConfiguredRuntime;

let runtime = ConfiguredRuntime::from_path("reliquary.cfg")?;
println!("{}", runtime.readiness());
# Ok::<(), reliquary_memory::ConfiguredRuntimeError>(())
```

`ReliquaryConfig::open(path)` exposes settings, model routes, and credential IDs and kinds. Use `configured_runtime()` to bind endpoints from the opened configuration. `ReliquaryConfig::new(path)` prepares a new configuration in memory with the product defaults.

The companion key path is derived from the config path. For `reliquary.cfg` it is:

```text
reliquary.master-key.json
```

Opening, readiness inspection and runtime construction are read-only. A master key is created only when an explicit configuration mutation needs one and no key exists.

## Persisted configuration

The existing `CVCFG\0\r\n` 1.0 format remains canonical. Switchboard owns the document framing, unknown-object preservation, atomic replacement, model-route codecs, encrypted credential objects and master-key handling. Root owns the product-domain configuration wrapper.

Existing route objects include:

- `models.general`
- `models.insomnia`
- `models.insomnia_metadata`
- `models.entity_extraction`
- `models.general_decision`
- `models.entity_resolution_decision`
- `models.memory_retrieval_decision`
- `models.entity_resolution`
- `models.dream`
- `models.embedding`

Existing product objects such as `archive.fragments`, `retrieval.default`, and `runtime.default` are preserved. Configured passive search uses the stored Retrieval policy. Fragment settings and Dream inference concurrency are available for inspection and editing. Unknown objects survive saves byte-for-byte.

Credentials remain encrypted `credential.<id>` objects in the config. The companion master-key backend remains the existing version-1 JSON file until a separately approved security change replaces it.

## Selection

The configured route is selected once per operation:

- Insomnia: dedicated -> General.
- Dream: dedicated -> General.
- Entity Resolution decision: dedicated -> General Decision -> adapted General -> unavailable.
- Entity Resolution generative resolver: explicit `models.entity_resolution` when the worker needs a resolver.
- Memory Retrieval decision: dedicated -> General Decision -> adapted General -> unavailable.
- Chronos decision: General Decision -> adapted General -> unavailable.
- Embedding: explicit compatible embedding route only.

A missing dedicated Memory Retrieval route is valid when shared/general fallback exists. Failure of a selected provider is not permission to silently select a lower tier.

## Credential lifecycle

The rebuilt CLI/API restores the established operations:

```text
reliquary --config reliquary.cfg config show
reliquary --config reliquary.cfg config verify
reliquary --config reliquary.cfg config credential list
reliquary --config reliquary.cfg config credential add-api-key <ID>
reliquary --config reliquary.cfg config credential login-codex <ID>
reliquary --config reliquary.cfg config credential refresh-codex <ID>
reliquary --config reliquary.cfg config credential remove <ID>
```

API keys are collected without placing them on argv. Codex login uses device authorization; refresh uses the encrypted stored refresh token. Configuration writes atomically replace the file and preserve unrelated/unknown objects. Secret material is never included in readiness, show, logs or Debug output.

`--config` defaults to `reliquary.cfg` in the current directory. `add-api-key` uses a hidden prompt, or accepts one line on standard input with `--stdin`. Use `config model set-<route>` and `config model clear-<route>` for the ten route names above, replacing underscores with hyphens and omitting the `models.` prefix.

## Checks

`config verify` validates framing, known product/model objects, credentials and route compatibility without contacting providers or rewriting files.

`config verify --online` sends synthetic inputs through configured transports after local validation. Add `--route <label>` to probe just one configured transport. Reports contain route and failure categories.

`probe_embedding_profile()` retains the existing bounded compatibility-probe behavior for the configured embedding endpoint.

### Offline compatibility evidence

The rebuilt CLI opened a disposable copy of the existing configuration and companion key, listed five credential IDs and kinds, and verified seven configured routes. A fresh process reported the same readiness. SHA-256 checks matched before and after the read-only commands for both originals and both copies.

The copied configuration had no dedicated Memory Retrieval, shared General Decision, or General route. On a separate disposable copy, adding General selected the adapted General fallback. Adding shared General Decision then selected that route without a dedicated Memory Retrieval object. Clearing the temporary routes restored the initial readiness. The live provider probe remains pending credential authorization.

## Related docs

- [Switchboard configuration restoration](switchboard-configuration-restoration-spec-2026-11-08.md)
- [Public API / CLI operational completion](public-api-cli-operational-completion-spec-2026-11-07.md)
- [Public product API](public-product-api.md)

## Notes

The previous Reliquary configuration documentation in `Reliquary-reference/docs/configuration.md` is the behavior oracle for the persisted format. OC02 selectively restores that Switchboard ownership; it does not reintroduce the historical Root implementation.
