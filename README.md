# Reliquary

## Purpose

Reliquary is an embedded, file-backed memory library. The Rust product package is `reliquary-memory`; the separate `reliquary` executable is a local operator client. REL files hold project conversation history and semantic memory. PHY files hold a separate private memory owner.

## Overview

The supported runtime reads and writes current REL 2.1 and PHY 2.1 files through opaque `Rel` and `Phylactery` handles. Hosts mount owners in `Gateway`, authenticate an `Instance`, and use grant-checked product operations. Internal stores and writable containers are not public API. Provider routes and credentials are loaded from the existing binary `reliquary.cfg` configuration.

## Getting started

Prerequisites: Rust toolchain supporting edition 2024, a local filesystem path for the owner files, and configured provider credentials for workflows that invoke inference.

Build the standalone command line program:

```powershell
cargo build --manifest-path cli/Cargo.toml --locked
```

Create a REL and inspect it without contacting a provider:

```text
reliquary rel create project.rel --scope project
reliquary rel info project.rel
reliquary rel verify project.rel
```

Expected result: each successful invocation writes one JSON object to stdout. A failed or incomplete requested operation exits nonzero and writes a JSON error to stderr. If a command needs model routes, configure them and run `reliquary config verify` first. See the [operator workflow](docs/operator-workflow.md) for import, processing, search, inspection, configuration and recovery steps.

## Product surfaces

- Rust library and opaque owner/host API: [Public product API](docs/public-product-api.md)
- CLI command grammar and errors: [CLI reference](docs/reference/cli.md)
- Provider configuration and key handling: [Configured runtime](docs/configured-runtime.md)
- Operator workflows and recovery: [Operator workflow](docs/operator-workflow.md)
- Contributor workflow: [Development workflow](docs/development/workflow.md)
- Documentation navigation: [Documentation index](docs/INDEX.md)
- Canonical architecture: [System](docs/architecture/system.md), [storage](docs/architecture/storage.md), [owner session](docs/architecture/owner-session.md), [application](docs/architecture/application.md), [semantic processing](docs/architecture/semantic-processing.md), [gateway](docs/architecture/gateway.md), [Switchboard](docs/architecture/switchboard.md)

## Limits

The normal runtime supports current REL 2.1 and PHY 2.1 files. It does not read REL 2.0/FRS07 or provide the offline converter. Reliquary is an embedded library and local CLI, not a network service or daemon. Provider readiness checks are distinct from provider calls; only `config verify --online` sends synthetic probe requests. Search and display are inspection operations and do not assert that a host consumed a Memory.

## Related docs

- [Documentation index](docs/INDEX.md)
- [Maintainer map](docs/architecture/maintainer-map.md)
- [Coverage map](docs/development/documentation-coverage.md)
- [Behavioral contract map](docs/development/behavioral-contracts.md)
- [Planning](docs/planning/roadmap.md) and [known limits](docs/limits/current-limitations.md)

## Notes

This page describes the current product surface. Dated acceptance reports and rebuilding specifications under `docs/` are historical evidence; they do not supersede the current guides or imply that a later compliance gate has passed.
