# Reliquary CLI

Repo-local bring-up CLI for Reliquary Memory v2.

This is a separate Cargo package, not a binary target of the `reliquary-memory` library package and not installed by default. It depends only on the library's public Rust API through `reliquary-memory = { path = ".." }`.

Run it from the repository without installing it:

```text
cargo run --manifest-path cli/Cargo.toml -- --help
```

The package can be removed or moved without changing the core library. If detached from this repository, replace the path dependency in `cli/Cargo.toml` with the desired Git or package source.

## Current command groups

- `cva`: create, inspect, and verify CVAs.
- `import graph-jsonl`: ingest the current graph JSONL development corpus format.
- `archive`: list conversations/branches, show branch turns, inspect fragments, inspect typed conversation lifecycle, and tombstone a conversation directly. Tombstoning is terminal, idempotent, and does not cascade to downstream Memory or Entity state.
- `config`: inspect/verify config, manage encrypted credentials, and select General/Insomnia/Insomnia-metadata/Dream/Embedding routes. Route validation is library-owned.
- `vectors`: inspect profiles/generations and dispatch configured live probe/build operations through the library runtime surface.
- `insomnia run`: dispatch the configured finite Insomnia workflow with operator concurrency/routing options; queueing, model composition, owner routing, persistence, and Memory-vector completion are library-owned.
- `dev`: exercise compatibility profiles, vector generation, and retrieval through the deterministic simulated embedding endpoint.

Direct provider transport and configured runtime composition are implemented by `reliquary-memory`. The CLI does not construct provider endpoints or assemble Insomnia stages. `ConfiguredRuntime` resolves the main Insomnia, metadata/ownership, Dream, and embedding routes; ownership currently prefers Insomnia metadata and falls back to main Insomnia. The CLI supplies paths/options and renders reports.

## Credentials

API keys are prompted without terminal echo by default:

```text
cargo run --manifest-path cli/Cargo.toml -- \
  --config ./reliquary.cfg \
  config credential add-api-key ready
```

For automation/tests, `--stdin` reads the first line from standard input instead. Passing secrets directly as command-line arguments is intentionally unsupported.

ChatGPT/Codex authentication uses device codes rather than pasted tokens:

```text
cargo run --manifest-path cli/Cargo.toml -- \
  --config ./reliquary.cfg \
  config credential login-codex codex
```

The command prints the OpenAI verification URL and one-time code, waits for authorization, then stores the returned OAuth credential encrypted in `reliquary.cfg`.
