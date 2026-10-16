# Local Configuration

Parent index: [Documentation index](INDEX.md)

## Purpose

This document owns the local Reliquary configuration format, mutation semantics, and boundary from Reliquary semantic state.

## Overview

Reliquary uses one small purpose-built `reliquary.cfg` file for machine-local current configuration. It uses replaceable logical objects, whole-file atomic replacement, and no internal history.

## Ownership

`reliquary.cfg` is a purpose-built local application-configuration file. It is not part of a `.rel`, does not participate in Reliquary semantic clocks, and has no historical or append-only semantics.

Configuration is current-state only:

```text
load current objects
    ↓
replace typed values in memory
    ↓
serialize one complete current file image
    ↓
write + sync temporary file
    ↓
atomically replace reliquary.cfg
```

If a user wants configuration history, that belongs in an external version-control system such as Git.

## File header

All multi-byte values are little-endian.

```text
8 bytes   magic = "CVCFG\0\r\n"
u16       major = 1
u16       minor = 0
u32       object count
```

## Object framing

Each logical configuration object is independently framed:

```text
u16       UTF-8 key byte length
u16       object schema version
u32       flags
u32       payload byte length
u32       reserved = 0
N bytes   UTF-8 logical key
M bytes   object payload
```

Logical keys are stable names rather than content hashes. Replacing a setting replaces the object associated with that key in the next complete file image; superseded objects are not retained.

Unknown logical objects are preserved byte-for-byte when a newer/foreign object is present and known objects are changed. This lets the object vocabulary expand without requiring config history or an append-only store.

Flags are `0` for ordinary objects. Credential objects use flag `1` to mark an authenticated encrypted payload; the whole configuration file remains unencrypted.

## Implemented objects

### `archive.fragments`

Schema `1`, flags `0`:

```text
u64       turns
u64       overlap
```

Current defaults are `turns = 8`, `overlap = 2`. `turns` must be non-zero and `overlap < turns`.

### `retrieval.default`

Schema `1`, flags `0`:

```text
u64       candidate_limit
u64       result_limit
f64       lexical_weight
f64       semantic_weight
```

Current defaults are `30`, `10`, `0.45`, and `0.55`. Limits must be non-zero, result limit cannot exceed candidate limit, and candidate limit cannot exceed the semantic-search maximum. Weights must be finite and positive; search normalizes them before hybrid fusion.

`Cva::search_with_config` consumes `RetrievalConfig`. `Cva::search` remains the default-policy convenience method.

### `models.general`

Schema `3`, flags `0`:

```text
u8        provider
u8        reasoning effort; 0=unset, 1=None, 2=Minimal, 3=Low, 4=Medium, 5=High, 6=XHigh, 7=Max
2 bytes   reserved = 0
string    model
string    URL; empty for provider-owned routing
string    credential ID
```

The general endpoint accepts `openai-codex` or `openai-ready`. `openai-codex` uses provider-owned routing and therefore stores no URL; its Responses transport requires an explicit reasoning effort. `openai-ready` requires an explicit `http://` or `https://` endpoint URL and currently leaves reasoning effort unset because that transport uses Chat Completions. The credential ID names a separate encrypted `credential.<id>` object. Schema-2 General objects remain readable and decode with reasoning unset; new saves write schema 3.

### `models.insomnia`

Schema `3`, flags `0`, with the same payload shape and schema-2 read compatibility as `models.general`.

This route is optional. When configured, Insomnia uses it for extraction. When absent, `ModelSwitchboard::insomnia()` and `insomnia_auth()` fall back to the configured `models.general` route and credential. This keeps General as the default model path while allowing Insomnia to be independently routed when desired.

### `models.insomnia_metadata`

Schema `3`, flags `0`, with the same payload shape and schema-2 read compatibility as `models.general`.

This route is optional. When configured, Insomnia uses it for the bounded fixed-group category/type/lifecycle classifier and for the post-wording Entity-mention enrichment pass. When absent, fixed-group metadata classification is skipped, while configured finite/runtime-host Entity enrichment still runs by falling back to the effective main Insomnia route. User/Project ownership classification has no separately persisted route: when enabled it also prefers `models.insomnia_metadata` and otherwise falls back to effective main Insomnia. Lexical Memory routing is derived deterministically from Memory title/content and uses no model route.

### `models.dream`

Schema `3`, flags `0`, with the same payload shape and schema-2 read compatibility as `models.general`.

This route is optional. When configured, Dream pair classification and independent verification use it. When absent, `ModelSwitchboard::dream()` and `dream_auth()` fall back to the configured `models.general` route and credential. Dream and Insomnia routing are independent; configuring one does not affect the other.

### `models.memory_retrieval_decision`

This optional route uses decision-endpoint schema `1`, flags `0`, with the same payload encoding as `models.entity_resolution_decision`: model, explicit `http://` or `https://` URL, and credential ID. It is reserved for Memory-retrieval decision evaluation; the controller operations remain tracked separately by the agentic retrieval implementation plan. When unset, it is absent and does not fall back to `models.general` or another route. Its credential must be an API-key credential; missing or wrong-kind credentials, empty models, and invalid URLs are rejected during runtime validation. The CLI commands are `config model set-memory-retrieval-decision`, `clear-memory-retrieval-decision`, and `config show`.

### `models.embedding`

Schema `2`, flags `0`:

```text
u8        provider
u8        normalization: 0=None, 1=L2
2 bytes   reserved = 0
u32       dimensions
string    model
string    URL
string    credential ID
```

The embedding endpoint currently accepts `openai-ready` only, requires non-zero dimensions, and requires an explicit endpoint URL. The current provider tags are `1=openai-codex` and `2=openai-ready`.

### `credential.<id>`

Schema `1`, flags `1`:

```text
12 bytes  AES-GCM nonce
N bytes   AES-256-GCM ciphertext + 16-byte authentication tag
```

Authenticated additional data is the fixed domain `CVCFG-CREDENTIAL-V1\0` followed by the exact logical object key. A credential object therefore cannot be copied to another credential ID without failing authentication.

The encrypted plaintext begins with a credential-kind tag plus three reserved zero bytes. Kind `1` stores one API-key string. Kind `2` stores ChatGPT OAuth ID token, access token, refresh token, and optional account ID as length-prefixed strings. Each encryption uses a fresh 96-bit nonce from operating-system entropy.

`ModelSwitchboardConfig` owns endpoint selections and their credential references. `ModelSwitchboard` additionally receives `CredentialsConfig`, rejects missing/wrong-kind references, and can produce request auth. `openai-ready` produces `Authorization: Bearer <api-key>`; `openai-codex` produces `Authorization: Bearer <access-token>` plus `ChatGPT-Account-ID` when present.

## Replacement and durability

`ReliquaryConfig::save` validates all known objects before touching the existing file. It writes the new image to a temporary file in the same directory, flushes it, then performs a replace operation. Windows uses `MoveFileExW` with replace/write-through flags; Unix uses same-filesystem rename and synchronizes the parent directory.

Repeated replacement does not accumulate superseded configuration objects or require compaction.

## Security boundary

A locally generated 256-bit `MasterKey` is implemented. `ReliquaryConfig::load_or_create_master_key()` currently persists it beside the config as `reliquary.master-key.json`:

```text
reliquary.cfg
reliquary.master-key.json
    version: 1
    master_key_hex: 64 hex characters
```

Generation uses operating-system entropy (`BCryptGenRandom` on Windows and `/dev/urandom` on Unix). The key is created once and reused; normal load-or-create behavior never overwrites an existing key, and `Debug` output redacts the key material.

The JSON store is explicitly temporary development plumbing, not the final security boundary. Because the master key is plaintext on disk, it provides no meaningful at-rest protection against an actor who can read both files. The intended production replacement remains the operating-system credential store while encrypted credential payloads remain in `reliquary.cfg`.

Credential objects are now encrypted independently with AES-256-GCM. Ordinary settings remain inexpensive and inspectable. Secret strings redact `Debug` output and are zeroized when dropped; plaintext codec buffers are also zeroized after encryption/decryption.

Encrypted credential bytes are intentionally nondeterministic because each save uses fresh nonces. Object ordering and ordinary object encoding remain deterministic.

## Current limitations

- The default operating-system config location is not selected yet; callers currently supply a path.
- General/Insomnia/Insomnia-metadata/Dream/embedding routes, encrypted credentials, credential references, request-auth attachment, direct `openai-ready` embedding/General HTTP execution, and provider-native `openai-codex` General/Insomnia/Insomnia-metadata/Dream Responses execution are implemented. Insomnia and Dream independently fall back to General when their dedicated routes are unset; fixed-group metadata classification is skipped when its optional route is absent, while production Entity-mention enrichment falls back to effective main Insomnia. Lexical Memory routing is deterministic and makes no model request.
- `openai-codex` ChatGPT device-code acquisition stores the returned ID/access/refresh tokens plus ChatGPT account ID as an encrypted credential object. OAuth token refresh is not implemented yet.
- The master key currently lives in temporary plaintext JSON; Windows Credential Manager integration is not implemented yet.
- No import/export text format exists yet.

## Related docs

- [Architecture](architecture.md)
- [Rust API](api.md)
- [Repo-local CLI](cli.md)
- [Current limitations](current-limitations.md)
- [ADR 0008](decisions/0008-purpose-built-local-configuration.md)
- [ADR 0009](decisions/0009-expandable-model-switchboard.md)
- [ADR 0010](decisions/0010-encrypted-credential-objects.md)
- [ADR 0011](decisions/0011-detachable-repo-local-cli.md)

## Notes

The config object vocabulary is intentionally concrete. Shared framing does not make the file a generalized semantic store.

Configuration, credential, and switchboard tests use UUID-isolated scratch paths so parallel test execution cannot collide on timestamp-derived temporary directories. This is test infrastructure only and does not change configuration persistence or replacement semantics.
