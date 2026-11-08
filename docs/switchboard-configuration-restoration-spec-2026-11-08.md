# Switchboard configuration restoration — 2026-11-08

Parent: [public API / CLI operational completion](public-api-cli-operational-completion-spec-2026-11-07.md). Source audit: [public API / CLI gap audit](public-api-cli-gap-audit-2026-11-07.md).

**Status: implementation-ready correction to OC02; not implemented or accepted.** The isolated OC02 commit `8f75f230` introduced a JSON/runtime-environment configuration bootstrap. That shape is **superseded and must not be integrated**. The canonical persisted configuration remains the existing `reliquary.cfg` plus its companion `reliquary.master-key.json`.

## Problem Statement

The rebuild retained Switchboard route selection and provider transports but omitted the existing product configuration bootstrap. OC02 then filled that gap with a new JSON route document and environment-variable credentials. That changed a persisted user-facing contract without a product requirement and made the rebuilt application unable to use the existing Reliquary configuration directly.

The old format is not disposable internal code. Existing `reliquary.cfg` files are persisted machine-local user data, the format is documented, the CLI historically manages it, and encrypted credential objects depend on the companion master key. A replacement Reliquary must open that configuration without conversion before it can claim ordinary operational parity.

## Target Architecture

Canonical call direction:

`CLI / host -> Root ReliquaryConfig / ConfiguredRuntime -> Switchboard config + credential owners -> existing provider endpoints`

Ownership remains the architecture already documented before the rebuild:

- **Switchboard owns** the `CVCFG\0\r\n` document framing, logical-object preservation, model-route codecs, encrypted credential objects, master-key handling, model authentication, provider transports, device login/refresh, and route-selection policy.
- **Root owns** the product configuration wrapper and product-domain objects such as fragment, retrieval, and runtime policy. It composes those values with Switchboard's model/credential state and exposes a safe public configuration/runtime surface.
- **CLI owns no configuration semantics.** It selects operations, collects secrets safely, calls Root, and renders redacted results.
- **Application/Gateway do not decode config files.** They receive already validated configured endpoints/policies.

The normal operational path reads the existing binary `reliquary.cfg`. There is no JSON sidecar, format-conversion step, environment-only secret model, or fallback to the custom Codex/Python bridge.

## Persisted compatibility boundary

The following existing behavior is normative:

- Header/version remain `CVCFG\0\r\n`, format 1.0, little-endian.
- Logical objects remain replaceable current-state objects; unknown objects survive a save byte-for-byte.
- Existing model object keys and schemas remain readable, including schema-2 General compatibility and current schema-3 General routes.
- Existing `credential.<id>` objects remain AES-256-GCM encrypted with their logical key included in authenticated data.
- The companion key remains `<config-stem>.master-key.json`, version 1 with the existing 256-bit key encoding.
- Opening, showing, verifying, probing, or constructing a runtime does not rewrite either file.
- Save uses same-directory temporary write, sync, and atomic replacement; it does not accumulate obsolete configuration history.
- New credential encryption may legitimately produce different ciphertext because nonces are fresh. Semantic values, unrelated objects, and unknown objects must remain unchanged.

This is an **external-boundary compatibility requirement**, not a permanent dual-format adapter.

## Required route semantics

Restore the route vocabulary already represented by the existing config:

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

Preserve the established selection rules rather than requiring new objects:

- Insomnia dedicated route, otherwise General.
- Insomnia metadata when configured; ownership may use metadata and otherwise the effective main Insomnia route according to the accepted service contract.
- Dream dedicated route, otherwise General.
- Memory Retrieval decision: dedicated -> shared General Decision -> adapted General -> unavailable.
- Entity Resolution decision: dedicated -> shared General Decision -> adapted General -> unavailable, while the generative resolver remains its separate configured route where required.
- Embedding has no silent fallback; it requires its configured compatible embedding route.

A missing dedicated Memory Retrieval route is therefore **not an error** when shared/general fallback is available. Provider failure after route selection also does not trigger a lower-tier fallback.

## Public product surface

Restore one public product configuration type, equivalent in responsibility to the previous `ReliquaryConfig`, with these operations:

- create a default configuration at a supplied path without writing until save;
- open an existing `reliquary.cfg`;
- inspect path, product settings, model routes, and credential IDs/auth kinds without exposing secrets;
- validate all known product and Switchboard objects;
- save atomically while preserving unknown objects;
- load/create the companion master key only when a write requiring it needs to occur;
- construct `ConfiguredRuntime` directly from the opened configuration;
- perform explicit online probes through the selected real endpoints.

Exact Rust signatures may follow the clean rebuild's naming, but the CLI and an external Root-only consumer must not need Switchboard/internal-crate imports.

The canonical `ConfiguredRuntime::from_path` (or equivalent) opens the binary configuration and its companion master key. It must not require an `EnvironmentCredentialResolver` for normal use. If an alternate programmatic secret source is ever retained for a separately proven host requirement, it is an explicit secondary constructor and cannot become the CLI's canonical bootstrap or change the on-disk format.

## Credential management

Restore the existing credential lifecycle under Switchboard ownership:

- API-key insertion from a secret value supplied by the caller.
- ChatGPT/Codex device-code login.
- Explicit ChatGPT/Codex refresh using the encrypted stored refresh token.
- Removal with runtime validation preventing a route from silently referencing an absent credential.
- Redacted Debug/display/reporting and zeroization of plaintext secret buffers.

The present live 401 should be resolved by the restored `config credential refresh-codex <id>` path against the same existing configuration. If the refresh token itself is no longer valid, `login-codex <id>` replaces that credential under the same ID after explicit user authorization. Neither case requires a new config format.

Do not copy credentials into environment variables, JSON, command-line arguments, logs, test fixtures, or chat.

## CLI contract

Restore the configuration command family to the rebuilt CLI, with `--config reliquary.cfg` (defaulting to `reliquary.cfg` in the current working directory) and the prior operator capabilities:

- `config show`
- `config verify`
- `config credential list`
- `config credential add-api-key <id>` with hidden prompt and explicit stdin mode for automation
- `config credential login-codex <id>`
- `config credential refresh-codex <id>`
- `config credential remove <id>`
- model set/clear for General, Insomnia, Insomnia Metadata, Entity Extraction, General Decision, Entity Resolution Decision, Memory Retrieval Decision, Entity Resolution, Dream, and Embedding.

The CLI continues to depend only on the public Root package among Reliquary packages. Route validation, encryption, file replacement, provider construction, refresh logic, and fallback policy remain library-owned.

## Implementation Decisions

1. **Port/reuse the correct owner, not the old Root.** Selectively restore the prior Switchboard modules for config document framing, model codecs, credential codec/crypto, master key, and device refresh. Restore a small Root product wrapper for product-domain objects. Do not copy the historical product facade wholesale.
2. **Preserve existing bytes and object schemas.** This work is not a config migration. Do not introduce `CVCFG2`, JSON export/import, dual writers, or automatic rewrite-on-open.
3. **Reject the unaccepted JSON bootstrap.** The OC02 JSON `runtime_config.rs` and environment-only credential bootstrap are an abandoned internal shape. Rework that branch from the spec baseline or replace those hunks before integration. Do not support both shapes just to preserve the intermediate branch.
4. **Keep useful new transport work only where behavior matches.** Provider endpoint/probe improvements from OC02 may be retained if they consume the canonical decoded Switchboard configuration and preserve route/auth semantics.
5. **Restore retrieval/product settings too.** `retrieval.default`, `archive.fragments`, and `runtime.default` remain part of the existing file contract. Where the clean rebuild still consumes a setting, map it to the current owner. Where a setting is temporarily unused, preserve and expose it rather than deleting it from the file.
6. **No secret migration during load.** Existing encrypted credentials decrypt with the existing master key. Secret changes occur only through an explicit credential command/API mutation.

## Migration Strategy

This is compatibility-preserving **only at the persisted configuration boundary** because real user configuration already exists.

Internally it is a hard cut:

1. Make the old binary format the only canonical operational config.
2. Restore the Switchboard config/security owner.
3. Bind Root `ConfiguredRuntime` directly to it.
4. Restore CLI config management.
5. Delete the JSON/env-only canonical bootstrap and its tests/examples.
6. Continue OC03–OC09 using the restored path.

No converter from JSON to `reliquary.cfg` is required because the JSON implementation has not been accepted as a user format.

## OC02 execution sequence

OC02 remains one top-level ticket; these are its four cohesive implementation steps, not four new project phases.

### OC02-A — restore Switchboard persistence/security

Deliver the existing config document framing, unknown-object preservation, model-route codecs, credential encryption/decryption, master-key store, device login/refresh, and atomic replacement under the clean Switchboard crate.

Acceptance:
- exact format/header/object-schema tests;
- credential tamper/wrong-key/wrong-object-ID rejection;
- unknown object preserved through known-object mutation;
- read-only open/verify leaves config and key hashes unchanged;
- no internal Reliquary dependency added to Switchboard.

### OC02-B — restore Root product configuration/runtime

Deliver the product wrapper for fragment/retrieval/runtime settings plus model/credential state and construct the configured runtime from that wrapper.

Acceptance:
- current `reliquary.cfg` copy opens with its current companion key without conversion;
- redacted route/credential inventory matches the prior implementation;
- route fallback tests cover Insomnia, Dream, Entity Resolution decision, and Memory Retrieval decision;
- missing/wrong-kind credentials fail typed before a corpus operation;
- runtime creation does not mutate configuration.

### OC02-C — restore CLI configuration management

Deliver the `config` commands above through Root only.

Acceptance:
- separate CLI package has no lower Reliquary dependency;
- show/list output contains no credential values;
- API-key secret never appears in argv;
- device login/refresh replace encrypted credential state atomically;
- model set/clear round-trips and preserves unknown/unrelated objects;
- nonzero structured failure for malformed config, wrong key, invalid route, missing credential, or failed refresh.

### OC02-D — real existing-config acceptance

Against a **disposable copy** of the actual `C:\!bin\workspace\Reliquary\reliquary.cfg` and its companion key:

1. Open and show the configuration without changing either hash.
2. Verify that Memory Retrieval correctly inherits shared/general routing when its dedicated object is absent.
3. Validate all route bindings.
4. Exercise a synthetic online probe only after the selected credential is valid.
5. Reopen in a fresh process.

After those checks, and only with explicit user intent, refresh the actual stale Codex credential in place using the rebuilt CLI. Preserve a before/after backup/hash record locally; never copy secret material into evidence. OC09's two-day corpus then uses this same canonical configuration path.

## Testing Decisions

Reuse the prior Switchboard configuration tests as behavior oracles, adapted to the clean package boundary rather than copied blindly. Required tests include:

- document decode/encode and malformed/truncated/duplicate-object rejection;
- deterministic ordinary-object encoding;
- unknown-object byte preservation;
- atomic replacement/collision failure behavior;
- master-key create/reload/no-overwrite and malformed-key rejection;
- credential round-trip, fresh nonce behavior, tamper/wrong-key/wrong-ID rejection, Debug redaction, and zeroization-sensitive code review;
- schema-2 General read compatibility and schema-3 save;
- every model route set/clear and fallback selection;
- Codex device login/refresh through mock HTTP;
- existing config copied from disk opens and resolves semantic routes without mutation;
- CLI/API parity for show, verify, credential lifecycle, and model changes;
- full OC02 configured-runtime provider probes after credential refresh.

The final OC09 run must use this config path. A JSON config, environment-only secret bootstrap, custom Python/Codex bridge, or hand-constructed endpoint does not satisfy the live acceptance gate.

## Out of Scope

Changing the `reliquary.cfg` format; migrating the master key to Windows Credential Manager; automatic OAuth refresh during provider calls; exporting secrets to JSON; old REL/PHY data conversion; Hermes configuration; adding a daemon; redesigning model routing; adding a dedicated Memory Retrieval route merely because it is absent; unrelated config cleanup.

## Completion Gates

OC02 is complete only when:

- the rebuild opens the existing binary config and companion key directly;
- all prior route and credential semantics needed by the current configuration are available;
- the CLI can show/verify, manage credentials, refresh Codex auth, and set/clear routes through Root;
- ordinary reads are byte-neutral and writes preserve unknown/unrelated objects;
- route fallback works without fabricating missing dedicated routes;
- the abandoned JSON/env-only operational bootstrap is absent from the integrated product;
- Switchboard remains the sole owner of config framing/security/provider policy with no dependency-cycle/DAG regression;
- targeted configuration/CLI tests and the full integration gates are green;
- the subsequent two-day live run uses the restored configuration successfully.

## Related docs

- [Operational completion](public-api-cli-operational-completion-spec-2026-11-07.md)
- [Gap audit](public-api-cli-gap-audit-2026-11-07.md)
- [Public product API](public-product-api.md)
- [P01 consumer map](p01-consumer-contract-map.md)
- Reference behavior: `Reliquary-reference/docs/configuration.md`, ADRs 0008–0011, and the reference Switchboard/config CLI tests.

## Notes

The current 401 is evidence that the existing config was successfully located and decoded far enough to select its Insomnia credential, but that credential is no longer authorized. It is not evidence that the file format should change.

The existing config currently has no dedicated Memory Retrieval decision route; that is valid because the accepted policy inherits shared/general decision routing. Restoring this configuration must preserve that behavior instead of forcing a new route object.
