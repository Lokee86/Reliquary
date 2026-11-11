# Operator workflow

Parent index: [Documentation](./INDEX.md)

## Purpose

This guide gives practical CLI procedures for creating owners, configuring routes, staging and processing sources, inspecting results, and recovering from common failures.

## Overview

The CLI is a local trusted host that opens owner paths and requests only the access required for each command. Successful one-shot commands return JSON. Errors go to stderr as JSON and exit nonzero. Commands that call models use the existing `reliquary.cfg`; no credentials belong in command arguments, reports or source files.

## Prerequisites

- Build `reliquary` with `cargo build --manifest-path cli/Cargo.toml --locked`.
- Choose protected paths for `.rel` and `.phy` owner files and back them up with the documented verified copy workflow.
- For provider work, set up OS credential storage and route credentials in [configured runtime](configured-runtime.md). Run offline `config verify` before an import/process/query workflow.
- Keep a durable copy of the import ID printed after staging; it identifies resumable processing state.

## Create and check owners

```text
reliquary rel create project.rel --scope project
reliquary phy create private.phy
reliquary rel info project.rel
reliquary phy verify private.phy
```

Expected result: current REL/PHY files are created at new paths and each command returns owner kind, identity and current metadata. REL defaults to project scope if `--scope` is omitted. A destination that already exists fails; create never replaces an owner. If verification reports wrong format or corruption, preserve the file and consult the configured recovery/backup procedures; the normal runtime does not convert REL 2.0/FRS07.

## Configure providers

Start with a standard local config and routes, then add protected credentials and verify:

```text
reliquary --config reliquary.cfg config show
reliquary --config reliquary.cfg config credential login-codex codex
reliquary --config reliquary.cfg config credential login-codex codex-alt
reliquary --config reliquary.cfg config credential add-api-key embedding
reliquary --config reliquary.cfg config verify
```

For an OpenAI-compatible key, `add-api-key` prompts without echo. `--stdin` reads one bounded line if a protected secret-input mechanism is needed. Avoid shell history and environment/argument expansion containing secrets. Expected result: offline verify reports selected route readiness without network activity. If a stored default route points to an ID with no credential, add that credential or replace/clear the route. `config verify --online --route embedding` is a separate explicit synthetic provider probe.

## Import and process a source

First inspect an import plan without writing, then stage it:

```text
reliquary rel import project.rel --source conversations.json --format chatgpt --start 2025-01-01T00:00:00Z --end 2025-01-03T00:00:00Z --dry-run
reliquary rel import project.rel --source conversations.json --format chatgpt --start 2025-01-01T00:00:00Z --end 2025-01-03T00:00:00Z
```

`--start` and `--end` are a required pair when selecting a half-open RFC3339 range. `--format normalized` consumes Reliquary's visible-turn manifest; its selected range is in the manifest. A dry run reports the plan and does not stage it. The stage report returns the durable `import_id`; save it.

Run the configured processing path:

```text
reliquary rel process project.rel --import IMPORT_ID --config reliquary.cfg --until ready --phy private.phy
reliquary rel work status project.rel --import IMPORT_ID
```

The pipeline uses the recorded processing plan for that import, including model routes, compatibility profile and stage policy. `--phy` routes private work when supported. Expected result is a processing report showing completed/held/remaining work and import state. A result with `ok: false` is nonzero and must not be treated as ready.

For an existing live REL conversation queue, use `rel process project.rel --config reliquary.cfg --until idle` to drain Insomnia; named stage commands include `rel dream run project.rel --config reliquary.cfg`, `rel entity-resolution run project.rel --config reliquary.cfg`, and `rel vectors build project.rel --config reliquary.cfg`. Check status using `rel insomnia status project.rel`, `rel dream status project.rel`, `rel entity-resolution status project.rel` and `rel freshness status project.rel`. `dream` and `entity-resolution` live commands accept `--phy`; the current CLI vector status requires `--profile HEX`.

## Resume or recover work

Check durable state before retrying:

```text
reliquary rel work status project.rel --import IMPORT_ID
reliquary rel work resume project.rel --import IMPORT_ID --config reliquary.cfg --phy private.phy
```

If a specific held import Episode needs retry, include its `--episode` ID. Resume reloads the recorded plan rather than silently selecting a new route. A credential with the same configured identity may be renewed for the same account. If the first run was held because no PHY destination was attached, attach the authorized PHY on resume before vector work. If state is stale or conflicting, preserve the JSON error and inspect the import/status and current owner before retrying; do not stage the same source under a new ID as a blind retry.

`rel work stop` cannot signal a different process. It returns an Unsupported error because the one-shot invocation has no control channel to an already-running job. An embedding host can stop its `OwnerProcessingJob` and join it. For an active CLI job, stop its owning process through the host's process-control mechanism, then inspect durable status before resuming.

## Search and inspect

```text
reliquary rel search project.rel --query "movement packet" --mode lexical
reliquary rel search project.rel --query "movement packet" --mode hybrid --config reliquary.cfg
reliquary rel query project.rel --query "How does the server handle movement?" --config reliquary.cfg
reliquary rel interact project.rel --config reliquary.cfg
reliquary rel memory list project.rel --limit 20
reliquary rel archive conversations project.rel --limit 20
```

Lexical search is local. Hybrid and semantic queries need a compatible configured embedding route. Interactive retrieval state lasts only for that process. It accepts query/refine/traverse/show/source/neighbors/status/new-turn/quit; `--commands FILE` reads JSONL requests, one per line. A query or display is inspection only: CLI output, piping and JSON export do not prove host consumption and do not award Freshness/accepted-use credit. Use exact inspection for revisions, provenance and source conversations when validating a result.

## Create a verified maintenance copy

Copy operations prepare/verify a source plan and create a new destination. For example, first export or prepare a valid rewrite policy, inspect it:

```text
reliquary rel rewrite project.rel project-pruned.rel --policy rewrite-policy.json --dry-run
reliquary rel rewrite project.rel project-pruned.rel --policy rewrite-policy.json
reliquary rel verify project-pruned.rel
```

Expected result is a separate verified file with reported retained history. Existing destinations are never overwritten. If destination collision occurs, choose a new path; do not delete a prior destination automatically. Reconcile accepts two sources and a new destination. Quiesce writable users before maintenance and verify the resulting file after reopening.

## Related docs

- [CLI reference](reference/cli.md)
- [Configured runtime](configured-runtime.md)
- [Public product API](public-product-api.md)
- [Gateway architecture](architecture/gateway.md)
- [Switchboard architecture](architecture/switchboard.md)
- [Operational CLI evidence](public-api-cli-operational-verification-2026-11-07.md)

## Notes

CLI behavior is implemented in `cli/src/` and exercised by `cli/tests/cli.rs`, `cli/src/operational_tests.rs` and `cli/tests/g4_corpus.rs`. These tests support command behavior but do not establish provider quality or a product-wide readiness claim. Current route, owner and command contracts are linked from the [reference index](reference/INDEX.md).
