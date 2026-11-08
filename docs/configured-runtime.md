# Configured provider runtime

Load an explicit JSON file through the `reliquary-memory` crate. Construction
validates the configuration and resolves credential references locally.
`readiness()` reports route availability and decision selection offline.

```rust
use reliquary_memory::{ConfiguredRuntime, EnvironmentCredentialResolver};

let runtime = ConfiguredRuntime::from_path("runtime.json", &EnvironmentCredentialResolver)?;
println!("{}", runtime.readiness());
# Ok::<(), reliquary_memory::ConfiguredRuntimeError>(())
```

## Existing protected configuration

Pass the CVCFG file and its version 1 JSON master-key file explicitly to load
an existing provider configuration offline.

```rust
use reliquary_memory::ConfiguredRuntime;

let runtime =
    ConfiguredRuntime::from_protected_config("reliquary.cfg", "reliquary.master-key.json")?;
println!("{}", runtime.readiness());
# Ok::<(), reliquary_memory::ConfiguredRuntimeError>(())
```

The reader accepts CVCFG 1.0 files up to 32 MiB. It loads `models.general`
(schema 2 or 3), `models.general_decision`, `models.entity_resolution_decision`,
and `models.memory_retrieval_decision` (schema 1), and `models.embedding`
(schema 2). Other records are skipped after framing validation. Selected routes
use the URL rules below. Protected Codex routes store an empty URL and use the
built-in Codex endpoint. Configured reasoning effort is passed to the general
transport. The worker lease uses its default.

Switchboard reads the key file only when it needs to authenticate a selected
credential. The file is limited to 4 KiB and contains a JSON object with
`version` set to 1 and `master_key_hex` set to 64 hexadecimal digits.
Switchboard decrypts each distinct referenced `credential.<id>` record using
AES-256-GCM with the record name as associated data. Codex requires the stored
access token and account ID. Invalid selected routes or unavailable credentials
fail construction. The reader leaves both files untouched.

## JSON schema

The top-level object accepts the following optional keys. Omit a route to leave
it unavailable. An empty object is valid and reports no configured routes.
Files are limited to 64 KiB. Unknown keys and fields fail validation.

| Key | Purpose | Additional fields |
| --- | --- | --- |
| `general` | Structured-output general model | `provider` is `openai_ready` by default. Use `openai_codex` for the Codex streaming transport. |
| `decision` | Shared direct decision endpoint | Uses an API-key credential. |
| `entity_resolution_decision` | Dedicated Entity Resolution decision endpoint | Uses an API-key credential. |
| `memory_retrieval_decision` | Dedicated Memory Retrieval decision endpoint | Uses an API-key credential. |
| `embedding` | OpenAI-compatible embedding endpoint | Requires positive integer `dimensions` and an API-key credential. `normalization` is `none` by default or `l2`. |
| `worker` | Worker lease policy | Optional integer `lease_duration_ms` from 30000 through 600000. The default is 120000. |

Each route requires a nonempty `model`, an absolute `url` for its request
endpoint, and a `credential_id`. URLs use HTTPS or loopback HTTP and must omit
user information and fragments. Keep secrets out of URLs. Credential IDs may
contain 1–128 ASCII letters, digits, dots, underscores, or hyphens.

```json
{
  "general": {
    "provider": "openai_ready",
    "model": "general-model",
    "url": "https://inference.example.invalid/v1/chat/completions",
    "credential_id": "RELIQUARY_GENERAL_KEY"
  },
  "embedding": {
    "model": "embedding-model",
    "url": "https://inference.example.invalid/v1/embeddings",
    "credential_id": "RELIQUARY_EMBEDDING_KEY",
    "dimensions": 1024,
    "normalization": "l2"
  },
  "worker": { "lease_duration_ms": 120000 }
}
```

## Credential resolution

`EnvironmentCredentialResolver` reads the environment variable named by the
credential ID for API-key routes. For `openai_codex`, an ID of
`codex.RELIQUARY_CODEX` refers to `RELIQUARY_CODEX_ACCESS_TOKEN` and
`RELIQUARY_CODEX_ACCOUNT_ID`. A host can implement `CredentialResolver` to
supply `CredentialMaterial` from its own selected source. Resolution runs
once per distinct referenced ID. Missing credentials, mismatched material
types, and invalid header values fail construction.

## Selection and checks

Entity Resolution and Memory Retrieval use their dedicated route when
configured, then the shared decision route, then the general endpoint adapted
for structured decisions. Chronos uses the shared route and the same general
fallback. A missing or failed binding for a selected route produces an error.
Provider failures are returned from the selected route.

`online_probe()` sends one synthetic request per configured transport.
It probes general output, direct decision output, and one query embedding as
applicable. A successful general probe covers decisions adapted from that
transport. Probing stops at the first failure. The transports enforce
a 30-second request timeout and a 1 MiB response limit. Reports include route
labels and failure categories while omitting credential references, model
names, URLs, and provider payloads.

`probe_embedding_profile()` explicitly runs the existing bounded query and
document compatibility suite. It returns `Ok(None)` when the embedding route
is absent. It returns an in-memory profile for the configured endpoint when
the probes pass. `worker_lease_duration_ns()` returns the configured or default
lease duration without making a request.
