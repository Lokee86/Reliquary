# Configured provider runtime

**Operational target for OC02.** See the [Switchboard configuration restoration specification](switchboard-configuration-restoration-spec-2026-11-08.md). The canonical local configuration is the existing binary `reliquary.cfg` plus its companion `reliquary.master-key.json`. The interim JSON/environment bootstrap is superseded and is not a supported product format.

## Loading configuration

The public Root API must open the existing configuration directly and construct the provider runtime from its model routes and encrypted credential objects.

```rust
use reliquary_memory::ConfiguredRuntime;

let runtime = ConfiguredRuntime::from_path("reliquary.cfg")?;
println!("{}", runtime.readiness());
# Ok::<(), reliquary_memory::ConfiguredRuntimeError>(())
```

The exact final constructor name may change during OC02 implementation, but callers must not need Switchboard/internal imports, a JSON sidecar, or environment-variable copies of stored secrets.

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

Existing product objects such as `archive.fragments`, `retrieval.default`, and `runtime.default` are preserved. Unknown objects survive saves byte-for-byte.

Credentials remain encrypted `credential.<id>` objects in the config. The companion master-key backend remains the existing version-1 JSON file until a separately approved security change replaces it.

## Selection

The configured route is selected once per operation:

- Insomnia: dedicated -> General.
- Dream: dedicated -> General.
- Entity Resolution decision: dedicated -> General Decision -> adapted General -> unavailable.
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

## Checks

`config verify` validates framing, known product/model objects, credentials and route compatibility without contacting providers or rewriting files.

An explicit online probe sends only synthetic inputs through selected transports after local validation. It reports route/failure categories without secrets or private corpus content.

`probe_embedding_profile()` retains the existing bounded compatibility-probe behavior for the configured embedding endpoint.

## Related docs

- [Switchboard configuration restoration](switchboard-configuration-restoration-spec-2026-11-08.md)
- [Public API / CLI operational completion](public-api-cli-operational-completion-spec-2026-11-07.md)
- [Public product API](public-product-api.md)

## Notes

The previous Reliquary configuration documentation in `Reliquary-reference/docs/configuration.md` is the behavior oracle for the persisted format. OC02 selectively restores that Switchboard ownership; it does not reintroduce the historical Root implementation.
