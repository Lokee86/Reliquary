# Repo-local CLI

Parent index: [Documentation index](INDEX.md)

## Purpose

This document owns the development CLI package boundary, invocation model, and currently exposed operator commands.

## Overview

`cli/` is a separate Cargo package named `reliquary-cli`. It is intentionally not a binary target of the core `reliquary-memory` package and is not installed by repository builds or tests.

Run it directly from the repository:

```text
cargo run --manifest-path cli/Cargo.toml -- --help
```

The package depends only on the public `reliquary-memory` Rust API through a path dependency. Deleting or moving `cli/` does not change the core library package. A detached copy only needs its dependency source changed from `path = ".."` to the desired Git/package source.

## Command surface

```text
reliquary
├── rel
│   ├── create [--type LABEL]
│   ├── info
│   ├── verify
│   └── reclaim <path> <output>  (alias: vacuum)
├── phy
│   ├── create
│   ├── info
│   ├── verify
│   └── reclaim <path> <output>  (alias: vacuum)
├── migrate <source> <output>
├── migrate-conversation-titles <canonical_conversations.csv[.gz]> <rel>
├── import
│   └── graph-jsonl
├── archive
│   ├── conversations
│   ├── show
│   ├── fragments
│   └── fragment
├── config
│   ├── show
│   ├── verify
│   ├── credential ...
│   └── model ...
├── vectors
│   ├── status
│   ├── profiles
│   ├── probe
│   └── build
├── insomnia
│   └── run
└── dev
    ├── establish-profile
    ├── build-vectors
    └── search
```

`vectors probe` and `vectors build` call `ConfiguredRuntime`, which owns configured live embedding-route resolution and vector execution. `build` establishes/reuses the compatibility profile, embeds the Archive, and publishes one generation inside the library. `dev` commands deliberately remain on `SimulatedEmbeddingEndpoint` for deterministic development work.

`insomnia run <rel>` is the whole-file bring-up surface. The CLI passes file targets and operator options to `ConfiguredRuntime::run_insomnia_files`; queue finalization, endpoint construction, model-route selection, routed publication, vector completion, and sync are library-owned. Without `--phy`, the operation remains REL-only. With `--phy <phy>`, the library enables the dedicated `user | project` compatibility ownership classifier, where `project` means the active/non-user REL rather than a REL type, routes User-owned Memories into that PHY, and records owner-qualified external Memory references in the REL completion receipt. Ownership inference uses the `insomnia_metadata` route when present and otherwise falls back to the main `insomnia` route. `--embedding-batch-size` and `--embedding-concurrency` remain operator controls for the derived-vector phase.

## Configuration and credentials

The global `--config` option defaults to `reliquary.cfg` in the current directory. This is a CLI working-directory default, not a selected operating-system application-config location.

`config credential add-api-key <id>` prompts for the API key without terminal echo. `--stdin` is available for automation and tests. Secrets are not accepted as command-line arguments so they are not placed directly into shell history/process arguments.

`config credential login-codex <id>` performs the ChatGPT/Codex device-code flow directly. It requests a one-time code, prints the verification URL and code, waits for authorization, exchanges the resulting authorization code for OAuth ID/access/refresh tokens, extracts the ChatGPT account ID, and saves the credential under the requested ID. Manual token copy/paste is not part of the CLI. `config credential refresh-codex <id>` exchanges an existing stored ChatGPT refresh token for a new access/refresh token pair and preserves the existing ID token/account binding. Automatic refresh during model requests is not implemented yet.

`config model set-general`, `set-insomnia`, `set-insomnia-metadata`, and `set-dream` persist model-route selection and accept `--reasoning` for `openai-codex` routes. If `models.insomnia` is unset, Insomnia inherits the General route. `insomnia_metadata` is optional: when present it serves fixed-group metadata classification and post-wording Entity-mention enrichment; when absent, classification is skipped but production Entity enrichment falls back to effective main Insomnia. Ownership classification has no separate persisted route yet: library policy likewise prefers `insomnia_metadata`, then falls back to effective main Insomnia. Lexical Memory routing is deterministic and uses no model route. `set-embedding` persists the separate embedding route. `config verify` delegates route/credential compatibility validation to `ReliquaryConfig` without sending network requests.

## REL, PHY, and import behavior

`rel create` creates a homogeneous REL. Optional `--type LABEL` stores any free-form organizational type label; the label has no behavioral effect. `rel info` reports the stored type label, dependency owner IDs, and whether the file is a legacy CVA physical form. `rel verify` performs a normal `Reliquary::open`, so the same format/reopen/reference validation used by the library is exercised. The old `cva` command name remains a compatibility alias for `rel`.

`phy create <path>` creates a user-global Phylactery, which remains a distinct file kind from Reliquary. `phy info` reports `kind: phylactery` plus Memory, Graph, packed-vector, Memory-vector, and compatibility-profile state. `phy verify` performs a normal `Phylactery::open`, including exact file-kind validation, rejection of REL-local Memory provenance, structural validation of optional external `MemorySourceRef`, Graph endpoint/global-version validation, and vector/profile reference validation. PHY has no `--scope` option.

`rel reclaim <path> <output>` and `phy reclaim <path> <output>` (also exposed as `vacuum`) write a separate compacted file and never replace the source. The library computes conservative reachability over never-published/staged backing data and explicit compaction free extents, relocates physical version references, syncs the output, and reopens it through normal REL/PHY validation before reporting success. Published semantic history is not retention-pruned by this command.

`migrate <source> <output>` auto-detects a legacy 16-byte Project CVA or earlier 24-byte typed REL/PHY and semantically repacks it into a new 40-byte identified file. The source is never replaced in place. When an old REL contains `WorkspaceMetadata.id`, migration derives the new UUID deterministically from that ID so independently diverged copies retain the same logical owner identity; otherwise a new UUID is generated. Legacy WorkspaceMetadata itself is not copied into the output.

`migrate-conversation-titles <canonical_conversations.csv[.gz]> <rel>` backfills conversation-owned title metadata into an already-imported REL from the normalized ChatGPT canonical-conversation catalog. Only conversation IDs already present in the REL are updated; empty titles are skipped, repeated execution is idempotent, and the source catalog is read-only.

`import graph-jsonl` accepts the current generic development graph JSONL records used by the corpus smoke harness. The format is not ChatGPT-specific; any source can use it after normalization into node/branch/Echo records. It creates the target REL when absent or appends idempotent/compatible records to an existing Reliquary (including a legacy CVA), then materializes branch fragments with the requested window/overlap policy. Node records may include an optional `attachments` array. Each attachment has `path`, optional `filename`, and optional `mime_type`; relative paths resolve from the JSONL file's directory. The importer reads the bytes and submits the node plus all attachments through `Cva::ingest_turn`, so embedding the files and recording their source-turn provenance is one core ingestion operation rather than importer-side coordination. Branch records may also carry an optional `title`; when present it is published through the conversation-metadata owner rather than stored on the Branch.

An Echo record uses `kind = "echo"` plus `conversation_id`, `message_id`, `sequence`, `timestamp_ns`, optional `model_round`, `event_kind`, optional `correlation_id`, optional `name`, and `content`. `event_kind` uses the provider-neutral snake-case values `reasoning_summary`, `commentary`, `reasoning_trace`, `tool_call`, `tool_result`, `activity_started`, `activity_completed`, and `activity_failed`. Importers for ChatGPT, Codex, or other providers normalize whatever execution evidence their source exposes into these records; final user/assistant text remains a node record. Echo is written through `Cva::put_echo_event` into native REL `CVAECHO1` chunks, not embedded JSON storage.

The CLI does not expose a single-turn live-ingestion command, file-management commands, or a persistent capture service. The graph importer is currently the only CLI surface that drives turn ingestion.

Archive inspection is read-only. `archive conversations` uses the public current-branch inventory; `archive show` resolves one current branch; fragment commands list or expand durable fragments.

## Verification

The CLI package has its own lockfile and must be checked separately from the library package:

```text
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo check --manifest-path cli/Cargo.toml --locked
cargo test --manifest-path cli/Cargo.toml --locked
```

Repository verification also performs command-level smoke tests for REL create/info/verify and PHY create/info/verify, graph import/archive inspection, simulated profile/vector/search execution, and config show/verify without installing the binary.

## Build dependency alignment

The detachable CLI is a Cargo root. It repeats Reliquary's vendored quinn-proto override because Cargo does not inherit dependency-root patches. Its lockfile resolves the same immutable Arcana revision as the library, including the v2 indexed metadata reader. This is build compatibility; CLI/storage semantic ownership is unchanged.

## Related docs

- [Architecture](architecture.md)
- [Rust API](api.md)
- [Local configuration](configuration.md)
- [Development](development.md)
- [ADR 0011](decisions/0011-detachable-repo-local-cli.md)

## Notes

The CLI owns only interface concerns: argument parsing, terminal interaction, human-readable rendering, and explicit operator command dispatch. Configured provider/model composition and finite Insomnia/vector workflows are owned by the library `ConfiguredRuntime`; retrieval policy remains library-owned. A CLI boundary regression test prevents the known low-level runtime-composition types from returning to command modules.